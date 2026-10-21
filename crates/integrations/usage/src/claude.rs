//! Uso do Claude: a resposta de `GET https://api.anthropic.com/api/oauth/usage` e o token do
//! Claude Code em `~/.claude/.credentials.json`. O token nunca é renovado nem gravado aqui.

use serde_json::Value;
use std::path::PathBuf;

use crate::origins::{Account, Origin, sort_accounts};

use crate::format::parse_iso_utc;

/// Uma janela de uso: porcentagem (0–100, como a API manda) e quando reinicia.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Limit { pub percent: f32, pub resets_at: Option<i64> }

#[derive(Debug, Clone, Default, PartialEq)]
pub struct ClaudeUsage {
    pub five_hour: Option<Limit>,
    pub seven_day: Option<Limit>,
    /// Limites semanais por modelo (`seven_day_opus` → "Opus"), quando a API manda.
    pub extra: Vec<(String, Limit)>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UsageError { UnknownFormat, Http(u16), Expired, Missing, Network }

/// Token do Claude Code. `Debug` não mostra o token.
#[derive(Clone, PartialEq, Eq)]
pub enum TokenState { Valid(String), Expired, Missing }

impl std::fmt::Debug for TokenState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            TokenState::Valid(_) => "Valid(<oculto>)",
            TokenState::Expired => "Expired",
            TokenState::Missing => "Missing",
        })
    }
}

/// Estado do token em `claudeAiOauth` (`expiresAt` em ms).
pub fn token_state(credentials_json: &str, now_ms: i64) -> TokenState {
    let Ok(json) = serde_json::from_str::<Value>(credentials_json) else { return TokenState::Missing };
    let oauth = &json["claudeAiOauth"];
    let Some(token) = oauth["accessToken"].as_str().filter(|t| !t.is_empty()) else { return TokenState::Missing };
    match oauth["expiresAt"].as_i64() {
        Some(expires) if expires <= now_ms => TokenState::Expired,
        _ => TokenState::Valid(token.to_string()),
    }
}

/// % do chip: o resultado atual ou, numa falha passageira (rede, HTTP), o último valor bom. Login
/// perdido ou formato desconhecido deixam o chip sem número.
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

/// Plano pelo `subscriptionType` das credenciais: "pro" → "Pro".
pub fn plan_name(credentials_json: &str) -> Option<String> {
    let json: Value = serde_json::from_str(credentials_json).ok()?;
    let plan = json["claudeAiOauth"]["subscriptionType"].as_str().filter(|s| !s.is_empty())?;
    let mut chars = plan.chars();
    chars.next().map(|first| first.to_uppercase().chain(chars).collect())
}

/// Uma conta do Claude. `credentials` vem na ordem de preferência (Windows, depois as distros).
#[derive(Debug, Clone, PartialEq)]
pub struct ClaudeEntry {
    pub email: Option<String>,
    pub plan: Option<String>,
    pub credentials: Vec<PathBuf>,
    pub result: Option<Result<ClaudeUsage, UsageError>>,
    /// Último uso lido com sucesso: o chip continua com ele numa falha passageira.
    pub last_ok: Option<ClaudeUsage>,
}

/// Uma origem com `.claude/.credentials.json`.
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
        let credentials = f.origin.home.join(".claude").join(".credentials.json");
        match accounts.iter_mut().find(|a| a.key == key) {
            Some(a) => {
                a.origins.push(f.origin.kind);
                a.data.credentials.push(credentials);
                a.data.email = a.data.email.take().or(f.identity.email);
                a.data.plan = a.data.plan.take().or(f.plan);
            }
            None => accounts.push(Account {
                key,
                origins: vec![f.origin.kind],
                stopped: false,
                data: ClaudeEntry { email: f.identity.email, plan: f.plan, credentials: vec![credentials], result: None, last_ok: None },
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
        }
    }
}

