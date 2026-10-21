//! Uso do Claude pelo endpoint OAuth do Claude Code, numa thread própria, para cada conta achada no
//! Windows e nas distros do WSL rodando. O arquivo de credenciais é relido a cada consulta (o Claude
//! Code renova o token sozinho); o Zefiro nunca renova nem grava o token, e ele nunca vai para o log.

use std::path::PathBuf;
use std::sync::mpsc::{self, RecvTimeoutError, Sender};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use windows::Win32::System::WinRT::{RO_INIT_MULTITHREADED, RoInitialize, RoUninitialize};
use zefiro_integration_usage::claude::{ClaudeEntry, ClaudeFound, ClaudeSnapshot, ClaudeUsage, TokenState, UsageError, first_valid_token, group_claude, inherit_claude, parse_claude, parse_identity, plan_name};
use zefiro_integration_usage::origins::{Account, Discover, Origin, accounts_summary, carry_stopped, many_distros, origins_label};

const URL: &str = "https://api.anthropic.com/api/oauth/usage";
const BETA: &str = "oauth-2025-04-20";
const INTERVAL: Duration = Duration::from_secs(120);
/// Abrir o painel só consulta de novo se o dado tiver mais que isto.
const FRESH: Duration = Duration::from_secs(60);
const TIMEOUT: Duration = Duration::from_secs(15);

enum Wake { Refresh, Stop }

type Shared = Arc<Mutex<Option<ClaudeSnapshot>>>;

pub struct ClaudeService {
    tx: Sender<Wake>,
    state: Shared,
}

fn now_ms() -> i64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |d| d.as_millis() as i64)
}

/// Uso de uma conta, com o primeiro token válido entre as credenciais dela.
fn query(credentials: &[PathBuf]) -> Result<ClaudeUsage, UsageError> {
    let texts: Vec<String> = credentials.iter().map(|p| std::fs::read_to_string(p).unwrap_or_default()).collect();
    let token = match first_valid_token(&texts, now_ms()) {
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

/// Uma origem com credenciais do Claude Code: quem é (`.claude.json`) e o plano.
fn found(origin: &Origin) -> Option<ClaudeFound> {
    let credentials = std::fs::read_to_string(origin.home.join(".claude").join(".credentials.json")).ok()?;
    let identity = parse_identity(&std::fs::read_to_string(origin.home.join(".claude.json")).unwrap_or_default());
    Some(ClaudeFound { origin: origin.clone(), identity, plan: plan_name(&credentials) })
}

/// Um ciclo: descobre as contas, consulta cada uma e lembra as de distros que pararam.
fn cycle(discover: &Discover, previous: &[Account<ClaudeEntry>]) -> ClaudeSnapshot {
    let discovery = discover();
    let mut accounts = group_claude(discovery.origins.iter().filter_map(found).collect());
    inherit_claude(previous, &mut accounts);
    let many = many_distros(&accounts);
    for account in &mut accounts {
        let result = query(&account.data.credentials);
        let old = previous.iter().find(|p| p.key == account.key).and_then(|p| p.data.result.as_ref());
        // Só registra quando o erro muda (sem login, seriam 720 linhas por dia).
        if let Err(e) = &result
            && old != Some(&result)
        {
            tracing::info!(origem = %origins_label(&account.origins, many), ?e, "uso do Claude indisponível");
        }
        if let Ok(u) = &result {
            account.data.last_ok = Some(u.clone());
        }
        account.data.result = Some(result);
    }
    carry_stopped(previous, &mut accounts, &discovery.running);
    ClaudeSnapshot { accounts, wsl_stopped: discovery.wsl_stopped, at: Some(Instant::now()) }
}

impl ClaudeService {
    /// Consulta ao iniciar, a cada 120 s e no `refresh` (se o dado tiver mais de 60 s). `discover`
    /// acha as origens (Windows e WSL rodando) a cada consulta; `notify` é chamado (em outra thread)
    /// depois de cada consulta.
    pub fn start(discover: Discover, notify: Arc<dyn Fn() + Send + Sync>) -> std::io::Result<ClaudeService> {
        let (tx, rx) = mpsc::channel();
        let state: Shared = Default::default();
        let shared = state.clone();
        std::thread::Builder::new().name("zefiro-claude".into()).spawn(move || {
            // SAFETY: WinRT nesta thread, desfeito no fim dela.
            let inited = unsafe { RoInitialize(RO_INIT_MULTITHREADED) }.is_ok();
            let mut last: Option<Instant> = None;
            let mut summary = String::new();
            let mut wake = Wake::Refresh;
            while let Wake::Refresh = wake {
                if last.is_none_or(|t| t.elapsed() >= FRESH) {
                    let previous = shared.lock().ok().and_then(|s| s.as_ref().map(|s| s.accounts.clone())).unwrap_or_default();
                    let snapshot = cycle(&discover, &previous);
                    last = Some(Instant::now());
                    let text = accounts_summary("Claude", &snapshot.accounts);
                    if text != summary {
                        tracing::info!("{text}");
                        summary = text;
                    }
                    if let Ok(mut s) = shared.lock() {
                        *s = Some(snapshot);
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
        Ok(ClaudeService { tx, state })
    }

    /// As contas, se há WSL parado e quando terminou a última consulta.
    pub fn latest(&self) -> Option<ClaudeSnapshot> {
        self.state.lock().ok().and_then(|s| s.clone())
    }

    /// Ao abrir o painel.
    pub fn refresh(&self) {
        let _ = self.tx.send(Wake::Refresh);
    }
}

/// Não espera a thread: ela pode estar presa num `wsl.exe`, num `\wsl.localhost` ou num HTTP, e
/// quem solta o serviço é a thread da UI. A thread sai sozinha ao ver o `Stop`.
impl Drop for ClaudeService {
    fn drop(&mut self) {
        let _ = self.tx.send(Wake::Stop);
    }
}
