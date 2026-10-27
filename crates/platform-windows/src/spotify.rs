//! Serviço do Spotify numa thread própria (`zefiro-spotify`, ADR-0024): o login, a sessão (o token
//! de acesso só existe aqui), os pedidos do painel e as capas. Com o painel fechado, a thread fica
//! parada no canal e não consulta nada; aberto, relê o player a cada 5 s.

use std::path::PathBuf;
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use windows::Win32::System::WinRT::{RO_INIT_MULTITHREADED, RoInitialize, RoUninitialize};
use zefiro_integration_spotify::api::{self, Kind, Request, Source};
use zefiro_integration_spotify::auth::{Pkce, Stored, authorize_url, pkce};
use zefiro_integration_spotify::cache::{COVERS, CoverCache};
use zefiro_integration_spotify::model::{self, ApiError, Item, Page, Player};
use zefiro_integration_spotify::session::{Session, exchange_code};
use zefiro_platform::image::Image;

use crate::spotify_auth::{HttpTransport, ListenResult, listen};

const POLL: Duration = Duration::from_secs(5);
const IDLE: Duration = Duration::from_secs(600);
const LOGIN_WAIT: Duration = Duration::from_secs(300);
const COVER_TIMEOUT: Duration = Duration::from_secs(10);
/// Esperas de um 429 maiores que isto ficam neste teto.
const MAX_BACKOFF_S: u64 = 60;
/// Páginas de `/me/playlists` lidas para "Adicionar a playlist…" (50 cada).
const PLAYLIST_PAGES: u32 = 10;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Job {
    Login,
    Logout,
    SetClientId(String),
    Player,
    Recent,
    Queue,
    Load { depth: usize, source: Source, offset: u32, after: Option<String> },
    Search { seq: u64, query: String, kinds: Vec<Kind>, limit: u32, offset: u32 },
    Send(Request),
    ToggleLike(String),
    CheckLiked(String),
    Playlists,
    Cover { url: String, side: u32 },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AuthState { NeedsClientId, NeedsLogin, Waiting, PortBusy, Ready }

#[derive(Debug, Clone, PartialEq)]
pub enum Reply {
    Auth(AuthState),
    Player(Result<Option<Box<Player>>, ApiError>),
    Recent(Vec<Item>),
    UpNext(Option<String>),
    Loaded { depth: usize, offset: u32, result: Result<Option<Page>, ApiError> },
    Searched { seq: u64, offset: u32, result: Result<Vec<(Kind, Page)>, ApiError> },
    Sent(Result<(), ApiError>),
    Liked { uri: String, liked: bool },
    Playlists(Vec<Item>),
    Cover,
    /// 10 min de painel fechado: as listas em memória podem ir embora.
    Idle,
}

enum Wake { Job(Job), Panel(bool), Callback(ListenResult), Stop }

type Covers = Arc<Mutex<CoverCache<Image>>>;

pub struct SpotifyService {
    tx: Sender<Wake>,
    replies: Arc<Mutex<Vec<Reply>>>,
    covers: Covers,
    thread: Option<std::thread::JoinHandle<()>>,
}

/// 0: o que o usuário mandou fazer; 1: leituras; 2: capas.
fn priority(job: &Job) -> u8 {
    match job {
        Job::Login | Job::Logout | Job::SetClientId(_) | Job::Send(_) | Job::ToggleLike(_) => 0,
        Job::Cover { .. } => 2,
        _ => 1,
    }
}

/// A ordem de um lote de pedidos: comandos primeiro, capas por último, só a busca mais nova e um
/// pedido de cada leitura repetida.
fn order(jobs: Vec<Job>, newest_seq: u64) -> Vec<Job> {
    let mut out: Vec<Job> = Vec::with_capacity(jobs.len());
    for job in jobs {
        if matches!(&job, Job::Search { seq, .. } if *seq < newest_seq) {
            continue;
        }
        let repeated = matches!(job, Job::Player | Job::Queue | Job::Recent | Job::Playlists | Job::Cover { .. }) && out.contains(&job);
        if !repeated {
            out.push(job);
        }
    }
    out.sort_by_key(priority);
    out
}

fn now_s() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |d| d.as_secs())
}

