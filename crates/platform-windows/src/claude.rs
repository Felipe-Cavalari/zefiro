//! Uso do Claude pelo endpoint OAuth do Claude Code, numa thread própria. O arquivo de credenciais é
//! relido a cada consulta (o Claude Code renova o token sozinho); o Zefiro nunca renova nem grava o
//! token, e ele nunca vai para o log.

use std::path::PathBuf;
use std::sync::mpsc::{self, RecvTimeoutError, Sender};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use windows::Win32::System::WinRT::{RO_INIT_MULTITHREADED, RoInitialize, RoUninitialize};
use zefiro_integration_usage::claude::{ClaudeUsage, TokenState, UsageError, parse_claude, token_state};

const URL: &str = "https://api.anthropic.com/api/oauth/usage";
const BETA: &str = "oauth-2025-04-20";
const INTERVAL: Duration = Duration::from_secs(120);
/// Abrir o painel só consulta de novo se o dado tiver mais que isto.
const FRESH: Duration = Duration::from_secs(60);
const TIMEOUT: Duration = Duration::from_secs(15);

enum Wake { Refresh, Stop }

type Shared = Arc<Mutex<Option<(Instant, Result<ClaudeUsage, UsageError>)>>>;

pub struct ClaudeService {
    tx: Sender<Wake>,
    state: Shared,
    thread: Option<std::thread::JoinHandle<()>>,
}

fn now_ms() -> i64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |d| d.as_millis() as i64)
}

fn query(credentials: &PathBuf) -> Result<ClaudeUsage, UsageError> {
    let text = std::fs::read_to_string(credentials).unwrap_or_default();
    let token = match token_state(&text, now_ms()) {
        TokenState::Valid(token) => token,
        TokenState::Expired => return Err(UsageError::Expired),
        TokenState::Missing => return Err(UsageError::Missing),
    };
    let auth = format!("Bearer {token}");
    let (status, body) = crate::http::get_with(URL, &[("Authorization", &auth), ("anthropic-beta", BETA)], TIMEOUT).map_err(|e| {
        tracing::debug!("uso do Claude: falha de rede: {e}");
        UsageError::Network
    })?;
    match status {
        200 => parse_claude(&body),
        401 => Err(UsageError::Expired),
        other => Err(UsageError::Http(other)),
    }
}

impl ClaudeService {
    /// Consulta ao iniciar, a cada 120 s e no `refresh` (se o dado tiver mais de 60 s). `notify` é
    /// chamado (em outra thread) depois de cada consulta.
    pub fn start(credentials: PathBuf, notify: Arc<dyn Fn() + Send + Sync>) -> std::io::Result<ClaudeService> {
        let (tx, rx) = mpsc::channel();
        let state: Shared = Default::default();
        let shared = state.clone();
        let thread = std::thread::Builder::new().name("zefiro-claude".into()).spawn(move || {
            // SAFETY: WinRT nesta thread, desfeito no fim dela.
            let inited = unsafe { RoInitialize(RO_INIT_MULTITHREADED) }.is_ok();
            let mut last: Option<Instant> = None;
            let mut wake = Wake::Refresh;
            while let Wake::Refresh = wake {
                if last.is_none_or(|t| t.elapsed() >= FRESH) {
                    let result = query(&credentials);
                    last = Some(Instant::now());
                    let changed = shared
                        .lock()
                        .map(|mut s| {
                            let changed = s.as_ref().is_none_or(|(_, old)| *old != result);
                            *s = Some((Instant::now(), result.clone()));
                            changed
                        })
                        .unwrap_or(false);
                    // Só registra quando o erro muda (sem login, seriam 720 linhas por dia).
                    if changed && let Err(e) = &result {
                        tracing::info!(?e, "uso do Claude indisponível");
                    }
                    // Avisa sempre: mesmo sem mudança, o "Atualizado há" do painel muda.
                    notify();
                }
                wake = match rx.recv_timeout(INTERVAL) {
                    Ok(w) => w,
                    Err(RecvTimeoutError::Timeout) => {
                        // O ciclo de 120 s sempre consulta.
                        last = None;
                        Wake::Refresh
                    }
                    Err(RecvTimeoutError::Disconnected) => Wake::Stop,
                };
            }
            if inited {
                // SAFETY: par do RoInitialize.
                unsafe { RoUninitialize() };
            }
        })?;
        Ok(ClaudeService { tx, state, thread: Some(thread) })
    }

    /// Último resultado e quando chegou.
    pub fn latest(&self) -> Option<(Instant, Result<ClaudeUsage, UsageError>)> {
        self.state.lock().ok().and_then(|s| s.clone())
    }

    /// Ao abrir o painel.
    pub fn refresh(&self) {
        let _ = self.tx.send(Wake::Refresh);
    }
}

impl Drop for ClaudeService {
    fn drop(&mut self) {
        let _ = self.tx.send(Wake::Stop);
        if let Some(t) = self.thread.take() {
            let _ = t.join();
        }
    }
}
