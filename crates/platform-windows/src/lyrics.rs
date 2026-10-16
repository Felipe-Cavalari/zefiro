//! Letras em segundo plano: cache em disco (`%LOCALAPPDATA%\zefiro\lyrics`, até 25 MB, os mais
//! antigos saem primeiro), senão LRCLIB pela ordem da ilha C++. Só guarda faixas que têm letra.

use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, Sender};
use std::sync::{Arc, Mutex};
use windows::Win32::System::WinRT::{RO_INIT_MULTITHREADED, RoInitialize, RoUninitialize};
use zefiro_island::lyrics::{Lyrics, Pick, cache_key, consider, from_cache, requests, to_cache};

const CACHE_LIMIT: u64 = 25 * 1024 * 1024;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LyricsRequest { pub artist: String, pub title: String, pub album: String, pub duration_s: u32 }

/// Resultado para a faixa pedida por último: `None` = ainda buscando; `Some(None)` = sem letra.
pub type LyricsState = Option<(LyricsRequest, Option<Lyrics>)>;

pub struct LyricsService {
    tx: Option<Sender<LyricsRequest>>,
    state: Arc<Mutex<LyricsState>>,
    thread: Option<std::thread::JoinHandle<()>>,
}

impl LyricsService {
    pub fn start(cache_dir: PathBuf, notify: Arc<dyn Fn() + Send + Sync>) -> std::io::Result<LyricsService> {
        let (tx, rx) = mpsc::channel::<LyricsRequest>();
        let state: Arc<Mutex<LyricsState>> = Arc::new(Mutex::new(None));
        let shared = state.clone();
        let thread = std::thread::Builder::new().name("zefiro-lyrics".into()).spawn(move || {
            // SAFETY: WinRT nesta thread (HttpClient), desfeito no fim.
            let inited = unsafe { RoInitialize(RO_INIT_MULTITHREADED) }.is_ok();
            while let Ok(mut req) = rx.recv() {
                // Só a última faixa pedida interessa (pular várias faixas seguidas).
                while let Ok(newer) = rx.try_recv() {
                    req = newer;
                }
                let lyrics = lookup(&cache_dir, &req);
                if let Ok(mut st) = shared.lock() {
                    *st = Some((req, lyrics));
                }
                notify();
            }
            if inited {
                // SAFETY: par do RoInitialize.
                unsafe { RoUninitialize() };
            }
        })?;
        Ok(LyricsService { tx: Some(tx), state, thread: Some(thread) })
    }

    pub fn request(&self, r: LyricsRequest) {
        if let Some(tx) = &self.tx {
            let _ = tx.send(r);
        }
    }

    pub fn state(&self) -> LyricsState {
        self.state.lock().ok().and_then(|s| s.clone())
    }
}

impl Drop for LyricsService {
    fn drop(&mut self) {
        self.tx.take();
        if let Some(t) = self.thread.take() {
            let _ = t.join();
        }
    }
}

fn lookup(dir: &Path, r: &LyricsRequest) -> Option<Lyrics> {
    let path = dir.join(cache_key(&r.artist, &r.title, r.duration_s));
    if let Some(l) = std::fs::read_to_string(&path).ok().and_then(|t| from_cache(&t)) {
        return Some(l);
    }
    let mut best = Pick::default();
    for (url, trusted) in requests(&r.title, &r.artist, &r.album, r.duration_s) {
        match crate::http::get(&url) {
            Ok((200, body)) => consider(&body, &r.title, &r.artist, r.duration_s, trusted, &mut best),
            Ok((404, _)) => {}
            Ok((status, _)) => {
                tracing::debug!(status, "LRCLIB respondeu com erro; sem letra por enquanto");
                break;
            }
            Err(e) => {
                tracing::debug!("LRCLIB indisponível: {e}");
                break;
            }
        }
        if best.synced() {
            break;
        }
    }
    let lyrics = best.lyrics?;
    if std::fs::create_dir_all(dir).is_ok() && std::fs::write(&path, to_cache(&lyrics)).is_ok() {
        trim_cache(dir);
    }
    Some(lyrics)
}

/// Mantém o cache em até 25 MB, apagando os arquivos mais antigos (só os `.lrc` do Zefiro).
fn trim_cache(dir: &Path) {
    let Ok(entries) = std::fs::read_dir(dir) else { return };
    let mut files: Vec<(std::time::SystemTime, u64, PathBuf)> = entries
        .flatten()
        .filter(|e| e.path().extension().is_some_and(|x| x == "lrc"))
        .filter_map(|e| {
            let m = e.metadata().ok()?;
            Some((m.modified().ok()?, m.len(), e.path()))
        })
        .collect();
    let mut total: u64 = files.iter().map(|f| f.1).sum();
    files.sort_by_key(|f| f.0);
    for (_, len, path) in files {
        if total <= CACHE_LIMIT {
            break;
        }
        if std::fs::remove_file(&path).is_ok() {
            total -= len;
        }
    }
}
