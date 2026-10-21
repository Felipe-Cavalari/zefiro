//! Sonda: acompanha as sessões de mídia por `ZEFIRO_PROBE_SECS` segundos (padrão 60) e imprime cada
//! mudança, lado a lado: o que o Windows publica (leitura direta a cada 150 ms) e o retrato do
//! `MediaService` (a cada aviso). Títulos saem como hash curto, para não expor o que se ouve.

use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::sync::Arc;
use std::time::{Duration, Instant};
use windows::Media::Control::GlobalSystemMediaTransportControlsSessionManager as Manager;
use zefiro_platform_windows::media::MediaService;

fn short_hash(s: &str) -> String {
    let mut h = DefaultHasher::new();
    s.hash(&mut h);
    format!("{:04x}", h.finish() & 0xffff)
}

fn raw(manager: &Manager) -> String {
    let Ok(sessions) = manager.GetSessions() else { return "GetSessions falhou".into() };
    let mut parts = Vec::new();
    for s in sessions {
        let id = s.SourceAppUserModelId().map(|h| h.to_string()).unwrap_or_else(|e| format!("?{e}"));
        let info = s.GetPlaybackInfo().ok();
        let status = info.as_ref().and_then(|i| i.PlaybackStatus().ok()).map(|st| st.0);
        let controls = info.as_ref().and_then(|i| i.Controls().ok());
        let next = controls.as_ref().and_then(|c| c.IsNextEnabled().ok());
        let prev = controls.as_ref().and_then(|c| c.IsPreviousEnabled().ok());
        let props = s.TryGetMediaPropertiesAsync().and_then(|op| op.get());
        let (title, thumb) = match &props {
            Ok(p) => (p.Title().map(|t| short_hash(&t.to_string())).unwrap_or_default(), p.Thumbnail().is_ok()),
            Err(e) => (format!("props: {e}"), false),
        };
        let cover = props.ok().and_then(|p| p.Thumbnail().ok()).and_then(|t| zefiro_platform_windows::imaging::decode(&t, 16, true).ok().flatten());
        let cover = cover.map(|c| {
            let mut h = DefaultHasher::new();
            c.bgra().hash(&mut h);
            format!("{:04x}", h.finish() & 0xffff)
        });
        parts.push(format!("{id} st={status:?} t={title} thumb={thumb} capa={cover:?} next={next:?} prev={prev:?}"));
    }
    if parts.is_empty() { "(nenhuma)".into() } else { parts.join(" | ") }
}

fn main() {
    let secs = std::env::var("ZEFIRO_PROBE_SECS").ok().and_then(|s| s.parse().ok()).unwrap_or(60);
    let start = Instant::now();
    let service = Arc::new(std::sync::Mutex::new(None::<Arc<MediaService>>));
    let svc = service.clone();
    let service_started = MediaService::start(Arc::new(move || {
        let Some(s) = svc.lock().ok().and_then(|g| g.clone()) else { return };
        let snap: Vec<String> = s
            .sessions()
            .iter()
            .map(|m| format!("{} {:?} t={} capa={:?} next={} prev={}", m.id, m.playback, short_hash(&m.title), m.cover.as_ref().map(|c| c.id()), m.can_next, m.can_previous))
            .collect();
        println!("[{:>6.2}s] SERVIÇO: {}", start.elapsed().as_secs_f64(), if snap.is_empty() { "(nenhuma)".into() } else { snap.join(" | ") });
    }))
    .expect("thread de mídia");
    *service.lock().unwrap() = Some(Arc::new(service_started));
    let manager = Manager::RequestAsync().and_then(|op| op.get()).expect("gerenciador");
    // `ZEFIRO_PROBE_NEXT=5,20`: pede "próxima" à sessão atual nesses segundos e imprime a resposta.
    let mut next_at: Vec<f64> = std::env::var("ZEFIRO_PROBE_NEXT").ok().map(|s| s.split(',').filter_map(|x| x.trim().parse().ok()).collect()).unwrap_or_default();
    let mut last = String::new();
    while start.elapsed() < Duration::from_secs(secs) {
        if next_at.first().is_some_and(|t| start.elapsed().as_secs_f64() >= *t) {
            next_at.remove(0);
            let answer = manager.GetCurrentSession().and_then(|s| s.TrySkipNextAsync()).and_then(|op| op.get());
            println!("[{:>6.2}s] PRÓXIMA → {answer:?}", start.elapsed().as_secs_f64());
        }
        let now = raw(&manager);
        if now != last {
            println!("[{:>6.2}s] WINDOWS: {now}", start.elapsed().as_secs_f64());
            last = now;
        }
        std::thread::sleep(Duration::from_millis(150));
    }
}
