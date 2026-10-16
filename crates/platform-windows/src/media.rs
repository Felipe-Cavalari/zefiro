//! Sessões de mídia do Windows (SMTC, `GlobalSystemMediaTransportControlsSessionManager`).
//!
//! Uma thread própria (MTA) guarda o gerenciador e as sessões, escuta os eventos e monta um retrato
//! (`Vec<MediaSession>`) a cada mudança. A thread de UI só lê o retrato e manda comandos; nenhuma
//! chamada assíncrona do WinRT espera na thread de UI.

use std::collections::HashMap;
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::{Arc, Mutex};
use std::time::Duration;
use windows::Foundation::TypedEventHandler;
use windows::Media::Control::{
    GlobalSystemMediaTransportControlsSession as Session, GlobalSystemMediaTransportControlsSessionManager as Manager,
    GlobalSystemMediaTransportControlsSessionPlaybackStatus as Status,
};
use windows::Win32::System::WinRT::{RO_INIT_MULTITHREADED, RoInitialize, RoUninitialize};
use windows::core::Result;
use zefiro_island::media::{MediaSession, Playback, app_name_from_aumid};
use zefiro_platform::image::Image;

/// Lado da capa decodificada (a maior exibição é 64 DIP; 128 cobre 200 % de escala).
const COVER_PX: u32 = 128;

#[derive(Debug, Clone, PartialEq)]
pub enum MediaCommand { PlayPause(String), Next(String), Previous(String), Seek(String, f64) }

enum Job { Refresh { full: bool }, Command(MediaCommand), Stop }

pub struct MediaService {
    tx: Sender<Job>,
    shared: Arc<Mutex<Vec<MediaSession>>>,
    thread: Option<std::thread::JoinHandle<()>>,
}

impl MediaService {
    /// `notify` é chamado (em outra thread) sempre que o retrato muda.
    pub fn start(notify: Arc<dyn Fn() + Send + Sync>) -> std::io::Result<MediaService> {
        let (tx, rx) = mpsc::channel();
        let shared = Arc::new(Mutex::new(Vec::new()));
        let (worker_tx, worker_shared) = (tx.clone(), shared.clone());
        let thread = std::thread::Builder::new().name("zefiro-media".into()).spawn(move || {
            // SAFETY: inicializa o WinRT nesta thread; desfeito no fim dela.
            let inited = unsafe { RoInitialize(RO_INIT_MULTITHREADED) }.is_ok();
            if let Err(e) = run(&rx, &worker_tx, &worker_shared, notify.as_ref()) {
                tracing::warn!("mídia do Windows indisponível: {e}");
            }
            if inited {
                // SAFETY: par do RoInitialize acima.
                unsafe { RoUninitialize() };
            }
        })?;
        Ok(MediaService { tx, shared, thread: Some(thread) })
    }

    pub fn sessions(&self) -> Vec<MediaSession> {
        self.shared.lock().map(|s| s.clone()).unwrap_or_default()
    }

    pub fn command(&self, c: MediaCommand) {
        let _ = self.tx.send(Job::Command(c));
    }
}

impl Drop for MediaService {
    fn drop(&mut self) {
        let _ = self.tx.send(Job::Stop);
        if let Some(t) = self.thread.take() {
            let _ = t.join();
        }
    }
}

struct Watched { session: Session, tokens: [i64; 3] }

fn unwatch(w: &Watched) {
    let _ = w.session.RemoveMediaPropertiesChanged(w.tokens[0]);
    let _ = w.session.RemovePlaybackInfoChanged(w.tokens[1]);
    let _ = w.session.RemoveTimelinePropertiesChanged(w.tokens[2]);
}

fn watch(session: Session, tx: &Sender<Job>) -> Result<Watched> {
    let (a, b, c) = (tx.clone(), tx.clone(), tx.clone());
    let t0 = session.MediaPropertiesChanged(&TypedEventHandler::new(move |_, _| { let _ = a.send(Job::Refresh { full: true }); Ok(()) }))?;
    let t1 = session.PlaybackInfoChanged(&TypedEventHandler::new(move |_, _| { let _ = b.send(Job::Refresh { full: false }); Ok(()) }))?;
    let t2 = session.TimelinePropertiesChanged(&TypedEventHandler::new(move |_, _| { let _ = c.send(Job::Refresh { full: false }); Ok(()) }))?;
    Ok(Watched { session, tokens: [t0, t1, t2] })
}

fn run(rx: &Receiver<Job>, tx: &Sender<Job>, shared: &Mutex<Vec<MediaSession>>, notify: &(dyn Fn() + Send + Sync)) -> Result<()> {
    let manager = Manager::RequestAsync()?.get()?;
    let changed = tx.clone();
    let sessions_token = manager.SessionsChanged(&TypedEventHandler::new(move |_, _| {
        let _ = changed.send(Job::Refresh { full: true });
        Ok(())
    }))?;
    let mut watched: Vec<Watched> = Vec::new();
    let mut covers: HashMap<(String, String, String), Option<Image>> = HashMap::new();
    let mut last: Vec<MediaSession> = Vec::new();
    let mut pending = Some(true);
    loop {
        if let Some(full) = pending.take() {
            if full {
                for w in watched.drain(..) {
                    unwatch(&w);
                }
                for s in manager.GetSessions()? {
                    match watch(s, tx) {
                        Ok(w) => watched.push(w),
                        Err(e) => tracing::warn!("não consegui escutar uma sessão de mídia: {e}"),
                    }
                }
            }
            let snapshot = snapshot(&watched, full, &last, &mut covers);
            if snapshot != last {
                last = snapshot.clone();
                if let Ok(mut s) = shared.lock() {
                    *s = snapshot;
                }
                notify();
            }
        }
        let job = match rx.recv() {
            Ok(job) => job,
            Err(_) => break,
        };
        // Agrupa a rajada de eventos (o SMTC manda vários de uma vez na troca de faixa).
        let mut full = None;
        for job in std::iter::once(job).chain(std::iter::from_fn(|| rx.recv_timeout(Duration::from_millis(30)).ok())) {
            match job {
                Job::Refresh { full: f } => full = Some(full.unwrap_or(false) || f),
                Job::Command(c) => run_command(&watched, &c),
                Job::Stop => {
                    for w in &watched {
                        unwatch(w);
                    }
                    let _ = manager.RemoveSessionsChanged(sessions_token);
                    return Ok(());
                }
            }
        }
        pending = full;
    }
    Ok(())
}

