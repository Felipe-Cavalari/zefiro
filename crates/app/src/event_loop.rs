//! O laço de eventos: entradas → controlador → visões.

use windows::Win32::UI::WindowsAndMessaging::PostQuitMessage;
use windows::core::Result;
use zefiro_bar::calendar::{calendar_names, calendar_paint};
use zefiro_bar::controller::{PanelController, PanelEffect, PanelInput, PanelKind, SETTINGS_CHIP};
use zefiro_bar::menu::{MenuAction, MenuModel, NavKey, ThemeSwatch};
use zefiro_bar::settings::{SettingsAction, SettingsModel};
use zefiro_core::config::set_value;
use zefiro_integration_clock::format::ClockText;
use zefiro_integration_clock::grid::{month_grid, ms_to_next_minute, shift_month};
use zefiro_integration_clock::windows::now_local;
use zefiro_platform::geometry::Point;
use zefiro_platform_windows::window::{self, WinEvent};
use zefiro_platform_windows::system;
use zefiro_resources::registry::export_builtin;
use zefiro_ui::paint::DisplayList;
use zefiro_ui::paint::DrawOp;
use zefiro_platform::geometry::{Rect, Size};
use zefiro_platform::text::{Align, Family, FontWeight, TextStyle};

use crate::app::AppCtx;
use crate::bar_view::BarView;
use crate::panel_view::PanelView;

pub const USER_CONFIG_CHANGED: u32 = 2;
pub const USER_THEMES_CHANGED: u32 = 3;
pub const USER_EDITOR_IO: u32 = 6;
/// As sessões de mídia mudaram (ilha embutida).
pub const USER_MEDIA: u32 = 7;
/// Chegou áudio novo para o visualizador (no máximo a 60 Hz).
pub const USER_VISUALIZER: u32 = 8;
/// O volume ou o mudo da saída padrão mudou.
pub const USER_VOLUME: u32 = 9;
/// Câmera ou microfone passaram a ser usados (ou pararam).
pub const USER_PRIVACY: u32 = 10;
/// Chegou uma notificação do Windows.
pub const USER_NOTIFICATION: u32 = 11;
/// "Não perturbe" foi ligado ou desligado.
pub const USER_DND: u32 = 12;
/// Um dispositivo Bluetooth conectou ou desconectou.
pub const USER_BLUETOOTH: u32 = 13;
/// Nova leitura do clima.
pub const USER_WEATHER: u32 = 14;
/// Nova amostra do monitor de hardware.
pub const USER_HARDWARE: u32 = 15;
/// Chegou (ou não) a letra da faixa atual.
pub const USER_LYRICS: u32 = 16;
/// O volume ou o mudo da entrada padrão (microfone) mudou.
pub const USER_MIC: u32 = 17;
/// Nova leitura do brilho dos monitores.
pub const USER_BRIGHTNESS: u32 = 18;
/// O uso do Codex mudou.
pub const USER_CODEX: u32 = 19;
/// O uso do Claude mudou.
pub const USER_CLAUDE: u32 = 20;
/// A lista de projetos do VS Code foi lida.
pub const USER_VSCODE: u32 = 21;
/// Nome e foto da conta (painel Energia) foram lidos.
pub const USER_POWER: u32 = 22;
/// O serviço de atualização mudou de estado (versão nova, baixando, pronto, falhou).
pub const USER_UPDATE: u32 = 23;
/// A thread do Terminal terminou de ler os perfis.
pub const USER_TERMINAL: u32 = 24;
/// O processo do chat mandou sinais (texto chegando, fim do turno, erro).
pub const USER_CHAT: u32 = 25;
/// Uma imagem colada no chat foi convertida (ou miniaturas de imagens foram lidas).
pub const USER_CHAT_IMAGE: u32 = 26;
/// O host da bandeja viu ícones mudarem.
pub const USER_TRAY: u32 = 27;
/// O serviço do Spotify respondeu (login, player, listas, busca, capa).
pub const USER_SPOTIFY: u32 = 28;
pub const TIMER_CLOCK: usize = 1;
pub const TIMER_DISPLAY: usize = 2;
/// Modo de medição (`ZEFIRO_CYCLES=N`): abre e fecha o calendário N vezes e sai.
pub const TIMER_CYCLE: usize = 4;
pub const TIMER_GRAPHICS_TRIM: usize = 5;
/// Fim da espiada da ilha (volta da pílula larga para a recolhida).
pub const TIMER_ISLAND_PEEK: usize = 6;
/// Linha do tempo do painel da ilha, uma vez por segundo.
pub const TIMER_ISLAND_TICK: usize = 7;
/// Timer de foco, uma vez por segundo enquanto roda.
pub const TIMER_FOCUS: usize = 8;
/// Karaokê da aba de letras (15 Hz, só com a aba na tela e tocando).
pub const TIMER_LYRICS: usize = 9;
/// Animação de troca de fonte no painel da ilha (60 Hz por menos de meio segundo).
pub const TIMER_ISLAND_ANIM: usize = 10;
/// Desarma a confirmação do painel Energia depois de 3 s.
pub const TIMER_POWER: usize = 11;
/// Barra escondida por tela cheia: confere a cada 1 s se o app saiu (o aviso do Windows pode chegar
/// antes de a janela encolher, ou não chegar).
pub const TIMER_FULLSCREEN: usize = 12;
/// HUD de debug: uma atualização por segundo, só com ele aberto.
pub const TIMER_HUD: usize = 13;
/// Chat sem uso: encerra o processo do `claude` (10 min).
pub const TIMER_CHAT_IDLE: usize = 14;
/// Ocultar automaticamente: fim da espera no topo (descer) ou fora da barra (subir).
pub const TIMER_AUTOHIDE: usize = 16;
/// Painel do Spotify aberto: progresso da música e fim do aviso do rodapé, uma vez por segundo.
pub const TIMER_SPOTIFY_TICK: usize = 17;
/// Busca do Spotify: 300 ms depois da última tecla.
pub const TIMER_SPOTIFY_SEARCH: usize = 18;
const CYCLE_HALF_MS: u32 = 450;

