//! A ilha embutida no laço de eventos: sessões de mídia, visualizador, espiada e o painel expandido.

use std::sync::{Arc, Mutex};
use std::time::Instant;
use windows::core::Result;
use zefiro_bar::controller::PanelKind;
use zefiro_integration_clock::windows::now_local;
use zefiro_island::accent::cover_accent;
use zefiro_island::view::{ExpandedAnim, ExpandedView, IslandAction, dock_index, expanded, hit, transformed};
use zefiro_island::waveform::Waveform;
use zefiro_platform::color::Rgba;
use zefiro_platform::geometry::Point;
use zefiro_platform_windows::audio_capture::AudioCapture;
use zefiro_platform_windows::media::{MediaCommand, MediaService, now_unix_ms};
use zefiro_platform_windows::volume::VolumeWatcher;
use zefiro_platform_windows::audio_devices::{self, AudioDevice, Flow, RouteState};
use zefiro_platform_windows::popup_menu;
use zefiro_platform_windows::privacy::PrivacyWatcher;
use zefiro_platform_windows::notifications::NotificationWatcher;
use zefiro_platform_windows::system_events;
use zefiro_platform_windows::dnd;
use zefiro_platform_windows::bluetooth::BluetoothWatcher;
use zefiro_platform_windows::weather::WeatherService;
use zefiro_platform_windows::hardware::HardwareMonitor;
use zefiro_platform_windows::lyrics::{LyricsRequest, LyricsService};
use zefiro_island::tabs::{self, Tab};
use zefiro_island::view::Privacy;
use zefiro_island::menu::{self, AppOutput, MenuCommand, MenuInput};
use zefiro_island::state::{Activity, Notice};
use zefiro_island::timer::{FocusTimer, TimerKind};
use zefiro_platform_windows::window;
use zefiro_ui::paint::DisplayList;

use crate::bar_view::ISLAND_CHIP;
use crate::event_loop::{Loop, TIMER_FOCUS, TIMER_ISLAND_ANIM, TIMER_LYRICS, TIMER_ISLAND_PEEK, TIMER_ISLAND_TICK, USER_BLUETOOTH, USER_DND, USER_HARDWARE, USER_LYRICS, USER_MEDIA, USER_NOTIFICATION, USER_PRIVACY, USER_VISUALIZER, USER_VOLUME, USER_WEATHER};

/// Destaque quando a capa não tem cor viva (o mesmo padrão da ilha C++).
const DEFAULT_ACCENT: Rgba = Rgba { r: 0x4C as f32 / 255.0, g: 0xC9 as f32 / 255.0, b: 0xF0 as f32 / 255.0, a: 1.0 };
/// O visualizador atualiza no máximo a 60 Hz.
const VISUALIZER_FRAME_MS: u128 = 16;

#[derive(Default)]
pub struct IslandRuntime {
    media: Option<MediaService>,
    volume: Option<VolumeWatcher>,
    privacy: Option<PrivacyWatcher>,
    notifications: Option<NotificationWatcher>,
    bluetooth: Option<BluetoothWatcher>,
    weather: Option<WeatherService>,
    hardware: Option<HardwareMonitor>,
    tab: Tab,
    lyrics: Option<LyricsService>,
    /// Faixa cuja letra foi pedida por último.
    lyrics_for: Option<LyricsRequest>,
    swap: Option<Swap>,
    carousel: tabs::Carousel,
    /// Último quadro das animações do painel (passo da mola do carrossel).
    anim_last: Option<Instant>,
    /// Área de transferência e Caps Lock ligados na janela de mensagens.
    listening: bool,
    capture: Option<AudioCapture>,
    waveform: Arc<Mutex<Waveform>>,
    expanded: Option<ExpandedView>,
}

impl Loop {
    fn island_bar(&self) -> Option<usize> {
        self.bars.iter().position(|b| b.has_island())
    }

