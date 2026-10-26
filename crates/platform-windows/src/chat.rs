//! Processo do chat: o `claude -p` (stream-json) ou o `codex app-server` (JSON-RPC), do Windows ou
//! do WSL, com stdin e stdout em pipes. Uma thread (`zefiro-chat`) escolhe a origem, inicia,
//! escreve, conduz o JSON-RPC do Codex e encerra; outras leem a saída do processo. Os sinais vão
//! para uma fila e acordam a interface com um único `post_user` até ela ler a fila.

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
use zefiro_integration_chat::command::{Session, UserDirs, claude_args, codex_args, windows_candidates, wsl_args, wsl_probe};
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
    /// Sessão do Claude ou `thread` do Codex (vazio = conversa nova no Codex).
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

    /// Uma pergunta, com as imagens PNG em base64. `model`: o do Codex neste turno (o Claude recebe
    /// o modelo na partida).
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

enum Backend { Claude, Codex(CodexState) }

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
    };
    let mut command = match &origin {
        Origin::Windows(exe) => {
            std::fs::create_dir_all(&start.windows_dir).map_err(|e| format!("Não consegui criar a pasta do chat: {e}"))?;
            let mut c = Command::new(exe);
            c.args(&args).current_dir(&start.windows_dir);
            c
        }
        Origin::Wsl(distro) => {
            let mut c = Command::new("wsl.exe");
            c.args(wsl_args(distro.as_deref(), agent.binary(), &args));
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