/// O primeiro token válido entre as credenciais da conta; senão vencido (se algum venceu) ou ausente.
pub fn first_valid_token(credentials: &[String], now_ms: i64) -> TokenState {
    let mut state = TokenState::Missing;
    for text in credentials {
        match token_state(text, now_ms) {
            TokenState::Valid(token) => return TokenState::Valid(token),
            TokenState::Expired => state = TokenState::Expired,
            TokenState::Missing => {}
        }
    }
    state
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

/// `Some(Ok)` se for uma janela válida, `Some(Err)` se o formato mudou, `None` se nula ou ausente.
fn limit(v: &Value) -> Option<Result<Limit, UsageError>> {
    let obj = v.as_object()?;
    let Some(percent) = obj.get("utilization").and_then(Value::as_f64) else { return Some(Err(UsageError::UnknownFormat)) };
    let resets_at = obj.get("resets_at").and_then(Value::as_str).and_then(parse_iso_utc);
    Some(Ok(Limit { percent: percent as f32, resets_at }))
}

/// Resposta do endpoint de uso. Campos ausentes ou nulos são "sem dado"; sem nenhuma das duas
/// janelas principais, ou com uma delas num formato diferente, é `UnknownFormat`.
pub fn parse_claude(json: &str) -> Result<ClaudeUsage, UsageError> {
    let v: Value = serde_json::from_str(json).map_err(|_| UsageError::UnknownFormat)?;
    let map = v.as_object().ok_or(UsageError::UnknownFormat)?;
    let five_hour = map.get("five_hour").and_then(limit).transpose()?;
    let seven_day = map.get("seven_day").and_then(limit).transpose()?;
    if five_hour.is_none() && seven_day.is_none() {
        return Err(UsageError::UnknownFormat);
    }
    let mut extra: Vec<(String, Limit)> = map
        .iter()
        .filter_map(|(key, value)| {
            let model = key.strip_prefix("seven_day_")?;
            let limit = limit(value)?.ok()?;
            let mut name = model.chars();
            let label = name.next()?.to_uppercase().chain(name).collect::<String>();
            Some((label, limit))
        })
        .collect();
    extra.sort_by(|a, b| a.0.cmp(&b.0));
    Ok(ClaudeUsage { five_hour, seven_day, extra })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::origins::OriginKind;

    const USAGE: &str = include_str!("../tests/fixtures/claude_usage.json");
    // Valores inventados; nenhum token real.
    const CREDENTIALS: &str = r#"{"mcpOAuth": {}, "claudeAiOauth": {"accessToken": "sk-teste-falso", "refreshToken": "r-falso", "expiresAt": 2000, "subscriptionType": "max"}}"#;

    #[test]
    fn parses_five_hour_and_seven_day() {
        let u = parse_claude(USAGE).unwrap();
        assert_eq!(u.five_hour, Some(Limit { percent: 42.0, resets_at: Some(1792450200) }));
        assert_eq!(u.seven_day, Some(Limit { percent: 19.0, resets_at: Some(1792861200) }));
    }

    #[test]
    fn utilization_is_already_a_percent() {
        // Medido pela sonda: 2.0 na API = 2 % (escala 0–100).
        let u = parse_claude(r#"{"five_hour": {"utilization": 2.0, "resets_at": null}}"#).unwrap();
        assert_eq!(u.five_hour.unwrap().percent, 2.0);
    }

    #[test]
    fn unknown_format_is_error_not_panic() {
        assert_eq!(parse_claude("[]"), Err(UsageError::UnknownFormat));
        assert_eq!(parse_claude("não é json"), Err(UsageError::UnknownFormat));
        assert_eq!(parse_claude(r#"{"amber_gauge": null}"#), Err(UsageError::UnknownFormat));
        assert_eq!(parse_claude(r#"{"five_hour": {"utilization": "muito"}}"#), Err(UsageError::UnknownFormat));
    }

    #[test]
    fn extra_weekly_limits_keep_api_names() {
        let u = parse_claude(USAGE).unwrap();
        // `seven_day_opus` vira "Opus"; nulos e a divisão por produto (`breakdown`) ficam fora.
        assert_eq!(u.extra, vec![("Opus".to_string(), Limit { percent: 12.5, resets_at: Some(1792861200) })]);
    }

    #[test]
    fn token_state_valid_and_expired() {
        assert_eq!(token_state(CREDENTIALS, 1999), TokenState::Valid("sk-teste-falso".into()));
        assert_eq!(token_state(CREDENTIALS, 2000), TokenState::Expired);
    }

    #[test]
    fn token_state_missing_file_and_missing_key() {
        assert_eq!(token_state("", 0), TokenState::Missing);
        assert_eq!(token_state(r#"{"mcpOAuth": {}}"#, 0), TokenState::Missing);
        assert_eq!(token_state(r#"{"claudeAiOauth": {"expiresAt": 5}}"#, 0), TokenState::Missing);
    }

    #[test]
    fn valid_token_debug_hides_secret() {
        let shown = format!("{:?}", token_state(CREDENTIALS, 0));
        assert!(!shown.contains("sk-teste-falso"), "{shown}");
    }

    #[test]
    fn chip_keeps_last_value_on_transient_errors() {
        let ok = parse_claude(USAGE).unwrap();
        assert_eq!(chip_percent(&Ok(ok.clone()), None), Some(42.0));
        assert_eq!(chip_percent(&Err(UsageError::Network), Some(&ok)), Some(42.0));
        assert_eq!(chip_percent(&Err(UsageError::Http(503)), Some(&ok)), Some(42.0));
        assert_eq!(chip_percent(&Err(UsageError::Network), None), None);
        // Login perdido ou formato novo: sem número, para não enganar.
        assert_eq!(chip_percent(&Err(UsageError::Expired), Some(&ok)), None);
        assert_eq!(chip_percent(&Err(UsageError::Missing), Some(&ok)), None);
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
        Account { key: "k".into(), origins: vec![OriginKind::Windows], stopped, data: ClaudeEntry { email: None, plan: None, credentials: Vec::new(), result: Some(result), last_ok } }
    }

    #[test]
    fn identity_from_claude_json() {
        let id = parse_identity(r#"{"oauthAccount": {"accountUuid": "u1", "emailAddress": "a@b.c"}}"#);
        assert_eq!(id, ClaudeIdentity { account_uuid: Some("u1".into()), email: Some("a@b.c".into()) });
        assert_eq!(parse_identity("{}"), ClaudeIdentity::default());
        assert_eq!(parse_identity("não é json"), ClaudeIdentity::default());
    }

    #[test]
    fn plan_names() {
        assert_eq!(plan_name(CREDENTIALS).as_deref(), Some("Max"));
        assert_eq!(plan_name(r#"{"claudeAiOauth": {"subscriptionType": "pro"}}"#).as_deref(), Some("Pro"));
        assert_eq!(plan_name(r#"{"claudeAiOauth": {}}"#), None);
    }

    #[test]
    fn same_uuid_in_two_origins_is_one_account() {
        let one = group_claude(vec![found(OriginKind::Wsl("Ubuntu".into()), Some("u1")), found(OriginKind::Windows, Some("u1"))]);
        assert_eq!(one.len(), 1);
        assert_eq!(one[0].origins, vec![OriginKind::Windows, OriginKind::Wsl("Ubuntu".into())]);
        assert!(one[0].data.credentials[0].starts_with("C:\\casa"));
        assert!(one[0].data.credentials[0].ends_with(".claude/.credentials.json"));
        assert_eq!(group_claude(vec![found(OriginKind::Windows, Some("u1")), found(OriginKind::Wsl("U".into()), Some("u2"))]).len(), 2);
        // Sem `.claude.json`: cada origem é uma conta.
        assert_eq!(group_claude(vec![found(OriginKind::Windows, None), found(OriginKind::Wsl("U".into()), None)]).len(), 2);
    }

    #[test]
    fn first_valid_token_skips_expired() {
        let valid = r#"{"claudeAiOauth": {"accessToken": "t-falso", "expiresAt": 5000}}"#.to_string();
        let expired = r#"{"claudeAiOauth": {"accessToken": "t-velho", "expiresAt": 10}}"#.to_string();
        assert_eq!(first_valid_token(&[expired.clone(), valid], 100), TokenState::Valid("t-falso".into()));
        assert_eq!(first_valid_token(std::slice::from_ref(&expired), 100), TokenState::Expired);
        assert_eq!(first_valid_token(&[String::new()], 100), TokenState::Missing);
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
}
