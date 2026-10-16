//! Sonda: estado do "Não perturbe" lido pelo WNF, sem mudar nada.
fn main() {
    zefiro_platform_windows::dnd::watch(std::sync::Arc::new(|| println!("mudou: {}", zefiro_platform_windows::dnd::active())));
    println!("não perturbe ativo: {}", zefiro_platform_windows::dnd::active());
    std::thread::sleep(std::time::Duration::from_millis(500));
    zefiro_platform_windows::dnd::unwatch();
}
