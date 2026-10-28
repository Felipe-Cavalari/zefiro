//! Uso do Grok pelo `_x.ai/billing` do próprio Grok Build (ADR-0027), numa thread própria
//! (`zefiro-grok-usage`), para cada origem com o `grok`: o Windows e as distros do WSL rodando. Cada
//! consulta é um `grok agent stdio` curto: `initialize`, `_x.ai/auth/info` e `_x.ai/billing`, sem
//! sessão e sem pergunta ao modelo (não gasta cota). Quem guarda o login e fala com a xAI é o `grok`.

use std::io::{BufRead, BufReader, Write};
use std::os::windows::process::CommandExt;
use std::process::{Command, Stdio};
use std::sync::mpsc::{self, RecvTimeoutError, Sender};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use zefiro_integration_chat::agent::Agent;
use zefiro_integration_chat::command::{GROK_ENV, grok_usage_args, wsl_args_with};
use zefiro_integration_chat::grok;
use zefiro_integration_usage::claude::{MIN_FORCED, UsageError};
use zefiro_integration_usage::grok::{GrokEntry, GrokFound, GrokIdentity, GrokSnapshot, GrokUsage, billing_error, group_grok, inherit_grok, parse_billing, parse_identity};
use zefiro_integration_usage::origins::{Account, OriginKind, accounts_summary, carry_stopped, is_ignored_distro, many_distros, origins_label};

/// Como os outros chips de uso. A própria TUI do Grok relê a cota a cada 60 s.
const INTERVAL: Duration = Duration::from_secs(300);
/// Abrir o painel só consulta de novo se o dado tiver mais que isto.
const FRESH: Duration = Duration::from_secs(120);
/// O `initialize` leva menos de 3 s; um WSL que acabou de acordar, mais.
const TIMEOUT: Duration = Duration::from_secs(30);
/// Depois do fim do stdin, tempo para o `grok` sair sozinho.
const EXIT_GRACE: Duration = Duration::from_millis(1500);
const CREATE_NO_WINDOW: u32 = 0x0800_0000;

/// `Force` é o botão de atualizar do painel: consulta se a última tiver mais de 60 s.
enum Wake { Refresh, Force, Stop }

type Shared = Arc<Mutex<Option<GrokSnapshot>>>;

pub struct GrokUsageService {
    tx: Sender<Wake>,
    state: Shared,
}

/// O `grok agent stdio` da origem, sem perfil e com o ambiente do chat.
fn command(origin: &OriginKind) -> Result<Command, UsageError> {
    match origin {
        OriginKind::Windows => {
            let exe = crate::chat::find_exe(Agent::Grok).ok_or(UsageError::NotInstalled)?;
            let dir = crate::chat::windows_dir();
            std::fs::create_dir_all(&dir).map_err(|_| UsageError::Network)?;
            let mut c = Command::new(exe);
            c.args(grok_usage_args()).current_dir(dir).envs(GROK_ENV);
            Ok(c)
        }
        OriginKind::Wsl(distro) => {
            // Rodar `wsl.exe -d` numa distro que parou desde a descoberta a ligaria.
            if !crate::wsl::running_distros().iter().any(|r| r.eq_ignore_ascii_case(distro)) {
                return Err(UsageError::Network);
            }
            let mut c = Command::new("wsl.exe");
            c.args(wsl_args_with(Some(distro), "", &GROK_ENV, Agent::Grok.binary(), &grok_usage_args()));
            Ok(c)
        }
    }
}

/// Uma origem: quem está logado e a cota. Sem o `grok`, `NotInstalled`.
fn query(origin: &OriginKind) -> GrokFound {
    let started = Instant::now();
    let (identity, result) = match command(origin) {
        Ok(c) => ask(c),
        Err(e) => (GrokIdentity::default(), Err(e)),
    };
    tracing::debug!(ms = started.elapsed().as_millis() as u64, ok = result.is_ok(), "uso do Grok: consulta");
    GrokFound { origin: origin.clone(), identity, result }
}