    /// Liga ou desliga os serviços conforme `[island] engine` (no início e quando a configuração muda).
    pub fn island_services(&mut self) -> Result<()> {
        let embedded = self.ctx.config.island.embedded();
        if embedded && self.island.media.is_none() {
            match MediaService::start(Arc::new(|| window::post_user(USER_MEDIA))) {
                Ok(service) => self.island.media = Some(service),
                Err(e) => tracing::error!("não consegui iniciar a thread de mídia: {e}"),
            }
        } else if !embedded && self.island.media.is_some() {
            self.island.media = None;
            self.island.capture = None;
            self.ctx.island.update(Vec::new(), self.ctx.now());
        }
        if embedded && self.island.volume.is_none() {
            match VolumeWatcher::start(Arc::new(|| window::post_user(USER_VOLUME))) {
                Ok(w) => self.island.volume = Some(w),
                Err(e) => tracing::error!("não consegui iniciar a thread de volume: {e}"),
            }
        } else if !embedded {
            self.island.volume = None;
        }
        if embedded && self.island.privacy.is_none() {
            match PrivacyWatcher::start(Arc::new(|| window::post_user(USER_PRIVACY))) {
                Ok(w) => self.island.privacy = Some(w),
                Err(e) => tracing::warn!("pontos de privacidade indisponíveis: {e}"),
            }
            self.island_privacy_changed()?;
        } else if !embedded {
            self.island.privacy = None;
        }
        if embedded && self.island.notifications.is_none() {
            match NotificationWatcher::start(Arc::new(|| window::post_user(USER_NOTIFICATION))) {
                Ok(w) => self.island.notifications = Some(w),
                Err(e) => tracing::warn!("notificações indisponíveis: {e}"),
            }
        } else if !embedded {
            self.island.notifications = None;
        }
        if embedded && self.island.weather.is_none() {
            match WeatherService::start(String::new(), Arc::new(|| window::post_user(USER_WEATHER))) {
                Ok(w) => self.island.weather = Some(w),
                Err(e) => tracing::warn!("clima indisponível: {e}"),
            }
        } else if !embedded {
            self.island.weather = None;
            self.ctx.island.weather = None;
        }
        if embedded && self.island.lyrics.is_none() {
            let dir = std::env::var_os("LOCALAPPDATA").map(std::path::PathBuf::from).unwrap_or_default().join("zefiro").join("lyrics");
            match LyricsService::start(dir, Arc::new(|| window::post_user(USER_LYRICS))) {
                Ok(s) => self.island.lyrics = Some(s),
                Err(e) => tracing::warn!("letras indisponíveis: {e}"),
            }
        } else if !embedded {
            self.island.lyrics = None;
            self.ctx.island.lyrics = None;
        }
        if embedded && self.island.bluetooth.is_none() {
            match BluetoothWatcher::start(Arc::new(|| window::post_user(USER_BLUETOOTH))) {
                Ok(w) => self.island.bluetooth = Some(w),
                Err(e) => tracing::warn!("Bluetooth indisponível: {e}"),
            }
        } else if !embedded {
            self.island.bluetooth = None;
        }
        if embedded != self.island.listening {
            if embedded {
                dnd::watch(Arc::new(|| window::post_user(USER_DND)));
                if let Err(e) = system_events::listen(window::message_window()) {
                    tracing::warn!("avisos de área de transferência e Caps Lock indisponíveis: {e}");
                }
            } else {
                system_events::stop(window::message_window());
            }
            self.island.listening = embedded;
        }
        Ok(())
    }

    pub fn island_bluetooth(&mut self) -> Result<()> {
        let Some(w) = &self.island.bluetooth else { return Ok(()) };
        if let Some(e) = w.take().pop() {
            let now = self.ctx.now();
            self.ctx.island.state.activity(Activity::Bluetooth { name: e.name, connected: e.connected }, now);
            self.island_refresh()?;
        }
        Ok(())
    }

    pub fn island_device(&mut self, arrived: bool, volume: bool) -> Result<()> {
        if !self.ctx.config.island.embedded() {
            return Ok(());
        }
        let name = if volume { "Pendrive" } else { "Dispositivo serial" };
        let now = self.ctx.now();
        self.ctx.island.state.activity(Activity::Device { name: name.into(), connected: arrived }, now);
        self.island_refresh()
    }

