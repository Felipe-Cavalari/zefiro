//! Processo do chat: o `claude -p` (stream-json), o `codex app-server` (JSON-RPC) ou o `grok agent
//! stdio` (ACP), do Windows ou do WSL, com stdin e stdout em pipes. Uma thread (`zefiro-chat`)
//! escolhe a origem, inicia, escreve, conduz o JSON-RPC do Codex e do Grok e encerra; outras leem a
//! saída do processo. Os sinais vão para uma fila e acordam a interface com um único `post_user` até
//! ela ler a fila.

use std::io::{BufRead, BufReader, Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdin, Command, Stdio};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::mpsc::{Receiver, Sender, channel};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use windows::Win32::System::Com::CoCreateGuid;
use zefiro_integration_chat::agent::Agent;
use zefiro_integration_chat::codex::{self, Incoming};
use zefiro_integration_chat::command::{GROK_ENV, GROK_PROFILE, Session, UserDirs, claude_args, codex_args, grok_args, grok_profile, grok_profile_script, windows_candidates, wsl_args, wsl_args_with, wsl_probe};
use zefiro_integration_chat::grok;
use zefiro_integration_chat::origin::{Choice, Origin, Prefer, choices, pick_error_text, pick_with};
use zefiro_integration_chat::protocol::{Event, interrupt_line, parse_line, user_message};

const CREATE_NO_WINDOW: u32 = 0x0800_0000;
/// Fim do stderr guardado para explicar uma saída sem resposta.
const STDERR_TAIL: usize = 2000;
/// Ao encerrar: tempo para o processo sair sozinho depois do fim do stdin.
const EXIT_GRACE: Duration = Duration::from_millis(1500);

/// O que o processo conta à interface.
#[derive(Debug, Clone, PartialEq)]
pub enum Signal {
    /// O processo subiu nesta origem ("Windows", nome da distro).
    Ready { origin: String },
    Event(Event),
    /// O processo terminou (sozinho ou caiu). `stderr`: o fim do que ele escreveu lá.
    Exited { stderr: String },
    /// Não deu para iniciar (sem executável, distro inexistente…). Texto para o painel.
    Failed(String),
    /// Origens possíveis para o seletor do agente.
    Origins { agent: Agent, choices: Vec<Choice> },
}

/// O que a interface pede para a partida.
#[derive(Debug, Clone)]
pub struct Start {
    pub agent: Agent,
    pub choice: Choice,
    /// O que o automático tenta primeiro.
    pub prefer: Prefer,
    pub model: String,
    /// Sessão do Claude ou do Grok, ou `thread` do Codex (vazio = conversa nova no Codex e no Grok).
    pub session: String,
    /// A sessão já existe nesta origem: retoma.
    pub resume: bool,
    /// Pasta do chat no Windows (`%LOCALAPPDATA%\zefiro\chat`).
    pub windows_dir: PathBuf,
}

enum Request {
    Start(Start),
    Send { text: String, images: Vec<String>, model: String },
    Interrupt,
    Stop,
    /// Uma linha do `codex app-server`, do processo `generation`.
    Server(u64, Incoming),
    /// Uma linha do `grok agent stdio`, do processo `generation`.
    Grok(u64, grok::Incoming),
}

struct Inbox {
    queue: Mutex<Vec<Signal>>,
    posted: AtomicBool,
    /// Processo atual; sinais de um processo anterior (encerrado) são descartados.
    generation: AtomicU64,
    code: u32,
}

impl Inbox {
    fn push_any(&self, signal: Signal) {
        if let Ok(mut q) = self.queue.lock() {
            q.push(signal);
        }
        if !self.posted.swap(true, Ordering::SeqCst) {
            crate::window::post_user(self.code);
        }
    }

    fn push(&self, generation: u64, signal: Signal) {
        if generation == self.generation.load(Ordering::SeqCst) {
            self.push_any(signal);
        }
    }
}

pub struct ChatService { tx: Sender<Request>, inbox: Arc<Inbox> }

impl ChatService {
    /// `code`: o `WinEvent::User` que acorda a interface.
    pub fn new(code: u32) -> std::io::Result<ChatService> {
        let inbox = Arc::new(Inbox { queue: Mutex::new(Vec::new()), posted: AtomicBool::new(false), generation: AtomicU64::new(0), code });
        let (tx, rx) = channel();
        let (worker, back) = (inbox.clone(), tx.clone());
        std::thread::Builder::new().name("zefiro-chat".into()).spawn(move || run(rx, back, worker))?;
        Ok(ChatService { tx, inbox })
    }

