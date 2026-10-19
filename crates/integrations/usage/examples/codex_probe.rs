//! Sonda só de leitura: o último uso do Codex que a barra mostraria (só números, sem conteúdo).
fn main() {
    let dir = std::path::PathBuf::from(std::env::var_os("USERPROFILE").unwrap_or_default()).join(".codex").join("sessions");
    match zefiro_integration_usage::codex::read_codex(&dir) {
        Some((_, _, u)) => {
            let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_secs() as i64);
            for (name, w) in [("primária", u.primary), ("secundária", u.secondary)] {
                if let Some(w) = w {
                    println!("{name}: {}% ({} min), efetivo {}%, reinicia em {} s", w.used_percent, w.window_minutes, zefiro_integration_usage::codex::effective_percent(&w, now), w.resets_at - now);
                }
            }
            println!("plano: {:?}, evento: {:?}", u.plan, u.event_at);
        }
        None => println!("nenhum uso do Codex encontrado"),
    }
}
