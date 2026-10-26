//! Sonda das imagens do chat: converte um arquivo (e, se for `.bmp`, o mesmo como `CF_DIB`) para
//! PNG reduzido, gera a miniatura e pergunta ao agente o que a imagem mostra. Com
//! `ZEFIRO_PROBE_CLIPBOARD=1`, usa a imagem da área de transferência no lugar do arquivo.
//!
//! `cargo run -p zefiro-platform-windows --example chat_image_probe -- <imagem> [pergunta]`
//! `ZEFIRO_CHAT_AGENT=claude|codex`, `ZEFIRO_CHAT_ORIGIN=auto|windows|<distro>`.

use std::time::{Duration, Instant};
use zefiro_integration_chat::agent::Agent;
use zefiro_integration_chat::image::{MAX_SIDE, base64, dib_to_bmp};
use zefiro_integration_chat::origin::{Prefer, parse_choice};
use zefiro_integration_chat::protocol::Event;
use zefiro_platform_windows::chat::{ChatService, Signal, Start, new_id};
use zefiro_platform_windows::{imaging, system_events};

fn run() -> Result<(), String> {
    let mut args = std::env::args().skip(1);
    let bytes = if std::env::var("ZEFIRO_PROBE_CLIPBOARD").is_ok_and(|v| v == "1") {
        system_events::clipboard_image().ok_or("sem imagem na área de transferência")?
    } else {
        let path = args.next().ok_or("falta o arquivo de imagem")?;
        let bytes = std::fs::read(&path).map_err(|e| e.to_string())?;
        if path.to_lowercase().ends_with(".bmp") {
            let bmp = dib_to_bmp(&bytes[14..]).ok_or("DIB inválido")?;
            assert_eq!(bmp, bytes, "o BMP refeito a partir do DIB é igual ao arquivo");
            println!("DIB → BMP igual ao arquivo");
        }
        bytes
    };
    let question = args.next().unwrap_or_else(|| "Que número aparece na imagem? Só o número.".into());
    let started = Instant::now();
    let png = imaging::to_png(&bytes, MAX_SIDE).map_err(|e| e.to_string())?;
    let thumb = imaging::decode_bytes(&png, 128).ok_or("miniatura falhou")?;
    println!("{} bytes → PNG {} bytes, miniatura {}x{} em {} ms", bytes.len(), png.len(), thumb.width(), thumb.height(), started.elapsed().as_millis());
    let agent = Agent::parse(&std::env::var("ZEFIRO_CHAT_AGENT").unwrap_or_default());
    let service = ChatService::new(0).map_err(|e| e.to_string())?;
    let choice = parse_choice(&std::env::var("ZEFIRO_CHAT_ORIGIN").unwrap_or_default());
    service.start(Start { agent, choice, prefer: Prefer::Windows, model: String::new(), session: new_id(), resume: false, windows_dir: std::env::temp_dir().join("zefiro-chat-probe") });
    service.send(question, vec![base64(&png)], String::new());
    let mut reply = String::new();
    let started = Instant::now();
    while started.elapsed() < Duration::from_secs(120) {
        for s in service.take() {
            match s {
                Signal::Ready { origin } => println!("pronto em {origin}"),
                Signal::Event(Event::Delta { text, .. }) => reply.push_str(&text),
                Signal::Event(Event::Assistant { text, .. }) if reply.is_empty() => reply = text,
                Signal::Event(Event::Finished { error: Some(e), .. }) => return Err(e),
                Signal::Event(Event::Finished { .. }) => {
                    println!("resposta em {:.1} s: {reply}", started.elapsed().as_secs_f32());
                    service.stop();
                    std::thread::sleep(Duration::from_secs(2));
                    return Ok(());
                }
                Signal::Exited { stderr } => return Err(format!("processo saiu: {stderr}")),
                Signal::Failed(t) => return Err(t),
                _ => {}
            }
        }
        std::thread::sleep(Duration::from_millis(30));
    }
    Err("tempo esgotado".into())
}

fn main() {
    if let Err(e) = run() {
        eprintln!("falhou: {e}");
        std::process::exit(1);
    }
}