/// O que o log diz de um pedido: método e caminho, sem a query (que tem a busca e os uris).
fn describe(req: &Request) -> String {
    format!("{} {}", req.method.as_str(), req.path.split('?').next().unwrap_or_default())
}

struct Worker {
    dir: PathBuf,
    client_id: String,
    session: Option<Session>,
    me: Option<String>,
    pkce: Option<Pkce>,
    blocked_until: Option<Instant>,
    replies: Arc<Mutex<Vec<Reply>>>,
    covers: Covers,
    notify: Arc<dyn Fn() + Send + Sync>,
    tx: Sender<Wake>,
}

impl Worker {
    fn token_path(&self) -> PathBuf {
        self.dir.join("token.bin")
    }

    fn reply(&self, r: Reply) {
        if let Ok(mut q) = self.replies.lock() {
            q.push(r);
        }
        (self.notify)();
    }

    fn load_token(&mut self) {
        let stored = std::fs::read(self.token_path()).ok().and_then(|b| crate::secret::unprotect(&b).ok()).and_then(|b| Stored::from_bytes(&b));
        self.session = match stored {
            Some(s) if s.client_id == self.client_id && !self.client_id.is_empty() => Some(Session::new(s.client_id, s.refresh)),
            _ => None,
        };
    }

    fn save_token(&self, stored: &Stored) {
        let sealed = match crate::secret::protect(&stored.to_bytes()) {
            Ok(b) => b,
            Err(e) => {
                tracing::warn!("spotify: não consegui cifrar o token: {}", e.message());
                return;
            }
        };
        let _ = std::fs::create_dir_all(&self.dir);
        let tmp = self.dir.join("token.bin.tmp");
        if let Err(e) = std::fs::write(&tmp, sealed).and_then(|()| std::fs::rename(&tmp, self.token_path())) {
            tracing::warn!("spotify: não consegui gravar o token: {e}");
        }
    }

    fn forget_token(&mut self) {
        let _ = std::fs::remove_file(self.token_path());
        self.session = None;
        self.me = None;
    }

    fn auth_state(&self) -> AuthState {
        match (&self.session, self.client_id.is_empty()) {
            (_, true) => AuthState::NeedsClientId,
            (Some(_), false) => AuthState::Ready,
            (None, false) => AuthState::NeedsLogin,
        }
    }

    /// Um pedido à API, esperando um 429 anterior e gravando o token de renovação novo.
    fn call(&mut self, req: &Request) -> Result<(u16, String), ApiError> {
        if let Some(until) = self.blocked_until.take() {
            let now = Instant::now();
            if until > now {
                std::thread::sleep(until - now);
            }
        }
        let Some(session) = &mut self.session else { return Err(ApiError::Unauthorized) };
        let result = session.execute(&HttpTransport, req, now_s());
        if let Some(refresh) = session.take_new_refresh() {
            let stored = Stored { client_id: self.client_id.clone(), refresh };
            self.save_token(&stored);
        }
        match result {
            Err(ApiError::Unauthorized) => {
                tracing::warn!("spotify: renovação recusada, saindo");
                self.forget_token();
                self.reply(Reply::Auth(AuthState::NeedsLogin));
            }
            Err(ApiError::RateLimited(n)) => {
                let wait = n.min(MAX_BACKOFF_S);
                tracing::info!("spotify: 429, esperando {wait} s");
                self.blocked_until = Some(Instant::now() + Duration::from_secs(wait));
            }
            _ => {}
        }
        result
    }

    /// O id do usuário (para "sua" e para saber que playlists dá para editar), lido uma vez.
    fn me(&mut self) -> String {
        if self.me.is_none()
            && let Ok((_, body)) = self.call(&api::me())
        {
            self.me = model::parse_me(&body);
        }
        self.me.clone().unwrap_or_default()
    }

