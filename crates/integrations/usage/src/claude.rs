//! Uso do Claude: a resposta de `GET https://api.anthropic.com/api/oauth/usage` e o token do
//! Claude Code em `~/.claude/.credentials.json`. O token nunca é renovado nem gravado aqui.

use serde_json::Value;

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
}