    /// Sobe o processo se ele não está rodando (sem custo de tokens: nada é pedido ainda).
    pub fn start(&self, start: Start) {
        let _ = self.tx.send(Request::Start(start));
    }

    /// Uma pergunta, com as imagens PNG em base64. `model`: o do Codex ou do Grok neste turno (o
    /// Claude recebe o modelo na partida).
    pub fn send(&self, text: String, images: Vec<String>, model: String) {
        let _ = self.tx.send(Request::Send { text, images, model });
    }

    /// Para a resposta em andamento; o processo continua.
    pub fn interrupt(&self) {
        let _ = self.tx.send(Request::Interrupt);
    }

    /// Encerra o processo (fim do stdin e, se ele não sair, `kill`).
    pub fn stop(&self) {
        let _ = self.tx.send(Request::Stop);
    }

    /// Procura as origens do agente numa thread própria (consulta o WSL); chega como `Origins`.
    pub fn discover(&self, agent: Agent) {
        let inbox = self.inbox.clone();
        let spawned = std::thread::Builder::new().name("zefiro-chat-origins".into()).spawn(move || {
            let windows = find_exe(agent).is_some();
            let found: Vec<String> = crate::wsl::running_distros().into_iter().filter(|d| wsl_has(d, agent.binary())).collect();
            inbox.push_any(Signal::Origins { agent, choices: choices(windows, &found) });
        });
        if let Err(e) = spawned {
            tracing::warn!("chat: não consegui procurar as origens: {e}");
        }
    }

    /// Os sinais que chegaram desde a última leitura.
    pub fn take(&self) -> Vec<Signal> {
        self.inbox.posted.store(false, Ordering::SeqCst);
        self.inbox.queue.lock().map(|mut q| std::mem::take(&mut *q)).unwrap_or_default()
    }
}

impl Drop for ChatService {
    fn drop(&mut self) {
        let _ = self.tx.send(Request::Stop);
    }
}

/// O JSON-RPC do Codex: ids, a conversa (`thread`), o turno e as perguntas esperando o `thread`.
#[derive(Default)]
struct CodexState {
    next: u64,
    thread: Option<String>,
    turn: Option<String>,
    /// Pedido de `thread/start` ou `thread/resume` em andamento.
    opening: Option<u64>,
    models: u64,
    /// O padrão do `model/list`: vale quando a interface ainda não sabia os modelos (o do
    /// `config.toml` do Codex pode não existir nesta versão).
    default_model: Option<String>,
    queued: Vec<Question>,
}

/// Pergunta esperando o `thread` do Codex.
struct Question { text: String, images: Vec<String>, model: String }

impl CodexState {
    fn id(&mut self) -> u64 {
        self.next += 1;
        self.next
    }

    fn turn(&mut self, thread: &str, q: &Question) -> String {
        let model = if q.model.is_empty() { self.default_model.clone().unwrap_or_default() } else { q.model.clone() };
        let id = self.id();
        codex::turn_start(id, thread, &q.text, &q.images, &model)
    }
}

/// O ACP do Grok: a partida (`initialize`, `authenticate`), a sessão, a pergunta em andamento, a
/// troca de modelo e as perguntas que esperam.
#[derive(Default)]
struct GrokState {
    next: u64,
    /// `initialize` ou `authenticate` em andamento: as perguntas esperam a partida.
    starting: Option<u64>,
    /// O `initialize` já respondeu.
    initialized: bool,
    /// A pasta de trabalho do processo (`initialize`): o `cwd` das sessões.
    cwd: String,
    session: Option<String>,
    /// Sessão gravada a retomar no fim da partida.
    resume: Option<String>,
    /// `session/new` ou `session/load` em andamento.
    opening: Option<u64>,
    /// A sessão que o `session/load` em andamento retoma: o histórico que o Grok reenvia até a
    /// resposta não vai para a tela.
    loading: Option<String>,
    /// `session/prompt` em andamento.
    prompt: Option<u64>,
    /// `session/set_config_option` em andamento: o id, o modelo e a pergunta que espera por ele.
    switching: Option<(u64, String, Question)>,
    /// O modelo da sessão e o padrão (o primeiro atual que o processo anunciou).
    model: Option<String>,
    default_model: Option<String>,
    queued: Vec<Question>,
    /// Chamadas do `image_gen` deste processo (uma falha delas é imagem que não veio).
    images: Vec<String>,
}

