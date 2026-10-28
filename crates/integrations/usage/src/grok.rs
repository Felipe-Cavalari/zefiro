//! Uso do Grok pelo próprio Grok Build (ADR-0027): o `_x.ai/billing` do `grok agent stdio`, que não
//! chama o modelo, e o `_x.ai/auth/info`, que diz de quem é o login. Quem guarda o login e fala com a
//! xAI é o `grok`; o Zefiro não lê o `~/.grok/auth.json`.

use crate::claude::UsageError;
use crate::format::parse_iso_utc;
use crate::origins::{Account, OriginKind, sort_accounts};
use serde_json::Value;

/// A cota da assinatura: a % usada no período (semanal no SuperGrok), quando ele termina, o plano e
/// o saldo de créditos extras em centavos de dólar.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct GrokUsage {
    pub percent: Option<f32>,
    pub period_end: Option<i64>,
    /// Duração do período em minutos (do `start` ao `end`), para o texto do "reinicia".
    pub period_minutes: Option<u32>,
    pub plan: Option<String>,
    pub prepaid_cents: Option<i64>,
}

/// Quem está logado numa origem: o `principalId` (chave da conta, nunca vai para o log) e o e-mail
/// (só no painel).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct GrokIdentity { pub key: Option<String>, pub email: Option<String> }

pub fn parse_identity(result: &Value) -> GrokIdentity {
    let field = |name: &str| result[name].as_str().filter(|s| !s.is_empty()).map(str::to_string);
    GrokIdentity { key: field("principalId"), email: field("email") }
}

/// Centavos num `{"val": …}` (número ou texto).
fn cents(v: &Value) -> Option<i64> {
    v["val"].as_i64().or_else(|| v["val"].as_str().and_then(|s| s.trim().parse().ok()))
}

/// O resultado do `_x.ai/billing`. Sem a % e sem saldo, o formato mudou.
pub fn parse_billing(result: &Value) -> Result<GrokUsage, UsageError> {
    let config = &result["config"];
    let percent = config["creditUsagePercent"].as_f64().map(|p| p.clamp(0.0, 100.0) as f32);
    let prepaid_cents = cents(&config["prepaidBalance"]);
    if percent.is_none() && prepaid_cents.is_none() {
        return Err(UsageError::UnknownFormat);
    }
    let period = &config["currentPeriod"];
    let start = period["start"].as_str().and_then(parse_iso_utc);
    let period_end = period["end"].as_str().and_then(parse_iso_utc);
    let period_minutes = start.zip(period_end).filter(|(s, e)| e > s).map(|(s, e)| ((e - s) / 60) as u32);
    let plan = result["subscription_tier"].as_str().filter(|s| !s.is_empty()).map(str::to_string);
    Ok(GrokUsage { percent, period_end, period_minutes, plan, prepaid_cents })
}

/// O texto de um erro do `_x.ai/billing`: sem login vira `Missing`; o resto, formato desconhecido.
pub fn billing_error(message: &str) -> UsageError {
    let lower = message.to_lowercase();
    if ["auth", "login", "token", "credential"].iter().any(|w| lower.contains(w)) { UsageError::Missing } else { UsageError::UnknownFormat }
}

/// Uma conta do Grok: o e-mail, o plano, o último resultado e o último bom.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct GrokEntry {
    pub email: Option<String>,
    pub plan: Option<String>,
    pub result: Option<Result<GrokUsage, UsageError>>,
    /// Último uso lido com sucesso: o chip continua com ele numa falha passageira.
    pub last_ok: Option<GrokUsage>,
}

/// O que o serviço publica: as contas, se há WSL parado e quando terminou a última consulta.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct GrokSnapshot { pub accounts: Vec<Account<GrokEntry>>, pub wsl_stopped: bool, pub at: Option<std::time::Instant> }