/// Conduz o ACP: `initialize`, depois `_x.ai/auth/info` e `_x.ai/billing`; fecha o stdin quando os
/// dois respondem (ou no `TIMEOUT`).
fn ask(mut command: Command) -> (GrokIdentity, Result<GrokUsage, UsageError>) {
    command.stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::null()).creation_flags(CREATE_NO_WINDOW);
    let Ok(mut child) = command.spawn() else { return (GrokIdentity::default(), Err(UsageError::NotInstalled)) };
    let (Some(mut stdin), Some(stdout)) = (child.stdin.take(), child.stdout.take()) else {
        let _ = child.kill();
        return (GrokIdentity::default(), Err(UsageError::Network));
    };
    let (tx, rx) = mpsc::channel::<String>();
    std::thread::spawn(move || {
        for line in BufReader::new(stdout).lines().map_while(Result::ok) {
            if tx.send(line).is_err() {
                break;
            }
        }
    });
    let mut write = |line: String| writeln!(stdin, "{line}").and_then(|_| stdin.flush()).is_ok();
    let mut identity = None;
    let mut billing: Option<Result<GrokUsage, UsageError>> = None;
    let mut lines = 0;
    let deadline = Instant::now() + TIMEOUT;
    let mut alive = write(grok::initialize(1, env!("CARGO_PKG_VERSION")));
    while alive && (identity.is_none() || billing.is_none()) {
        let left = deadline.saturating_duration_since(Instant::now());
        let line = match rx.recv_timeout(left) {
            Ok(line) => line,
            Err(_) => break,
        };
        lines += 1;
        match grok::parse(&line) {
            grok::Incoming::Response { id: 1, .. } => alive = write(grok::auth_info(2)) && write(grok::billing(3)),
            grok::Incoming::Failure { id: 1, .. } => break,
            grok::Incoming::Response { id: 2, result } => identity = Some(parse_identity(&result)),
            grok::Incoming::Failure { id: 2, .. } => identity = Some(GrokIdentity::default()),
            grok::Incoming::Response { id: 3, result } => billing = Some(parse_billing(&result)),
            grok::Incoming::Failure { id: 3, message } => billing = Some(Err(billing_error(&message))),
            _ => {}
        }
    }
    drop(write);
    drop(stdin);
    let started = Instant::now();
    let code = loop {
        match child.try_wait() {
            Ok(Some(status)) => break status.code(),
            Ok(None) if started.elapsed() < EXIT_GRACE => std::thread::sleep(Duration::from_millis(30)),
            _ => {
                let _ = child.kill();
                let _ = child.wait();
                break None;
            }
        }
    };
    let result = match billing {
        Some(r) => r,
        // 127: o bash não achou o `grok`.
        None if lines == 0 && code == Some(127) => Err(UsageError::NotInstalled),
        None => Err(UsageError::Network),
    };
    (identity.unwrap_or_default(), result)
}

/// As origens possíveis (o Windows e as distros rodando), as distros rodando e se há distro parada.
fn origins() -> (Vec<OriginKind>, Vec<String>, bool) {
    let running = crate::wsl::running_distros();
    let stopped = crate::wsl::installed_distros().iter().any(|d| !is_ignored_distro(d) && !running.iter().any(|r| r.eq_ignore_ascii_case(d)));
    let mut origins = vec![OriginKind::Windows];
    origins.extend(running.iter().map(|d| OriginKind::Wsl(d.clone())));
    (origins, running, stopped)
}

/// Um ciclo: consulta cada origem (as sem `grok` ficam de fora), junta as contas e lembra as de
/// distros que pararam.
fn cycle(origins: &[OriginKind], running: &[String], wsl_stopped: bool, previous: &[Account<GrokEntry>], query: &dyn Fn(&OriginKind) -> GrokFound) -> GrokSnapshot {
    let found: Vec<GrokFound> = origins.iter().map(query).filter(|f| f.result != Err(UsageError::NotInstalled)).collect();
    let mut accounts = group_grok(found);
    inherit_grok(previous, &mut accounts);
    let many = many_distros(&accounts);
    for account in &accounts {
        let old = previous.iter().find(|p| p.key == account.key).and_then(|p| p.data.result.as_ref());
        // Só registra quando o erro muda.
        if let Some(Err(e)) = &account.data.result
            && old != account.data.result.as_ref()
        {
            tracing::info!(origem = %origins_label(&account.origins, many), ?e, "uso do Grok indisponível");
        }
    }
    carry_stopped(previous, &mut accounts, running);
    GrokSnapshot { accounts, wsl_stopped, at: Some(Instant::now()) }
}