    pub fn island_notifications(&mut self) -> Result<()> {
        let Some(w) = &self.island.notifications else { return Ok(()) };
        // Várias de uma vez: a mais recente fica na pílula. Com "Não perturbe", nenhuma aparece.
        if let Some(n) = w.take().pop().filter(|_| !dnd::active()) {
            let now = self.ctx.now();
            self.ctx.island.state.activity(Activity::Notification(Notice { app: n.app, text: n.text, icon: n.icon }), now);
            self.island_refresh()?;
        }
        Ok(())
    }

    /// Um segundo do timer de foco: atualiza a pílula e avisa ao terminar.
    pub fn island_focus_tick(&mut self) -> Result<()> {
        let now = self.ctx.now();
        match self.ctx.island.state.timer {
            None => window::kill_timer(TIMER_FOCUS),
            Some(t) if t.finished(now) => {
                self.ctx.island.state.timer = None;
                window::kill_timer(TIMER_FOCUS);
                let text = if t.kind == TimerKind::Focus { "Foco concluído; hora da pausa" } else { "Pausa concluída" };
                self.ctx.island.state.activity(Activity::Notification(Notice { app: "Timer de foco".into(), text: text.into(), icon: None }), now);
            }
            Some(_) => {}
        }
        self.island_refresh()
    }

    pub fn island_dnd_changed(&mut self) -> Result<()> {
        if !self.ctx.config.island.embedded() {
            return Ok(());
        }
        let now = self.ctx.now();
        self.ctx.island.state.activity(Activity::DoNotDisturb { on: dnd::active() }, now);
        self.island_refresh()
    }

    pub fn island_clipboard(&mut self) -> Result<()> {
        if !self.ctx.config.island.embedded() {
            return Ok(());
        }
        if let Some(text) = system_events::clipboard_text() {
            let now = self.ctx.now();
            self.ctx.island.state.activity(Activity::Clipboard { text }, now);
            self.island_refresh()?;
        }
        Ok(())
    }

    pub fn island_lock_key(&mut self, caps: bool) -> Result<()> {
        if !self.ctx.config.island.embedded() {
            return Ok(());
        }
        let now = self.ctx.now();
        self.ctx.island.state.activity(Activity::Lock { caps, on: system_events::lock_on(caps) }, now);
        self.island_refresh()
    }

    /// Câmera ou microfone mudaram: os pontos aparecem ou somem na pílula.
    pub fn island_privacy_changed(&mut self) -> Result<()> {
        let state = self.island.privacy.as_ref().map(PrivacyWatcher::state).unwrap_or_default();
        let privacy = Privacy { mic: state.mic, cam: state.cam };
        if privacy != self.ctx.island.privacy {
            self.ctx.island.privacy = privacy;
            self.island_refresh()?;
        }
        Ok(())
    }

    /// Volume ou mudo mudou (por qualquer app, tecla ou o mixer): aviso de 1,8 s na pílula.
    pub fn island_volume_changed(&mut self) -> Result<()> {
        let Some(v) = self.island.volume.as_ref().and_then(VolumeWatcher::state) else { return Ok(()) };
        let now = self.ctx.now();
        self.ctx.island.state.activity(Activity::Volume { level: v.level, muted: v.muted }, now);
        self.island_refresh()
    }

    /// O retrato das sessões mudou.
    pub fn island_media_changed(&mut self) -> Result<()> {
        let Some(service) = &self.island.media else { return Ok(()) };
        let sessions = service.sessions();
        let now = self.ctx.now();
        let changes = self.ctx.island.update(sessions, now);
        if changes.cover && let Some(cover) = self.ctx.island.media().and_then(|s| s.cover.as_ref()) {
            let accent = cover_accent(cover).unwrap_or(DEFAULT_ACCENT);
            if self.ctx.theme.set_cover_accent(Some(accent)) {
                self.apply_bars(true)?;
            }
        }
        self.island_request_lyrics();
        self.island_refresh()?;
        if self.controller.open().is_some_and(|o| o.kind == PanelKind::Island) {
            self.redraw_island_panel()?;
        }
        self.island_capture();
        Ok(())
    }

