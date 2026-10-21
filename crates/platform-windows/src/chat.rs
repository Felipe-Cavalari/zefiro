//! Processo do chat: o `claude -p` do Windows ou do WSL com stdin e stdout em pipes. Uma thread
//! (`zefiro-chat`) escolhe a origem, inicia, escreve e encerra; outras duas leem a saída do processo.
//! Os sinais vão para uma fila e acordam a interface com um único `post_user` até ela ler a fila.

use std::io::{BufRead, BufReader, Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdin, Command, Stdio};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::mpsc::{Receiver, Sender, channel};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use windows::Win32::System::Com::CoCreateGuid;
use zefiro_integration_chat::command::{Session, WSL_PROBE, claude_args, windows_candidates, wsl_args};
use zefiro_integration_chat::origin::{Choice, Origin, pick, pick_error_text};
use zefiro_integration_chat::protocol::{Event, interrupt_line, parse_line};

const CREATE_NO_WINDOW: u32 = 0x0800_0000;
/// Fim do stderr guardado para explicar uma saída sem `result`.
const STDERR_TAIL: usize = 2000;
/// Ao encerrar: tempo para o `claude` sair sozinho depois do fim do stdin.
const EXIT_GRACE: Duration = Duration::from_millis(1500);

/// O que o processo conta à interface.
#[derive(Debug, Clone, PartialEq)]
pub enum Signal {
    /// O processo subiu nesta origem ("Windows", nome da distro).
    Ready { origin: String },
    Event(Event),
    /// O processo terminou (sozinho ou caiu). `stderr`: o fim do que ele escreveu lá.
    Exited { stderr: String },
    /// Não deu para iniciar (sem `claude`, distro inexistente…). Texto para o painel.
    Failed(String),
}

/// O que a interface pede para a partida.
#[derive(Debug, Clone)]
pub struct Start {
    pub choice: Choice,
    pub model: String,
    pub session: String,
    /// `--resume` (a sessão já existe nesta origem) ou `--session-id`.
    pub resume: bool,
    /// Pasta do chat no Windows (`%LOCALAPPDATA%\zefiro\chat`).
    pub windows_dir: PathBuf,
}

enum Request { Start(Start), Send(String), Interrupt, Stop }

struct Inbox {
    queue: Mutex<Vec<Signal>>,
    posted: AtomicBool,
    /// Processo atual; sinais de um processo anterior (encerrado) são descartados.
    generation: AtomicU64,
    code: u32,
}

impl Inbox {
    fn push(&self, generation: u64, signal: Signal) {
        if generation != self.generation.load(Ordering::SeqCst) {
            return;
        }
        if let Ok(mut q) = self.queue.lock() {
            q.push(signal);
        }
        if !self.posted.swap(true, Ordering::SeqCst) {
            crate::window::post_user(self.code);
        }
    }
}

pub struct ChatService { tx: Sender<Request>, inbox: Arc<Inbox> }

impl ChatService {
    /// `code`: o `WinEvent::User` que acorda a interface.
    pub fn new(code: u32) -> std::io::Result<ChatService> {
        let inbox = Arc::new(Inbox { queue: Mutex::new(Vec::new()), posted: AtomicBool::new(false), generation: AtomicU64::new(0), code });
        let (tx, rx) = channel();
        let worker = inbox.clone();
        std::thread::Builder::new().name("zefiro-chat".into()).spawn(move || run(rx, worker))?;
        Ok(ChatService { tx, inbox })
    }

    /// Sobe o processo se ele não está rodando (sem custo de tokens: nada é pedido ainda).
    pub fn start(&self, start: Start) {
        let _ = self.tx.send(Request::Start(start));
    }

    /// Uma linha do protocolo (uma pergunta).
    pub fn send(&self, line: String) {
        let _ = self.tx.send(Request::Send(line));
    }

    /// Para a resposta em andamento; o processo continua.
    pub fn interrupt(&self) {
        let _ = self.tx.send(Request::Interrupt);
    }

