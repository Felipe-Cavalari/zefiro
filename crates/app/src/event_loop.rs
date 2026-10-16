//! O laço de eventos: entradas → controlador → visões.

use windows::Win32::UI::WindowsAndMessaging::PostQuitMessage;
use windows::core::Result;
use zefiro_bar::calendar::{calendar_names, calendar_paint};
use zefiro_bar::controller::{PanelController, PanelEffect, PanelInput, PanelKind};
use zefiro_bar::menu::{MenuAction, MenuModel, NavKey, ThemeSwatch};
use zefiro_core::config::set_value;
use zefiro_integration_clock::format::ClockText;
use zefiro_integration_clock::grid::{month_grid, ms_to_next_minute, shift_month};
use zefiro_integration_clock::windows::now_local;
use zefiro_platform::geometry::Point;
use zefiro_platform_windows::window::{self, WinEvent};
use zefiro_platform_windows::{island_zorder, system};
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
pub const USER_ISLAND: u32 = 4;
/// O `island-state.ini` mudou: a ilha pode ter publicado outro destaque.
pub const USER_ISLAND_ACCENT: u32 = 5;
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
pub const TIMER_CLOCK: usize = 1;
pub const TIMER_DISPLAY: usize = 2;
pub const TIMER_RAISE: usize = 3;
/// Modo de medição (`ZEFIRO_CYCLES=N`): abre e fecha o calendário N vezes e sai.
pub const TIMER_CYCLE: usize = 4;
pub const TIMER_GRAPHICS_TRIM: usize = 5;
/// Fim da espiada da ilha (volta da pílula larga para a recolhida).
pub const TIMER_ISLAND_PEEK: usize = 6;
/// Linha do tempo do painel da ilha, uma vez por segundo.
pub const TIMER_ISLAND_TICK: usize = 7;
/// Timer de foco, uma vez por segundo enquanto roda.
pub const TIMER_FOCUS: usize = 8;
const CYCLE_HALF_MS: u32 = 450;