    fn login(&mut self) {
        if self.client_id.is_empty() {
            self.reply(Reply::Auth(AuthState::NeedsClientId));
            return;
        }
        let p = pkce(&crate::secret::random::<96>());
        let tx = self.tx.clone();
        let notify = self.notify.clone();
        let listening = listen(p.state.clone(), LOGIN_WAIT, move |result| {
            let _ = tx.send(Wake::Callback(result));
            notify();
        });
        match listening {
            Ok(()) => {
                crate::system::open_uri(&authorize_url(&self.client_id, &p));
                self.pkce = Some(p);
                tracing::info!("spotify: login aberto no navegador");
                self.reply(Reply::Auth(AuthState::Waiting));
            }
            Err(e) => {
                tracing::warn!("spotify: não consegui esperar o login na porta 47823: {e}");
                self.reply(Reply::Auth(AuthState::PortBusy));
            }
        }
    }

    fn callback(&mut self, result: ListenResult) {
        let verifier = self.pkce.take().map(|p| p.verifier);
        let state = match (result, verifier) {
            (ListenResult::Code(code), Some(verifier)) => match exchange_code(&HttpTransport, &self.client_id, &code, &verifier) {
                Ok(tokens) => match tokens.refresh {
                    Some(refresh) => {
                        let stored = Stored { client_id: self.client_id.clone(), refresh: refresh.clone() };
                        self.save_token(&stored);
                        self.session = Some(Session::new(self.client_id.clone(), refresh));
                        self.me = None;
                        tracing::info!("spotify: login feito");
                        AuthState::Ready
                    }
                    None => AuthState::NeedsLogin,
                },
                Err(e) => {
                    tracing::warn!("spotify: troca do código falhou: {e:?}");
                    AuthState::NeedsLogin
                }
            },
            (other, _) => {
                tracing::info!("spotify: login não concluído ({other:?})");
                AuthState::NeedsLogin
            }
        };
        self.reply(Reply::Auth(state));
    }

    fn cover(&self, url: &str, side: u32) {
        if self.covers.lock().is_ok_and(|mut c| c.get(url).is_some()) {
            return;
        }
        let image = match crate::http::get_bytes_with(url, COVER_TIMEOUT) {
            Ok((200, bytes)) => crate::imaging::decode_bytes(&bytes, side),
            _ => None,
        };
        if let Some(image) = image {
            if let Ok(mut c) = self.covers.lock() {
                c.insert(url.to_string(), image);
            }
            self.reply(Reply::Cover);
        }
    }

    /// Nome e capa dos contextos de "Tocadas há pouco".
    fn recent(&mut self) -> Vec<Item> {
        let Ok((_, body)) = self.call(&api::recently_played()) else { return Vec::new() };
        let me = self.me();
        let mut out = Vec::new();
        for uri in model::recent_contexts(&body) {
            let mut parts = uri.split(':').skip(1);
            let (Some(kind), Some(id)) = (parts.next(), parts.next()) else { continue };
            let (req, kind) = match kind {
                "playlist" => (api::playlist(id), Kind::Playlist),
                "album" => (api::album(id), Kind::Album),
                "artist" => (api::artist(id), Kind::Artist),
                _ => continue,
            };
            if let Ok((_, body)) = self.call(&req)
                && let Some(item) = model::parse_one(kind, &body, &me)
            {
                out.push(item);
            }
        }
        out
    }

    fn liked(&mut self, uri: &str) -> Result<bool, ApiError> {
        let (_, body) = self.call(&api::library_contains(&[uri.to_string()]))?;
        Ok(serde_contains(&body))
    }

