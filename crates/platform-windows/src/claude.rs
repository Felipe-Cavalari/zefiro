//! Uso do Claude pelo endpoint OAuth do Claude Code, numa thread própria, para cada conta achada no
//! Windows e nas distros do WSL rodando. O arquivo de credenciais é relido a cada consulta (o Claude
//! Code renova o token sozinho); o Zefiro nunca renova nem grava o token, e ele nunca vai para o log.

use std::sync::mpsc::{self, RecvTimeoutError, Sender};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use windows::Win32::System::WinRT::{RO_INIT_MULTITHREADED, RoInitialize, RoUninitialize};
use zefiro_integration_usage::claude::{ClaudeEntry, ClaudeFound, ClaudeSnapshot, ClaudeUsage, MIN_FORCED, TokenState, UsageError, first_valid_token, group_claude, inherit_claude, parse_claude, parse_identity, plan_name, rate_limit_wait, should_query};
use zefiro_integration_usage::origins::{Account, Discover, Origin, accounts_summary, carry_stopped, many_distros, origins_label};

const URL: &str = "https://api.anthropic.com/api/oauth/usage";
const BETA: &str = "oauth-2025-04-20";
/// A API de uso responde 429 com consultas a cada 2 min (medido em 2026-10-21, já sem o cache).
const INTERVAL: Duration = Duration::from_secs(300);
/// Abrir o painel só consulta de novo se o dado tiver mais que isto.
const FRESH: Duration = Duration::from_secs(120);
const TIMEOUT: Duration = Duration::from_secs(15);

/// `Force` é o botão de atualizar do painel: consulta se a última tiver mais de 30 s.
enum Wake { Refresh, Force, Stop }

/// Resposta de uma consulta e o `Retry-After` de um 429, em segundos.
type Reply = (Result<ClaudeUsage, UsageError>, Option<u64>);

type Shared = Arc<Mutex<Option<ClaudeSnapshot>>>;

pub struct ClaudeService {
    tx: Sender<Wake>,
    state: Shared,
}

fn now_ms() -> i64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |d| d.as_millis() as i64)
}

/// Uso de uma conta, com o primeiro token válido entre os textos das credenciais dela.
fn query(texts: &[String]) -> Reply {
    let token = match first_valid_token(texts, now_ms()) {
        TokenState::Valid(token) => token,
        TokenState::Expired => return (Err(UsageError::Expired), None),
        TokenState::Missing => return (Err(UsageError::Missing), None),
    };
    let auth = format!("Bearer {token}");
    let response = match crate::http::get_with_retry_after(URL, &[("Authorization", &auth), ("anthropic-beta", BETA)], TIMEOUT) {
        Ok(r) => r,
        Err(e) => {
            tracing::debug!("uso do Claude: falha de rede: {e}");
            return (Err(UsageError::Network), None);
        }
    };
    let (status, body, retry_after) = response;
    let result = match status {
        200 => parse_claude(&body),
        401 => Err(UsageError::Expired),
        other => Err(UsageError::Http(other)),
    };
    (result, retry_after)
}

/// Uma origem com credenciais do Claude Code: quem é (`.claude.json`) e o plano.
fn found(origin: &Origin) -> Option<ClaudeFound> {
    let credentials = std::fs::read_to_string(origin.home.join(".claude").join(".credentials.json")).ok()?;
    let identity = parse_identity(&std::fs::read_to_string(origin.home.join(".claude.json")).unwrap_or_default());
    Some(ClaudeFound { origin: origin.clone(), identity, plan: plan_name(&credentials) })
}

