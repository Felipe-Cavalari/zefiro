//! A ilha embutida no laço de eventos: sessões de mídia, visualizador, espiada e o painel expandido.

use std::sync::{Arc, Mutex};
use std::time::Instant;
use windows::core::Result;
use zefiro_bar::controller::PanelKind;
use zefiro_integration_clock::windows::now_local;
use zefiro_island::accent::media_accent;
use zefiro_island::view::{ExpandedView, IslandAction, dock_index, expanded, hit};
use crate::tab_strip::LayerSpec;
use windows::Foundation::TimeSpan;
use windows::core::HSTRING;
use windows_numerics::Vector2;
use zefiro_island::waveform::Waveform;
use zefiro_platform::geometry::Point;
use zefiro_platform_windows::audio_capture::AudioCapture;
use zefiro_platform_windows::media::{MediaCommand, MediaService, now_unix_ms};
use zefiro_platform_windows::audio_devices::{self, AudioDevice, Flow, RouteState};
use zefiro_platform_windows::popup_menu;
use zefiro_platform_windows::privacy::PrivacyWatcher;
use zefiro_platform_windows::system_events;
use zefiro_platform_windows::dnd;
use zefiro_platform_windows::bluetooth::BluetoothWatcher;
use zefiro_platform_windows::weather::WeatherService;
use zefiro_platform_windows::hardware::HardwareMonitor;
use zefiro_platform_windows::lyrics::{LyricsRequest, LyricsService};
use zefiro_island::tabs::{self, Tab, TabSet};
use zefiro_island::view::Privacy;
use zefiro_island::menu::{self, AppOutput, MenuCommand, MenuInput};
use zefiro_island::state::{Activity, Notice};
use zefiro_island::timer::{FocusTimer, TimerKind};
use zefiro_platform_windows::window;
use zefiro_ui::paint::DisplayList;

use crate::bar_view::PLAYER_CHIP;
use crate::event_loop::{Loop, TIMER_FOCUS, TIMER_ISLAND_ANIM, TIMER_LYRICS, TIMER_ISLAND_PEEK, TIMER_ISLAND_TICK, USER_BLUETOOTH, USER_DND, USER_HARDWARE, USER_LYRICS, USER_MEDIA, USER_PRIVACY, USER_VISUALIZER, USER_WEATHER};

/// O visualizador atualiza no máximo a 60 Hz.
const VISUALIZER_FRAME_MS: u128 = 16;

#[derive(Default)]
pub struct IslandRuntime {
    media: Option<MediaService>,
    privacy: Option<PrivacyWatcher>,
    bluetooth: Option<BluetoothWatcher>,
    weather: Option<WeatherService>,
    hardware: Option<HardwareMonitor>,
    tab: Tab,
    /// Última amostra do hardware (a aba aparece no carrossel mesmo sem estar amostrando).
    last_hardware: zefiro_island::tabs::Hardware,
    lyrics: Option<LyricsService>,
    /// Faixa cuja letra foi pedida por último.
    lyrics_for: Option<LyricsRequest>,
    swap: Option<Swap>,
    /// Área de transferência e Caps Lock ligados na janela de mensagens.
    listening: bool,
    capture: Option<AudioCapture>,
    waveform: Arc<Mutex<Waveform>>,
    expanded: Option<ExpandedView>,
    /// Último resumo das sessões que foi para o log (só loga quando muda).
    described: String,
}

impl Loop {
    /// A mídia serve à ilha embutida ou a algum chip do player.
    fn media_wanted(&self) -> bool {
        self.ctx.config.island.embedded() || self.ctx.config.bar.uses(PLAYER_CHIP)
    }