impl GrokUsageService {
    /// Consulta ao iniciar, a cada 5 min, no `refresh` (se o dado tiver mais de 2 min) e no
    /// `refresh_now` (se tiver mais de 60 s). `notify` é chamado (em outra thread) depois de cada
    /// consulta.
    pub fn start(notify: Arc<dyn Fn() + Send + Sync>) -> std::io::Result<GrokUsageService> {
        let (tx, rx) = mpsc::channel();
        let state: Shared = Default::default();
        let shared = state.clone();
        std::thread::Builder::new().name("zefiro-grok-usage".into()).spawn(move || {
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
                    let (origins, running, stopped) = origins();
                    let snapshot = cycle(&origins, &running, stopped, &previous, &query);
                    last = Some(Instant::now());
                    let text = accounts_summary("Grok", &snapshot.accounts);
                    if text != summary {
                        tracing::info!("{text}");
                        summary = text;
                    }
                    if let Ok(mut s) = shared.lock() {
                        *s = Some(snapshot);
                    }
                    notify();
                } else if matches!(wake, Wake::Force) {
                    // Botão recusado (menos de 60 s): o painel sai do "Atualizando…".
                    notify();
                }
                wake = match rx.recv_timeout(INTERVAL) {
                    Ok(w) => w,
                    Err(RecvTimeoutError::Timeout) => {
                        last = None;
                        Wake::Refresh
                    }
                    Err(RecvTimeoutError::Disconnected) => Wake::Stop,
                };
            }
        })?;
        Ok(GrokUsageService { tx, state })
    }

    pub fn latest(&self) -> Option<GrokSnapshot> {
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

/// Não espera a thread: ela pode estar presa num `grok` ou num `wsl.exe`. Ela sai sozinha ao ver o
/// `Stop`.
impl Drop for GrokUsageService {
    fn drop(&mut self) {
        let _ = self.tx.send(Wake::Stop);
    }
}

/// Uma consulta, para a sonda `grok_usage_probe`: a origem, o tempo e o que voltou.
pub fn probe(origin: &OriginKind) -> (Duration, GrokFound) {
    let started = Instant::now();
    let found = query(origin);
    (started.elapsed(), found)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn found(origin: &OriginKind, key: &str, percent: f32) -> GrokFound {
        GrokFound { origin: origin.clone(), identity: GrokIdentity { key: Some(key.into()), email: None }, result: Ok(GrokUsage { percent: Some(percent), ..Default::default() }) }
    }

    #[test]
    fn origins_without_grok_are_left_out_and_stopped_distros_are_remembered() {
        let wsl = OriginKind::Wsl("Ubuntu-24.04".into());
        let origins = [OriginKind::Windows, wsl.clone()];
        let query = |o: &OriginKind| match o {
            OriginKind::Windows => GrokFound { origin: o.clone(), identity: GrokIdentity::default(), result: Err(UsageError::NotInstalled) },
            _ => found(o, "p1", 25.0),
        };
        let first = cycle(&origins, &["Ubuntu-24.04".into()], false, &[], &query);
        assert_eq!(first.accounts.len(), 1);
        assert_eq!(first.accounts[0].origins, std::slice::from_ref(&wsl));
        // A distro parou: a conta continua, marcada, com o último valor.
        let second = cycle(&[OriginKind::Windows], &[], true, &first.accounts, &query);
        assert_eq!((second.accounts.len(), second.accounts[0].stopped, second.wsl_stopped), (1, true, true));
    }
}
