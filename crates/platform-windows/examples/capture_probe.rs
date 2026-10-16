//! Sonda: 2 s de captura do som do sistema e as barras do visualizador.
use std::sync::{Arc, Mutex};
fn main() {
    let wave = Arc::new(Mutex::new(zefiro_island::waveform::Waveform::new()));
    let w = wave.clone();
    let calls = Arc::new(Mutex::new((0usize, 0f32)));
    let c = calls.clone();
    let cap = zefiro_platform_windows::audio_capture::AudioCapture::start(Box::new(move |mono: &[f32], rate: f32| {
        w.lock().unwrap().push(mono, rate);
        let mut g = c.lock().unwrap();
        g.0 += 1;
        g.1 = g.1.max(mono.iter().fold(0f32, |m, s| m.max(s.abs())));
    })).unwrap();
    std::thread::sleep(std::time::Duration::from_secs(2));
    drop(cap);
    println!("pacotes={:?} barras={:?}", calls.lock().unwrap(), wave.lock().unwrap().bars(10));
}
