//! Sonda: lista as sessões de mídia vistas pelo `MediaService` (título, estado, capa, cor da capa).

use std::sync::Arc;
use zefiro_platform_windows::media::{MediaService, now_unix_ms};

fn main() {
    let service = MediaService::start(Arc::new(|| {})).expect("thread de mídia");
    std::thread::sleep(std::time::Duration::from_secs(3));
    let sessions = service.sessions();
    println!("{} sessões", sessions.len());
    for s in sessions {
        let cover = s.cover.as_ref().map(|c| format!("{}x{}", c.width(), c.height()));
        let accent = s.cover.as_ref().and_then(zefiro_island::accent::cover_accent);
        println!("{} [{}] {:?} | {} | pos {:.1}/{:.1}s | capa {cover:?} | destaque {accent:?} | ícone {:?}", s.app_name, s.id, s.playback, s.headline(), s.position_at(now_unix_ms()), s.duration_s, s.app_icon.as_ref().map(|i| i.width()));
    }
}
