//! Sonda: requer a ilha iniciada; falha se o fork ainda não publicar a reserva visual.

fn main() {
    let reserve = zefiro_platform_windows::window::island_reservation().expect("a ilha não publicou Zefiro.ReservedWidth");
    assert!(reserve > 0.0);
    println!("Reserva visual máxima publicada: {reserve} px");
}
