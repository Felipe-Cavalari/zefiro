//! Sonda só de leitura: brilho de cada monitor por DDC/CI e quanto a leitura demora (não muda nada).
fn main() {
    for round in 0..3 {
        for (device, name, percent, took) in zefiro_platform_windows::brightness::probe() {
            println!("rodada {round}: {device} | {name} | brilho {percent:?} | {:.1} ms", took.as_secs_f64() * 1000.0);
        }
    }
}
