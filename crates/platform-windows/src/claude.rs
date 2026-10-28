//! Uso do Claude pelo `/usage` do próprio Claude Code (`claude -p /usage`, ADR-0025), numa thread
//! própria, para cada conta achada no Windows e nas distros do WSL rodando. Quem lê o token, renova
//! o login e fala com a Anthropic é o Claude Code; o Zefiro só lê o `.claude.json` (conta e plano).

use std::io::Read;
use std::os::windows::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::mpsc::{self, RecvTimeoutError, Sender};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use zefiro_integration_chat::agent::Agent;
use zefiro_integration_usage::claude::{ClaudeEntry, ClaudeFound, ClaudeSnapshot, ClaudeUsage, MIN_FORCED, UsageError, group_claude, inherit_claude, parse_identity, parse_usage_command, plan_name, rate_limit_wait, should_query, usage_args, usage_wsl_args, wsl_user};
use zefiro_integration_usage::origins::{Account, Discover, Origin, OriginKind, accounts_summary, carry_stopped, many_distros, origins_label};

/// O serviço de uso respondia 429 com consultas a cada 2 min (medido em 2026-10-21); o Claude Code
/// usa o mesmo serviço.
const INTERVAL: Duration = Duration::from_secs(300);
/// Abrir o painel só consulta de novo se o dado tiver mais que isto.
const FRESH: Duration = Duration::from_secs(120);
/// O `/usage` leva ~2,5 s; um WSL que acabou de acordar, mais.
const TIMEOUT: Duration = Duration::from_secs(30);
const CREATE_NO_WINDOW: u32 = 0x0800_0000;

/// `Force` é o botão de atualizar do painel: consulta se a última tiver mais de 60 s.
enum Wake { Refresh, Force, Stop }

type Shared = Arc<Mutex<Option<ClaudeSnapshot>>>;

/// As origens de uma conta, cada uma com a pasta pessoal, na ordem de preferência.
type Origins = [(OriginKind, PathBuf)];

pub struct ClaudeService {
    tx: Sender<Wake>,
    state: Shared,
}

/// Saída de um processo sem janela, com o código de saída. O stdout é lido numa thread, porque o
/// stream-json passa do buffer do pipe; passou do `TIMEOUT`, o processo é encerrado.
fn run(mut command: Command) -> Result<(Option<i32>, String), UsageError> {
    command.stdin(Stdio::null()).stdout(Stdio::piped()).stderr(Stdio::null()).creation_flags(CREATE_NO_WINDOW);
    let mut child = command.spawn().map_err(|_| UsageError::NotInstalled)?;
    let mut stdout = child.stdout.take().ok_or(UsageError::Network)?;
    let (tx, rx) = mpsc::channel();
    std::thread::spawn(move || {
        let mut text = String::new();
        let _ = stdout.read_to_string(&mut text);
        let _ = tx.send(text);
    });
    let start = Instant::now();
    let status = loop {
        match child.try_wait() {
            Ok(Some(status)) => break status,
            Ok(None) if start.elapsed() < TIMEOUT => std::thread::sleep(Duration::from_millis(50)),
            _ => {
                let _ = child.kill();
                let _ = child.wait();
                tracing::debug!("uso do Claude: o claude passou de {} s", TIMEOUT.as_secs());
                return Err(UsageError::Network);
            }
        }
    };
    // Um neto que herdou o pipe o seguraria aberto: espera pouco pelo resto.
    let text = rx.recv_timeout(Duration::from_secs(5)).map_err(|_| UsageError::Network)?;
    Ok((status.code(), text))
}