    /// Faixa nova: pede a letra (cache ou LRCLIB) e esquece a da faixa anterior.
    fn island_request_lyrics(&mut self) {
        let req = self.ctx.island.media().map(|s| LyricsRequest { artist: s.artist.clone(), title: s.title.clone(), album: s.album.clone(), duration_s: s.duration_s.round() as u32 });
        if req == self.island.lyrics_for {
            return;
        }
        self.ctx.island.lyrics = None;
        if let (Some(r), Some(service)) = (&req, &self.island.lyrics) {
            service.request(r.clone());
        }
        self.island.lyrics_for = req;
    }

    pub fn island_lyrics(&mut self) -> Result<()> {
        let Some((req, lyrics)) = self.island.lyrics.as_ref().and_then(LyricsService::state) else { return Ok(()) };
        if Some(&req) != self.island.lyrics_for.as_ref() {
            return Ok(());
        }
        self.ctx.island.lyrics = lyrics;
        if self.controller.open().is_some_and(|o| o.kind == PanelKind::Island) {
            self.redraw_island_panel()?;
        }
        Ok(())
    }

    /// Karaokê: redesenha a aba de letras a 15 Hz enquanto toca e ela está na tela.
    pub fn island_lyrics_tick(&mut self) -> Result<()> {
        let visible = self.controller.open().is_some_and(|o| o.kind == PanelKind::Island) && self.island.tab == Tab::Lyrics;
        if visible && self.ctx.island.playing() && self.ctx.island.lyrics.as_ref().is_some_and(|l| l.synced) {
            self.redraw_island_panel()
        } else {
            window::kill_timer(TIMER_LYRICS);
            Ok(())
        }
    }

    /// Redesenha a pílula e agenda o fim da espiada.
    pub fn island_refresh(&mut self) -> Result<()> {
        if let Some(i) = self.island_bar() {
            self.bars[i].refresh_island(&self.ctx, true)?;
        }
        match self.ctx.island.state.next_change(self.ctx.now()) {
            Some(secs) => window::set_timer(TIMER_ISLAND_PEEK, (secs * 1000.0).ceil() as u32 + 20),
            None => window::kill_timer(TIMER_ISLAND_PEEK),
        }
        Ok(())
    }

    pub fn island_peek_timer(&mut self) -> Result<()> {
        window::kill_timer(TIMER_ISLAND_PEEK);
        self.island_refresh()
    }

    pub fn island_hover(&mut self, on: bool) -> Result<()> {
        if self.ctx.island.state.hover(on) {
            self.island_refresh()?;
        }
        Ok(())
    }

    /// Captura do som só com mídia tocando e a ilha visível.
    fn island_capture(&mut self) {
        let visible = self.island_bar().is_some_and(|i| !self.bars[i].is_hidden());
        let needed = self.ctx.config.island.embedded() && self.ctx.island.playing() && visible;
        if needed && self.island.capture.is_none() {
            let waveform = self.island.waveform.clone();
            let mut last = Instant::now();
            let sink = Box::new(move |mono: &[f32], rate: f32| {
                let fresh = waveform.lock().map(|mut w| w.push(mono, rate)).unwrap_or(false);
                if fresh && last.elapsed().as_millis() >= VISUALIZER_FRAME_MS {
                    last = Instant::now();
                    window::post_user(USER_VISUALIZER);
                }
            });
            match AudioCapture::start(sink) {
                Ok(c) => self.island.capture = Some(c),
                Err(e) => tracing::warn!("não consegui iniciar a captura de áudio: {e}"),
            }
        } else if !needed && self.island.capture.is_some() {
            self.island.capture = None;
            if let Ok(mut w) = self.island.waveform.lock() {
                *w = Waveform::new();
            }
            if let Some(i) = self.island_bar() {
                let _ = self.bars[i].set_visualizer(None);
            }
        }
    }

    /// A barra que hospeda a ilha apareceu ou sumiu (tela cheia).
    pub fn island_visibility_changed(&mut self) {
        self.island_capture();
    }

    pub fn island_visualizer(&mut self) -> Result<()> {
        if self.island.capture.is_none() {
            return Ok(());
        }
        let Some(i) = self.island_bar() else { return Ok(()) };
        let wave = self.island.waveform.lock().map(|w| w.clone()).ok();
        self.bars[i].set_visualizer(wave.as_ref())
    }

