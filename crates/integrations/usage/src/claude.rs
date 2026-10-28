//! Uso do Claude pelo `/usage` do próprio Claude Code (`claude -p /usage`, ADR-0025): os argumentos,
//! a leitura do stream-json e as contas. O Zefiro não lê nem usa o token do Claude Code.

use serde_json::Value;
use std::path::PathBuf;

use crate::origins::{Account, Origin, sort_accounts};

use crate::format::parse_iso_utc;

/// Uma janela de uso: porcentagem (0–100, como o Claude Code manda) e quando reinicia.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Limit { pub percent: f32, pub resets_at: Option<i64> }

#[derive(Debug, Clone, Default, PartialEq)]
pub struct ClaudeUsage {
    pub five_hour: Option<Limit>,
    pub seven_day: Option<Limit>,
    /// Limites semanais por modelo (`weekly_opus` → "Opus"), quando o Claude Code manda.
    pub extra: Vec<(String, Limit)>,
}

/// `NotInstalled`: a origem tem login, mas o `claude` não foi achado nela.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UsageError { UnknownFormat, Http(u16), Expired, Missing, NotInstalled, Network }

/// % do chip: o resultado atual ou, numa falha passageira (rede, HTTP), o último valor bom. Login
/// perdido, Claude Code sumido ou formato desconhecido deixam o chip sem número.
pub fn chip_percent(result: &Result<ClaudeUsage, UsageError>, last_ok: Option<&ClaudeUsage>) -> Option<f32> {
    let usage = match result {
        Ok(u) => Some(u),
        Err(UsageError::Network | UsageError::Http(_)) => last_ok,
        Err(_) => None,
    };
    usage.and_then(|u| u.five_hour).map(|l| l.percent)
}

/// Quem é a conta, pelo `oauthAccount` do `~/.claude.json`.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ClaudeIdentity { pub account_uuid: Option<String>, pub email: Option<String> }

pub fn parse_identity(claude_json: &str) -> ClaudeIdentity {
    let Ok(json) = serde_json::from_str::<Value>(claude_json) else { return ClaudeIdentity::default() };
    let field = |name: &str| json["oauthAccount"][name].as_str().filter(|s| !s.is_empty()).map(str::to_string);
    ClaudeIdentity { account_uuid: field("accountUuid"), email: field("emailAddress") }
}

/// "pro" → "Pro".
fn capitalized(s: &str) -> Option<String> {
    let mut chars = s.chars();
    chars.next().map(|first| first.to_uppercase().chain(chars).collect())
}

/// Plano pelo `oauthAccount.organizationType` do `~/.claude.json`: "claude_pro" → "Pro".
pub fn plan_name(claude_json: &str) -> Option<String> {
    let json: Value = serde_json::from_str(claude_json).ok()?;
    let kind = json["oauthAccount"]["organizationType"].as_str()?;
    capitalized(kind.strip_prefix("claude_").unwrap_or(kind))
}

/// Uma conta do Claude. `homes` é a pasta pessoal de cada origem, na ordem de `Account::origins`
/// (Windows, depois as distros).
#[derive(Debug, Clone, Default, PartialEq)]
pub struct ClaudeEntry {
    pub email: Option<String>,
    pub plan: Option<String>,
    pub homes: Vec<PathBuf>,
    pub result: Option<Result<ClaudeUsage, UsageError>>,
    /// Último uso lido com sucesso: o chip continua com ele numa falha passageira.
    pub last_ok: Option<ClaudeUsage>,
    /// Depois de um 429, a conta só é consultada de novo a partir daqui (unix).
    pub retry_at: Option<i64>,
    /// 429 seguidos: cada um dobra a pausa.
    pub limited: u32,
}

/// Uma origem com login no Claude Code.
#[derive(Debug, Clone, PartialEq)]
pub struct ClaudeFound { pub origin: Origin, pub identity: ClaudeIdentity, pub plan: Option<String> }

/// O que o serviço publica: as contas, se há WSL parado e quando terminou a última consulta.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct ClaudeSnapshot { pub accounts: Vec<Account<ClaudeEntry>>, pub wsl_stopped: bool, pub at: Option<std::time::Instant> }