    fn run(&mut self, job: Job) {
        match job {
            Job::Login => self.login(),
            Job::Logout => {
                self.forget_token();
                tracing::info!("spotify: saiu da conta");
                self.reply(Reply::Auth(self.auth_state()));
            }
            Job::SetClientId(id) => {
                if id != self.client_id {
                    self.client_id = id;
                    self.forget_token();
                    self.reply(Reply::Auth(self.auth_state()));
                }
            }
            Job::Player => {
                let result = self.call(&api::player()).map(|(status, body)| model::parse_player(status, &body).map(Box::new));
                self.reply(Reply::Player(result));
            }
            Job::Queue => {
                let next = self.call(&api::list(&Source::Queue, 0, None)).ok().and_then(|(_, body)| model::parse_queue(&body).1.into_iter().next()).map(|i| i.name);
                self.reply(Reply::UpNext(next));
            }
            Job::Recent => {
                let items = self.recent();
                self.reply(Reply::Recent(items));
            }
            Job::Load { depth, source, offset, after } => {
                let me = self.me();
                let started = Instant::now();
                let result = match self.call(&api::list(&source, offset, after.as_deref())) {
                    Ok((_, body)) => match model::parse_page(&source, &body, &me) {
                        Some(page) => Ok(Some(page)),
                        None if matches!(source, Source::Playlist(_)) => Ok(None),
                        None => Err(ApiError::Http(200)),
                    },
                    // Playlist de outro dono: a API pode recusar as músicas em vez de omitir.
                    Err(ApiError::Http(403 | 404)) if matches!(source, Source::Playlist(_)) => Ok(None),
                    Err(e) => Err(e),
                };
                tracing::debug!("spotify: lista em {} ms", started.elapsed().as_millis());
                self.reply(Reply::Loaded { depth, offset, result });
            }
            Job::Search { seq, query, kinds, limit, offset } => {
                let me = self.me();
                let started = Instant::now();
                let result = self.call(&api::search(&query, &kinds, limit, offset)).map(|(_, body)| model::parse_search(&body, &me));
                if let Ok(groups) = &result {
                    let count: usize = groups.iter().map(|(_, p)| p.items.len()).sum();
                    tracing::info!("spotify: busca, {count} resultados em {} ms", started.elapsed().as_millis());
                }
                self.reply(Reply::Searched { seq, offset, result });
            }
            Job::Send(req) => {
                tracing::info!("spotify: {}", describe(&req));
                let result = self.call(&req).map(|_| ());
                self.reply(Reply::Sent(result));
            }
            Job::ToggleLike(uri) => {
                let result = self.liked(&uri).and_then(|liked| {
                    let uris = [uri.clone()];
                    self.call(&if liked { api::library_remove(&uris) } else { api::library_save(&uris) }).map(|_| !liked)
                });
                match result {
                    Ok(liked) => self.reply(Reply::Liked { uri, liked }),
                    Err(e) => self.reply(Reply::Sent(Err(e))),
                }
            }
            Job::CheckLiked(uri) => {
                if let Ok(liked) = self.liked(&uri) {
                    self.reply(Reply::Liked { uri, liked });
                }
            }
            Job::Playlists => {
                let me = self.me();
                let mut list = Vec::new();
                let mut offset = 0;
                for _ in 0..PLAYLIST_PAGES {
                    let Ok((_, body)) = self.call(&api::list(&Source::Playlists, offset, None)) else { break };
                    let Some(page) = model::parse_page(&Source::Playlists, &body, &me) else { break };
                    list.extend(page.items.into_iter().filter(|p| p.editable));
                    match page.next_offset {
                        Some(next) => offset = next,
                        None => break,
                    }
                }
                self.reply(Reply::Playlists(list));
            }
            Job::Cover { url, side } => self.cover(&url, side),
        }
    }
}

/// `[true]` → `true` (resposta do `/me/library/contains`).
fn serde_contains(body: &str) -> bool {
    body.trim().trim_start_matches('[').trim_start().starts_with("true")
}