    fn island_tabs(&self) -> Vec<Tab> {
        tabs::available(self.ctx.island.media().is_some(), self.ctx.island.weather.is_some(), self.ctx.island.lyrics.is_some())
    }

    /// O painel da ilha: a aba atual (mídia, clima ou hardware) e os pontos das abas.
    pub fn island_panel_list(&mut self, scale: f32) -> DisplayList {
        let available = self.island_tabs();
        if !available.contains(&self.island.tab) {
            self.island.tab = available[0];
        }
        let tab = self.island.tab;
        let index = available.iter().position(|t| *t == tab).unwrap_or(0);
        if self.island.carousel.count() != available.len() {
            self.island.carousel.snap(index, available.len());
        }
        self.island.carousel.go(index);
        // O monitor de hardware só amostra com a aba na tela.
        if tab == Tab::Hardware && self.island.hardware.is_none() {
            self.island.hardware = HardwareMonitor::start(Arc::new(|| window::post_user(USER_HARDWARE))).map_err(|e| tracing::warn!("monitor de hardware indisponível: {e}")).ok();
        } else if tab != Tab::Hardware {
            self.island.hardware = None;
        }
        let size = zefiro_platform::geometry::Size { w: zefiro_island::view::EXPANDED_DESIGN.w * scale, h: zefiro_island::view::EXPANDED_DESIGN.h * scale };
        let mut list = DisplayList::new(size);
        self.island.expanded = None;
        let layers = self.island.carousel.layers();
        let moving = self.island.carousel.moving();
        let center = Point { x: size.w / 2.0, y: size.h / 2.0 };
        for layer in layers {
            let content = self.island_tab_content(available[layer.index], scale, layer.index == index && !moving);
            list.ops.extend(transformed(content.ops, layer.alpha, layer.dx * scale, layer.scale, center));
        }
        if moving {
            // Enquanto o carrossel anda, os cliques não valem: as áreas estão fora do lugar.
            self.island.expanded = None;
            window::set_timer(TIMER_ISLAND_ANIM, 16);
        }
        tabs::tab_dots(&mut list, available.len(), index, scale, &self.ctx.theme.tokens);
        list
    }

    /// Conteúdo de uma aba, sem os pontos. `interactive`: guarda as áreas de clique da mídia.
    fn island_tab_content(&mut self, tab: Tab, scale: f32, interactive: bool) -> DisplayList {
        let anim = self.island_expanded_anim();
        let font_scale = self.ctx.theme.theme.font.scale;
        let tokens = &self.ctx.theme.tokens;
        match (tab, self.ctx.island.media(), &self.ctx.island.weather) {
            (Tab::Media, Some(s), _) => {
                let view = expanded(s, &self.ctx.island.source_slots(), now_unix_ms(), scale, font_scale, tokens, anim);
                let list = view.list.clone();
                if interactive {
                    self.island.expanded = Some(view);
                    window::set_timer(TIMER_ISLAND_TICK, 1000);
                }
                list
            }
            (Tab::Weather, _, Some(w)) => tabs::weather_tab(w, &forecast_names(), scale, font_scale, tokens),
            (Tab::Lyrics, Some(s), _) if self.ctx.island.lyrics.is_some() => {
                let position_ms = (s.position_at(now_unix_ms()) * 1000.0) as i64;
                if interactive && self.ctx.island.playing() {
                    window::set_timer(TIMER_LYRICS, 66);
                }
                tabs::lyrics_tab(self.ctx.island.lyrics.as_ref().expect("conferido no padrão"), position_ms, scale, font_scale, tokens, &self.ctx.text)
            }
            _ => {
                let h = self.island.hardware.as_ref().and_then(HardwareMonitor::latest).unwrap_or_default();
                tabs::hardware_tab(&h, scale, font_scale, tokens)
            }
        }
    }

    /// Roda do mouse no painel da ilha: troca de aba.
    pub fn island_wheel(&mut self, delta: i32) -> Result<()> {
        let available = self.island_tabs();
        let next = tabs::step(&available, self.island.tab, delta);
        if next != self.island.tab {
            self.island.tab = next;
            if !self.ctx.theme.animations {
                let index = available.iter().position(|t| *t == next).unwrap_or(0);
                self.island.carousel.snap(index, available.len());
            } else if !self.island.carousel.moving() {
                // Primeiro quadro do movimento: o anterior pode ter sido há muito tempo.
                self.island.anim_last = Some(Instant::now() - std::time::Duration::from_millis(8));
            }
            self.redraw_island_panel()?;
        }
        Ok(())
    }

