//! Sonda somente de leitura: não altera topologia, foco ou posição do mouse.

fn main() {
    let displays = zefiro_platform_windows::monitors::list();
    assert!(!displays.is_empty(), "nenhum monitor enumerado");
    for (index, display) in displays.iter().enumerate() {
        println!("Monitor {}: {} | principal={} | DPI={} | identidade={:?}", index + 1, display.identity.name, display.primary, display.dpi, display.identity.id);
    }
    assert!(displays.iter().any(|m| m.identity.id.is_some()), "nenhuma identidade persistente disponível nesta máquina");
}