/// Um ciclo: descobre as contas, consulta cada uma e lembra as de distros que pararam.
fn cycle(discover: &Discover, previous: &[Account<ClaudeEntry>], query: &dyn Fn(&[String]) -> Reply) -> ClaudeSnapshot {
    let discovery = discover();
    let mut accounts = group_claude(discovery.origins.iter().filter_map(found).collect());
    inherit_claude(previous, &mut accounts);
    let many = many_distros(&accounts);
    // Tudo de `\\wsl.localhost` é lido agora, logo depois de ver a distro rodando: depois do HTTP
    // das outras contas ela pode ter parado, e ler de novo a ligaria.
    let texts: Vec<Vec<String>> = accounts.iter().map(|a| a.data.credentials.iter().map(|p| std::fs::read_to_string(p).unwrap_or_default()).collect()).collect();
    for (account, texts) in accounts.iter_mut().zip(texts) {
        let now = SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |d| d.as_secs() as i64);
        // Em pausa depois de um 429: fica com o último resultado.
        if !should_query(&account.data, now) {
            continue;
        }
        let (result, retry_after) = query(&texts);
        if result == Err(UsageError::Http(429)) {
            account.data.limited += 1;
            account.data.retry_at = Some(now + rate_limit_wait(retry_after, account.data.limited));
        } else {
            account.data.limited = 0;
            account.data.retry_at = None;
        }
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
    /// Consulta ao iniciar, a cada 5 min, no `refresh` (se o dado tiver mais de 2 min) e no
    /// `refresh_now` (se tiver mais de 30 s). Conta com 429 fica em pausa (`rate_limit_wait`). `discover`
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
            loop {
                let rest = match wake {
                    Wake::Refresh => FRESH,
                    Wake::Force => MIN_FORCED,
                    Wake::Stop => break,
                };
                if last.is_none_or(|t| t.elapsed() >= rest) {
                    let previous = shared.lock().ok().and_then(|s| s.as_ref().map(|s| s.accounts.clone())).unwrap_or_default();
                    let snapshot = cycle(&discover, &previous, &query);
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
                } else if matches!(wake, Wake::Force) {
                    // Botão recusado (menos de 30 s): o painel sai do "Atualizando…".
                    notify();
                }
                wake = match rx.recv_timeout(INTERVAL) {
                    Ok(w) => w,
                    Err(RecvTimeoutError::Timeout) => {
                        // O ciclo de 5 min sempre consulta.
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

    /// Botão de atualizar do painel.
    pub fn refresh_now(&self) {
        let _ = self.tx.send(Wake::Force);
    }
}

/// Não espera a thread: ela pode estar presa num `wsl.exe`, num `\\wsl.localhost` ou num HTTP, e
/// quem solta o serviço é a thread da UI. A thread sai sozinha ao ver o `Stop`.
impl Drop for ClaudeService {
    fn drop(&mut self) {
        let _ = self.tx.send(Wake::Stop);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;
    use zefiro_integration_usage::origins::{Discovery, OriginKind};

    /// Pasta pessoal de teste com credenciais inventadas (token falso, válido até 2100).
    fn home(name: &str, uuid: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("zefiro-claude-{name}-{}", std::process::id()));
        std::fs::create_dir_all(dir.join(".claude")).unwrap();
        std::fs::write(dir.join(".claude").join(".credentials.json"), r#"{"claudeAiOauth": {"accessToken": "t-falso", "expiresAt": 4102444800000}}"#).unwrap();
        std::fs::write(dir.join(".claude.json"), format!(r#"{{"oauthAccount": {{"accountUuid": "{uuid}"}}}}"#)).unwrap();
        dir
    }

    #[test]
    fn credentials_are_read_before_any_query() {
        // Ler `\wsl.localhost` depois do HTTP das outras contas pode cair numa distro que acabou de
        // parar e ligá-la de novo: as credenciais são lidas todas logo depois de descobrir as origens.
        let (win, wsl) = (home("win", "u1"), home("wsl", "u2"));
        let origins = vec![Origin { kind: OriginKind::Windows, home: win.clone() }, Origin { kind: OriginKind::Wsl("U".into()), home: wsl.clone() }];
        let discover: Discover = Arc::new(move || Discovery { origins: origins.clone(), running: vec!["U".into()], wsl_stopped: false });
        let wsl_credentials = wsl.join(".claude").join(".credentials.json");
        let query = |texts: &[String]| -> Reply {
            let _ = std::fs::remove_file(&wsl_credentials);
            match first_valid_token(texts, now_ms()) {
                TokenState::Valid(_) => (Ok(ClaudeUsage::default()), None),
                _ => (Err(UsageError::Missing), None),
            }
        };
        let snapshot = cycle(&discover, &[], &query);
        assert_eq!(snapshot.accounts.len(), 2);
        assert!(snapshot.accounts.iter().all(|a| matches!(a.data.result, Some(Ok(_)))), "{:?}", snapshot.accounts.iter().map(|a| &a.data.result).collect::<Vec<_>>());
        std::fs::remove_dir_all(&win).unwrap();
        std::fs::remove_dir_all(&wsl).unwrap();
    }

    #[test]
    fn rate_limited_account_pauses_and_is_not_queried_again() {
        let win = home("limited", "u1");
        let origins = vec![Origin { kind: OriginKind::Windows, home: win.clone() }];
        let discover: Discover = Arc::new(move || Discovery { origins: origins.clone(), ..Default::default() });
        let calls = std::cell::Cell::new(0);
        let limited = |_: &[String]| -> Reply {
            calls.set(calls.get() + 1);
            (Err(UsageError::Http(429)), Some(90))
        };
        let first = cycle(&discover, &[], &limited);
        let entry = &first.accounts[0].data;
        let now = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs() as i64;
        assert!(entry.retry_at.is_some_and(|t| (now + 85..=now + 95).contains(&t)), "{:?}", entry.retry_at);
        assert_eq!(entry.limited, 1);
        let second = cycle(&discover, &first.accounts, &limited);
        assert_eq!(calls.get(), 1, "a conta em pausa não é consultada");
        assert_eq!(second.accounts[0].data.result, Some(Err(UsageError::Http(429))));
        let ok = |_: &[String]| -> Reply { (Ok(ClaudeUsage::default()), None) };
        let mut resumed = second.accounts.clone();
        resumed[0].data.retry_at = Some(0);
        let third = cycle(&discover, &resumed, &ok);
        assert_eq!((third.accounts[0].data.retry_at, third.accounts[0].data.limited), (None, 0));
        std::fs::remove_dir_all(&win).unwrap();
    }
}
