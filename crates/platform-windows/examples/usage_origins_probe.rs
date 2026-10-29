//! Sonda: as origens que o uso do Claude e do Codex enxerga (Windows e distros do WSL rodando). Não
//! imprime pastas nem nomes de usuário; só a origem e se há Claude e Codex nela.
use std::time::Instant;
use zefiro_platform_windows::wsl;

fn main() {
    let home = std::path::PathBuf::from(std::env::var_os("USERPROFILE").unwrap_or_default());
    let found = wsl::discover(&home);
    for o in &found.origins {
        let yes = |p: std::path::PathBuf| if p.exists() { "sim" } else { "não" };
        println!("{:?}: claude={} codex={}", o.kind, yes(o.home.join(".claude").join(".credentials.json")), yes(o.home.join(".codex").join("sessions")));
        // O que o leitor do Codex acha nesta origem: a conta vira um código curto (nunca o id).
        for r in zefiro_integration_usage::codex::read_codex_accounts(&o.home.join(".codex").join("sessions"), Some(matches!(o.kind, zefiro_integration_usage::origins::OriginKind::Wsl(_)))) {
            use std::hash::{Hash, Hasher};
            let mut h = std::collections::hash_map::DefaultHasher::new();
            r.account.hash(&mut h);
            println!("    codex: conta #{:04x}, evento {:?}, primária {:?}%", h.finish() & 0xffff, r.usage.event_at, r.usage.primary.map(|w| w.used_percent));
        }
    }
    println!("rodando: {:?} · WSL parado: {}", found.running, found.wsl_stopped);
    let t = Instant::now();
    let _ = wsl::running_distros();
    println!("2ª consulta das distros (cache): {:.2} ms", t.elapsed().as_secs_f64() * 1000.0);
}
