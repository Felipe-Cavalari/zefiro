//! Sessões de mídia do Windows (SMTC, `GlobalSystemMediaTransportControlsSessionManager`).
//!
//! Uma thread própria (MTA) guarda o gerenciador e as sessões, escuta os eventos e monta um retrato
//! (`Vec<MediaSession>`) a cada mudança. A thread de UI só lê o retrato e manda comandos; nenhuma
//! chamada assíncrona do WinRT espera na thread de UI.

use std::collections::{HashMap, HashSet};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender};
use std::sync::{Arc, Mutex};
use std::time::Duration;
use windows::Foundation::TypedEventHandler;
use windows::Media::Control::{
    GlobalSystemMediaTransportControlsSession as Session, GlobalSystemMediaTransportControlsSessionManager as Manager,
    GlobalSystemMediaTransportControlsSessionPlaybackStatus as Status,
};
use windows::Storage::Streams::{Buffer, DataReader, IRandomAccessStream, InputStreamOptions};
use windows::Win32::System::WinRT::{RO_INIT_MULTITHREADED, RoInitialize, RoUninitialize};
use windows::core::{Interface, Result};
use zefiro_island::media::{COVER_SETTLE_MS, CoverCache, MediaSession, Playback, VanishHold, app_name_from_aumid, fingerprint, new_tracks};
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
    let mut covers = CoverCache::default();
    let mut icons = Icons::default();
    let mut last: Vec<MediaSession> = Vec::new();
    let mut failing: HashSet<String> = HashSet::new();
    let mut hold = VanishHold::default();
    // Releituras marcadas depois de uma faixa nova (ms desde a época Unix).
    let mut settle: Vec<i64> = Vec::new();
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
            let published = snapshot(&watched, full, &last, &mut covers, &mut failing);
            let now = now_unix_ms();
            if full && new_tracks(&last, &published) {
                settle = COVER_SETTLE_MS.iter().map(|ms| now + ms).collect();
            }
            let mut snapshot = hold.apply(published, now);
            icons.fill(&mut snapshot);
            if snapshot != last {
                last = snapshot.clone();
                if let Ok(mut s) = shared.lock() {
                    *s = snapshot;
                }
                notify();
            }
        }
        // Sem evento, acorda para reler a capa ou soltar uma sessão segurada.
        let wake = settle.first().copied().into_iter().chain(hold.deadline()).min();
        let job = match wake {
            None => match rx.recv() {
                Ok(job) => job,
                Err(_) => break,
            },
            Some(at) => match rx.recv_timeout(Duration::from_millis((at - now_unix_ms()).max(0) as u64)) {
                Ok(job) => job,
                Err(RecvTimeoutError::Timeout) => {
                    let now = now_unix_ms();
                    let reread = settle.first().is_some_and(|t| *t <= now);
                    settle.retain(|t| *t > now);
                    pending = Some(reread);
                    continue;
                }
                Err(RecvTimeoutError::Disconnected) => break,
            },
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
    let Some(w) = watched.iter().find(|w| w.session.SourceAppUserModelId().is_ok_and(|s| s == id.as_str())) else {
        tracing::info!(?c, "comando de mídia sem sessão (o app trocou de faixa ou fechou)");
        return;
    };
    let result = match c {
        MediaCommand::PlayPause(_) => w.session.TryTogglePlayPauseAsync().and_then(|op| op.get()),
        MediaCommand::Next(_) => w.session.TrySkipNextAsync().and_then(|op| op.get()),
        MediaCommand::Previous(_) => w.session.TrySkipPreviousAsync().and_then(|op| op.get()),
        MediaCommand::Seek(_, seconds) => w.session.TryChangePlaybackPositionAsync((seconds * 1e7) as i64).and_then(|op| op.get()),
    };
    match result {
        Ok(true) => {}
        Ok(false) => tracing::info!(?c, "o app recusou o comando de mídia"),
        Err(e) => tracing::warn!(?c, "comando de mídia falhou: {e}"),
    }
}