const VK_ESCAPE: u32 = 0x1B;
const VK_RETURN: u32 = 0x0D;
const VK_LEFT: u32 = 0x25;
const VK_UP: u32 = 0x26;
const VK_RIGHT: u32 = 0x27;
const VK_DOWN: u32 = 0x28;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MouseButton { Left, Middle }

pub struct Loop {
    pub ctx: AppCtx,
    pub bars: Vec<BarView>,
    pub panels: Vec<PanelView>,
    pub controller: PanelController,
    pub editor: Option<crate::editor_view::EditorRuntime>,
    pub editor_worker: Option<crate::config_worker::ConfigWorker>,
    pub editor_pending: Option<usize>,
    pub editor_generation: u64,
    pub editor_started: Option<std::time::Instant>,
    overflow_ids: Vec<String>,
    overflow_focus: usize,
    overflow_scroll: usize,
    menu: Option<MenuModel>,
    settings: Option<SettingsModel>,
    pub(crate) shown_month: (i32, u32),
    pub(crate) island: crate::island_loop::IslandRuntime,
    pub(crate) debug: crate::debug_loop::DebugRuntime,
    pub(crate) update: crate::update_loop::UpdateRuntime,
    pub(crate) devices: crate::devices_loop::DevicesRuntime,
    pub(crate) usage: crate::usage_loop::UsageRuntime,
    pub(crate) vscode: crate::vscode_loop::VsCodeRuntime,
    pub(crate) terminal: crate::terminal_loop::TerminalRuntime,
    pub(crate) systray: crate::systray_loop::SystrayRuntime,
    pub(crate) chat: crate::chat_loop::ChatRuntime,
    pub(crate) power: crate::power_loop::PowerRuntime,
    pub(crate) system: crate::system_loop::SystemRuntime,
    pub(crate) autohide: crate::autohide_loop::AutoHideRuntime,
    pub(crate) spotify: crate::spotify_loop::SpotifyRuntime,
    /// Ciclos de medição: (total, feitos).
    cycles: Option<(u32, u32)>,
}

fn swatches(ctx: &AppCtx) -> Vec<ThemeSwatch> {
    ctx.theme.registry.list().iter().map(|e| ThemeSwatch {
        id: e.theme.id.clone(),
        name: e.theme.name.clone(),
        background: e.theme.colors.background,
        text: e.theme.colors.text,
        border: e.theme.colors.border,
    }).collect()
}

impl Loop {
    pub fn new(ctx: AppCtx, bars: Vec<BarView>) -> Result<Loop> {
        let mut panels = Vec::new();
        for b in &bars {
            panels.push(PanelView::create(&ctx, b.monitor())?);
        }
        let today = now_local().date;
        Ok(Loop { ctx, bars, panels, controller: PanelController::new(), editor: None, editor_worker: None, editor_pending: None, editor_generation: 0, editor_started: None, overflow_ids: Vec::new(), overflow_focus: 0, overflow_scroll: 0, menu: None, settings: None, shown_month: (today.year, today.month), island: Default::default(), debug: Default::default(), update: Default::default(), devices: Default::default(), usage: Default::default(), vscode: Default::default(), terminal: Default::default(), systray: Default::default(), chat: Default::default(), power: Default::default(), system: Default::default(), autohide: Default::default(), spotify: Default::default(), cycles: None })
    }

    pub(crate) fn bar_index(&self, hwnd: isize) -> Option<usize> { self.bars.iter().position(|b| b.hwnd().0 as isize == hwnd) }
    pub(crate) fn panel_index(&self, hwnd: isize) -> Option<usize> { self.panels.iter().position(|p| p.hwnd().0 as isize == hwnd) }

    fn panel_list(&mut self, kind: PanelKind, scale: f32) -> DisplayList {
        match kind {
            PanelKind::Menu => {
                let m = self.menu.get_or_insert_with(|| MenuModel::new(self.ctx.config.bar.height, self.ctx.config.bar.format, &self.ctx.config.theme, swatches(&self.ctx)));
                m.set_update(self.update.label.clone());
                m.set_acrylic(self.ctx.theme.acrylic_wanted(), zefiro_platform_windows::system::transparency_enabled());
                m.paint(scale, &self.ctx.theme.tokens)
            }
            PanelKind::Settings => {
                let hud = self.hud_open();
                let m = self.settings.get_or_insert_with(|| SettingsModel::new(zefiro_platform_windows::system::autostart_enabled(), hud, env!("CARGO_PKG_VERSION")));
                m.hud = hud;
                m.paint(scale, &self.ctx.theme.tokens)
            }
            PanelKind::Calendar => {
                let today = now_local().date;
                let (y, mo) = self.shown_month;
                let grid = month_grid(y, mo, self.ctx.clock.first_weekday(), today);
                calendar_paint(&grid, &calendar_names(&self.ctx.clock, y, mo, today), scale, &self.ctx.theme.tokens)
            }
            PanelKind::Overflow => {
                let count = self.overflow_ids.len().saturating_sub(self.overflow_scroll).min(12);
                let mut list = DisplayList::new(Size { w: 280.0 * scale, h: (48.0 + count as f32 * 32.0) * scale });
                let style = TextStyle { size: 13.0 * scale, weight: FontWeight::Regular, family: Family::Text };
                list.push(DrawOp::Text { rect: Rect::new(16.0 * scale, 4.0 * scale, 248.0 * scale, 28.0 * scale), text: "Mais itens".into(), style, color: self.ctx.theme.tokens.text_secondary, align: Align::Leading });
                for (index, id) in self.overflow_ids.iter().enumerate().skip(self.overflow_scroll).take(count) {
                    let rect = Rect::new(12.0 * scale, (32.0 + (index - self.overflow_scroll) as f32 * 32.0) * scale, 256.0 * scale, 32.0 * scale);
                    if index == self.overflow_focus { list.push(DrawOp::RoundRect { rect, radius: 8.0 * scale, color: self.ctx.theme.tokens.raised_strong }); }
                    list.push(DrawOp::Text { rect, text: zefiro_bar::catalog::name(id).into(), style, color: self.ctx.theme.tokens.text, align: Align::Center });
                }
                list
            }
            PanelKind::Island | PanelKind::Player | PanelKind::Weather | PanelKind::Hardware => self.island_panel_list(scale),
            PanelKind::Audio | PanelKind::Mic | PanelKind::Brightness => self.devices_panel_list(kind, scale),
            PanelKind::Claude | PanelKind::Codex => self.usage_panel_list(kind, scale),
            PanelKind::VsCode => self.vscode_panel_list(scale),
            PanelKind::Terminal => self.terminal_panel_list(scale),
            PanelKind::Chat => self.chat_panel_list(scale),
            PanelKind::Power => self.power_panel_list(scale),
            PanelKind::Tray => self.systray_panel_list(scale),
            PanelKind::Spotify => self.spotify_panel_list(scale),
        }
    }