/// Junta as origens da mesma conta (`accountUuid`); sem ele, cada origem é uma conta.
pub fn group_claude(mut found: Vec<ClaudeFound>) -> Vec<Account<ClaudeEntry>> {
    found.sort_by(|a, b| a.origin.kind.cmp(&b.origin.kind));
    let mut accounts: Vec<Account<ClaudeEntry>> = Vec::new();
    for f in found {
        let key = f.identity.account_uuid.clone().unwrap_or_else(|| format!("origem:{:?}", f.origin.kind));
        match accounts.iter_mut().find(|a| a.key == key) {
            Some(a) => {
                a.origins.push(f.origin.kind);
                a.data.homes.push(f.origin.home);
                a.data.email = a.data.email.take().or(f.identity.email);
                a.data.plan = a.data.plan.take().or(f.plan);
            }
            None => accounts.push(Account {
                key,
                origins: vec![f.origin.kind],
                stopped: false,
                data: ClaudeEntry { email: f.identity.email, plan: f.plan, homes: vec![f.origin.home], ..Default::default() },
            }),
        }
    }
    sort_accounts(&mut accounts);
    accounts
}

/// Traz o último resultado de cada conta do ciclo anterior.
pub fn inherit_claude(previous: &[Account<ClaudeEntry>], current: &mut [Account<ClaudeEntry>]) {
    for account in current {
        if let Some(old) = previous.iter().find(|p| p.key == account.key) {
            account.data.result = old.data.result.clone();
            account.data.last_ok = old.data.last_ok.clone();
            account.data.retry_at = old.data.retry_at;
            account.data.limited = old.data.limited;
        }
    }
}

/// Pausa depois de um 429, em segundos: o `Retry-After` do servidor ou 5, 10, 20 e 30 min para 429
/// seguidos; sempre entre 1 e 30 min.
pub fn rate_limit_wait(retry_after: Option<u64>, streak: u32) -> i64 {
    let doubled = 300i64 << streak.saturating_sub(1).min(4);
    retry_after.map_or(doubled, |s| s.min(1800) as i64).clamp(60, 1800)
}

/// A conta não está em pausa depois de um 429.
pub fn should_query(entry: &ClaudeEntry, now_unix: i64) -> bool {
    entry.retry_at.is_none_or(|t| t <= now_unix)
}

/// Menor intervalo entre duas consultas pedidas pelo botão de atualizar: o serviço de uso respondeu
/// 429 a uma consulta 63 s depois da anterior (2026-10-21).
pub const MIN_FORCED: std::time::Duration = std::time::Duration::from_secs(60);

/// O botão de atualizar vale: alguma conta não está em pausa depois de um 429. O piso de 60 s entre
/// consultas forçadas (`MIN_FORCED`) é do serviço.
pub fn can_refresh(snapshot: &ClaudeSnapshot, now_unix: i64) -> bool {
    snapshot.accounts.is_empty() || snapshot.accounts.iter().any(|a| should_query(&a.data, now_unix))
}

/// % do chip com várias contas: a maior janela de 5 h, com 0 % na janela que já reiniciou.
pub fn chip_percent_accounts(accounts: &[Account<ClaudeEntry>], now_unix: i64) -> Option<f32> {
    accounts
        .iter()
        .filter_map(|a| {
            let result = a.data.result.as_ref()?;
            let percent = chip_percent(result, a.data.last_ok.as_ref())?;
            let usage = match result { Ok(u) => Some(u), Err(_) => a.data.last_ok.as_ref() };
            let reset = usage.and_then(|u| u.five_hour).and_then(|l| l.resets_at).is_some_and(|t| t <= now_unix);
            Some(if reset { 0.0 } else { percent })
        })
        .reduce(f32::max)
}

/// Argumentos do `claude`: só o comando local `/usage`, que não chama o modelo. Sem hooks, plugins e
/// MCP do usuário, e sem gravar sessão em `~/.claude/projects`.
pub fn usage_args() -> Vec<String> {
    ["-p", "/usage", "--output-format", "stream-json", "--verbose", "--setting-sources", "", "--no-session-persistence", "--strict-mcp-config"].iter().map(|s| s.to_string()).collect()
}