/// O que uma origem respondeu.
#[derive(Debug, Clone, PartialEq)]
pub struct GrokFound { pub origin: OriginKind, pub identity: GrokIdentity, pub result: Result<GrokUsage, UsageError> }

/// Junta as origens da mesma conta (`principalId`); sem ele, cada origem é uma conta. Numa conta com
/// várias origens vale o primeiro resultado bom.
pub fn group_grok(mut found: Vec<GrokFound>) -> Vec<Account<GrokEntry>> {
    found.sort_by(|a, b| a.origin.cmp(&b.origin));
    let mut accounts: Vec<Account<GrokEntry>> = Vec::new();
    for f in found {
        let key = f.identity.key.clone().unwrap_or_else(|| format!("origem:{:?}", f.origin));
        let plan = f.result.as_ref().ok().and_then(|u| u.plan.clone());
        match accounts.iter_mut().find(|a| a.key == key) {
            Some(a) => {
                a.origins.push(f.origin);
                a.data.email = a.data.email.take().or(f.identity.email);
                a.data.plan = a.data.plan.take().or(plan);
                if !matches!(a.data.result, Some(Ok(_))) {
                    a.data.result = Some(f.result);
                }
            }
            None => accounts.push(Account { key, origins: vec![f.origin], stopped: false, data: GrokEntry { email: f.identity.email, plan, result: Some(f.result), last_ok: None } }),
        }
    }
    sort_accounts(&mut accounts);
    accounts
}

/// O último uso bom de cada conta vem do ciclo anterior (e passa a ser o atual, se ele for bom).
pub fn inherit_grok(previous: &[Account<GrokEntry>], current: &mut [Account<GrokEntry>]) {
    for account in current {
        let old = previous.iter().find(|p| p.key == account.key).and_then(|p| p.data.last_ok.clone());
        account.data.last_ok = match &account.data.result {
            Some(Ok(u)) => Some(u.clone()),
            _ => old,
        };
    }
}

/// % da conta: a atual ou, numa falha passageira, a última boa; 0 depois do fim do período.
fn account_percent(entry: &GrokEntry, now_unix: i64) -> Option<f32> {
    let usage = match entry.result.as_ref()? {
        Ok(u) => Some(u),
        Err(UsageError::Network) => entry.last_ok.as_ref(),
        Err(_) => None,
    }?;
    let percent = usage.percent?;
    Some(if usage.period_end.is_some_and(|t| t <= now_unix) { 0.0 } else { percent })
}

/// % do chip: a maior entre as contas.
pub fn grok_chip_percent(accounts: &[Account<GrokEntry>], now_unix: i64) -> Option<f32> {
    accounts.iter().filter_map(|a| account_percent(&a.data, now_unix)).reduce(f32::max)
}