    /// Nova leitura do clima: pílula ociosa e aba do clima.
    pub fn island_weather(&mut self) -> Result<()> {
        let w = self.island.weather.as_ref().and_then(WeatherService::weather);
        if w != self.ctx.island.weather {
            self.ctx.island.weather = w;
            self.island_refresh()?;
            if self.controller.open().is_some_and(|o| o.kind == PanelKind::Island) && self.island.tab == Tab::Weather {
                self.redraw_island_panel()?;
            }
        }
        Ok(())
    }

    /// Nova amostra do hardware, com a aba aberta.
    pub fn island_hardware(&mut self) -> Result<()> {
        if self.controller.open().is_some_and(|o| o.kind == PanelKind::Island) && self.island.tab == Tab::Hardware {
            self.redraw_island_panel()?;
        } else {
            self.island.hardware = None;
        }
        Ok(())
    }

    pub fn effective_open_kind(&self) -> Option<PanelKind> {
        Some(self.controller.open()?.kind)
    }

    fn redraw_island_panel(&mut self) -> Result<()> {
        let Some(open) = self.controller.open().cloned() else { return Ok(()) };
        let scale = self.bars[open.monitor].scale();
        let list = self.island_panel_list(scale);
        let Some(chip) = self.bars[open.monitor].chip_rect(ISLAND_CHIP) else { return Ok(()) };
        let band_bottom = self.bars[open.monitor].band_bottom();
        self.panels[open.monitor].resize(&self.ctx, chip, band_bottom, &list)
    }

    /// A linha do tempo anda uma vez por segundo enquanto o painel da ilha está aberto.
    pub fn island_tick(&mut self) -> Result<()> {
        if !self.controller.open().is_some_and(|o| o.kind == PanelKind::Island) || self.ctx.island.media().is_none() || self.island.tab != Tab::Media {
            window::kill_timer(TIMER_ISLAND_TICK);
            self.island.expanded = None;
            return Ok(());
        }
        if self.ctx.island.playing() {
            self.redraw_island_panel()?;
        }
        Ok(())
    }

    /// Clique no painel expandido (`p` em coordenadas do painel). `true` se acertou um controle.
    pub fn island_panel_click(&mut self, p: Point) -> Result<bool> {
        let (Some(view), Some(s)) = (&self.island.expanded, self.ctx.island.media()) else { return Ok(false) };
        let Some(action) = hit(view, s, p) else { return Ok(false) };
        let id = s.id.clone();
        let command = match action {
            IslandAction::Previous => MediaCommand::Previous(id),
            IslandAction::Next => MediaCommand::Next(id),
            IslandAction::PlayPause => MediaCommand::PlayPause(id),
            IslandAction::Seek(seconds) => MediaCommand::Seek(id, seconds),
            IslandAction::Source(source) => {
                self.island_switch_source(source)?;
                return Ok(true);
            }
        };
        if let Some(service) = &self.island.media {
            service.command(command);
        }
        Ok(true)
    }

