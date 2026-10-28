//! Sonda do uso do Grok (ADR-0027): consulta o Windows e cada distro do WSL rodando como o chip faz
//! (`initialize`, `_x.ai/auth/info`, `_x.ai/billing`, nenhuma sessão) e imprime o tempo, a %, o fim
//! do período e o plano. Nunca imprime o e-mail nem o id da conta. Sai com 1 se nenhuma origem
//! responder.

use zefiro_integration_usage::grok::format_usd;
use zefiro_integration_usage::origins::OriginKind;
use zefiro_platform_windows::grok_usage::probe;

fn main() {
    let mut origins = vec![OriginKind::Windows];
    origins.extend(zefiro_platform_windows::wsl::running_distros().into_iter().map(OriginKind::Wsl));
    let mut ok = 0;
    for origin in &origins {
        let (took, found) = probe(origin);
        let conta = if found.identity.key.is_some() { "com id de conta" } else { "sem id de conta" };
        match &found.result {
            Ok(u) => {
                ok += 1;
                let saldo = u.prepaid_cents.map(format_usd).unwrap_or_else(|| "—".into());
                println!("{origin:?}: {:.1} s, {conta}, {:?} %, fim do período {:?} (unix), {:?} min, plano {:?}, créditos extras {saldo}", took.as_secs_f32(), u.percent, u.period_end, u.period_minutes, u.plan);
            }
            Err(e) => println!("{origin:?}: {:.1} s, {conta}, erro {e:?}", took.as_secs_f32()),
        }
    }
    if ok == 0 {
        eprintln!("nenhuma origem respondeu");
        std::process::exit(1);
    }
}