/// "US$ 4,00" a partir de centavos.
pub fn format_usd(cents: i64) -> String {
    format!("US$ {},{:02}", cents / 100, (cents % 100).abs())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    // Resposta do Grok Build 1.0.70 (sonda de 2026-10-28), sem os campos que não importam.
    const BILLING: &str = r#"{"config":{"creditUsagePercent":25.0,"currentPeriod":{"type":"USAGE_PERIOD_TYPE_WEEKLY","start":"2026-10-23T23:04:17.875+00:00","end":"2026-10-30T23:04:17.875+00:00"},"prepaidBalance":{"val":0},"isUnifiedBillingUser":true},"subscription_tier":"SuperGrok"}"#;

    fn usage(percent: f32, end: i64) -> GrokUsage {
        GrokUsage { percent: Some(percent), period_end: Some(end), ..Default::default() }
    }

    fn found(origin: OriginKind, key: Option<&str>, result: Result<GrokUsage, UsageError>) -> GrokFound {
        GrokFound { origin, identity: GrokIdentity { key: key.map(String::from), email: Some("a@b.c".into()) }, result }
    }

    #[test]
    fn billing_is_read() {
        let u = parse_billing(&serde_json::from_str(BILLING).unwrap()).unwrap();
        assert_eq!(u.percent, Some(25.0));
        assert_eq!(u.period_end, parse_iso_utc("2026-10-30T23:04:17Z"));
        assert_eq!(u.period_minutes, Some(7 * 24 * 60), "semanal");
        assert_eq!((u.plan.as_deref(), u.prepaid_cents), (Some("SuperGrok"), Some(0)));
        let only_balance = json!({ "config": { "prepaidBalance": { "val": "400" } } });
        assert_eq!(parse_billing(&only_balance).map(|u| (u.percent, u.prepaid_cents)), Ok((None, Some(400))), "chave de API sem assinatura");
        assert_eq!(parse_billing(&json!({ "config": {} })), Err(UsageError::UnknownFormat));
        assert_eq!(parse_billing(&json!({ "config": { "creditUsagePercent": 130.0 } })).unwrap().percent, Some(100.0));
    }

    #[test]
    fn identity_and_errors() {
        let id = parse_identity(&json!({ "methodId": "cached_token", "email": "a@b.c", "principalId": "444495b0", "firstName": "x" }));
        assert_eq!(id, GrokIdentity { key: Some("444495b0".into()), email: Some("a@b.c".into()) });
        assert_eq!(parse_identity(&json!(null)), GrokIdentity::default());
        assert_eq!(billing_error("Authentication required"), UsageError::Missing);
        assert_eq!(billing_error("Method not found"), UsageError::UnknownFormat);
        assert_eq!(format_usd(400), "US$ 4,00");
        assert_eq!(format_usd(1234), "US$ 12,34");
    }

    #[test]
    fn same_principal_in_two_origins_is_one_account() {
        let wsl = OriginKind::Wsl("Ubuntu-24.04".into());
        let accounts = group_grok(vec![found(wsl.clone(), Some("p1"), Err(UsageError::Network)), found(OriginKind::Windows, Some("p1"), Ok(usage(25.0, 100)))]);
        assert_eq!(accounts.len(), 1);
        assert_eq!(accounts[0].origins, [OriginKind::Windows, wsl.clone()], "Windows primeiro");
        assert!(matches!(accounts[0].data.result, Some(Ok(_))), "vale o resultado bom");
        let apart = group_grok(vec![found(OriginKind::Windows, None, Ok(usage(1.0, 100))), found(wsl, None, Ok(usage(2.0, 100)))]);
        assert_eq!(apart.len(), 2, "sem principalId, uma conta por origem");
    }

    #[test]
    fn chip_keeps_the_last_value_on_network_errors_and_resets_after_the_period() {
        let mut previous = group_grok(vec![found(OriginKind::Windows, Some("p1"), Ok(usage(40.0, 1000)))]);
        inherit_grok(&[], &mut previous);
        let mut current = group_grok(vec![found(OriginKind::Windows, Some("p1"), Err(UsageError::Network))]);
        inherit_grok(&previous, &mut current);
        assert_eq!(grok_chip_percent(&current, 500), Some(40.0), "falha passageira");
        assert_eq!(grok_chip_percent(&current, 1000), Some(0.0), "a semana virou");
        let mut lost = group_grok(vec![found(OriginKind::Windows, Some("p1"), Err(UsageError::Missing))]);
        inherit_grok(&previous, &mut lost);
        assert_eq!(grok_chip_percent(&lost, 500), None, "sem login, sem número");
        assert_eq!(grok_chip_percent(&[], 0), None);
        let mut two = group_grok(vec![found(OriginKind::Windows, Some("a"), Ok(usage(10.0, 1000))), found(OriginKind::Wsl("U".into()), Some("b"), Ok(usage(70.0, 1000)))]);
        inherit_grok(&[], &mut two);
        assert_eq!(grok_chip_percent(&two, 0), Some(70.0), "a maior");
        assert!(two.iter().all(|a| a.data.last_ok.is_some()));
    }
}