fn run_command(watched: &[Watched], c: &MediaCommand) {
    let id = match c {
        MediaCommand::PlayPause(id) | MediaCommand::Next(id) | MediaCommand::Previous(id) | MediaCommand::Seek(id, _) => id,
    };
    let Some(w) = watched.iter().find(|w| w.session.SourceAppUserModelId().is_ok_and(|s| s == id.as_str())) else { return };
    let result = match c {
        MediaCommand::PlayPause(_) => w.session.TryTogglePlayPauseAsync().and_then(|op| op.get()),
        MediaCommand::Next(_) => w.session.TrySkipNextAsync().and_then(|op| op.get()),
        MediaCommand::Previous(_) => w.session.TrySkipPreviousAsync().and_then(|op| op.get()),
        MediaCommand::Seek(_, seconds) => w.session.TryChangePlaybackPositionAsync((seconds * 1e7) as i64).and_then(|op| op.get()),
    };
    if let Err(e) = result {
        tracing::warn!(?c, "comando de mídia falhou: {e}");
    }
}

/// 100 ns desde 1601 → ms desde 1970.
fn unix_ms(universal: i64) -> i64 {
    universal / 10_000 - 11_644_473_600_000
}

fn snapshot(watched: &[Watched], full: bool, last: &[MediaSession], covers: &mut HashMap<(String, String, String), Option<Image>>) -> Vec<MediaSession> {
    let mut out = Vec::new();
    for w in watched {
        match read_session(&w.session, full, last, covers) {
            Ok(s) => out.push(s),
            Err(e) => tracing::debug!("sessão de mídia ilegível: {e}"),
        }
    }
    // Capas de faixas que já não estão em nenhuma sessão saem do cache.
    covers.retain(|k, _| out.iter().any(|s| s.track_key() == *k));
    out
}

fn read_session(s: &Session, full: bool, last: &[MediaSession], covers: &mut HashMap<(String, String, String), Option<Image>>) -> Result<MediaSession> {
    let id = s.SourceAppUserModelId()?.to_string();
    let previous = last.iter().find(|p| p.id == id);
    let playback = s.GetPlaybackInfo()?;
    let controls = playback.Controls()?;
    let status = match playback.PlaybackStatus()? {
        Status::Playing => Playback::Playing,
        Status::Paused => Playback::Paused,
        _ => Playback::Stopped,
    };
    let timeline = s.GetTimelineProperties()?;
    let start = timeline.StartTime()?.Duration;
    let end = timeline.EndTime()?.Duration;
    let mut session = MediaSession {
        app_name: app_name_from_aumid(&id),
        id,
        title: String::new(),
        artist: String::new(),
        album: String::new(),
        playback: status,
        position_s: (timeline.Position()?.Duration - start) as f64 / 1e7,
        sampled_ms: unix_ms(timeline.LastUpdatedTime()?.UniversalTime),
        duration_s: ((end - start) as f64 / 1e7).max(0.0),
        can_previous: controls.IsPreviousEnabled()?,
        can_next: controls.IsNextEnabled()?,
        can_play_pause: controls.IsPlayPauseToggleEnabled()?,
        can_seek: controls.IsPlaybackPositionEnabled()?,
        cover: None,
    };
    match previous.filter(|_| !full) {
        Some(p) => {
            session.title = p.title.clone();
            session.artist = p.artist.clone();
            session.album = p.album.clone();
            session.cover = p.cover.clone();
        }
        None => {
            let props = s.TryGetMediaPropertiesAsync()?.get()?;
            session.title = props.Title()?.to_string();
            session.artist = props.Artist()?.to_string();
            session.album = props.AlbumTitle()?.to_string();
            let key = session.track_key();
            session.cover = match covers.get(&key) {
                Some(c) => c.clone(),
                None => {
                    let cover = decode_cover(&props).unwrap_or_else(|e| {
                        tracing::debug!("capa ilegível: {e}");
                        None
                    });
                    // Uma capa ausente agora pode chegar num evento seguinte: só guarda as que vieram.
                    if cover.is_some() {
                        covers.insert(key, cover.clone());
                    }
                    cover
                }
            };
        }
    }
    // Browsers sem posição publicada mandam LastUpdatedTime zerado: trata como amostra de agora.
    if session.sampled_ms < 0 {
        session.sampled_ms = now_unix_ms();
    }
    Ok(session)
}

pub fn now_unix_ms() -> i64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_millis() as i64)
}

fn decode_cover(props: &windows::Media::Control::GlobalSystemMediaTransportControlsSessionMediaProperties) -> Result<Option<Image>> {
    match props.Thumbnail() {
        Ok(thumb) => crate::imaging::decode(&thumb, COVER_PX, true),
        Err(_) => Ok(None),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn converts_windows_time_to_unix() {
        // 2026-01-01T00:00:00Z
        let universal = (1_767_225_600_000i64 + 11_644_473_600_000) * 10_000;
        assert_eq!(unix_ms(universal), 1_767_225_600_000);
    }
}