    pub(crate) fn run_effects(&mut self, fx: Vec<PanelEffect>) -> Result<()> {
        self.run_effects_with_focus(fx, true)
    }

    fn run_effects_with_focus(&mut self, fx: Vec<PanelEffect>, restore_focus: bool) -> Result<()> {
        let opens_another = fx.iter().any(|e| matches!(e, PanelEffect::Open(_)));
        for e in fx {
            match e {
                PanelEffect::Close { monitor } => {
                    tracing::info!(monitor, "painel fechando");
                    self.panels[monitor].close(&self.ctx, !opens_another && restore_focus)?;
                    for b in &mut self.bars {
                        b.set_active(&self.ctx, None)?;
                    }
                    for b in &self.bars {
                        for id in zefiro_bar::catalog::ITEMS.iter().map(|i| i.id) {
                            b.fade_chip(&self.ctx, id, true)?;
                        }
                    }
                    // O mouse saiu do player para o painel sem a barra saber: ele para de espiar.
                    for b in &mut self.bars {
                        if b.set_player_hover(false) {
                            b.refresh_media(&self.ctx, true)?;
                        }
                    }
                    self.menu = None;
                    self.settings = None;
                    self.island_panel_closed();
                    self.devices_panel_closed();
                    self.power_panel_closed();
                    self.terminal_panel_closed();
                    self.systray_panel_closed();
                    self.chat_panel_closed();
                    self.spotify_panel_closed();
                }
                PanelEffect::Open(p) => {
                    if p.kind == PanelKind::Overflow {
                        self.overflow_ids = self.bars[p.monitor].overflow_items(&p.chip);
                        self.overflow_focus = 0;
                        self.overflow_scroll = 0;
                    }
                    tracing::info!(monitor = p.monitor, chip = %p.chip, "painel abrindo");
                    self.devices_prepare(p.kind);
                    self.usage_prepare(p.kind);
                    self.vscode_prepare(p.kind);
                    self.terminal_prepare(p.kind);
                    self.systray_prepare(p.kind);
                    self.chat_prepare(p.kind)?;
                    self.power_prepare(p.kind);
                    self.spotify_prepare(p.kind)?;
                    if p.kind.tabbed() {
                        self.island_panel_opening();
                    }
                    if p.kind == PanelKind::Calendar {
                        let today = now_local().date;
                        self.shown_month = (today.year, today.month);
                    }
                    let scale = self.bars[p.monitor].scale();
                    let list = self.panel_list(p.kind, scale);
                    let chip = self.panel_anchor(p.monitor, &p.chip);
                    let band_bottom = self.bars[p.monitor].band_bottom();
                    self.panels[p.monitor].open(&self.ctx, chip, band_bottom, &list)?;
                    if p.kind.tabbed() {
                        self.island_sync_tabs(p.monitor, false)?;
                    } else {
                        self.panels[p.monitor].clear_tabs()?;
                    }
                    self.bars[p.monitor].set_active(&self.ctx, Some(&p.chip))?;
                    self.bars[p.monitor].fade_chip(&self.ctx, &p.chip, false)?;                }
            }
        }
        Ok(())
    }

    /// Fecha o painel aberto, se houver (como o Esc).
    pub fn close_open_panel(&mut self) -> Result<()> {
        let fx = self.controller.handle(PanelInput::Escape);
        self.run_effects(fx)
    }

    pub fn close_panel_for_editor(&mut self) -> Result<()> {
        let effects = self.controller.handle(PanelInput::Escape);
        self.run_effects_with_focus(effects, false)
    }

    pub(crate) fn redraw_open_panel(&mut self) -> Result<()> {
        let Some(open) = self.controller.open().cloned() else { return Ok(()) };
        let scale = self.bars[open.monitor].scale();
        let list = self.panel_list(open.kind, scale);
        let chip = self.panel_anchor(open.monitor, &open.chip);
        let band_bottom = self.bars[open.monitor].band_bottom();
        self.panels[open.monitor].resize(&self.ctx, chip, band_bottom, &list)?;
        if open.kind.tabbed() {
            self.island_sync_tabs(open.monitor, false)?;
        }
        Ok(())
    }

    /// Clique num chip: abre (ou fecha) o painel dele. Um chip sem o que mostrar (player parado,
    /// clima sem leitura) conta como clique fora.
    fn chip_click(&self, monitor: usize, chip: String) -> PanelInput {
        if self.chip_has_panel(&chip) { PanelInput::ChipClick { monitor, chip } } else { PanelInput::ClickOutside }
    }

    pub(crate) fn panel_anchor(&self, monitor: usize, id: &str) -> Rect {
        // As Configurações abrem no lugar do menu do Z.
        let id = if id == SETTINGS_CHIP { "zefiro" } else { id };
        self.bars[monitor].chip_rect(id).unwrap_or_else(|| {
            let bar = &self.bars[monitor];
            let rect = bar.monitor().rect;
            Rect::new(rect.x + 8.0 * bar.scale(), rect.y, 32.0 * bar.scale(), bar.band_bottom() - rect.y)
        })
    }