    /// Botão direito na ilha: fonte de mídia, saída de som do app, saída e microfone padrão.
    pub fn island_menu(&mut self, bar: usize) -> Result<()> {
        let pairs = |list: Vec<AudioDevice>| list.into_iter().map(|d| (d.id, d.name)).collect::<Vec<_>>();
        let (outputs, default_output) = audio_devices::devices(Flow::Output).unwrap_or_default();
        let (inputs, default_input) = audio_devices::devices(Flow::Input).unwrap_or_default();
        let current = self.ctx.island.current().map(|s| (s.id.clone(), s.app_name.clone()));
        let route = current.as_ref().map(|(id, name)| audio_devices::app_route(id, name));
        let app_output = route.as_ref().map(|r| match r {
            None => AppOutput::Unavailable,
            Some((_, RouteState::Current(d))) => AppOutput::Current(d.clone()),
            Some((_, RouteState::NoAudioYet)) => AppOutput::NoAudioYet,
            Some((_, RouteState::NotRunning)) => AppOutput::NotRunning,
        });
        let input = MenuInput {
            sources: self.ctx.island.sources(),
            current_source: current.map(|(id, _)| id),
            outputs: pairs(outputs),
            default_output,
            inputs: pairs(inputs),
            default_input,
            app_output,
            mic_muted: audio_devices::mic_muted(),
            timer_running: self.ctx.island.state.timer.map(|t| t.running()),
        };
        let entries = menu::build(&input);
        tracing::info!(itens = entries.len(), "menu da ilha");
        let Some(id) = popup_menu::show(self.bars[bar].hwnd(), &entries)? else { return Ok(()) };
        let result = match menu::command(&input, id) {
            Some(MenuCommand::Source(source)) => {
                let now = self.ctx.now();
                self.ctx.island.pick(&source, now);
                return self.island_refresh();
            }
            Some(MenuCommand::AppOutput(device)) => match route.flatten() {
                Some((r, _)) => audio_devices::set_app_output(&r, device.as_deref()),
                None => Ok(()),
            },
            Some(MenuCommand::DefaultOutput(d) | MenuCommand::DefaultInput(d)) => audio_devices::set_default(&d),
            Some(MenuCommand::ToggleMicMute) => audio_devices::set_mic_muted(!input.mic_muted.unwrap_or(false)),
            Some(MenuCommand::StartTimer(focus, minutes)) => {
                let kind = if focus { TimerKind::Focus } else { TimerKind::Break };
                self.ctx.island.state.timer = Some(FocusTimer::start(kind, minutes, self.ctx.now()));
                window::set_timer(TIMER_FOCUS, 1000);
                return self.island_refresh();
            }
            Some(MenuCommand::PauseResumeTimer) => {
                let now = self.ctx.now();
                if let Some(t) = &mut self.ctx.island.state.timer {
                    if t.running() { t.pause(now) } else { t.resume(now) }
                }
                return self.island_refresh();
            }
            Some(MenuCommand::StopTimer) => {
                self.ctx.island.state.timer = None;
                window::kill_timer(TIMER_FOCUS);
                return self.island_refresh();
            }
            None => Ok(()),
        };
        if let Err(e) = result {
            tracing::warn!(id, "ação do menu da ilha falhou: {e}");
        }
        Ok(())
    }

    /// Troca de fonte pela barra de fontes, animada como a ilha C++: o conteúdo antigo sai
    /// deslizando 14 px e some (0,17 s), a fonte troca, o novo entra do outro lado (0,24 s); o
    /// indicador desliza até a fonte nova em 0,30 s.
    fn island_switch_source(&mut self, target: String) -> Result<()> {
        let current = self.ctx.island.current().map(|s| s.id.clone()).unwrap_or_default();
        if target == current {
            return Ok(());
        }
        if !self.ctx.theme.animations {
            let now = self.ctx.now();
            self.ctx.island.pick(&target, now);
            self.island_refresh()?;
            return self.redraw_island_panel();
        }
        let slots = self.ctx.island.source_slots();
        let from = dock_index(&slots, &current, &current).unwrap_or(0) as f32;
        let to = dock_index(&slots, &current, &target).unwrap_or(0) as f32;
        let now = Instant::now();
        self.island.swap = Some(Swap { target, dir: if to >= from { 1.0 } else { -1.0 }, phase: SwapPhase::Out, start: now, dock: (self.island.swap.as_ref().map_or(from, |s| s.dock_at(now)), to, now) });
        window::set_timer(TIMER_ISLAND_ANIM, 16);
        self.redraw_island_panel()
    }