    /// Liga ou desliga os serviços conforme `[island] engine` e os chips da barra (no início e
    /// quando a configuração muda). Mídia e clima também servem aos chips do player e do clima.
    pub fn island_services(&mut self) -> Result<()> {
        let embedded = self.ctx.config.island.embedded();
        let media = self.media_wanted();
        let weather = embedded || self.ctx.config.bar.uses("weather");
        if media && self.island.media.is_none() {
            match MediaService::start(Arc::new(|| window::post_user(USER_MEDIA))) {
                Ok(service) => self.island.media = Some(service),
                Err(e) => tracing::error!("não consegui iniciar a thread de mídia: {e}"),
            }
        } else if !media && self.island.media.is_some() {
            self.island.media = None;
            self.island.capture = None;
            self.ctx.island.update(Vec::new(), self.ctx.now());
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
        if weather && self.island.weather.is_none() {
            match WeatherService::start(String::new(), Arc::new(|| window::post_user(USER_WEATHER))) {
                Ok(w) => self.island.weather = Some(w),
                Err(e) => tracing::warn!("clima indisponível: {e}"),
            }
        } else if !weather {
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
        let Some(w) = &self.system.notifications else { return Ok(()) };
        if !self.ctx.config.island.embedded() {
            // Só o chip usa o serviço: descarta os avisos para a fila não crescer.
            w.take();
            return Ok(());
        }
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
        if !self.ctx.config.island.embedded() {
            return Ok(());
        }
        let Some(v) = self.ctx.devices.speaker else { return Ok(()) };
        let now = self.ctx.now();
        self.ctx.island.state.activity(Activity::Volume { level: v.level, muted: v.muted }, now);
        self.island_refresh()
    }

    /// Brilho mudado pela barra (roda ou slider): aviso de 1,8 s na pílula.
    pub fn island_brightness_changed(&mut self, percent: u32) -> Result<()> {
        if !self.ctx.config.island.embedded() {
            return Ok(());
        }
        let now = self.ctx.now();
        self.ctx.island.state.activity(Activity::Brightness { level: percent as f32 / 100.0 }, now);
        self.island_refresh()
    }

    /// O retrato das sessões mudou.
    pub fn island_media_changed(&mut self) -> Result<()> {
        let Some(service) = &self.island.media else { return Ok(()) };
        let sessions = service.sessions();
        let now = self.ctx.now();
        let changes = self.ctx.island.update(sessions, now);
        let described = self.ctx.island.describe();
        if described != self.island.described {
            tracing::info!(media = self.ctx.island.media().is_some(), "sessões de mídia: {described}");
            self.island.described = described;
        }
        // Sem mídia ou sem capa, o destaque volta ao do tema (não fica preso na última capa).
        if (changes.cover || self.ctx.island.media().is_none()) && self.ctx.theme.set_cover_accent(media_accent(self.ctx.island.media())) {
            self.apply_bars(true)?;
            self.redraw_open_panel()?;
        }
        self.island_request_lyrics();
        self.island_refresh()?;
        if self.controller.open().is_some_and(|o| o.kind.tabbed()) {
            if self.island_tabs().is_empty() {
                // A mídia sumiu com o painel do player aberto: não há o que mostrar.
                self.close_open_panel()?;
            } else {
                self.redraw_island_panel()?;
            }
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

    /// Redesenha a pílula da ilha e as dos players e agenda o fim da espiada.
    pub fn island_refresh(&mut self) -> Result<()> {
        for b in &mut self.bars {
            b.refresh_media(&self.ctx, true)?;
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

    /// Mouse em cima do player da barra `bar`: espia só ele. Com o painel dele aberto, não espia
    /// (o painel nasce do chip e não pode andar sob o cursor).
    pub fn player_hover(&mut self, bar: usize, on: bool) -> Result<()> {
        if self.controller.open().is_some_and(|o| o.monitor == bar && o.chip == PLAYER_CHIP) {
            return Ok(());
        }
        if self.bars[bar].set_player_hover(on) {
            self.bars[bar].refresh_media(&self.ctx, true)?;
        }
        Ok(())
    }

    /// Captura do som só com mídia tocando e a ilha ou um player visível.
    fn island_capture(&mut self) {
        let visible = self.bars.iter().any(|b| b.shows_media() && !b.is_hidden());
        let needed = self.media_wanted() && self.ctx.island.playing() && visible;
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
            tracing::info!("captura de áudio da ilha ligada");
            match AudioCapture::start(sink) {
                Ok(c) => self.island.capture = Some(c),
                Err(e) => tracing::warn!("não consegui iniciar a captura de áudio: {e}"),
            }
        } else if !needed && self.island.capture.is_some() {
            tracing::info!(playing = self.ctx.island.playing(), visible, "captura de áudio da ilha desligada");
            self.island.capture = None;
            if let Ok(mut w) = self.island.waveform.lock() {
                *w = Waveform::new();
            }
            for b in &self.bars {
                let _ = b.set_visualizer(None);
            }
        }
    }

    /// Uma barra com a ilha ou um player apareceu ou sumiu (tela cheia).
    pub fn island_visibility_changed(&mut self) {
        self.island_capture();
    }

    pub fn island_visualizer(&mut self) -> Result<()> {
        if self.island.capture.is_none() {
            return Ok(());
        }
        let wave = self.island.waveform.lock().map(|w| w.clone()).ok();
        for b in self.bars.iter().filter(|b| b.shows_media() && !b.is_hidden()) {
            b.set_visualizer(wave.as_ref())?;
        }
        Ok(())
    }

    /// Abas do painel aberto: o carrossel da ilha, ou a aba única de um chip.
    fn island_tabs(&self) -> Vec<Tab> {
        let kind = self.controller.open().map(|o| o.kind);
        tabs::for_set(tab_set(kind), self.ctx.island.media().is_some(), self.ctx.island.weather.is_some(), self.ctx.island.lyrics.is_some())
    }

    /// O chip tem o que mostrar no painel? (O player sem mídia e o clima sem leitura não abrem nada.)
    pub fn chip_has_panel(&self, chip: &str) -> bool {
        let set = tab_set(zefiro_bar::controller::panel_kind_for(chip));
        set == TabSet::Island || !tabs::for_set(set, self.ctx.island.media().is_some(), self.ctx.island.weather.is_some(), self.ctx.island.lyrics.is_some()).is_empty()
    }

    /// Conteúdo normal do painel da ilha: só os pontos das abas. As abas em si ficam no compositor
    /// (`island_sync_tabs`), para a troca entre elas rodar na taxa do monitor.
    pub fn island_panel_list(&mut self, scale: f32) -> DisplayList {
        let available = self.island_tabs();
        if let Some(first) = available.first().filter(|_| !available.contains(&self.island.tab)) {
            self.island.tab = *first;
        }
        let index = available.iter().position(|t| *t == self.island.tab).unwrap_or(0);
        let mut list = DisplayList::new(zefiro_platform::geometry::Size { w: zefiro_island::view::EXPANDED_DESIGN.w * scale, h: zefiro_island::view::EXPANDED_DESIGN.h * scale });
        tabs::tab_dots(&mut list, available.len(), index, scale, &self.ctx.theme.tokens);
        list
    }

    /// Desenha (só o que mudou) as abas no compositor do painel aberto. `animate`: a troca de aba
    /// anda com a mola 250/31; senão, pula para a aba atual.
    pub fn island_sync_tabs(&mut self, monitor: usize, animate: bool) -> Result<()> {
        let available = self.island_tabs();
        let Some(first) = available.first().copied() else { return Ok(()) };
        if !available.contains(&self.island.tab) {
            self.island.tab = first;
        }
        let tab = self.island.tab;
        let index = available.iter().position(|t| *t == tab).unwrap_or(0);
        // O monitor de hardware só amostra com a aba na tela (a última amostra fica para o carrossel).
        if tab == Tab::Hardware && self.island.hardware.is_none() {
            self.island.hardware = HardwareMonitor::start(Arc::new(|| window::post_user(USER_HARDWARE))).map_err(|e| tracing::warn!("monitor de hardware indisponível: {e}")).ok();
        } else if tab != Tab::Hardware {
            self.island.hardware = None;
        }
        let scale = self.bars[monitor].scale();
        let font_scale = self.ctx.theme.theme.font.scale;
        self.island.expanded = None;
        let mut specs = Vec::new();
        for t in &available {
            let tokens = &self.ctx.theme.tokens;
            let current = *t == tab;
            let layers = match (t, self.ctx.island.media(), &self.ctx.island.weather) {
                (Tab::Media, Some(s), _) => {
                    let sources = self.ctx.island.source_slots();
                    let view = expanded(s, &sources, now_unix_ms(), scale, font_scale, tokens);
                    // Durante a troca de fonte, o indicador já vai para a fonte escolhida.
                    let ind = view.indicator.as_ref().map(|ind| {
                        let slot = self.island.swap.as_ref().and_then(|w| dock_index(&sources, &s.id, &w.target)).unwrap_or(ind.active);
                        LayerSpec { list: ind.list.clone(), offset: Point { x: ind.slot_x.get(slot).copied().unwrap_or(ind.slot_x[0]), y: ind.y }, slide_s: Some(DOCK_SLIDE_S) }
                    });
                    let empty = LayerSpec::at_origin(DisplayList::new(zefiro_platform::geometry::Size { w: 1.0, h: 1.0 }));
                    let layers = vec![LayerSpec::at_origin(view.list.clone()), ind.unwrap_or(empty), LayerSpec::at_origin(view.dock.clone()), LayerSpec::at_origin(view.group.clone())];
                    if current {
                        self.island.expanded = Some(view);
                        window::set_timer(TIMER_ISLAND_TICK, 1000);
                    }
                    layers
                }
                (Tab::Weather, _, Some(w)) => vec![LayerSpec::at_origin(tabs::weather_tab(w, &forecast_names(), scale, font_scale, tokens))],
                (Tab::Lyrics, Some(s), Some(_) | None) if self.ctx.island.lyrics.is_some() => {
                    let position_ms = (s.position_at(now_unix_ms()) * 1000.0) as i64;
                    if current && self.ctx.island.playing() {
                        window::set_timer(TIMER_LYRICS, 66);
                    }
                    vec![LayerSpec::at_origin(tabs::lyrics_tab(self.ctx.island.lyrics.as_ref().expect("conferido no padrão"), position_ms, scale, font_scale, tokens, &self.ctx.text))]
                }
                _ => {
                    if let Some(h) = self.island.hardware.as_ref().and_then(HardwareMonitor::latest) {
                        self.island.last_hardware = h;
                    }
                    vec![LayerSpec::at_origin(tabs::hardware_tab(&self.island.last_hardware, scale, font_scale, tokens))]
                }
            };
            specs.push(layers);
        }
        self.panels[monitor].set_tabs(&self.ctx, &specs, index, animate)
    }

    /// Roda do mouse ou setas no painel da ilha: troca de aba (a mola fica com o compositor).
    pub fn island_wheel(&mut self, delta: i32) -> Result<()> {
        let next = tabs::step(&self.island_tabs(), self.island.tab, delta);
        if next != self.island.tab {
            self.island.tab = next;
            self.redraw_island_panel()?;
        }
        Ok(())
    }

    /// Nova leitura do clima: pílula ociosa e aba do clima.
    pub fn island_weather(&mut self) -> Result<()> {
        let w = self.island.weather.as_ref().and_then(WeatherService::weather);
        if w != self.ctx.island.weather {
            self.ctx.island.weather = w;
            if self.ctx.config.bar.uses("weather") {
                self.apply_bars(true)?;
            }
            self.island_refresh()?;
            if self.controller.open().is_some_and(|o| o.kind.tabbed()) && self.island.tab == Tab::Weather {
                self.redraw_island_panel()?;
            }
        }
        Ok(())
    }

    /// Nova amostra do hardware, com a aba aberta.
    pub fn island_hardware(&mut self) -> Result<()> {
        if self.controller.open().is_some_and(|o| o.kind.tabbed()) && self.island.tab == Tab::Hardware {
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
        let chip = self.panel_anchor(open.monitor, &open.chip);
        let band_bottom = self.bars[open.monitor].band_bottom();
        self.panels[open.monitor].resize(&self.ctx, chip, band_bottom, &list)?;
        self.island_sync_tabs(open.monitor, true)
    }

    /// A linha do tempo anda uma vez por segundo enquanto o painel da ilha está aberto.
    pub fn island_tick(&mut self) -> Result<()> {
        if !self.controller.open().is_some_and(|o| o.kind.tabbed()) || self.ctx.island.media().is_none() || self.island.tab != Tab::Media {
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
        if target == current || self.island.swap.is_some() {
            return Ok(());
        }
        if !self.ctx.theme.animations {
            let now = self.ctx.now();
            self.ctx.island.pick(&target, now);
            self.island_refresh()?;
            return self.redraw_island_panel();
        }
        let slots = self.ctx.island.source_slots();
        let from = dock_index(&slots, &current, &current).unwrap_or(0);
        let to = dock_index(&slots, &current, &target).unwrap_or(0);
        let dir = if to >= from { 1.0 } else { -1.0 };
        self.island.swap = Some(Swap { target, dir, phase: SwapPhase::Out });
        // O indicador começa a deslizar já (camada própria), e a capa com os textos sai.
        self.redraw_island_panel()?;
        self.island_animate_group(SwapPhase::Out, dir)?;
        window::set_timer(TIMER_ISLAND_ANIM, (SWITCH_OUT_S * 1000.0) as u32);
        Ok(())
    }

    /// Anima a camada capa + textos + tempo da aba de mídia no compositor: sai deslizando 14 px e
    /// some (0,17 s, entrada acelerada) ou entra do outro lado (0,24 s, saída cúbica).
    fn island_animate_group(&self, phase: SwapPhase, dir: f32) -> Result<()> {
        let Some(open) = self.controller.open() else { return Ok(()) };
        let Some(media) = self.island_tabs().iter().position(|t| *t == Tab::Media) else { return Ok(()) };
        let Some(group) = self.panels[open.monitor].tab_layer(media, GROUP_LAYER) else { return Ok(()) };
        let shift = SWITCH_SHIFT * self.bars[open.monitor].scale();
        let c = &self.ctx.comp.compositor;
        let (secs, easing, from_x, to_x, to_alpha) = match phase {
            SwapPhase::Out => (SWITCH_OUT_S, c.CreateCubicBezierEasingFunction(Vector2 { X: 0.11, Y: 0.0 }, Vector2 { X: 0.5, Y: 0.0 })?, 0.0, -dir * shift, 0.0),
            SwapPhase::In => (SWITCH_IN_S, c.CreateCubicBezierEasingFunction(Vector2 { X: 0.33, Y: 1.0 }, Vector2 { X: 0.68, Y: 1.0 })?, dir * shift, 0.0, 1.0),
        };
        let duration = TimeSpan { Duration: (secs as f64 * 1e7) as i64 };
        let offset = c.CreateScalarKeyFrameAnimation()?;
        offset.InsertKeyFrame(0.0, from_x)?;
        offset.InsertKeyFrameWithEasingFunction(1.0, to_x, &easing)?;
        offset.SetDuration(duration)?;
        let alpha = c.CreateScalarKeyFrameAnimation()?;
        alpha.InsertKeyFrame(0.0, 1.0 - to_alpha)?;
        alpha.InsertKeyFrameWithEasingFunction(1.0, to_alpha, &easing)?;
        alpha.SetDuration(duration)?;
        group.StartAnimation(&HSTRING::from("Offset.X"), &offset)?;
        group.StartAnimation(&HSTRING::from("Opacity"), &alpha)
    }

    /// Fim de uma fase da troca de fonte (um temporizador por fase, não por quadro).
    pub fn island_anim_tick(&mut self) -> Result<()> {
        window::kill_timer(TIMER_ISLAND_ANIM);
        let Some(swap) = self.island.swap.as_mut() else { return Ok(()) };
        match swap.phase {
            SwapPhase::Out => {
                swap.phase = SwapPhase::In;
                let (target, dir) = (swap.target.clone(), swap.dir);
                let now = self.ctx.now();
                self.ctx.island.pick(&target, now);
                self.island_refresh()?;
                self.redraw_island_panel()?;
                self.island_animate_group(SwapPhase::In, dir)?;
                window::set_timer(TIMER_ISLAND_ANIM, (SWITCH_IN_S * 1000.0) as u32);
            }
            SwapPhase::In => self.island.swap = None,
        }
        Ok(())
    }

    pub fn island_panel_opening(&mut self) {
        self.island.tab = self.island_tabs().first().copied().unwrap_or_default();
    }

    /// Ao fechar: o painel da ilha não guarda áreas de clique de um desenho antigo.
    pub fn island_panel_closed(&mut self) {
        self.island.expanded = None;
        self.island.hardware = None;
        window::kill_timer(TIMER_ISLAND_TICK);
    }
}

/// Conjunto de abas de cada painel com abas (qualquer outro é a ilha).
fn tab_set(kind: Option<PanelKind>) -> TabSet {
    match kind {
        Some(PanelKind::Player) => TabSet::Media,
        Some(PanelKind::Weather) => TabSet::Weather,
        Some(PanelKind::Hardware) => TabSet::Hardware,
        _ => TabSet::Island,
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
}

/// Camadas da aba de mídia: fixa, indicador, ícones das fontes, capa + textos + tempo.
const GROUP_LAYER: usize = 3;