    /// Clique num chip que age direto (Notificações, Não perturbe) e botão do meio em qualquer chip.
    /// Cada widget trata o seu caso.
    pub fn chip_action(&mut self, bar: usize, chip: &str, button: MouseButton) -> Result<()> {
        tracing::info!(bar, chip, ?button, "ação de chip");
        match chip {
            "notifications" => self.notifications_action(button),
            "dnd" => self.dnd_action(button),
            _ => Ok(()),
        }
    }

    pub fn apply_bars(&mut self, animate: bool) -> Result<()> {
        for b in &mut self.bars {
            b.apply(&self.ctx, animate)?;
        }
        Ok(())
    }

    fn menu_action(&mut self, a: MenuAction) -> Result<()> {
        let path = self.ctx.config_path.clone();
        match a {
            MenuAction::SetHeight(h) => {
                self.ctx.config.bar.height = h;
                let _ = set_value(&path, &["bar", "height"], h.as_str());
            }
            MenuAction::SetFormat(f) => {
                self.ctx.config.bar.format = f;
                let _ = set_value(&path, &["bar", "format"], f.as_str());
            }
            MenuAction::SetTheme(id) => {
                self.ctx.config.theme = id.clone();
                let _ = set_value(&path, &["theme"], &id);
                let dir = self.ctx.themes_dir.clone();
                self.ctx.theme.reload(&id, &dir);
                self.ctx.text.set_family(&self.ctx.theme.theme.font.family);
            }
            MenuAction::ReloadTheme => {
                let (id, dir) = (self.ctx.config.theme.clone(), self.ctx.themes_dir.clone());
                for e in self.ctx.theme.reload(&id, &dir) {
                    tracing::warn!(source = %e.source, line = ?e.line, "tema com erro: {}", e.message);
                }
            }
            MenuAction::OpenThemesFolder => {
                let dir = self.ctx.themes_dir.clone();
                if export_builtin(&self.ctx.config.theme, &dir).is_err() {
                    let _ = std::fs::create_dir_all(&dir);
                }
                system::open_folder(&dir);
                return Ok(());
            }
            MenuAction::OpenThemes | MenuAction::BackToMenu => {
                // A mesma janela troca de conteúdo; o painel acompanha o tamanho novo com a mola.
                if let Some(m) = &mut self.menu {
                    if a == MenuAction::OpenThemes { m.open_themes() } else { m.back() }
                }
                return self.redraw_open_panel();
            }
            MenuAction::InstallUpdate => return self.install_update(),
            MenuAction::OpenSettings => {
                let monitor = self.controller.open().map_or(0, |p| p.monitor);
                let fx = self.controller.handle(PanelInput::ChipClick { monitor, chip: SETTINGS_CHIP.into() });
                return self.run_effects(fx);
            }
            MenuAction::ToggleAcrylic => {
                // Sem "Efeitos de transparência" no Windows não há acrílico para ligar.
                if !zefiro_platform_windows::system::transparency_enabled() {
                    return Ok(());
                }
                let on = !self.ctx.theme.acrylic_wanted();
                self.ctx.config.acrylic = Some(on);
                self.ctx.theme.acrylic = Some(on);
                let _ = set_value(&path, &["acrylic"], on);
            }
            MenuAction::Exit => {
                // SAFETY: encerra o laço de mensagens desta thread.
                unsafe { PostQuitMessage(0) };
                return Ok(());
            }
            MenuAction::EditBar => {
                let monitor = self.controller.open().map_or(0, |p| p.monitor);
                return self.start_editor(monitor);
            }
        }
        // Altura/formato/tema/acrílico: barras animam e o menu continua aberto, na mesma tela, com a nova escolha.
        let themes = swatches(&self.ctx);
        if let Some(m) = &mut self.menu {
            m.set_choices(self.ctx.config.bar.height, self.ctx.config.bar.format, &self.ctx.config.theme);
            m.set_themes(themes);
        }
        self.apply_bars(true)?;
        self.redraw_open_panel()
    }

    /// Item das Configurações: vale na hora; as chaves continuam no painel aberto, na posição nova.
    fn settings_action(&mut self, a: SettingsAction) -> Result<()> {
        match a {
            // O menu do Z abre no lugar, como o "‹ Tema" volta da tela de temas.
            SettingsAction::Back => {
                let monitor = self.controller.open().map_or(0, |p| p.monitor);
                let fx = self.controller.handle(PanelInput::ChipClick { monitor, chip: "zefiro".into() });
                return self.run_effects(fx);
            }
            SettingsAction::ToggleAutostart => {
                crate::tray_loop::toggle_autostart();
                if let Some(m) = &mut self.settings {
                    m.autostart = zefiro_platform_windows::system::autostart_enabled();
                }
            }
            SettingsAction::ToggleHud => self.toggle_hud()?,
            SettingsAction::ExportDiagnostics => {
                self.close_open_panel()?;
                return self.export_diagnostics();
            }
        }
        self.redraw_open_panel()
    }

    /// Um passo do modo de medição: fecha o calendário aberto, ou abre o próximo e loga quanto
    /// tempo levou do "clique" até o painel estar pronto para o primeiro quadro.
    fn cycle_tick(&mut self) -> Result<()> {
        let Some((total, done)) = self.cycles else { return Ok(()) };
        window::set_timer(TIMER_CYCLE, CYCLE_HALF_MS);
        if self.controller.open().is_some() {
            let fx = self.controller.handle(PanelInput::Escape);
            return self.run_effects(fx);
        }
        if done >= total {
            window::kill_timer(TIMER_CYCLE);
            tracing::info!(ciclos = total, "medição concluída; saindo");
            // SAFETY: encerra o laço de mensagens desta thread.
            unsafe { PostQuitMessage(0) };
            return Ok(());
        }
        let started = std::time::Instant::now();
        let fx = self.controller.handle(PanelInput::ChipClick { monitor: 0, chip: "clock".into() });
        self.run_effects(fx)?;
        let ms = started.elapsed().as_secs_f64() * 1000.0;
        self.cycles = Some((total, done + 1));
        tracing::info!("ciclo {}: clique→primeiro quadro {ms:.2} ms", done + 1);
        Ok(())
    }