/// Maior imagem gerada que o chat lê (o Imagine manda JPGs de poucas centenas de KB).
const MAX_GENERATED_BYTES: u64 = 30 * 1024 * 1024;

/// Lê a imagem que o `image_gen` gravou na origem do Grok: no Windows, o arquivo; numa distro
/// (rodando, já que o Grok está nela), por `\\wsl.localhost`. "WSL" é a distro padrão: tenta as
/// que estão rodando.
fn read_generated(origin: &str, path: &str) -> Result<Vec<u8>, String> {
    if !grok::image_path_allowed(path) {
        return Err("caminho fora das sessões do Grok".into());
    }
    let candidates: Vec<PathBuf> = match origin {
        "Windows" => vec![PathBuf::from(path)],
        "WSL" => crate::wsl::running_distros().iter().filter_map(|d| grok::wsl_file(d, path)).collect(),
        distro => grok::wsl_file(distro, path).into_iter().collect(),
    };
    for file in candidates {
        if std::fs::metadata(&file).is_ok_and(|m| m.is_file() && m.len() <= MAX_GENERATED_BYTES) {
            return std::fs::read(&file).map_err(|e| e.to_string());
        }
    }
    Err("arquivo não encontrado".into())
}

impl GrokState {
    fn id(&mut self) -> u64 {
        self.next += 1;
        self.next
    }

    /// Os modelos para a interface, com o padrão marcado.
    fn models(&mut self, list: Vec<zefiro_integration_chat::agent::ModelOption>, current: Option<String>) -> Event {
        if self.default_model.is_none() {
            self.default_model.clone_from(&current);
        }
        if current.is_some() {
            self.model = current;
        }
        Event::Models(grok::with_default(list, self.default_model.as_deref()))
    }

    /// Abre a sessão (retoma a gravada ou começa outra) se ainda não há uma e a partida acabou.
    fn open(&mut self) -> Option<String> {
        if self.session.is_some() || self.opening.is_some() || self.starting.is_some() {
            return None;
        }
        let id = self.id();
        self.opening = Some(id);
        Some(match self.resume.take() {
            Some(session) => {
                let line = grok::session_load(id, &session, &self.cwd);
                self.loading = Some(session);
                line
            }
            None => grok::session_new(id, &self.cwd),
        })
    }

    /// A próxima pergunta da fila, com a troca de modelo antes (se ela pede outro).
    fn next_question(&mut self) -> Option<String> {
        if self.prompt.is_some() || self.switching.is_some() || self.queued.is_empty() {
            return None;
        }
        let session = self.session.clone()?;
        let q = self.queued.remove(0);
        if !q.model.is_empty() && self.model.as_deref() != Some(q.model.as_str()) {
            let id = self.id();
            let line = grok::set_model(id, &session, &q.model);
            self.switching = Some((id, q.model.clone(), q));
            return Some(line);
        }
        Some(self.ask(&session, &q))
    }

    fn ask(&mut self, session: &str, q: &Question) -> String {
        let id = self.id();
        self.prompt = Some(id);
        grok::prompt(id, session, &q.text, &q.images)
    }
}

/// O estado do Grok fica na caixa: ele é o maior dos três.
enum Backend { Claude, Codex(CodexState), Grok(Box<GrokState>) }

struct Running { child: Child, stdin: Option<ChildStdin>, origin: String, generation: u64, backend: Backend }

impl Running {
    fn write(&mut self, line: &str) {
        if let Some(stdin) = &mut self.stdin
            && let Err(e) = writeln!(stdin, "{line}").and_then(|_| stdin.flush())
        {
            // A saída do processo avisa a interface (Exited).
            tracing::warn!("chat: não consegui escrever para o processo: {e}");
        }
    }
}

