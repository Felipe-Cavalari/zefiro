//! Sonda só de leitura: saídas, entradas, padrões, mudo do microfone e a saída gravada de um app.
//! Uso: `cargo run -p zefiro-platform-windows --example audio_probe -- [aumid] [nome]`

use zefiro_platform_windows::audio_devices::{Flow, app_route, devices, mic_muted};
use windows::Win32::System::Com::{COINIT_APARTMENTTHREADED, CoInitializeEx};

fn main() {
    // SAFETY: COM nesta thread para a sonda.
    let _ = unsafe { CoInitializeEx(None, COINIT_APARTMENTTHREADED) };
    for flow in [Flow::Output, Flow::Input] {
        match devices(flow) {
            Ok((list, default)) => {
                println!("{flow:?}: {} dispositivos", list.len());
                for d in list {
                    let mark = if Some(&d.id) == default.as_ref() { "*" } else { " " };
                    println!(" {mark} {}", d.name);
                }
            }
            Err(e) => println!("{flow:?}: erro {e}"),
        }
    }
    println!("microfone mudo: {:?}", mic_muted());
    let args: Vec<String> = std::env::args().skip(1).collect();
    let aumid = args.first().map_or("Brave", String::as_str);
    let name = args.get(1).map_or("Brave", String::as_str);
    match app_route(aumid, name) {
        Some((_, state)) => println!("saída de {aumid}: {state:?}"),
        None => println!("IAudioPolicyConfig indisponível"),
    }
}
