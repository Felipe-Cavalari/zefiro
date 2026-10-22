//! Sonda das imagens geradas pelo Codex: pede uma imagem, passa os eventos pela `Conversation`
//! (como o painel) e grava o PNG do `imageGeneration` como o laço grava. Mostra o texto da
//! resposta (as mensagens do turno separadas por linha em branco) e o tamanho da imagem.
//!
//! `cargo run -p zefiro-platform-windows --example chat_generated_probe -- [pedido]`
//! `ZEFIRO_CHAT_ORIGIN=auto|windows|<distro>`.

use std::time::{Duration, Instant};
use zefiro_integration_chat::agent::Agent;
use zefiro_integration_chat::conversation::Conversation;
use zefiro_integration_chat::image::{decode_base64, is_png};
use zefiro_integration_chat::origin::{Prefer, parse_choice};
use zefiro_integration_chat::protocol::Event;
use zefiro_platform_windows::chat::{ChatService, Signal, Start, new_id};
use zefiro_platform_windows::imaging;

fn run() -> Result<(), String> {
    let request = std::env::args().nth(1).unwrap_or_else(|| "Gere uma imagem pequena e simples de um círculo azul num fundo branco.".into());
    let dir = std::env::temp_dir().join("zefiro-chat-probe");
    let service = ChatService::new(0).map_err(|e| e.to_string())?;
    let choice = parse_choice(&std::env::var("ZEFIRO_CHAT_ORIGIN").unwrap_or_default());
    service.start(Start { agent: Agent::Codex, choice, prefer: Prefer::Windows, model: String::new(), session: String::new(), resume: false, windows_dir: dir.clone() });
    let mut c = Conversation::new(new_id(), Agent::Codex);
    c.ask(&request);
    service.send(request, Vec::new(), String::new());
    let started = Instant::now();
    while started.elapsed() < Duration::from_secs(240) {
        for s in service.take() {
            match s {
                Signal::Ready { origin } => println!("pronto em {origin}"),
                Signal::Event(Event::ImageStarted) => {
                    c.apply(&Event::ImageStarted);
                    println!("gerando a imagem… ({:.1} s)", started.elapsed().as_secs_f32());
                }
                Signal::Event(Event::Image { item, data }) => {
                    let bytes = decode_base64(&data).ok_or("base64 inválido")?;
                    let path = dir.join(format!("{}.png", new_id()));
                    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
                    std::fs::write(&path, &bytes).map_err(|e| e.to_string())?;
                    let shown = imaging::decode_bytes(&bytes, 800).ok_or("não decodificou")?;
                    c.add_reply_image(item);
                    println!("imagem: {} KB, PNG = {}, na tela {}x{}, em {}", bytes.len() / 1024, is_png(&bytes), shown.width(), shown.height(), path.display());
                }
                Signal::Event(Event::Finished { error: Some(e), .. }) => return Err(e),
                Signal::Event(e @ Event::Finished { .. }) => {
                    c.apply(&e);
                    let reply = &c.messages[1];
                    println!("resposta em {:.1} s, {} imagem(ns):\n---\n{}\n---", started.elapsed().as_secs_f32(), reply.images.len(), reply.text);
                    service.stop();
                    std::thread::sleep(Duration::from_secs(2));
                    return if reply.images.is_empty() { Err("nenhuma imagem na resposta".into()) } else { Ok(()) };
                }
                Signal::Event(e) => {
                    c.apply(&e);
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
