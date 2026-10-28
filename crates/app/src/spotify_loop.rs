//! Chip Spotify no laço de eventos (ADR-0024): o serviço segue o chip na barra, as entradas do
//! painel viram `Effect`s e os `Effect`s viram pedidos ao serviço, à área de transferência ou ao
//! `config.toml`. Nada de rede aqui: tudo vai para a thread `zefiro-spotify`.

use std::collections::HashSet;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant};
use windows::core::Result;
use zefiro_bar::controller::PanelKind;
use zefiro_bar::spotify::{Auth, Effect, Filter, Key, SpotifyPanel, closed_panel_effects};
use zefiro_bar::spotify_view::{DASHBOARD_URL, Hit, hit_at, spotify_panel};
use zefiro_core::config::{SpotifyConfig, set_spotify};
use zefiro_integration_spotify::auth::REDIRECT_URI;
use zefiro_platform::geometry::{Point, Rect};
use zefiro_platform_windows::spotify::{AuthState, Job, Reply, SpotifyService};
use zefiro_platform_windows::{system, system_events, window};
use zefiro_ui::paint::DisplayList;

use crate::event_loop::{Loop, TIMER_SPOTIFY_SEARCH, TIMER_SPOTIFY_TICK, USER_SPOTIFY};

/// Atraso da busca depois da última tecla.
const SEARCH_DELAY_MS: u32 = 300;
/// Aviso do rodapé ("Copiado"…) fica na tela por isto.
const TOAST: Duration = Duration::from_secs(2);
/// Lado das capas das linhas (DIP).
const COVER_DIP: f32 = 36.0;

pub struct SpotifyRuntime {
    service: Option<SpotifyService>,
    client_id: String,
    pub(crate) panel: SpotifyPanel,
    hits: Vec<(Rect, Hit)>,
    /// Capas já pedidas ao serviço (uma vez por URL).
    requested: HashSet<String>,
    pending_search: Option<(u64, String, Filter, u32)>,
    toast_at: Option<Instant>,
    started: Instant,
    /// O painel do Spotify está aberto (os ganchos de fechar rodam para qualquer painel).
    open: bool,
}

impl Default for SpotifyRuntime {
    fn default() -> Self {
        let mut panel = SpotifyPanel::new(Auth::NeedsLogin);
        panel.opened();
        SpotifyRuntime { service: None, client_id: String::new(), panel, hits: Vec::new(), requested: HashSet::new(), pending_search: None, toast_at: None, started: Instant::now(), open: false }
    }
}

fn data_dir() -> PathBuf {
    std::env::var_os("LOCALAPPDATA").map(PathBuf::from).unwrap_or_default().join("zefiro").join("spotify")
}

fn auth_of(state: AuthState) -> Auth {
    match state {
        AuthState::NeedsClientId => Auth::NeedsClientId,
        AuthState::NeedsLogin => Auth::NeedsLogin,
        AuthState::Waiting => Auth::Waiting,
        AuthState::PortBusy => Auth::PortBusy,
        AuthState::Ready => Auth::Ready,
    }
}

impl SpotifyRuntime {
    fn now_ms(&self) -> u64 {
        self.started.elapsed().as_millis() as u64
    }

    fn send(&self, job: Job) {
        if let Some(s) = &self.service {
            s.send(job);
        }
    }
}

impl Loop {
    fn spotify_open(&self) -> bool {
        self.controller.open().is_some_and(|o| o.kind == PanelKind::Spotify)
    }

    /// O serviço só existe com o chip em algum layout; o `client_id` novo vai para ele.
    pub fn spotify_services(&mut self) {
        let wanted = self.ctx.config.bar.uses("spotify");
        let client_id = self.ctx.config.spotify.client_id.trim().to_string();
        let s = &mut self.spotify;
        match (wanted, s.service.is_some()) {
            (true, false) => {
                tracing::info!("spotify: serviço ligado");
                s.client_id = client_id.clone();
                s.panel.auth = if client_id.is_empty() { Auth::NeedsClientId } else { Auth::NeedsLogin };
                s.service = SpotifyService::start(data_dir(), client_id, Arc::new(|| window::post_user(USER_SPOTIFY))).map_err(|e| tracing::warn!("spotify indisponível: {e}")).ok();
            }
            (false, true) => {
                tracing::info!("spotify: serviço desligado (chip fora da barra)");
                s.service = None;
            }
            (true, true) if client_id != s.client_id => {
                s.client_id = client_id.clone();
                s.send(Job::SetClientId(client_id));
            }
            _ => {}
        }
    }

    pub fn spotify_prepare(&mut self, kind: PanelKind) -> Result<()> {
        if kind != PanelKind::Spotify {
            return Ok(());
        }
        if let Some(s) = &self.spotify.service {
            s.set_panel_open(true);
        }
        self.spotify.open = true;
        self.spotify.panel.now_ms = self.spotify.now_ms();
        self.spotify.requested.clear();
        let fx = self.spotify.panel.opened();
        window::set_timer(TIMER_SPOTIFY_TICK, 1000);
        self.spotify_run(fx)
    }