fn run(rx: Receiver<Request>, back: Sender<Request>, inbox: Arc<Inbox>) {
    let mut running: Option<Running> = None;
    let mut interrupts = 0u32;
    while let Ok(req) = rx.recv() {
        // O processo pode ter saído sozinho (caiu, ou a sessão não existia).
        if let Some(r) = &mut running
            && matches!(r.child.try_wait(), Ok(Some(_)))
        {
            running = None;
        }
        match req {
            Request::Start(start) => {
                if running.is_some() {
                    continue;
                }
                let generation = inbox.generation.fetch_add(1, Ordering::SeqCst) + 1;
                match spawn(&start, generation, &inbox, &back) {
                    Ok(r) => {
                        tracing::info!(agente = start.agent.label(), origem = %r.origin, retomando = start.resume, "chat: processo iniciado");
                        inbox.push(generation, Signal::Ready { origin: r.origin.clone() });
                        running = Some(r);
                    }
                    Err(text) => {
                        tracing::warn!("chat: não consegui iniciar o {}: {text}", start.agent.label());
                        inbox.push(generation, Signal::Failed(text));
                    }
                }
            }
            Request::Send { text, images, model } => {
                let Some(r) = &mut running else { continue };
                let q = Question { text, images, model };
                match &mut r.backend {
                    Backend::Claude => r.write(&user_message(&q.text, &q.images)),
                    Backend::Codex(c) => {
                        let line = match (c.thread.clone(), c.opening) {
                            (Some(thread), _) => Some(c.turn(&thread, &q)),
                            (None, Some(_)) => {
                                c.queued.push(q);
                                None
                            }
                            (None, None) => {
                                c.queued.push(q);
                                let id = c.id();
                                c.opening = Some(id);
                                Some(codex::thread_start(id))
                            }
                        };
                        if let Some(line) = line {
                            r.write(&line);
                        }
                    }
                    Backend::Grok(g) => {
                        g.queued.push(q);
                        let lines: Vec<String> = [g.open(), g.next_question()].into_iter().flatten().collect();
                        for line in lines {
                            r.write(&line);
                        }
                    }
                }
            }
            Request::Interrupt => {
                let Some(r) = &mut running else { continue };
                interrupts += 1;
                let line = match &mut r.backend {
                    Backend::Claude => Some(interrupt_line(&format!("zefiro-{interrupts}"))),
                    Backend::Codex(c) => match (c.thread.clone(), c.turn.clone()) {
                        (Some(thread), Some(turn)) => Some(codex::turn_interrupt(c.id(), &thread, &turn)),
                        _ => None,
                    },
                    Backend::Grok(g) => match (&g.session, g.prompt) {
                        (Some(session), Some(_)) => Some(grok::cancel(session)),
                        _ => None,
                    },
                };
                if let Some(line) = line {
                    r.write(&line);
                }
            }
            Request::Server(generation, incoming) => {
                if let Some(r) = &mut running
                    && r.generation == generation
                {
                    codex_incoming(r, incoming, &inbox);
                }
            }
            Request::Grok(generation, incoming) => {
                if let Some(r) = &mut running
                    && r.generation == generation
                {
                    grok_incoming(r, incoming, &inbox);
                }
            }
            Request::Stop => {
                // Os sinais do processo que está saindo não interessam mais.
                inbox.generation.fetch_add(1, Ordering::SeqCst);
                if let Some(r) = running.take() {
                    finish(r);
                }
            }
        }
    }
    if let Some(r) = running.take() {
        finish(r);
    }
}

/// Uma linha do `codex app-server`: abre a conversa, manda as perguntas que esperavam, guarda o
/// turno (para parar) e repassa o resto à interface.
fn codex_incoming(r: &mut Running, incoming: Incoming, inbox: &Inbox) {
    let generation = r.generation;
    let Backend::Codex(c) = &mut r.backend else { return };
    let mut lines = Vec::new();
    match incoming {
        Incoming::Response { id, result } if Some(id) == c.opening => {
            c.opening = None;
            match codex::thread_id(&result) {
                Some(thread) => {
                    inbox.push(generation, Signal::Event(Event::Started { session: thread.clone() }));
                    for q in std::mem::take(&mut c.queued) {
                        lines.push(c.turn(&thread, &q));
                    }
                    c.thread = Some(thread);
                }
                None => inbox.push(generation, Signal::Event(Event::Finished { text: None, error: Some("O Codex não abriu a conversa.".into()) })),
            }
        }
        Incoming::Response { id, result } if id == c.models => {
            let models = codex::models(&result);
            c.default_model = models.iter().find(|m| m.default).map(|m| m.id.clone());
            inbox.push(generation, Signal::Event(Event::Models(models)));
        }
        Incoming::Response { .. } => {}
        Incoming::Failure { id, message } => {
            if Some(id) == c.opening {
                c.opening = None;
                c.queued.clear();
            }
            if id != c.models {
                inbox.push(generation, Signal::Event(Event::Finished { text: None, error: Some(message) }));
            }
        }
        Incoming::Event(Event::Turn(turn)) => c.turn = Some(turn),
        Incoming::Event(e) => {
            if matches!(e, Event::Finished { .. }) {
                c.turn = None;
            }
            inbox.push(generation, Signal::Event(e));
        }
        Incoming::ServerRequest { id } => lines.push(codex::reject(&id)),
        Incoming::Other => {}
    }
    for line in lines {
        r.write(&line);
    }
}