/// Argumentos do `wsl.exe`: o `claude` do dono da pasta (`-u`) num shell de login, que acha o
/// executável dele. Sem a atualização automática, que deixaria um processo para trás.
pub fn usage_wsl_args(distro: &str, user: &str) -> Vec<String> {
    let quoted: Vec<String> = usage_args().iter().map(|a| format!("'{a}'")).collect();
    let script = format!("cd ~ && DISABLE_AUTOUPDATER=1 exec claude {}", quoted.join(" "));
    ["-d", distro, "-u", user, "--", "bash", "-lc", &script].iter().map(|s| s.to_string()).collect()
}

/// Usuário da distro pela pasta pessoal (`\\wsl.localhost\<distro>\home\<usuário>`). Só nomes de
/// usuário comuns do Linux, que o `wsl.exe` recebe sem surpresa.
pub fn wsl_user(home: &str) -> Option<String> {
    let name = home.trim_end_matches(['\\', '/']).rsplit(['\\', '/']).next()?;
    let ok = !name.is_empty() && !name.starts_with('-') && name.chars().all(|c| c.is_ascii_alphanumeric() || "._-".contains(c));
    ok.then(|| name.to_string())
}

/// Uma janela do `usage_report`; `Err` se o formato mudou.
fn report_limit(v: &Value) -> Result<Limit, UsageError> {
    let percent = v["percent"].as_f64().ok_or(UsageError::UnknownFormat)?;
    Ok(Limit { percent: percent as f32, resets_at: v["resets_at"].as_str().and_then(parse_iso_utc) })
}

/// Os limites do `usage_report.rate_limits`: `session` é a janela de 5 h, `weekly_all` a de 7 dias e
/// os outros `weekly_*` são por modelo. Sem limites (o serviço falhou) é passageiro.
fn from_report(report: &Value) -> Result<ClaudeUsage, UsageError> {
    let rate_limits = &report["rate_limits"];
    if rate_limits.is_null() {
        return Err(UsageError::Network);
    }
    let limits = rate_limits["limits"].as_array().ok_or(UsageError::UnknownFormat)?;
    let mut usage = ClaudeUsage::default();
    for v in limits {
        let kind = v["kind"].as_str().unwrap_or_default();
        match kind {
            "session" => usage.five_hour = Some(report_limit(v)?),
            "weekly_all" => usage.seven_day = Some(report_limit(v)?),
            _ => {
                let Some(model) = v["scope"].as_str().filter(|s| !s.is_empty()).or_else(|| kind.strip_prefix("weekly_")) else { continue };
                if let (Some(label), Ok(limit)) = (capitalized(model), report_limit(v)) {
                    usage.extra.push((label, limit));
                }
            }
        }
    }
    if usage.five_hour.is_none() && usage.seven_day.is_none() {
        return Err(UsageError::UnknownFormat);
    }
    usage.extra.sort_by(|a, b| a.0.cmp(&b.0));
    Ok(usage)
}