    pub fn schedule_clock(&self) {
        let now = now_local();
        let ms = if self.ctx.config.clock.seconds { 1000 - now.millis } else { ms_to_next_minute(now.second, now.millis) };
        window::set_timer(TIMER_CLOCK, ms.max(16));
    }

    pub fn handle(&mut self, e: WinEvent) -> Result<bool> {
        if let WinEvent::User { code } = e {
            self.debug_stamp(code);
        }
        if self.editor_event(e)? { return Ok(true); }
        match e {
            WinEvent::Quit => return Ok(false),
            // Alt+F4 num painel fecha só o painel; na barra (ou `taskkill` sem /f), encerra o app
            // pelo caminho normal, que tira a AppBar da tela.
            WinEvent::Close { hwnd } => {
                if self.panel_index(hwnd).is_some() {
                    let fx = self.controller.handle(PanelInput::Escape);
                    self.run_effects(fx)?;
                } else {
                    return Ok(false);
                }
            }
            WinEvent::LeftUp { hwnd, x, y } => {
                if let Some(i) = self.bar_index(hwnd) {
                    if let Some(chip) = self.bars[i].chip_at(x, y).filter(|c| zefiro_bar::controller::action_chip(c)) {
                        self.close_open_panel()?;
                        return self.chip_action(i, &chip, MouseButton::Left).map(|_| true);
                    }
                    let input = match self.bars[i].chip_at(x, y) {
                        Some(chip) => self.chip_click(i, chip),
                        // Faixa vazia: é "clicar fora" do painel aberto (a barra não rouba o foco,
                        // então nenhum FocusLost chegaria).
                        None => PanelInput::ClickOutside,
                    };
                    let fx = self.controller.handle(input);
                    self.run_effects(fx)?;
                } else if let Some(i) = self.panel_index(hwnd) {
                    let (ox, oy) = self.panels[i].content_origin();
                    let scale = self.bars[i].scale();
                    let hit = self.menu.as_ref().and_then(|m| m.hit(Point { x: x - ox, y: y - oy }, scale));
                    let setting = self.settings.as_ref().and_then(|m| m.hit(Point { x: x - ox, y: y - oy }, scale));
                    if let Some(a) = setting {
                        self.settings_action(a)?;
                    } else if self.controller.open().is_some_and(|o| o.kind.tabbed()) {
                        self.island_panel_click(Point { x: x - ox, y: y - oy })?;
                    } else if self.power_click(Point { x: x - ox, y: y - oy })? {
                        // Botão de energia (armar, confirmar ou agir).
                    } else if self.chat_click(Point { x: x - ox, y: y - oy })? {
                        // Chat: enviar, parar, conversa nova ou copiar.
                    } else if self.systray_up(Point { x: x - ox, y: y - oy })? {
                        // Ícone da bandeja: o app abre (o painel já fechou).
                    } else if self.terminal_up(Point { x: x - ox, y: y - oy })? {
                        // Perfil do Windows Terminal aberto (ou clique fora das linhas).
                    } else if self.usage_click(Point { x: x - ox, y: y - oy }, scale)? {
                        // Botão de atualizar do painel do Claude.
                    } else if self.spotify_click(Point { x: x - ox, y: y - oy }, false)? {
                        // Spotify: linha, filtro, menu de ações ou botão do cartão.
                    } else if self.vscode_click(Point { x: x - ox, y: y - oy })? {
                        // Projeto do VS Code aberto (ou clique fora das linhas).
                    } else if self.devices_up(Point { x: x - ox, y: y - oy })? {
                        // Som, microfone ou brilho: arraste terminado, mudo ou troca de dispositivo.
                    } else if let Some(a) = hit {
                        self.menu_action(a)?;
                    } else if self.controller.open().is_some_and(|o| o.kind == PanelKind::Overflow) {
                        let row = ((y - oy) / scale - 32.0) / 32.0;
                        if row >= 0.0 && let Some(id) = self.overflow_ids.get(self.overflow_scroll + row as usize).cloned() {
                            let input = self.chip_click(i, id);
                            let fx = self.controller.handle(input);
                            self.run_effects(fx)?;
                        }
                    }
                }
            }
            WinEvent::MiddleUp { hwnd, x, y } => {
                if let Some(i) = self.bar_index(hwnd)
                    && let Some(chip) = self.bars[i].chip_at(x, y)
                {
                    self.chip_action(i, &chip, MouseButton::Middle)?;
                }
            }
            WinEvent::RightUp { hwnd, x, y } => {
                tracing::debug!(hwnd, x, y, "botão direito");
                if let Some(i) = self.panel_index(hwnd) {
                    let (ox, oy) = self.panels[i].content_origin();
                    if self.systray_right_up(Point { x: x - ox, y: y - oy })? || self.spotify_click(Point { x: x - ox, y: y - oy }, true)? {
                        return Ok(true);
                    }
                }
                if let Some(i) = self.bar_index(hwnd) {
                    if self.bars[i].chip_at(x, y).as_deref() == Some(crate::bar_view::ISLAND_CHIP) {
                        self.close_open_panel()?;
                        return self.island_menu(i).map(|_| true);
                    }
                    let already = self.controller.open().is_some_and(|o| o.chip == "zefiro" && o.monitor == i);
                    if !already {
                        let fx = self.controller.handle(PanelInput::ChipClick { monitor: i, chip: "zefiro".into() });
                        self.run_effects(fx)?;
                    }
                }
            }
            WinEvent::LeftDown { hwnd, x, y } => {
                if let Some(i) = self.panel_index(hwnd) {
                    let (ox, oy) = self.panels[i].content_origin();
                    if !self.terminal_down(i, Point { x: x - ox, y: y - oy })? {
                        self.devices_down(i, Point { x: x - ox, y: y - oy })?;
                    }
                }
            }
            WinEvent::MouseMove { hwnd, x, y } => {
                if let Some(i) = self.panel_index(hwnd) {
                    let (ox, oy) = self.panels[i].content_origin();
                    self.devices_move(Point { x: x - ox, y: y - oy })?;
                    self.terminal_move(Point { x: x - ox, y: y - oy })?;
                    self.systray_move(Point { x: x - ox, y: y - oy })?;
                    self.chat_move(Point { x: x - ox, y: y - oy })?;
                    self.vscode_move(Point { x: x - ox, y: y - oy })?;
                    self.spotify_move(Point { x: x - ox, y: y - oy })?;
                    // Menu do Z: o item sob o mouse ganha o destaque (só redesenha quando muda).
                    let scale = self.bars[i].scale();
                    self.usage_move(Point { x: x - ox, y: y - oy }, scale)?;
                    let p = Point { x: x - ox, y: y - oy };
                    let lit = match self.effective_open_kind() {
                        Some(PanelKind::Menu) => self.menu.as_mut().is_some_and(|m| m.hover(p, scale)),
                        Some(PanelKind::Settings) => self.settings.as_mut().is_some_and(|m| m.hover(p, scale)),
                        _ => false,
                    };
                    if lit {
                        self.redraw_open_panel()?;
                    }
                }
                if let Some(i) = self.bar_index(hwnd) {
                    let chip = self.bars[i].chip_at(x, y);
                    let on_island = chip.as_deref() == Some(crate::bar_view::ISLAND_CHIP);
                    let on_player = chip.as_deref() == Some(crate::bar_view::PLAYER_CHIP);
                    // Ilha e player espiam em vez de ganhar o fundo de hover.
                    self.bars[i].set_hover(&self.ctx, chip.as_deref().filter(|_| !on_island && !on_player))?;
                    if self.bars[i].has_island() {
                        self.island_hover(on_island)?;
                    }
                    self.player_hover(i, on_player)?;
                }
            }
            WinEvent::MouseLeave { hwnd } => {
                if self.panel_index(hwnd).is_some() {
                    self.terminal_leave()?;
                    self.systray_leave()?;
                    self.chat_leave()?;
                    self.usage_leave()?;
                }
                if let Some(i) = self.bar_index(hwnd) {
                    self.bars[i].set_hover(&self.ctx, None)?;
                    if self.bars[i].has_island() {
                        self.island_hover(false)?;
                    }
                    self.player_hover(i, false)?;
                }
            }
            WinEvent::FocusLost { hwnd } => {
                // Um FocusLost atrasado (de um painel que acabou de fechar) não fecha o painel que
                // abriu no lugar dele e já tem o teclado.
                let focused = self.panels.iter().any(|p| p.open && p.has_focus());
                // O menu de um ícone da bandeja tira o foco do painel, que fica atrás dele.
                if self.panel_index(hwnd).is_some() && self.controller.open().is_some() && !focused && !self.systray.holds_panel() {
                    let fx = self.controller.handle(PanelInput::FocusLost);
                    self.run_effects(fx)?;
                }
            }
            WinEvent::Key { hwnd, vk } => {
                if self.panel_index(hwnd).is_none() {
                    return Ok(true);
                }
                if self.chat_key(vk)? || self.terminal_key(vk)? || self.systray_key(vk)? || self.spotify_key(vk)? || self.vscode_key(vk)? {
                    return Ok(true);
                }
                // Esc na tela de temas volta ao menu; no menu, fecha.
                if vk == VK_ESCAPE && self.effective_open_kind() == Some(PanelKind::Menu) && self.menu.as_ref().is_some_and(|m| m.page() == zefiro_bar::menu::MenuPage::Themes) {
                    return self.menu_action(MenuAction::BackToMenu).map(|_| true);
                }
                // Esc nas Configurações também volta ao menu.
                if vk == VK_ESCAPE && self.effective_open_kind() == Some(PanelKind::Settings) {
                    return self.settings_action(SettingsAction::Back).map(|_| true);
                }
                if vk == VK_ESCAPE {
                    let fx = self.controller.handle(PanelInput::Escape);
                    return self.run_effects(fx).map(|_| true);
                }
                // Ilha: as abas ficam lado a lado (os pontos no rodapé), então ← / → trocam de aba.
                // ↑ / ↓ fazem o mesmo, como na ilha C++, onde as telas eram empilhadas.
                if self.effective_open_kind().is_some_and(PanelKind::tabbed) && matches!(vk, VK_LEFT | VK_RIGHT | VK_UP | VK_DOWN) {
                    let delta = if vk == VK_LEFT || vk == VK_UP { 120 } else { -120 };
                    return self.island_wheel(delta).map(|_| true);
                }
                match self.effective_open_kind() {
                    Some(PanelKind::Menu) => {
                        let key = match vk { VK_UP => Some(NavKey::Up), VK_DOWN => Some(NavKey::Down), VK_LEFT => Some(NavKey::Left), VK_RIGHT => Some(NavKey::Right), VK_RETURN => Some(NavKey::Enter), _ => None };
                        if let (Some(k), Some(m)) = (key, self.menu.as_mut()) {
                            match m.key(k) {
                                Some(a) => self.menu_action(a)?,
                                None => self.redraw_open_panel()?,
                            }
                        }
                    }
                    Some(PanelKind::Settings) => {
                        let key = match vk { VK_UP => Some(Some(false)), VK_DOWN => Some(Some(true)), VK_RETURN => Some(None), _ => None };
                        if let (Some(k), Some(m)) = (key, self.settings.as_mut()) {
                            match m.key(k) {
                                Some(a) => self.settings_action(a)?,
                                None => self.redraw_open_panel()?,
                            }
                        }
                    }
                    Some(PanelKind::Calendar) if vk == VK_LEFT || vk == VK_RIGHT => {
                        let (y, m) = self.shown_month;
                        self.shown_month = shift_month(y, m, if vk == VK_LEFT { -1 } else { 1 });
                        self.redraw_open_panel()?;
                    }
                    Some(PanelKind::Overflow) => {
                        let monitor = self.controller.open().map_or(0, |p| p.monitor);
                        if vk == VK_RETURN && let Some(id) = self.overflow_ids.get(self.overflow_focus).cloned() {
                            let input = self.chip_click(monitor, id);
                            let fx = self.controller.handle(input);
                            self.run_effects(fx)?;
                        } else if vk == VK_UP || vk == VK_DOWN {
                            self.overflow_focus = if vk == VK_UP { self.overflow_focus.saturating_sub(1) } else { (self.overflow_focus + 1).min(self.overflow_ids.len().saturating_sub(1)) };
                            self.overflow_scroll = self.overflow_focus.saturating_sub(11);
                            self.redraw_open_panel()?;
                        }
                    }
                    _ => {}
                }
            }
            WinEvent::Wheel { hwnd, delta } => {
                if let Some(i) = self.bar_index(hwnd)
                    && self.devices_wheel(i, delta)?
                {
                    return Ok(true);
                }
                if self.panel_index(hwnd).is_some() && self.controller.open().is_some_and(|o| o.kind.tabbed()) {
                    return self.island_wheel(delta).map(|_| true);
                }
                if self.panel_index(hwnd).is_some() && (self.chat_wheel(delta)? || self.terminal_wheel(delta)? || self.systray_wheel(delta)? || self.spotify_wheel(delta)? || self.vscode_wheel(delta)?) {
                    return Ok(true);
                }
                if self.panel_index(hwnd).is_some() && self.controller.open().is_some_and(|o| o.kind == PanelKind::Overflow) {
                    self.overflow_scroll = if delta > 0 { self.overflow_scroll.saturating_sub(1) } else { (self.overflow_scroll + 1).min(self.overflow_ids.len().saturating_sub(1)) };
                    self.redraw_open_panel()?;
                }
                if self.panel_index(hwnd).is_some() && self.effective_open_kind() == Some(PanelKind::Calendar) {
                    let (y, m) = self.shown_month;
                    self.shown_month = shift_month(y, m, if delta > 0 { -1 } else { 1 });
                    self.redraw_open_panel()?;
                }
            }
            WinEvent::Timer { id: TIMER_CLOCK } => {
                self.apply_bars(true)?;
                if self.effective_open_kind() == Some(PanelKind::Calendar) {
                    self.redraw_open_panel()?;
                }
                self.schedule_clock();
            }
            WinEvent::Timer { id: TIMER_CYCLE } => self.cycle_tick()?,
            WinEvent::Timer { id: TIMER_HUD } => self.hud_tick()?,
            WinEvent::User { code: USER_UPDATE } => self.update_changed()?,
            WinEvent::Hotkey { id: zefiro_platform_windows::diagnostics::HOTKEY_HUD } => self.toggle_hud()?,
            WinEvent::Timer { id: TIMER_ISLAND_PEEK } => self.island_peek_timer()?,
            WinEvent::Timer { id: TIMER_ISLAND_TICK } => self.island_tick()?,
            WinEvent::Timer { id: TIMER_FOCUS } => self.island_focus_tick()?,
            WinEvent::User { code: USER_MEDIA } => self.island_media_changed()?,
            WinEvent::User { code: USER_VISUALIZER } => self.island_visualizer()?,
            WinEvent::User { code: USER_VOLUME } => self.devices_volume_changed(zefiro_platform_windows::audio_devices::Flow::Output)?,
            WinEvent::User { code: USER_MIC } => self.devices_volume_changed(zefiro_platform_windows::audio_devices::Flow::Input)?,
            WinEvent::User { code: USER_BRIGHTNESS } => self.devices_brightness_changed()?,
            WinEvent::User { code: USER_CODEX } => self.usage_codex_changed()?,
            WinEvent::User { code: USER_CLAUDE } => self.usage_claude_changed()?,
            WinEvent::User { code: USER_VSCODE } => self.vscode_loaded()?,
            WinEvent::User { code: USER_TERMINAL } => self.terminal_loaded()?,
            WinEvent::User { code: USER_TRAY } => self.systray_changed()?,
            WinEvent::MenuEnded => self.systray_menu_signal(crate::systray_loop::MenuSignal::Ended)?,
            WinEvent::User { code: USER_CHAT } => self.chat_signals()?,
            WinEvent::User { code: USER_CHAT_IMAGE } => self.chat_images()?,
            WinEvent::Timer { id: TIMER_CHAT_IDLE } => self.chat_idle(),
            WinEvent::User { code: USER_SPOTIFY } => self.spotify_replies()?,
            WinEvent::Timer { id: TIMER_SPOTIFY_TICK } => self.spotify_tick()?,
            WinEvent::Timer { id: TIMER_SPOTIFY_SEARCH } => self.spotify_search_timer(),
            WinEvent::Tray { mouse } => self.tray_event(mouse)?,
            WinEvent::Character { code, .. } => {
                if !self.chat_char(code)? && !self.spotify_char(code)? {
                    self.vscode_char(code)?;
                }
            }
            WinEvent::User { code: USER_POWER } => self.power_user_loaded()?,
            WinEvent::Timer { id: TIMER_POWER } => self.power_timer()?,
            WinEvent::User { code: USER_PRIVACY } => self.island_privacy_changed()?,
            WinEvent::User { code: USER_NOTIFICATION } => {
                self.island_notifications()?;
                let count = self.notification_count();
                if count != self.ctx.system.notifications {
                    self.ctx.system.notifications = count;
                    self.apply_bars(true)?;
                }
            }
            WinEvent::User { code: USER_DND } => self.dnd_changed()?,
            WinEvent::User { code: USER_BLUETOOTH } => self.island_bluetooth()?,
            WinEvent::User { code: USER_WEATHER } => self.island_weather()?,
            WinEvent::User { code: USER_HARDWARE } => self.island_hardware()?,
            WinEvent::User { code: USER_LYRICS } => self.island_lyrics()?,
            WinEvent::Timer { id: TIMER_LYRICS } => self.island_lyrics_tick()?,
            WinEvent::Timer { id: TIMER_ISLAND_ANIM } => self.island_anim_tick()?,
            WinEvent::DeviceChange { arrived, volume } => self.island_device(arrived, volume)?,
            WinEvent::ClipboardChanged => self.island_clipboard()?,
            WinEvent::LockKey { caps } => self.island_lock_key(caps)?,
            WinEvent::Timer { id: TIMER_GRAPHICS_TRIM } => {
                window::kill_timer(TIMER_GRAPHICS_TRIM);
                if self.controller.open().is_none() && self.editor.is_none() { self.ctx.gfx.trim_idle_resources(); }
            }
            WinEvent::PanelSettled { hwnd, ticket } => {
                if let Some(index) = self.panel_index(hwnd) { self.panels[index].hide_if_closed(&self.ctx, ticket)?; }
            }
            WinEvent::User { code: USER_CONFIG_CHANGED } => {
                let text = std::fs::read_to_string(&self.ctx.config_path).unwrap_or_default();
                match zefiro_core::config::parse_config(&text) {
                    // Só `[terminal]`, `[chat]` ou `[spotify]` mudaram (o próprio painel gravou): atualiza sem
                    // fechar o painel.
                    Ok(new) if new != self.ctx.config && new.same_except_panels(&self.ctx.config) => {
                        self.ctx.config = new;
                        self.terminal_config_changed()?;
                        self.chat_config_changed()?;
                        self.spotify_services();
                    }
                    Ok(new) if new != self.ctx.config => {
                        let fx = self.controller.handle(PanelInput::ConfigChanged);
                        self.run_effects(fx)?;
                        let theme_changed = new.theme != self.ctx.config.theme;
                        self.ctx.theme.acrylic = new.acrylic;
                        self.ctx.config = new;
                        if theme_changed {
                            let (id, dir) = (self.ctx.config.theme.clone(), self.ctx.themes_dir.clone());
                            self.ctx.theme.reload(&id, &dir);
                            self.ctx.text.set_family(&self.ctx.theme.theme.font.family);
                        }
                        self.apply_bars(true)?;
                        self.refresh_services();
                        self.schedule_clock();
                        self.clean_bar_sync()?;
                    }
                    Ok(_) => {}
                    Err(e) => tracing::warn!(line = ?e.line, "config.toml inválido: {}; mantendo a configuração anterior", e.message),
                }
            }
            WinEvent::User { code: USER_THEMES_CHANGED } => {
                let (id, dir) = (self.ctx.config.theme.clone(), self.ctx.themes_dir.clone());
                for e in self.ctx.theme.reload(&id, &dir) {
                    tracing::warn!(source = %e.source, line = ?e.line, "tema com erro: {}; mantendo a versão anterior", e.message);
                }
                self.ctx.text.set_family(&self.ctx.theme.theme.font.family);
                self.apply_bars(true)?;
                self.menu = None;
                self.redraw_open_panel()?;
            }
            _ => return crate::robustness::handle(self, e),
        }
        Ok(true)
    }
}