/// Uma linha do `grok agent stdio`: conduz a partida, abre a sessão, manda as perguntas que
/// esperavam, recusa os pedidos de permissão e repassa o resto à interface. Enquanto a sessão
/// gravada é retomada, o histórico que o Grok reenvia não vai para a tela.
fn grok_incoming(r: &mut Running, incoming: grok::Incoming, inbox: &Inbox) {
    let generation = r.generation;
    let Backend::Grok(g) = &mut r.backend else { return };
    let mut lines = Vec::new();
    let push = |e: Event| inbox.push(generation, Signal::Event(e));
    match incoming {
        grok::Incoming::Response { id, result } if g.starting == Some(id) && !g.initialized => {
            g.initialized = true;
            g.cwd = grok::workdir(&result).unwrap_or_default();
            if let Some(state) = grok::model_state(&result) {
                let (list, current) = grok::models(state);
                push(g.models(list, current));
            }
            let id = g.id();
            g.starting = Some(id);
            lines.push(grok::authenticate(id));
        }
        grok::Incoming::Failure { id, message } if g.starting == Some(id) && !g.initialized => {
            g.starting = None;
            g.queued.clear();
            push(Event::Finished { text: None, error: Some(message) });
        }
        // O login: sem ele, o `session/new` falha e diz o motivo.
        grok::Incoming::Response { id, .. } | grok::Incoming::Failure { id, .. } if g.starting == Some(id) => {
            g.starting = None;
            if g.resume.is_some() || !g.queued.is_empty() {
                lines.extend(g.open());
            }
        }
        grok::Incoming::Response { id, result } if g.opening == Some(id) => {
            g.opening = None;
            // O `session/load` não devolve o id: é o da sessão retomada.
            match grok::session_id(&result).or(g.loading.take()) {
                Some(session) => {
                    push(Event::Started { session: session.clone() });
                    if let Some(state) = grok::model_state(&result) {
                        let (list, current) = grok::models(state);
                        push(g.models(list, current));
                    }
                    g.session = Some(session);
                    lines.extend(g.next_question());
                }
                None => {
                    g.queued.clear();
                    push(Event::Finished { text: None, error: Some("O Grok não abriu a conversa.".into()) });
                }
            }
        }
        grok::Incoming::Failure { id, message } if g.opening == Some(id) => {
            g.opening = None;
            g.loading = None;
            g.queued.clear();
            push(Event::Finished { text: None, error: Some(message) });
        }
        // Troca de modelo: recusada, a pergunta vai com o modelo de antes.
        grok::Incoming::Response { id, .. } | grok::Incoming::Failure { id, .. } if g.switching.as_ref().is_some_and(|s| s.0 == id) => {
            let accepted = matches!(incoming, grok::Incoming::Response { .. });
            if let Some((_, model, q)) = g.switching.take() {
                if accepted {
                    g.model = Some(model);
                } else {
                    tracing::warn!(modelo = %model, "chat: o Grok recusou a troca de modelo");
                }
                if let Some(session) = g.session.clone() {
                    lines.push(g.ask(&session, &q));
                }
            }
        }
        grok::Incoming::Response { id, result } if g.prompt == Some(id) => {
            g.prompt = None;
            push(grok::stop(&result));
            lines.extend(g.next_question());
        }
        grok::Incoming::Failure { id, message } if g.prompt == Some(id) => {
            g.prompt = None;
            push(Event::Finished { text: None, error: Some(message) });
        }
        grok::Incoming::Response { .. } | grok::Incoming::Failure { .. } | grok::Incoming::Other => {}
        grok::Incoming::Event(Event::Delta { .. } | Event::Thinking) if g.loading.is_some() => {}
        grok::Incoming::Event(e) => push(e),
        grok::Incoming::Models { list, current } => push(g.models(list, current)),
        grok::Incoming::Tool(name) => tracing::debug!(ferramenta = %name, "chat: o Grok chamou uma ferramenta"),
        // O histórico reenviado pelo `session/load` não gera imagem de novo.
        grok::Incoming::ImageStarted(_) | grok::Incoming::ImageFile { .. } if g.loading.is_some() => {}
        grok::Incoming::ImageStarted(id) => {
            g.images.push(id);
            push(Event::ImageStarted);
        }
        // Lida aqui mesmo (poucos ms), para chegar antes do fim do turno.
        grok::Incoming::ImageFile { item, path } => match read_generated(&r.origin, &path) {
            Ok(bytes) => push(Event::Image { item, data: zefiro_integration_chat::image::base64(&bytes) }),
            Err(e) => {
                tracing::warn!(origem = %r.origin, "chat: não consegui ler a imagem do Grok: {e}");
                push(Event::ImageFailed);
            }
        },
        grok::Incoming::ToolFailed(id) => {
            if g.images.contains(&id) {
                push(Event::ImageFailed);
            }
        }
        grok::Incoming::ServerRequest(reply) => lines.push(reply),
    }
    for line in lines {
        r.write(&line);
    }
}

