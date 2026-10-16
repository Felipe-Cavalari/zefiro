//! A ilha embutida no laço de eventos: sessões de mídia, visualizador, espiada e o painel expandido.

use std::sync::{Arc, Mutex};
use std::time::Instant;
use windows::core::Result;
use zefiro_bar::calendar::{calendar_names, calendar_paint};
use zefiro_bar::controller::PanelKind;
use zefiro_integration_clock::format::ClockText;
use zefiro_integration_clock::grid::month_grid;
use zefiro_integration_clock::windows::now_local;
use zefiro_island::accent::cover_accent;
use zefiro_island::view::{ExpandedView, IslandAction, expanded, hit};
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
use zefiro_island::view::Privacy;
use zefiro_island::menu::{self, AppOutput, MenuCommand, MenuInput};
use zefiro_island::state::{Activity, Notice};
use zefiro_island::timer::{FocusTimer, TimerKind};
use zefiro_platform_windows::window;
use zefiro_ui::paint::DisplayList;

use crate::bar_view::ISLAND_CHIP;
use crate::event_loop::{Loop, TIMER_FOCUS, TIMER_ISLAND_PEEK, TIMER_ISLAND_TICK, USER_BLUETOOTH, USER_DND, USER_MEDIA, USER_NOTIFICATION, USER_PRIVACY, USER_VISUALIZER, USER_VOLUME};

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
        self.island_refresh()?;
        if self.controller.open().is_some_and(|o| o.kind == PanelKind::Island) {
            self.redraw_island_panel()?;
        }
        self.island_capture();
        Ok(())
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

    /// O painel da ilha: a mídia expandida ou, sem mídia, o calendário (a pílula ociosa é o relógio).
    pub fn island_panel_list(&mut self, scale: f32) -> DisplayList {
        let tokens = &self.ctx.theme.tokens;
        match self.ctx.island.media() {
            Some(s) => {
                let view = expanded(s, &self.ctx.island.sources(), now_unix_ms(), scale, self.ctx.theme.theme.font.scale, tokens);
                let list = view.list.clone();
                self.island.expanded = Some(view);
                window::set_timer(TIMER_ISLAND_TICK, 1000);
                list
            }
            None => {
                self.island.expanded = None;
                let today = now_local().date;
                let (y, mo) = self.shown_month;
                let grid = month_grid(y, mo, self.ctx.clock.first_weekday(), today);
                calendar_paint(&grid, &calendar_names(&self.ctx.clock, y, mo, today), scale, tokens)
            }
        }
    }

    /// Tipo efetivo do painel aberto (a ilha sem mídia se comporta como o calendário).
    pub fn effective_open_kind(&self) -> Option<PanelKind> {
        let kind = self.controller.open()?.kind;
        Some(if kind == PanelKind::Island && self.ctx.island.media().is_none() { PanelKind::Calendar } else { kind })
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
        if !self.controller.open().is_some_and(|o| o.kind == PanelKind::Island) || self.ctx.island.media().is_none() {
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
                let now = self.ctx.now();
                self.ctx.island.pick(&source, now);
                self.island_refresh()?;
                self.redraw_island_panel()?;
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

    /// Ao fechar: o painel da ilha não guarda áreas de clique de um desenho antigo.
    pub fn island_panel_closed(&mut self) {
        self.island.expanded = None;
        window::kill_timer(TIMER_ISLAND_TICK);
    }
}