/// `/usage` numa origem: o `claude.exe` do Windows ou o `claude` do dono da pasta na distro.
fn usage_at(kind: &OriginKind, home: &Path) -> Result<ClaudeUsage, UsageError> {
    let command = match kind {
        OriginKind::Windows => {
            let exe = crate::chat::find_exe(Agent::Claude).ok_or(UsageError::NotInstalled)?;
            let mut c = Command::new(exe);
            c.args(usage_args()).current_dir(home).env("DISABLE_AUTOUPDATER", "1");
            c
        }
        OriginKind::Wsl(distro) => {
            // Rodar `wsl.exe -d` numa distro que parou desde a descoberta a ligaria.
            if !crate::wsl::running_distros().iter().any(|r| r.eq_ignore_ascii_case(distro)) {
                return Err(UsageError::Network);
            }
            let user = wsl_user(&home.to_string_lossy()).ok_or(UsageError::Missing)?;
            let mut c = Command::new("wsl.exe");
            c.args(usage_wsl_args(distro, &user));
            c
        }
    };
    let (code, stdout) = run(command)?;
    // 127: o bash não achou o `claude`.
    if code == Some(127) && stdout.trim().is_empty() {
        return Err(UsageError::NotInstalled);
    }
    parse_usage_command(&stdout)
}

/// Uso de uma conta: a primeira origem que responder, na ordem (Windows, depois as distros). Sem
/// nenhuma, o primeiro erro que não seja "Claude Code não encontrado".
fn query(origins: &Origins) -> Result<ClaudeUsage, UsageError> {
    let mut error = UsageError::NotInstalled;
    for (kind, home) in origins {
        match usage_at(kind, home) {
            Ok(usage) => return Ok(usage),
            Err(e) if error == UsageError::NotInstalled => error = e,
            Err(_) => {}
        }
    }
    Err(error)
}

/// Uma origem com login no Claude Code (o `.credentials.json` existe; ele não é lido): quem é e o
/// plano, pelo `.claude.json`.
fn found(origin: &Origin) -> Option<ClaudeFound> {
    if !origin.home.join(".claude").join(".credentials.json").is_file() {
        return None;
    }
    let claude_json = std::fs::read_to_string(origin.home.join(".claude.json")).unwrap_or_default();
    Some(ClaudeFound { origin: origin.clone(), identity: parse_identity(&claude_json), plan: plan_name(&claude_json) })
}