fn finish(mut r: Running) {
    drop(r.stdin.take());
    let started = Instant::now();
    while started.elapsed() < EXIT_GRACE {
        if matches!(r.child.try_wait(), Ok(Some(_))) {
            tracing::info!(origem = %r.origin, "chat: processo encerrado");
            return;
        }
        std::thread::sleep(Duration::from_millis(30));
    }
    let _ = r.child.kill();
    let _ = r.child.wait();
    tracing::info!(origem = %r.origin, "chat: processo encerrado à força");
}

/// O executável do agente no Windows: os instaladores e o `PATH`.
pub fn find_exe(agent: Agent) -> Option<PathBuf> {
    let env = |name: &str| PathBuf::from(std::env::var_os(name).unwrap_or_default());
    let path: Vec<PathBuf> = std::env::var_os("PATH").map(|p| std::env::split_paths(&p).collect()).unwrap_or_default();
    let (home, appdata, local_appdata) = (env("USERPROFILE"), env("APPDATA"), env("LOCALAPPDATA"));
    windows_candidates(agent, &UserDirs { home: &home, appdata: &appdata, local_appdata: &local_appdata }, &path).into_iter().find(|p| p.is_file())
}

/// A distro (rodando) tem o executável no shell de login?
fn wsl_has(distro: &str, binary: &str) -> bool {
    crate::system::hidden_output("wsl.exe", &["-d", distro, "--", "bash", "-lc", &wsl_probe(binary)], Duration::from_secs(8)).is_some()
}

/// Guarda o fim do stderr numa thread.
fn tail_stderr(stderr: Option<std::process::ChildStderr>, tail: Arc<Mutex<String>>) -> Option<std::thread::JoinHandle<()>> {
    let mut s = stderr?;
    std::thread::Builder::new()
        .name("zefiro-chat-err".into())
        .spawn(move || {
            let mut buf = [0u8; 1024];
            while let Ok(n) = s.read(&mut buf) {
                if n == 0 {
                    break;
                }
                if let Ok(mut t) = tail.lock() {
                    t.push_str(&String::from_utf8_lossy(&buf[..n]));
                    if t.len() > STDERR_TAIL {
                        let cut = t.len() - STDERR_TAIL;
                        let cut = (cut..t.len()).find(|&i| t.is_char_boundary(i)).unwrap_or(t.len());
                        t.drain(..cut);
                    }
                }
            }
        })
        .ok()
}