/// Saída do `claude -p /usage --output-format stream-json`: o `usage_report` da mensagem do comando.
/// Sem ele, "Not logged in" é `Missing`, outro erro é passageiro e o resto é formato desconhecido.
pub fn parse_usage_command(stdout: &str) -> Result<ClaudeUsage, UsageError> {
    let mut error: Option<String> = None;
    for line in stdout.lines() {
        let Ok(v) = serde_json::from_str::<Value>(line) else { continue };
        if let Some(report) = v.get("usage_report") {
            return from_report(report);
        }
        if v["type"] == "result" && v["is_error"] == true {
            error = Some(v["result"].as_str().unwrap_or_default().to_string());
        }
    }
    match error {
        Some(text) if text.contains("/login") || text.to_lowercase().contains("not logged in") => Err(UsageError::Missing),
        Some(_) => Err(UsageError::Network),
        None => Err(UsageError::UnknownFormat),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::origins::OriginKind;

    const USAGE: &str = include_str!("../tests/fixtures/claude_usage_command.jsonl");

    fn result_line(is_error: bool, text: &str) -> String {
        format!(r#"{{"type":"result","subtype":"success","is_error":{is_error},"num_turns":0,"result":"{text}"}}"#)
    }

    fn report_line(rate_limits: &str) -> String {
        format!(r#"{{"type":"assistant","message":{{"content":[]}},"usage_report":{{"session":{{}},"rate_limits":{rate_limits}}}}}"#)
    }

    #[test]
    fn parses_session_and_weekly() {
        let u = parse_usage_command(USAGE).unwrap();
        assert_eq!(u.five_hour, Some(Limit { percent: 42.0, resets_at: Some(1792450200) }));
        assert_eq!(u.seven_day, Some(Limit { percent: 19.0, resets_at: Some(1792861200) }));
    }

    #[test]
    fn extra_weekly_limits_by_model() {
        let u = parse_usage_command(USAGE).unwrap();
        // `weekly_opus` vira "Opus"; o `scope`, quando vier, ganha do nome do `kind`.
        assert_eq!(u.extra, vec![("Opus".to_string(), Limit { percent: 12.5, resets_at: Some(1792861200) })]);
        let scoped = report_line(r#"{"limits":[{"kind":"session","percent":1,"resets_at":null},{"kind":"weekly_model","percent":3,"resets_at":null,"scope":"sonnet"}]}"#);
        assert_eq!(parse_usage_command(&scoped).unwrap().extra, vec![("Sonnet".to_string(), Limit { percent: 3.0, resets_at: None })]);
    }

    #[test]
    fn not_logged_in_is_missing() {
        // Medido com um `CLAUDE_CONFIG_DIR` vazio (Claude Code 2.1.312).
        assert_eq!(parse_usage_command(&result_line(true, "Not logged in · Please run /login")), Err(UsageError::Missing));
    }

    #[test]
    fn other_errors_and_missing_limits_are_transient() {
        assert_eq!(parse_usage_command(&result_line(true, "API Error: 500")), Err(UsageError::Network));
        // O `/usage` rodou, mas sem os limites (o serviço falhou): tenta de novo no próximo ciclo.
        assert_eq!(parse_usage_command(&report_line("null")), Err(UsageError::Network));
    }

    #[test]
    fn unknown_output_is_error_not_panic() {
        assert_eq!(parse_usage_command(""), Err(UsageError::UnknownFormat));
        assert_eq!(parse_usage_command("não é json"), Err(UsageError::UnknownFormat));
        assert_eq!(parse_usage_command(&result_line(false, "ok")), Err(UsageError::UnknownFormat));
        assert_eq!(parse_usage_command(&report_line(r#"{"limits":[]}"#)), Err(UsageError::UnknownFormat));
        assert_eq!(parse_usage_command(&report_line(r#"{"limits":[{"kind":"session","percent":"muito"}]}"#)), Err(UsageError::UnknownFormat));
    }

    #[test]
    fn args_run_only_the_usage_command() {
        let a = usage_args();
        let after = |flag: &str| a.iter().position(|x| x == flag).and_then(|i| a.get(i + 1)).map(String::as_str);
        assert_eq!(&a[..2], ["-p", "/usage"]);
        assert_eq!(after("--output-format"), Some("stream-json"));
        assert!(a.contains(&"--verbose".into()), "o stream-json do -p exige --verbose");
        assert_eq!(after("--setting-sources"), Some(""), "sem hooks e plugins do usuário");
        assert!(a.contains(&"--no-session-persistence".into()), "sem sessão gravada em ~/.claude/projects");
        assert!(a.contains(&"--strict-mcp-config".into()), "sem subir os MCP do usuário");
        assert!(!a.contains(&"--disable-slash-commands".into()));
    }

    #[test]
    fn wsl_runs_usage_as_the_home_owner() {
        let w = usage_wsl_args("Ubuntu-24.04", "felip");
        assert_eq!(&w[..7], ["-d", "Ubuntu-24.04", "-u", "felip", "--", "bash", "-lc"]);
        assert_eq!(w[7], "cd ~ && DISABLE_AUTOUPDATER=1 exec claude '-p' '/usage' '--output-format' 'stream-json' '--verbose' '--setting-sources' '' '--no-session-persistence' '--strict-mcp-config'");
    }

    #[test]
    fn wsl_user_from_home() {
        assert_eq!(wsl_user(r"\\wsl.localhost\Ubuntu-24.04\home\felip").as_deref(), Some("felip"));
        assert_eq!(wsl_user(r"\\wsl.localhost\Ubuntu\home\felip\").as_deref(), Some("felip"));
        // Nome que o wsl.exe ou o bash leriam como outra coisa: não roda.
        assert_eq!(wsl_user(r"\\wsl.localhost\Ubuntu\home\a b"), None);
        assert_eq!(wsl_user(r"\\wsl.localhost\Ubuntu\home\-x"), None);
        assert_eq!(wsl_user(""), None);
    }

    #[test]
    fn chip_keeps_last_value_on_transient_errors() {
        let ok = parse_usage_command(USAGE).unwrap();
        assert_eq!(chip_percent(&Ok(ok.clone()), None), Some(42.0));
        assert_eq!(chip_percent(&Err(UsageError::Network), Some(&ok)), Some(42.0));
        assert_eq!(chip_percent(&Err(UsageError::Http(503)), Some(&ok)), Some(42.0));
        assert_eq!(chip_percent(&Err(UsageError::Network), None), None);
        // Login perdido, Claude Code sumido ou formato novo: sem número, para não enganar.
        assert_eq!(chip_percent(&Err(UsageError::Expired), Some(&ok)), None);
        assert_eq!(chip_percent(&Err(UsageError::Missing), Some(&ok)), None);
        assert_eq!(chip_percent(&Err(UsageError::NotInstalled), Some(&ok)), None);
        assert_eq!(chip_percent(&Err(UsageError::UnknownFormat), Some(&ok)), None);
    }

    fn found(kind: OriginKind, uuid: Option<&str>) -> ClaudeFound {
        let home = match &kind { OriginKind::Windows => PathBuf::from("C:\\casa"), OriginKind::Wsl(d) => PathBuf::from(format!("\\\\wsl.localhost\\{d}\\home\\eu")) };
        ClaudeFound { origin: Origin { kind, home }, identity: ClaudeIdentity { account_uuid: uuid.map(String::from), email: Some("a@b.c".into()) }, plan: Some("Pro".into()) }
    }

    fn five_hour(percent: f32, resets_at: i64) -> ClaudeUsage {
        ClaudeUsage { five_hour: Some(Limit { percent, resets_at: Some(resets_at) }), ..Default::default() }
    }

    fn with(result: Result<ClaudeUsage, UsageError>, last_ok: Option<ClaudeUsage>, stopped: bool) -> Account<ClaudeEntry> {
        Account { key: "k".into(), origins: vec![OriginKind::Windows], stopped, data: ClaudeEntry { result: Some(result), last_ok, ..Default::default() } }
    }

    #[test]
    fn identity_from_claude_json() {
        let id = parse_identity(r#"{"oauthAccount": {"accountUuid": "u1", "emailAddress": "a@b.c"}}"#);
        assert_eq!(id, ClaudeIdentity { account_uuid: Some("u1".into()), email: Some("a@b.c".into()) });
        assert_eq!(parse_identity("{}"), ClaudeIdentity::default());
        assert_eq!(parse_identity("não é json"), ClaudeIdentity::default());
    }

    #[test]
    fn plan_names_from_claude_json() {
        // O plano sai do `.claude.json`: o `.credentials.json` (com o token) não é lido.
        assert_eq!(plan_name(r#"{"oauthAccount": {"organizationType": "claude_pro"}}"#).as_deref(), Some("Pro"));
        assert_eq!(plan_name(r#"{"oauthAccount": {"organizationType": "claude_max"}}"#).as_deref(), Some("Max"));
        assert_eq!(plan_name(r#"{"oauthAccount": {"organizationType": ""}}"#), None);
        assert_eq!(plan_name(r#"{"oauthAccount": {}}"#), None);
        assert_eq!(plan_name("não é json"), None);
    }

    #[test]
    fn same_uuid_in_two_origins_is_one_account() {
        let one = group_claude(vec![found(OriginKind::Wsl("Ubuntu".into()), Some("u1")), found(OriginKind::Windows, Some("u1"))]);
        assert_eq!(one.len(), 1);
        assert_eq!(one[0].origins, vec![OriginKind::Windows, OriginKind::Wsl("Ubuntu".into())]);
        assert_eq!(one[0].data.homes, vec![PathBuf::from("C:\\casa"), PathBuf::from("\\\\wsl.localhost\\Ubuntu\\home\\eu")]);
        assert_eq!(group_claude(vec![found(OriginKind::Windows, Some("u1")), found(OriginKind::Wsl("U".into()), Some("u2"))]).len(), 2);
        // Sem `.claude.json`: cada origem é uma conta.
        assert_eq!(group_claude(vec![found(OriginKind::Windows, None), found(OriginKind::Wsl("U".into()), None)]).len(), 2);
    }

    #[test]
    fn chip_is_max_across_accounts() {
        let now = 1000;
        let a = with(Ok(five_hour(9.0, now + 60)), None, false);
        let b = with(Ok(five_hour(17.0, now + 60)), None, false);
        assert_eq!(chip_percent_accounts(&[a.clone(), b], now), Some(17.0));
        let reset = with(Ok(five_hour(17.0, now - 1)), None, false);
        assert_eq!(chip_percent_accounts(&[a.clone(), reset], now), Some(9.0));
        let stopped = with(Err(UsageError::Network), Some(five_hour(30.0, now + 60)), true);
        assert_eq!(chip_percent_accounts(&[a, stopped], now), Some(30.0));
        assert_eq!(chip_percent_accounts(&[with(Err(UsageError::Expired), None, false)], now), None);
        assert_eq!(chip_percent_accounts(&[], now), None);
    }

    #[test]
    fn inherit_keeps_last_ok() {
        let previous = vec![Account { key: "u1".into(), ..with(Ok(five_hour(5.0, 9)), Some(five_hour(5.0, 9)), false) }];
        let mut current = group_claude(vec![found(OriginKind::Windows, Some("u1"))]);
        inherit_claude(&previous, &mut current);
        assert_eq!(current[0].data.last_ok, Some(five_hour(5.0, 9)));
        assert_eq!(current[0].data.result, Some(Ok(five_hour(5.0, 9))));
    }

    #[test]
    fn rate_limit_waits_grow_and_respect_retry_after() {
        assert_eq!(rate_limit_wait(None, 1), 300);
        assert_eq!(rate_limit_wait(None, 2), 600);
        assert_eq!(rate_limit_wait(None, 3), 1200);
        assert_eq!(rate_limit_wait(None, 9), 1800);
        assert_eq!(rate_limit_wait(Some(90), 1), 90);
        // Retry-After absurdo ou zero: entre 1 e 30 min.
        assert_eq!(rate_limit_wait(Some(0), 1), 60);
        assert_eq!(rate_limit_wait(Some(86_400), 1), 1800);
    }

    #[test]
    fn paused_account_is_not_queried() {
        let mut a = with(Err(UsageError::Http(429)), None, false);
        a.data.retry_at = Some(1000);
        assert!(!should_query(&a.data, 999));
        assert!(should_query(&a.data, 1000));
        assert!(should_query(&with(Ok(five_hour(1.0, 5)), None, false).data, 0));
    }

    #[test]
    fn refresh_button_rules() {
        let now = 10_000;
        let ok = with(Ok(five_hour(1.0, 5)), None, false);
        let mut paused = with(Err(UsageError::Http(429)), None, false);
        paused.data.retry_at = Some(now + 60);
        let snap = |accounts: Vec<Account<ClaudeEntry>>| ClaudeSnapshot { accounts, wsl_stopped: false, at: Some(std::time::Instant::now()) };
        // Acabou de consultar e mesmo assim vale: o piso de 60 s fica no serviço, senão o botão
        // ficaria apagado até o próximo ciclo (o painel só é redesenhado quando chega dado).
        assert!(can_refresh(&snap(vec![ok.clone()]), now));
        // Todas as contas em pausa depois de um 429.
        assert!(!can_refresh(&snap(vec![paused.clone()]), now));
        assert!(can_refresh(&snap(vec![paused, ok]), now));
        assert!(can_refresh(&ClaudeSnapshot::default(), now));
    }

    #[test]
    fn inherit_keeps_the_pause() {
        let mut old = Account { key: "u1".into(), ..with(Err(UsageError::Http(429)), None, false) };
        old.data.retry_at = Some(77);
        old.data.limited = 2;
        let mut current = group_claude(vec![found(OriginKind::Windows, Some("u1"))]);
        inherit_claude(&[old], &mut current);
        assert_eq!((current[0].data.retry_at, current[0].data.limited), (Some(77), 2));
    }
}