/// Um ciclo: descobre as contas, consulta cada uma e lembra as de distros que pararam.
fn cycle(discover: &Discover, previous: &[Account<ClaudeEntry>], query: &dyn Fn(&Origins) -> Result<ClaudeUsage, UsageError>) -> ClaudeSnapshot {
    let discovery = discover();
    let mut accounts = group_claude(discovery.origins.iter().filter_map(found).collect());
    inherit_claude(previous, &mut accounts);
    let many = many_distros(&accounts);
    for account in accounts.iter_mut() {
        let now = SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |d| d.as_secs() as i64);
        // Em pausa depois de um 429: fica com o último resultado.
        if !should_query(&account.data, now) {
            continue;
        }
        let origins: Vec<(OriginKind, PathBuf)> = account.origins.iter().cloned().zip(account.data.homes.iter().cloned()).collect();
        let result = query(&origins);
        if result == Err(UsageError::Http(429)) {
            account.data.limited += 1;
            account.data.retry_at = Some(now + rate_limit_wait(None, account.data.limited));
        } else {
            account.data.limited = 0;
            account.data.retry_at = None;
        }
        let old = previous.iter().find(|p| p.key == account.key).and_then(|p| p.data.result.as_ref());
        // Só registra quando o erro muda (sem login, seriam 288 linhas por dia).
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
    /// `refresh_now` (se tiver mais de 60 s). Conta com 429 fica em pausa (`rate_limit_wait`). `discover`
    /// acha as origens (Windows e WSL rodando) a cada consulta; `notify` é chamado (em outra thread)
    /// depois de cada consulta.
    pub fn start(discover: Discover, notify: Arc<dyn Fn() + Send + Sync>) -> std::io::Result<ClaudeService> {
        let (tx, rx) = mpsc::channel();
        let state: Shared = Default::default();
        let shared = state.clone();
        std::thread::Builder::new().name("zefiro-claude".into()).spawn(move || {
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
                    // Botão recusado (menos de 60 s): o painel sai do "Atualizando…".
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

/// Não espera a thread: ela pode estar presa num `claude`, num `wsl.exe` ou num `\\wsl.localhost`,
/// e quem solta o serviço é a thread da UI. A thread sai sozinha ao ver o `Stop`.
impl Drop for ClaudeService {
    fn drop(&mut self) {
        let _ = self.tx.send(Wake::Stop);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use zefiro_integration_usage::origins::Discovery;

    /// Pasta pessoal de teste com um login inventado: o `.credentials.json` só precisa existir.
    fn home(name: &str, uuid: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("zefiro-claude-{name}-{}", std::process::id()));
        std::fs::create_dir_all(dir.join(".claude")).unwrap();
        std::fs::write(dir.join(".claude").join(".credentials.json"), "{}").unwrap();
        std::fs::write(dir.join(".claude.json"), format!(r#"{{"oauthAccount": {{"accountUuid": "{uuid}", "organizationType": "claude_pro"}}}}"#)).unwrap();
        dir
    }

    #[test]
    fn each_account_is_queried_with_its_origins_in_order() {
        let (win, wsl) = (home("win", "u1"), home("wsl", "u1"));
        let origins = vec![Origin { kind: OriginKind::Wsl("U".into()), home: wsl.clone() }, Origin { kind: OriginKind::Windows, home: win.clone() }];
        let discover: Discover = Arc::new(move || Discovery { origins: origins.clone(), running: vec!["U".into()], wsl_stopped: false });
        let seen = std::cell::RefCell::new(Vec::new());
        let query = |o: &[(OriginKind, PathBuf)]| {
            seen.borrow_mut().push(o.to_vec());
            Ok(ClaudeUsage::default())
        };
        let snapshot = cycle(&discover, &[], &query);
        assert_eq!(snapshot.accounts.len(), 1);
        assert_eq!(snapshot.accounts[0].data.plan.as_deref(), Some("Pro"));
        assert_eq!(*seen.borrow(), vec![vec![(OriginKind::Windows, win.clone()), (OriginKind::Wsl("U".into()), wsl.clone())]]);
        std::fs::remove_dir_all(&win).unwrap();
        std::fs::remove_dir_all(&wsl).unwrap();
    }

    #[test]
    fn origin_without_login_is_not_an_account() {
        let dir = home("sem-login", "u1");
        std::fs::remove_file(dir.join(".claude").join(".credentials.json")).unwrap();
        assert!(found(&Origin { kind: OriginKind::Windows, home: dir.clone() }).is_none());
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn rate_limited_account_pauses_and_is_not_queried_again() {
        let win = home("limited", "u1");
        let origins = vec![Origin { kind: OriginKind::Windows, home: win.clone() }];
        let discover: Discover = Arc::new(move || Discovery { origins: origins.clone(), ..Default::default() });
        let calls = std::cell::Cell::new(0);
        let limited = |_: &[(OriginKind, PathBuf)]| {
            calls.set(calls.get() + 1);
            Err(UsageError::Http(429))
        };
        let first = cycle(&discover, &[], &limited);
        let entry = &first.accounts[0].data;
        let now = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs() as i64;
        assert!(entry.retry_at.is_some_and(|t| (now + 295..=now + 305).contains(&t)), "{:?}", entry.retry_at);
        assert_eq!(entry.limited, 1);
        let second = cycle(&discover, &first.accounts, &limited);
        assert_eq!(calls.get(), 1, "a conta em pausa não é consultada");
        assert_eq!(second.accounts[0].data.result, Some(Err(UsageError::Http(429))));
        let ok = |_: &[(OriginKind, PathBuf)]| Ok(ClaudeUsage::default());
        let mut resumed = second.accounts.clone();
        resumed[0].data.retry_at = Some(0);
        let third = cycle(&discover, &resumed, &ok);
        assert_eq!((third.accounts[0].data.retry_at, third.accounts[0].data.limited), (None, 0));
        std::fs::remove_dir_all(&win).unwrap();
    }
}