fn worker_loop(mut w: Worker, rx: Receiver<Wake>) {
    w.load_token();
    w.reply(Reply::Auth(w.auth_state()));
    let mut open = false;
    let mut closed_at: Option<Instant> = None;
    let mut newest_seq = 0;
    loop {
        let first = if open {
            rx.recv_timeout(POLL)
        } else if let Some(at) = closed_at {
            rx.recv_timeout(IDLE.saturating_sub(at.elapsed()))
        } else {
            rx.recv().map_err(|_| RecvTimeoutError::Disconnected)
        };
        let mut jobs = Vec::new();
        let mut batch = match first {
            Ok(wake) => vec![wake],
            Err(RecvTimeoutError::Timeout) => {
                if open {
                    jobs.push(Job::Player);
                } else if closed_at.is_some() {
                    closed_at = None;
                    if let Ok(mut c) = w.covers.lock() {
                        c.clear();
                    }
                    w.reply(Reply::Idle);
                }
                Vec::new()
            }
            Err(RecvTimeoutError::Disconnected) => return,
        };
        batch.extend(rx.try_iter());
        for wake in batch {
            match wake {
                Wake::Stop => return,
                Wake::Panel(now_open) => {
                    open = now_open;
                    closed_at = (!now_open).then(Instant::now);
                }
                Wake::Callback(result) => w.callback(result),
                Wake::Job(job) => {
                    if let Job::Search { seq, .. } = &job {
                        newest_seq = newest_seq.max(*seq);
                    }
                    jobs.push(job);
                }
            }
        }
        for job in order(jobs, newest_seq) {
            w.run(job);
        }
    }
}

impl SpotifyService {
    /// `dir` é `%LOCALAPPDATA%\zefiro\spotify` (onde fica o `token.bin`).
    pub fn start(dir: PathBuf, client_id: String, notify: Arc<dyn Fn() + Send + Sync>) -> std::io::Result<SpotifyService> {
        let (tx, rx) = mpsc::channel();
        let replies = Arc::new(Mutex::new(Vec::new()));
        let covers: Covers = Arc::new(Mutex::new(CoverCache::new(COVERS)));
        let worker = Worker { dir, client_id, session: None, me: None, pkce: None, blocked_until: None, replies: replies.clone(), covers: covers.clone(), notify, tx: tx.clone() };
        let thread = std::thread::Builder::new().name("zefiro-spotify".into()).spawn(move || {
            // SAFETY: WinRT nesta thread (HttpClient, imagens), desfeito no fim.
            let inited = unsafe { RoInitialize(RO_INIT_MULTITHREADED) }.is_ok();
            worker_loop(worker, rx);
            if inited {
                // SAFETY: par do RoInitialize.
                unsafe { RoUninitialize() };
            }
        })?;
        Ok(SpotifyService { tx, replies, covers, thread: Some(thread) })
    }

    pub fn send(&self, job: Job) {
        let _ = self.tx.send(Wake::Job(job));
    }

    pub fn set_panel_open(&self, open: bool) {
        let _ = self.tx.send(Wake::Panel(open));
    }

    pub fn replies(&self) -> Vec<Reply> {
        self.replies.lock().map(|mut q| std::mem::take(&mut *q)).unwrap_or_default()
    }

    pub fn cover(&self, url: &str) -> Option<Image> {
        self.covers.lock().ok().and_then(|mut c| c.get(url).cloned())
    }
}

impl Drop for SpotifyService {
    fn drop(&mut self) {
        let _ = self.tx.send(Wake::Stop);
        if let Some(t) = self.thread.take() {
            let _ = t.join();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn search(seq: u64) -> Job {
        Job::Search { seq, query: "x".into(), kinds: vec![Kind::Track], limit: 10, offset: 0 }
    }

    #[test]
    fn jobs_run_player_commands_first_and_skip_stale_searches() {
        let cover = Job::Cover { url: "u".into(), side: 72 };
        let jobs = vec![cover.clone(), Job::Player, search(1), Job::Send(api::pause()), Job::Player, cover.clone(), search(2), Job::ToggleLike("t".into())];
        assert_eq!(order(jobs, 2), vec![Job::Send(api::pause()), Job::ToggleLike("t".into()), Job::Player, search(2), cover]);
    }

    #[test]
    fn contains_reads_the_first_flag() {
        assert!(serde_contains("[true]"));
        assert!(serde_contains(" [ true ] "));
        assert!(!serde_contains("[false]"));
        assert!(!serde_contains(""));
    }

    #[test]
    fn log_line_has_no_query() {
        assert_eq!(describe(&api::search("segredo", &[Kind::Track], 10, 0)), "GET /search");
        assert_eq!(describe(&api::queue("spotify:track:T")), "POST /me/player/queue");
    }
}