/// 100 ns desde 1601 → ms desde 1970.
fn unix_ms(universal: i64) -> i64 {
    universal / 10_000 - 11_644_473_600_000
}

fn snapshot(watched: &[Watched], full: bool, last: &[MediaSession], covers: &mut CoverCache, failing: &mut HashSet<String>) -> Vec<MediaSession> {
    let mut out = Vec::new();
    for w in watched {
        let id = w.session.SourceAppUserModelId().map_or_else(|_| "?".to_string(), |s| s.to_string());
        match read_session(&w.session, full, last, covers) {
            Ok(s) => {
                if failing.remove(&id) {
                    tracing::info!(session = %id, "sessão de mídia legível de novo");
                }
                out.push(s);
            }
            // Uma sessão ilegível some da ilha: avisa uma vez, até ela voltar.
            Err(e) if failing.insert(id.clone()) => tracing::warn!(session = %id, "sessão de mídia ilegível; fica fora da ilha: {e}"),
            Err(_) => {}
        }
    }
    // Capas de faixas que já não estão em nenhuma sessão saem do cache.
    covers.retain(|k| out.iter().any(|s| s.track_key() == *k));
    out
}

fn read_session(s: &Session, full: bool, last: &[MediaSession], covers: &mut CoverCache) -> Result<MediaSession> {
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
        app_icon: None,
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
            // O Brave e o Chrome publicam o título novo antes da miniatura nova: a capa segue os
            // bytes da miniatura, não só a faixa, senão a da faixa anterior ficaria presa.
            let thumb = thumbnail(&props);
            session.cover = covers.resolve(&session.track_key(), thumb.as_ref().map(|(print, _)| *print), || {
                let (_, stream) = thumb?;
                crate::imaging::decode_stream(&stream, COVER_PX, true).unwrap_or_else(|e| {
                    tracing::debug!("capa ilegível: {e}");
                    None
                })
            });
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

/// Miniatura publicada agora: a impressão digital dos bytes e o fluxo, de volta ao início.
fn thumbnail(props: &windows::Media::Control::GlobalSystemMediaTransportControlsSessionMediaProperties) -> Option<(u64, IRandomAccessStream)> {
    let read = || -> Result<(u64, IRandomAccessStream)> {
        let stream = props.Thumbnail()?.OpenReadAsync()?.get()?;
        let size = u32::try_from(stream.Size()?).unwrap_or(u32::MAX);
        let buffer = stream.ReadAsync(&Buffer::Create(size)?, size, InputStreamOptions::None)?.get()?;
        let mut bytes = vec![0; buffer.Length()? as usize];
        DataReader::FromBuffer(&buffer)?.ReadBytes(&mut bytes)?;
        stream.Seek(0)?;
        Ok((fingerprint(&bytes), stream.cast()?))
    };
    read().map_err(|e| tracing::debug!("miniatura ilegível: {e}")).ok()
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

/// Ícones dos apps por AUMID. Sem ícone (app sem janela ainda), tenta de novo a cada 8 leituras,
/// como a ilha C++: enumerar janelas não é de graça.
#[derive(Default)]
struct Icons { cache: HashMap<String, (Option<Image>, u32)> }

const ICON_PX: u32 = 32;

impl Icons {
    fn fill(&mut self, sessions: &mut [MediaSession]) {
        for s in sessions.iter_mut() {
            let entry = self.cache.entry(s.id.clone()).or_insert((None, 0));
            if entry.0.is_none() {
                if entry.1.is_multiple_of(8) {
                    entry.0 = crate::app_icon::source_icon(&s.id, &s.app_name, ICON_PX);
                }
                entry.1 += 1;
            }
            s.app_icon = entry.0.clone();
        }
        self.cache.retain(|id, _| sessions.iter().any(|s| &s.id == id));
    }
}