const VK_ESCAPE: u32 = 0x1B;
const VK_RETURN: u32 = 0x0D;
const VK_LEFT: u32 = 0x25;
const VK_UP: u32 = 0x26;
const VK_RIGHT: u32 = 0x27;
const VK_DOWN: u32 = 0x28;

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
    pub(crate) shown_month: (i32, u32),
    pub(crate) island: crate::island_loop::IslandRuntime,
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
        Ok(Loop { ctx, bars, panels, controller: PanelController::new(), editor: None, editor_worker: None, editor_pending: None, editor_generation: 0, editor_started: None, overflow_ids: Vec::new(), overflow_focus: 0, overflow_scroll: 0, menu: None, shown_month: (today.year, today.month), island: Default::default(), cycles: None })
    }

    pub(crate) fn bar_index(&self, hwnd: isize) -> Option<usize> { self.bars.iter().position(|b| b.hwnd().0 as isize == hwnd) }
    pub(crate) fn panel_index(&self, hwnd: isize) -> Option<usize> { self.panels.iter().position(|p| p.hwnd().0 as isize == hwnd) }

    fn panel_list(&mut self, kind: PanelKind, scale: f32) -> DisplayList {
        match kind {
            PanelKind::Menu => {
                let m = self.menu.get_or_insert_with(|| MenuModel::new(self.ctx.config.bar.height, self.ctx.config.bar.format, &self.ctx.config.theme, swatches(&self.ctx)));
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
            PanelKind::Island => self.island_panel_list(scale),
        }
    }

    fn run_effects(&mut self, fx: Vec<PanelEffect>) -> Result<()> {
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
                        for id in ["zefiro", "clock", crate::bar_view::ISLAND_CHIP] {
                            b.fade_chip(&self.ctx, id, true)?;
                        }
                    }
                    self.menu = None;
                    self.island_panel_closed();
                }
                PanelEffect::Open(p) => {
                    if p.kind == PanelKind::Overflow {
                        self.overflow_ids = self.bars[p.monitor].overflow_items(&p.chip);
                        self.overflow_focus = 0;
                        self.overflow_scroll = 0;
                    }
                    tracing::info!(monitor = p.monitor, chip = %p.chip, "painel abrindo");
                    if matches!(p.kind, PanelKind::Calendar | PanelKind::Island) {
                        let today = now_local().date;
                        self.shown_month = (today.year, today.month);
                    }
                    let scale = self.bars[p.monitor].scale();
                    let list = self.panel_list(p.kind, scale);
                    let chip = self.panel_anchor(p.monitor, &p.chip);
                    let band_bottom = self.bars[p.monitor].band_bottom();
                    self.panels[p.monitor].open(&self.ctx, chip, band_bottom, &list)?;
                    self.bars[p.monitor].set_active(&self.ctx, Some(&p.chip))?;
                    self.bars[p.monitor].fade_chip(&self.ctx, &p.chip, false)?;
                }
            }
        }
        Ok(())
    }

    /// Lê o destaque que a ilha publicou (capa do álbum ou o dela) e, se mudou, redesenha a barra
    /// e o painel aberto — o calendário usa o mesmo destaque da ilha.
    pub fn refresh_island_accent(&mut self) -> Result<()> {
        // A ilha embutida publica o destaque direto (island_loop), sem passar pelo .ini.
        if self.ctx.config.island.embedded() {
            return Ok(());
        }
        let accent = if self.ctx.config.island.external() {
            let path = self.ctx.config_path.with_file_name("island-state.ini");
            std::fs::read_to_string(path).ok().and_then(|t| zefiro_island_host::ini::read_accent(&t))
        } else {
            None
        };
        if self.ctx.theme.set_cover_accent(accent) {
            self.apply_bars(true)?;
            self.redraw_open_panel()?;
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

    fn redraw_open_panel(&mut self) -> Result<()> {
        let Some(open) = self.controller.open().cloned() else { return Ok(()) };
        let scale = self.bars[open.monitor].scale();
        let list = self.panel_list(open.kind, scale);
        let chip = self.panel_anchor(open.monitor, &open.chip);
        let band_bottom = self.bars[open.monitor].band_bottom();
        self.panels[open.monitor].resize(&self.ctx, chip, band_bottom, &list)
    }

    fn panel_anchor(&self, monitor: usize, id: &str) -> Rect {
        self.bars[monitor].chip_rect(id).unwrap_or_else(|| {
            let bar = &self.bars[monitor];
            let rect = bar.monitor().rect;
            Rect::new(rect.x + 8.0 * bar.scale(), rect.y, 32.0 * bar.scale(), bar.band_bottom() - rect.y)
        })
    }

    pub fn apply_bars(&mut self, animate: bool) -> Result<()> {
        for b in &mut self.bars {
            b.apply(&self.ctx, animate)?;
        }
        island_zorder::raise_island();
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
            MenuAction::RestartIsland => {
                window::post_user(USER_ISLAND);
                return Ok(());
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
        // Altura/formato/tema: barras animam e o menu continua aberto, redesenhado com a nova escolha.
        self.menu = None;
        self.apply_bars(true)?;
        crate::island::sync(&self.ctx);
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
                    let input = match self.bars[i].chip_at(x, y) {
                        Some(chip) => PanelInput::ChipClick { monitor: i, chip },
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
                    if self.controller.open().is_some_and(|o| o.kind == PanelKind::Island) {
                        self.island_panel_click(Point { x: x - ox, y: y - oy })?;
                    } else if let Some(a) = hit {
                        self.menu_action(a)?;
                    } else if self.controller.open().is_some_and(|o| o.kind == PanelKind::Overflow) {
                        let row = ((y - oy) / scale - 32.0) / 32.0;
                        if row >= 0.0 && let Some(id) = self.overflow_ids.get(self.overflow_scroll + row as usize).cloned() {
                            let fx = self.controller.handle(PanelInput::ChipClick { monitor: i, chip: id });
                            self.run_effects(fx)?;
                        }
                    }
                }
            }
            WinEvent::RightUp { hwnd, x, y } => {
                tracing::debug!(hwnd, x, y, "botão direito");
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
            WinEvent::MouseMove { hwnd, x, y } => {
                if let Some(i) = self.bar_index(hwnd) {
                    let chip = self.bars[i].chip_at(x, y);
                    let on_island = chip.as_deref() == Some(crate::bar_view::ISLAND_CHIP);
                    self.bars[i].set_hover(&self.ctx, chip.as_deref().filter(|_| !on_island))?;
                    if self.bars[i].has_island() {
                        self.island_hover(on_island)?;
                    }
                }
            }
            WinEvent::MouseLeave { hwnd } => {
                if let Some(i) = self.bar_index(hwnd) {
                    self.bars[i].set_hover(&self.ctx, None)?;
                    if self.bars[i].has_island() {
                        self.island_hover(false)?;
                    }
                }
            }
            WinEvent::FocusLost { hwnd } => {
                // Um FocusLost atrasado (de um painel que acabou de fechar) não fecha o painel que
                // abriu no lugar dele e já tem o teclado.
                let focused = self.panels.iter().any(|p| p.open && p.has_focus());
                if self.panel_index(hwnd).is_some() && self.controller.open().is_some() && !focused {
                    let fx = self.controller.handle(PanelInput::FocusLost);
                    self.run_effects(fx)?;
                }
            }
            WinEvent::Key { hwnd, vk } => {
                if self.panel_index(hwnd).is_none() {
                    return Ok(true);
                }
                if vk == VK_ESCAPE {
                    let fx = self.controller.handle(PanelInput::Escape);
                    return self.run_effects(fx).map(|_| true);
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
                    Some(PanelKind::Calendar) if vk == VK_LEFT || vk == VK_RIGHT => {
                        let (y, m) = self.shown_month;
                        self.shown_month = shift_month(y, m, if vk == VK_LEFT { -1 } else { 1 });
                        self.redraw_open_panel()?;
                    }
                    Some(PanelKind::Overflow) => {
                        let monitor = self.controller.open().map_or(0, |p| p.monitor);
                        if vk == VK_RETURN && let Some(id) = self.overflow_ids.get(self.overflow_focus).cloned() {
                            let fx = self.controller.handle(PanelInput::ChipClick { monitor, chip: id });
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
            WinEvent::Timer { id: TIMER_ISLAND_PEEK } => self.island_peek_timer()?,
            WinEvent::Timer { id: TIMER_ISLAND_TICK } => self.island_tick()?,
            WinEvent::Timer { id: TIMER_FOCUS } => self.island_focus_tick()?,
            WinEvent::User { code: USER_MEDIA } => self.island_media_changed()?,
            WinEvent::User { code: USER_VISUALIZER } => self.island_visualizer()?,
            WinEvent::User { code: USER_VOLUME } => self.island_volume_changed()?,
            WinEvent::User { code: USER_PRIVACY } => self.island_privacy_changed()?,
            WinEvent::User { code: USER_NOTIFICATION } => self.island_notifications()?,
            WinEvent::User { code: USER_DND } => self.island_dnd_changed()?,
            WinEvent::ClipboardChanged => self.island_clipboard()?,
            WinEvent::LockKey { caps } => self.island_lock_key(caps)?,
            WinEvent::Timer { id: TIMER_GRAPHICS_TRIM } => {
                window::kill_timer(TIMER_GRAPHICS_TRIM);
                if self.controller.open().is_none() && self.editor.is_none() { self.ctx.gfx.trim_idle_resources(); }
            }
            WinEvent::PanelSettled { hwnd, ticket } => {
                if let Some(index) = self.panel_index(hwnd) { self.panels[index].hide_if_closed(&self.ctx, ticket)?; }
            }
            WinEvent::User { code: USER_ISLAND } => crate::island::restart(&self.ctx),
            WinEvent::User { code } if code == USER_ISLAND + 100 => window::set_timer(TIMER_RAISE, 500),
            WinEvent::Timer { id: TIMER_RAISE } => {
                window::kill_timer(TIMER_RAISE);
                island_zorder::raise_island();
            }
            WinEvent::User { code: USER_CONFIG_CHANGED } => {
                let text = std::fs::read_to_string(&self.ctx.config_path).unwrap_or_default();
                match zefiro_core::config::parse_config(&text) {
                    Ok(new) if new != self.ctx.config => {
                        let fx = self.controller.handle(PanelInput::ConfigChanged);
                        self.run_effects(fx)?;
                        let theme_changed = new.theme != self.ctx.config.theme;
                        let external = (self.ctx.config.island.external(), new.island.external());
                        self.ctx.config = new;
                        // Trocar `[island] engine` liga uma ilha e desliga a outra sem reiniciar a barra.
                        match external {
                            (false, true) => crate::island::start(&self.ctx),
                            (true, false) => crate::island::stop(),
                            _ => {}
                        }
                        if theme_changed {
                            let (id, dir) = (self.ctx.config.theme.clone(), self.ctx.themes_dir.clone());
                            self.ctx.theme.reload(&id, &dir);
                            self.ctx.text.set_family(&self.ctx.theme.theme.font.family);
                        }
                        self.apply_bars(true)?;
                        crate::island::sync(&self.ctx);
                        self.island_services()?;
                        self.schedule_clock();
                    }
                    Ok(_) => {}
                    Err(e) => tracing::warn!(line = ?e.line, "config.toml inválido: {}; mantendo a configuração anterior", e.message),
                }
            }
            WinEvent::User { code: USER_ISLAND_ACCENT } => self.refresh_island_accent()?,
            WinEvent::IslandGeometryChanged => self.apply_bars(true)?,
            WinEvent::User { code: USER_THEMES_CHANGED } => {
                let (id, dir) = (self.ctx.config.theme.clone(), self.ctx.themes_dir.clone());
                for e in self.ctx.theme.reload(&id, &dir) {
                    tracing::warn!(source = %e.source, line = ?e.line, "tema com erro: {}; mantendo a versão anterior", e.message);
                }
                self.ctx.text.set_family(&self.ctx.theme.theme.font.family);
                self.apply_bars(true)?;
                crate::island::sync(&self.ctx);
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
    l.schedule_clock();
    if let Err(e) = l.refresh_island_accent() {
        tracing::warn!(error = %e, "não consegui aplicar o destaque da ilha");
    }
    if let Some(n) = std::env::var("ZEFIRO_CYCLES").ok().and_then(|v| v.parse::<u32>().ok()) {
        tracing::info!(ciclos = n, "modo de medição: abrindo e fechando o calendário");
        l.cycles = Some((n, 0));
        window::set_timer(TIMER_CYCLE, 2000);
    }
    crate::island::start(&l.ctx);
    l.island_services()?;
    'pump: while window::pump() {
        for e in window::take_events() {
            match l.handle(e) {
                Ok(true) => {}
                Ok(false) => break 'pump,
                Err(err) => tracing::error!(error = %err, ?e, "erro tratando evento"),
            }
        }
    }
    shutdown(&mut l);
    Ok(())
}

/// Saída por qualquer caminho: primeiro as AppBars saem da tela (a área reservada volta na hora),
/// depois a ilha, que pode levar até 3 s para fechar.
fn shutdown(l: &mut Loop) {
    for b in l.bars.drain(..) {
        b.destroy();
    }
    for p in l.panels.drain(..) {
        p.destroy();
    }
    crate::island::stop();
}