pub fn run(ctx: AppCtx, bars: Vec<BarView>) -> Result<()> {
    let _context_hook = zefiro_platform_windows::bar_context::BarContextHook::install().map_err(|error| tracing::warn!("recuperação pelo botão direito indisponível: {error}")).ok();
    let mut l = Loop::new(ctx, bars)?;
    // Vive até o fim do laço; soltar desinstala o gancho.
    let foreground_hook = window::ForegroundHook::install();
    if foreground_hook.is_none() {
        tracing::warn!("gancho de foco indisponível: a barra só sai da frente da tela cheia pelo aviso do Windows");
    }
    l.schedule_clock();
    if zefiro_core::config::removed_island_keys(&std::fs::read_to_string(&l.ctx.config_path).unwrap_or_default()) {
        tracing::warn!("a ilha externa foi removida; usando a ilha embutida (as chaves engine e exe de [island] são ignoradas)");
    }
    if let Some(n) = std::env::var("ZEFIRO_CYCLES").ok().and_then(|v| v.parse::<u32>().ok()) {
        tracing::info!(ciclos = n, "modo de medição: abrindo e fechando o calendário");
        l.cycles = Some((n, 0));
        window::set_timer(TIMER_CYCLE, 2000);
    }
    crate::tray_loop::show_icon();
    if !zefiro_platform_windows::diagnostics::register_hud_hotkey() {
        tracing::warn!("Ctrl+Alt+Shift+D já é usado por outro app; o HUD fica só no menu do Z");
    }
    l.refresh_services();
    l.devices_start();
    if let Err(e) = l.autohide_sync() {
        tracing::warn!(error = %e, "não consegui preparar o ocultar automaticamente");
    }
    'pump: while window::pump() {
        for e in window::take_events() {
            match l.handle(e) {
                Ok(true) => {}
                Ok(false) => break 'pump,
                Err(err) => tracing::error!(error = %err, ?e, "erro tratando evento"),
            }
            // Cursor, painéis e tela cheia mudam por muitos caminhos: a regra roda depois de cada evento.
            if let Err(err) = l.autohide_update() {
                tracing::error!(error = %err, "erro no ocultar automaticamente");
            }
        }
    }
    shutdown(&mut l);
    crate::power_loop::run_pending(&mut l);
    Ok(())
}

/// Saída por qualquer caminho: as AppBars saem da tela primeiro (a área reservada volta na hora).
fn shutdown(l: &mut Loop) {
    l.debug_shutdown();
    l.chat_shutdown();
    for b in l.bars.drain(..) {
        b.destroy();
    }
    for p in l.panels.drain(..) {
        p.destroy();
    }
    zefiro_platform_windows::tray::remove();
}