    /// Encerra o processo (fim do stdin e, se ele não sair, `kill`).
    pub fn stop(&self) {
        let _ = self.tx.send(Request::Stop);
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

struct Running { child: Child, stdin: Option<ChildStdin>, origin: String }

fn run(rx: Receiver<Request>, inbox: Arc<Inbox>) {
    let mut running: Option<Running> = None;
    let mut interrupts = 0u32;
    while let Ok(cmd) = rx.recv() {
        // O processo pode ter saído sozinho (caiu, ou a sessão não existia).
        if let Some(r) = &mut running
            && matches!(r.child.try_wait(), Ok(Some(_)))
        {
            running = None;
        }
        match cmd {
            Request::Start(start) => {
                if running.is_some() {
                    continue;
                }
                let generation = inbox.generation.fetch_add(1, Ordering::SeqCst) + 1;
                match spawn(&start, generation, &inbox) {
                    Ok(r) => {
                        tracing::info!(origem = %r.origin, retomando = start.resume, "chat: processo do Claude iniciado");
                        inbox.push(generation, Signal::Ready { origin: r.origin.clone() });
                        running = Some(r);
                    }
                    Err(text) => {
                        tracing::warn!("chat: não consegui iniciar o Claude: {text}");
                        inbox.push(generation, Signal::Failed(text));
                    }
                }
            }
            Request::Send(line) => {
                if let Some(r) = &mut running
                    && let Some(stdin) = &mut r.stdin
                    && let Err(e) = writeln!(stdin, "{line}").and_then(|_| stdin.flush())
                {
                    // A saída do processo avisa a interface (Exited).
                    tracing::warn!("chat: não consegui escrever para o Claude: {e}");
                }
            }
            Request::Interrupt => {
                if let Some(r) = &mut running
                    && let Some(stdin) = &mut r.stdin
                {
                    interrupts += 1;
                    let _ = writeln!(stdin, "{}", interrupt_line(&format!("zefiro-{interrupts}"))).and_then(|_| stdin.flush());
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

fn finish(mut r: Running) {
    drop(r.stdin.take());
    let started = Instant::now();
    while started.elapsed() < EXIT_GRACE {
        if matches!(r.child.try_wait(), Ok(Some(_))) {
            tracing::info!(origem = %r.origin, "chat: processo do Claude encerrado");
            return;
        }
        std::thread::sleep(Duration::from_millis(30));
    }
    let _ = r.child.kill();
    let _ = r.child.wait();
    tracing::info!(origem = %r.origin, "chat: processo do Claude encerrado à força");
}

/// O `claude.exe` do Windows: instalador nativo, npm ou `PATH`.
pub fn find_claude_exe() -> Option<PathBuf> {
    let env = |name: &str| PathBuf::from(std::env::var_os(name).unwrap_or_default());
    let path: Vec<PathBuf> = std::env::var_os("PATH").map(|p| std::env::split_paths(&p).collect()).unwrap_or_default();
    windows_candidates(&env("USERPROFILE"), &env("APPDATA"), &path).into_iter().find(|p| p.is_file())
}

/// A distro (rodando) tem `claude` no shell de login?
fn wsl_has_claude(distro: &str) -> bool {
    crate::system::hidden_output("wsl.exe", &["-d", distro, "--", "bash", "-lc", WSL_PROBE], Duration::from_secs(8)).is_some()
}

fn spawn(start: &Start, generation: u64, inbox: &Arc<Inbox>) -> Result<Running, String> {
    use std::os::windows::process::CommandExt;
    let windows_exe = if matches!(start.choice, Choice::Auto | Choice::Windows) { find_claude_exe() } else { None };
    let running = if start.choice == Choice::Auto && windows_exe.is_none() { crate::wsl::running_distros() } else { Vec::new() };
    let origin = pick(&start.choice, windows_exe, &running, &mut |d| wsl_has_claude(d)).map_err(|e| pick_error_text(&e).to_string())?;
    let session = if start.resume { Session::Resume(&start.session) } else { Session::New(&start.session) };
    let args = claude_args(session, &start.model);
    let mut command = match &origin {
        Origin::Windows(exe) => {
            std::fs::create_dir_all(&start.windows_dir).map_err(|e| format!("Não consegui criar a pasta do chat: {e}"))?;
            let mut c = Command::new(exe);
            c.args(&args).current_dir(&start.windows_dir);
            c
        }
        Origin::Wsl(distro) => {
            let mut c = Command::new("wsl.exe");
            c.args(wsl_args(distro.as_deref(), &args));
            c
        }
    };
    let mut child = command
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .creation_flags(CREATE_NO_WINDOW)
        .spawn()
        .map_err(|e| format!("Não consegui iniciar o Claude Code ({}): {e}", origin.label()))?;
    let stdin = child.stdin.take();
    let stdout = child.stdout.take();
    let stderr = child.stderr.take();
    let tail = Arc::new(Mutex::new(String::new()));
    let err_thread = stderr.and_then(|mut s| {
        let tail = tail.clone();
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
    });
    if let Some(out) = stdout {
        let inbox = inbox.clone();
        let spawned = std::thread::Builder::new().name("zefiro-chat-out".into()).spawn(move || {
            for line in BufReader::new(out).lines() {
                let Ok(line) = line else { break };
                match parse_line(&line) {
                    Event::Other => {}
                    e => inbox.push(generation, Signal::Event(e)),
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
            return Err(format!("Não consegui ler o Claude Code: {e}"));
        }
    }
    Ok(Running { child, stdin, origin: origin.label() })
}

/// Id novo de sessão (UUID v4 do Windows).
pub fn new_session_id() -> String {
    // SAFETY: CoCreateGuid só preenche o GUID devolvido.
    let g = unsafe { CoCreateGuid() }.unwrap_or_default();
    let d = g.data4;
    format!("{:08x}-{:04x}-{:04x}-{:02x}{:02x}-{:02x}{:02x}{:02x}{:02x}{:02x}{:02x}", g.data1, g.data2, g.data3, d[0], d[1], d[2], d[3], d[4], d[5], d[6], d[7])
}

/// Pasta do chat no Windows: `%LOCALAPPDATA%\zefiro\chat`.
pub fn windows_dir() -> PathBuf {
    Path::new(&std::env::var_os("LOCALAPPDATA").unwrap_or_default()).join(r"zefiro\chat")
}