    pub fn spotify_panel_closed(&mut self) {
        let s = &mut self.spotify;
        if !std::mem::take(&mut s.open) {
            return;
        }
        if let Some(service) = &s.service {
            service.set_panel_open(false);
        }
        s.panel.menu = None;
        s.pending_search = None;
        s.hits.clear();
        window::kill_timer(TIMER_SPOTIFY_TICK);
        window::kill_timer(TIMER_SPOTIFY_SEARCH);
    }

    pub fn spotify_panel_list(&mut self, scale: f32) -> DisplayList {
        let s = &mut self.spotify;
        s.panel.now_ms = s.started.elapsed().as_millis() as u64;
        let drawn = {
            let service = s.service.as_ref();
            let covers = |url: &str| service.and_then(|sv| sv.cover(url));
            spotify_panel(&s.panel, &covers, &self.ctx.text, scale, &self.ctx.theme.tokens)
        };
        s.hits = drawn.hits;
        let side = (COVER_DIP * scale).round() as u32;
        for url in drawn.missing {
            if s.requested.insert(url.clone()) {
                s.send(Job::Cover { url, side });
            }
        }
        drawn.list
    }

    /// Executa o que o painel pediu e redesenha.
    fn spotify_run(&mut self, fx: Vec<Effect>) -> Result<()> {
        for effect in fx {
            match effect {
                Effect::Refresh => {
                    self.spotify.send(Job::Player);
                    self.spotify.send(Job::Queue);
                    self.spotify.send(Job::Recent);
                }
                Effect::Load { depth, source, offset, after } => self.spotify.send(Job::Load { depth, source, offset, after }),
                Effect::Search { seq, query, filter, offset } => {
                    if offset == 0 {
                        self.spotify.pending_search = Some((seq, query, filter, offset));
                        window::set_timer(TIMER_SPOTIFY_SEARCH, SEARCH_DELAY_MS);
                    } else {
                        self.spotify.send(Job::Search { seq, query, kinds: filter.kinds(), limit: filter.limit(), offset });
                    }
                }
                Effect::Send(req) => self.spotify.send(Job::Send(req)),
                Effect::ToggleLike(uri) => self.spotify.send(Job::ToggleLike(uri)),
                Effect::CheckLiked(uri) => self.spotify.send(Job::CheckLiked(uri)),
                Effect::LoadPlaylists => self.spotify.send(Job::Playlists),
                Effect::Copy(text) => self.spotify_copy(&text),
                Effect::CopyRedirect => self.spotify_copy(REDIRECT_URI),
                Effect::OpenDashboard => system::open_uri(DASHBOARD_URL),
                Effect::Close => return self.close_open_panel(),
                Effect::Login => self.spotify.send(Job::Login),
                Effect::Logout => self.spotify.send(Job::Logout),
                Effect::SaveClientId(client_id) => {
                    // O recarregamento do config manda o `client_id` ao serviço (`spotify_services`).
                    match set_spotify(&self.ctx.config_path, &SpotifyConfig { client_id }) {
                        Ok(()) => self.spotify.panel.auth = Auth::NeedsLogin,
                        Err(e) => tracing::warn!("spotify: não consegui gravar o client_id: {e}"),
                    }
                }
            }
        }
        if self.spotify.panel.toast.is_some() && self.spotify.toast_at.is_none() {
            self.spotify.toast_at = Some(Instant::now());
        }
        if self.spotify_open() {
            self.redraw_open_panel()?;
        }
        Ok(())
    }

    fn spotify_copy(&mut self, text: &str) {
        let Some(open) = self.controller.open() else { return };
        if !system_events::set_clipboard_text(self.panels[open.monitor].hwnd(), text) {
            tracing::warn!("spotify: não consegui copiar");
        }
        self.spotify.panel.toast = Some("Copiado".into());
    }

    /// Teclas com o painel aberto. `true` se tratou (Esc também: o painel decide se volta ou fecha).
    pub fn spotify_key(&mut self, vk: u32) -> Result<bool> {
        if !self.spotify_open() {
            return Ok(false);
        }
        let (ctrl, shift) = (system_events::ctrl_down(), system_events::shift_down());
        let key = match vk {
            0x26 => Key::Up,
            0x28 => Key::Down,
            0x0D if ctrl => Key::CtrlEnter,
            0x0D if shift => Key::ShiftEnter,
            0x0D => Key::Enter,
            0x09 if ctrl && shift => Key::CtrlShiftTab,
            0x09 if ctrl => Key::CtrlTab,
            0x09 => Key::Tab,
            0x1B => Key::Esc,
            0x4C if ctrl => Key::CtrlL,
            0x50 if ctrl => Key::CtrlP,
            0x43 if ctrl => Key::CtrlC,
            _ => return Ok(false),
        };
        self.spotify.panel.now_ms = self.spotify.now_ms();
        let fx = self.spotify.panel.key(key);
        self.spotify_run(fx)?;
        Ok(true)
    }