fn spawn(start: &Start, generation: u64, inbox: &Arc<Inbox>, back: &Sender<Request>) -> Result<Running, String> {
    use std::os::windows::process::CommandExt;
    let agent = start.agent;
    let windows_exe = if matches!(start.choice, Choice::Auto | Choice::Windows) { find_exe(agent) } else { None };
    let auto_wsl = start.choice == Choice::Auto && (windows_exe.is_none() || start.prefer == Prefer::Wsl);
    let running = if auto_wsl { crate::wsl::running_distros() } else { Vec::new() };
    let origin = pick_with(&start.choice, start.prefer, windows_exe, &running, &mut |d| wsl_has(d, agent.binary())).map_err(|e| pick_error_text(&e, agent))?;
    let args = match agent {
        Agent::Claude => {
            let session = if start.resume { Session::Resume(&start.session) } else { Session::New(&start.session) };
            claude_args(session, &start.model)
        }
        Agent::Codex => codex_args(),
        Agent::Grok => match &origin {
            Origin::Windows(_) => grok_args(&start.windows_dir.join(GROK_PROFILE).to_string_lossy()),
            Origin::Wsl(_) => grok_args(GROK_PROFILE),
        },
    };
    let mut command = match &origin {
        Origin::Windows(exe) => {
            std::fs::create_dir_all(&start.windows_dir).map_err(|e| format!("Não consegui criar a pasta do chat: {e}"))?;
            let mut c = Command::new(exe);
            c.args(&args).current_dir(&start.windows_dir);
            if agent == Agent::Grok {
                std::fs::write(start.windows_dir.join(GROK_PROFILE), grok_profile()).map_err(|e| format!("Não consegui gravar o perfil do Grok: {e}"))?;
                c.envs(GROK_ENV);
            }
            c
        }
        Origin::Wsl(distro) => {
            let mut c = Command::new("wsl.exe");
            match agent {
                Agent::Grok => c.args(wsl_args_with(distro.as_deref(), &grok_profile_script(), &GROK_ENV, agent.binary(), &args)),
                _ => c.args(wsl_args(distro.as_deref(), agent.binary(), &args)),
            };
            c
        }
    };
    let mut child = command
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .creation_flags(CREATE_NO_WINDOW)
        .spawn()
        .map_err(|e| format!("Não consegui iniciar o {} ({}): {e}", agent.label(), origin.label()))?;
    let stdin = child.stdin.take();
    let stdout = child.stdout.take();
    let tail = Arc::new(Mutex::new(String::new()));
    let err_thread = tail_stderr(child.stderr.take(), tail.clone());
    if let Some(out) = stdout {
        let (inbox, back) = (inbox.clone(), back.clone());
        let spawned = std::thread::Builder::new().name("zefiro-chat-out".into()).spawn(move || {
            for line in BufReader::new(out).lines() {
                let Ok(line) = line else { break };
                match agent {
                    Agent::Claude => match parse_line(&line) {
                        Event::Other => {}
                        e => inbox.push(generation, Signal::Event(e)),
                    },
                    Agent::Codex => match codex::parse(&line) {
                        Incoming::Other => {}
                        i => {
                            let _ = back.send(Request::Server(generation, i));
                        }
                    },
                    Agent::Grok => match grok::parse(&line) {
                        grok::Incoming::Other => {}
                        i => {
                            let _ = back.send(Request::Grok(generation, i));
                        }
                    },
                }
            }
            if let Some(t) = err_thread {
                let _ = t.join();
            }
            let stderr = tail.lock().map(|t| t.trim().to_string()).unwrap_or_default();
            inbox.push(generation, Signal::Exited { stderr });
        });
        if let Err(e) = spawned {
            let _ = child.kill();
            return Err(format!("Não consegui ler o {}: {e}", agent.label()));
        }
    }
    let backend = match agent {
        Agent::Claude => Backend::Claude,
        Agent::Codex => Backend::Codex(CodexState::default()),
        Agent::Grok => Backend::Grok(Box::new(GrokState { resume: (start.resume && !start.session.is_empty()).then(|| start.session.clone()), ..Default::default() })),
    };
    let mut r = Running { child, stdin, origin: origin.label(), generation, backend };
    if let Backend::Codex(c) = &mut r.backend {
        let mut lines = vec![codex::initialize(c.id(), env!("CARGO_PKG_VERSION")), codex::initialized()];
        c.models = c.id();
        lines.push(codex::model_list(c.models));
        if start.resume && !start.session.is_empty() {
            let id = c.id();
            c.opening = Some(id);
            lines.push(codex::thread_resume(id, &start.session));
        }
        for line in lines {
            r.write(&line);
        }
    }
    if let Backend::Grok(g) = &mut r.backend {
        let id = g.id();
        g.starting = Some(id);
        let line = grok::initialize(id, env!("CARGO_PKG_VERSION"));
        r.write(&line);
    }
    Ok(r)
}