    /// Quadro da animação de troca (60 Hz só enquanto ela dura).
    pub fn island_anim_tick(&mut self) -> Result<()> {
        let now = Instant::now();
        let open = self.controller.open().is_some_and(|o| o.kind == PanelKind::Island);
        // Carrossel das abas: passo da mola desde o último quadro.
        let dt = self.island.anim_last.map_or(1.0 / 120.0, |t| now.duration_since(t).as_secs_f32());
        self.island.anim_last = Some(now);
        let was_moving = self.island.carousel.moving();
        let carousel_moving = was_moving && self.island.carousel.step(dt, self.ctx.theme.theme.animation.speed);
        // Troca de fonte.
        if let Some(swap) = &mut self.island.swap {
            let elapsed = now.duration_since(swap.start).as_secs_f32();
            match swap.phase {
                SwapPhase::Out if elapsed >= SWITCH_OUT_S => {
                    let target = swap.target.clone();
                    swap.phase = SwapPhase::In;
                    swap.start = now;
                    let t = self.ctx.now();
                    self.ctx.island.pick(&target, t);
                    self.island_refresh()?;
                }
                SwapPhase::In if elapsed >= SWITCH_IN_S && now.duration_since(swap.dock.2).as_secs_f32() >= DOCK_SLIDE_S => self.island.swap = None,
                _ => {}
            }
        }
        if !open {
            self.island.swap = None;
            self.island.anim_last = None;
            window::kill_timer(TIMER_ISLAND_ANIM);
            return Ok(());
        }
        if !carousel_moving && self.island.swap.is_none() {
            // Último quadro, já parado: devolve as áreas de clique.
            window::kill_timer(TIMER_ISLAND_ANIM);
            self.island.anim_last = None;
        }
        self.redraw_island_panel()
    }

    fn island_expanded_anim(&self) -> ExpandedAnim {
        let Some(swap) = &self.island.swap else { return ExpandedAnim::default() };
        let now = Instant::now();
        let t = |d: f32| (now.duration_since(swap.start).as_secs_f32() / d).clamp(0.0, 1.0);
        let (alpha, dx) = match swap.phase {
            SwapPhase::Out => {
                let e = t(SWITCH_OUT_S).powi(2);
                (1.0 - e, -swap.dir * SWITCH_SHIFT * e)
            }
            SwapPhase::In => {
                let e = 1.0 - (1.0 - t(SWITCH_IN_S)).powi(3);
                (e, swap.dir * SWITCH_SHIFT * (1.0 - e))
            }
        };
        ExpandedAnim { dock_pos: Some(swap.dock_at(now)), swap_alpha: alpha, swap_dx: dx }
    }

    /// Ao abrir, o painel começa na primeira aba (a mídia, quando há).
    pub fn island_panel_opening(&mut self) {
        let available = self.island_tabs();
        self.island.tab = available[0];
        self.island.carousel.snap(0, available.len());
    }

    /// Ao fechar: o painel da ilha não guarda áreas de clique de um desenho antigo.
    pub fn island_panel_closed(&mut self) {
        self.island.expanded = None;
        self.island.hardware = None;
        window::kill_timer(TIMER_ISLAND_TICK);
    }
}

/// Rótulos da previsão: "Hoje", "Amanhã" e o dia da semana de depois de amanhã.
fn forecast_names() -> [String; 3] {
    const SHORT: [&str; 7] = ["dom", "seg", "ter", "qua", "qui", "sex", "sáb"];
    let after = (zefiro_integration_clock::grid::weekday(now_local().date) + 2) % 7;
    ["Hoje".into(), "Amanhã".into(), SHORT[after as usize].into()]
}

const SWITCH_OUT_S: f32 = 0.17;
const SWITCH_IN_S: f32 = 0.24;
const SWITCH_SHIFT: f32 = 14.0;
const DOCK_SLIDE_S: f32 = 0.30;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SwapPhase { Out, In }

/// Troca de fonte em andamento.
struct Swap {
    target: String,
    /// +1 = a fonte nova está à direita na barra.
    dir: f32,
    phase: SwapPhase,
    start: Instant,
    /// Indicador: de, até e início do deslize.
    dock: (f32, f32, Instant),
}

impl Swap {
    fn dock_at(&self, now: Instant) -> f32 {
        let (from, to, start) = self.dock;
        let t = (now.duration_since(start).as_secs_f32() / DOCK_SLIDE_S).clamp(0.0, 1.0);
        // Entrada e saída suaves (cúbica), como o SkipEaseInOut da ilha.
        let e = if t < 0.5 { 4.0 * t * t * t } else { 1.0 - (-2.0 * t + 2.0).powi(3) / 2.0 };
        from + (to - from) * e
    }
}