    pub fn spotify_char(&mut self, code: u32) -> Result<bool> {
        if !self.spotify_open() {
            return Ok(false);
        }
        let paste = (code == 22).then(system_events::clipboard_text).flatten();
        self.spotify.panel.now_ms = self.spotify.now_ms();
        let fx = self.spotify.panel.char(code, paste.as_deref());
        self.spotify_run(fx)?;
        Ok(true)
    }

    /// Clique no painel (`right` = botão direito). `true` se o painel é o do Spotify.
    pub fn spotify_click(&mut self, p: Point, right: bool) -> Result<bool> {
        if !self.spotify_open() {
            return Ok(false);
        }
        let hit = hit_at(&self.spotify.hits, p);
        self.spotify.panel.now_ms = self.spotify.now_ms();
        let fx = self.spotify.panel.click(hit, right);
        self.spotify_run(fx)?;
        Ok(true)
    }

    pub fn spotify_move(&mut self, p: Point) -> Result<()> {
        if self.spotify_open() && self.spotify.panel.hover(hit_at(&self.spotify.hits, p)) {
            self.redraw_open_panel()?;
        }
        Ok(())
    }

    pub fn spotify_wheel(&mut self, delta: i32) -> Result<bool> {
        if !self.spotify_open() {
            return Ok(false);
        }
        let fx = self.spotify.panel.wheel(delta);
        self.spotify_run(fx)?;
        Ok(true)
    }

    /// A busca dispara 300 ms depois da última tecla.
    pub fn spotify_search_timer(&mut self) {
        window::kill_timer(TIMER_SPOTIFY_SEARCH);
        if let Some((seq, query, filter, offset)) = self.spotify.pending_search.take() {
            self.spotify.send(Job::Search { seq, query, kinds: filter.kinds(), limit: filter.limit(), offset });
        }
    }

    /// Uma vez por segundo com o painel aberto: a barra de progresso anda e o aviso do rodapé some.
    /// Não é animação: é conteúdo novo uma vez por segundo, como a linha do tempo da ilha.
    pub fn spotify_tick(&mut self) -> Result<()> {
        if !self.spotify_open() {
            window::kill_timer(TIMER_SPOTIFY_TICK);
            return Ok(());
        }
        let s = &mut self.spotify;
        let mut redraw = false;
        if s.toast_at.is_some_and(|at| at.elapsed() >= TOAST) {
            s.toast_at = None;
            s.panel.toast = None;
            redraw = true;
        }
        let at_root = s.panel.stack.len() == 1;
        if at_root && s.panel.player.as_ref().is_some_and(|p| p.playing) {
            redraw = true;
        }
        if redraw {
            self.redraw_open_panel()?;
        }
        Ok(())
    }

    /// Respostas do serviço.
    pub fn spotify_replies(&mut self) -> Result<()> {
        let replies = self.spotify.service.as_ref().map(SpotifyService::replies).unwrap_or_default();
        let mut fx = Vec::new();
        let mut acted = false;
        for reply in replies {
            let now = self.spotify.now_ms();
            let panel = &mut self.spotify.panel;
            match reply {
                Reply::Auth(state) => fx.extend(panel.set_auth(auth_of(state))),
                Reply::Player(Ok(player)) => fx.extend(panel.set_player(player.map(|p| *p), now)),
                Reply::Player(Err(_)) => {}
                Reply::Recent(items) => panel.recent = items,
                Reply::UpNext(next) => panel.up_next = next,
                Reply::Loaded { depth, source, offset, result } => panel.loaded(depth, &source, offset, result),
                Reply::Searched { seq, offset, result } => panel.searched(seq, offset, result),
                Reply::Sent(result) => {
                    acted |= result.is_ok();
                    fx.extend(panel.sent(result));
                }
                Reply::Liked { uri, liked } => panel.set_liked(uri, liked),
                Reply::Playlists(list) => panel.set_playlists(list),
                Reply::Cover => {}
                Reply::Idle => {
                    panel.idle();
                    self.spotify.requested.clear();
                }
            }
        }
        if !self.spotify_open() {
            // Fechado: só guarda o estado; nada vai à rede (o `Refresh` de um login roda ao abrir).
            fx = closed_panel_effects(fx);
        } else if acted {
            // Depois de uma ação, o cartão e a fila mostram o resultado.
            self.spotify.send(Job::Player);
            self.spotify.send(Job::Queue);
        }
        self.spotify_run(fx)
    }
}