/// Id novo (UUID v4 do Windows): o das conversas e o das sessões do Claude.
pub fn new_id() -> String {
    // SAFETY: CoCreateGuid só preenche o GUID devolvido.
    let g = unsafe { CoCreateGuid() }.unwrap_or_default();
    let d = g.data4;
    format!("{:08x}-{:04x}-{:04x}-{:02x}{:02x}-{:02x}{:02x}{:02x}{:02x}{:02x}{:02x}", g.data1, g.data2, g.data3, d[0], d[1], d[2], d[3], d[4], d[5], d[6], d[7])
}

/// Pasta do chat no Windows: `%LOCALAPPDATA%\zefiro\chat`.
pub fn windows_dir() -> PathBuf {
    Path::new(&std::env::var_os("LOCALAPPDATA").unwrap_or_default()).join(r"zefiro\chat")
}

/// Pasta das conversas: `%LOCALAPPDATA%\zefiro\chat\conversas`.
pub fn conversations_dir() -> PathBuf {
    windows_dir().join("conversas")
}

/// Pasta das imagens coladas: `%LOCALAPPDATA%\zefiro\chat\imagens`.
pub fn images_dir() -> PathBuf {
    windows_dir().join("imagens")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn question(text: &str, model: &str) -> Question {
        Question { text: text.into(), images: Vec::new(), model: model.into() }
    }

    #[test]
    fn grok_opens_after_the_start_and_resumes_the_saved_session() {
        let mut g = GrokState { starting: Some(1), resume: Some("s0".into()), cwd: "/c".into(), ..Default::default() };
        assert_eq!(g.open(), None, "a partida ainda não acabou");
        g.starting = None;
        let load = g.open().unwrap();
        assert!(load.contains("\"session/load\"") && load.contains("\"s0\""), "{load}");
        assert_eq!(g.loading.as_deref(), Some("s0"), "o histórico reenviado não vai para a tela");
        assert_eq!(g.open(), None, "uma abertura por vez");
        let mut fresh = GrokState { cwd: "/c".into(), ..Default::default() };
        assert!(fresh.open().unwrap().contains("\"session/new\""));
        assert_eq!(fresh.loading, None);
    }

    #[test]
    fn grok_switches_the_model_before_the_question_and_asks_one_at_a_time() {
        let mut g = GrokState { session: Some("s1".into()), model: Some("grok-4.7".into()), ..Default::default() };
        g.queued.push(question("oi", "grok-4.5"));
        let switch = g.next_question().unwrap();
        assert!(switch.contains("session/set_config_option") && switch.contains("\"grok-4.5\""), "{switch}");
        assert!(g.switching.is_some() && g.prompt.is_none());
        g.queued.push(question("outra", "grok-4.5"));
        assert_eq!(g.next_question(), None, "espera a troca");
        g.switching = None;
        g.model = Some("grok-4.5".into());
        let ask = g.next_question().unwrap();
        assert!(ask.contains("session/prompt") && ask.contains("outra"), "{ask}");
        assert_eq!(g.next_question(), None, "uma pergunta por vez");
        let mut same = GrokState { session: Some("s1".into()), model: Some("grok-4.7".into()), ..Default::default() };
        same.queued.push(question("oi", ""));
        assert!(same.next_question().unwrap().contains("session/prompt"), "modelo vazio não troca nada");
    }

    #[test]
    fn grok_marks_the_first_announced_model_as_the_default() {
        let mut g = GrokState::default();
        let list = |ids: &[&str]| ids.iter().map(|id| zefiro_integration_chat::agent::ModelOption { id: (*id).into(), name: (*id).into(), default: false }).collect::<Vec<_>>();
        let Event::Models(first) = g.models(list(&["grok-4.7", "grok-4.5"]), Some("grok-4.7".into())) else { panic!() };
        assert!(first[0].default && !first[1].default);
        let Event::Models(after) = g.models(list(&["grok-4.7", "grok-4.5"]), Some("grok-4.5".into())) else { panic!() };
        assert!(after[0].default, "o padrão não muda com a troca de modelo");
        assert_eq!(g.model.as_deref(), Some("grok-4.5"));
    }
}
