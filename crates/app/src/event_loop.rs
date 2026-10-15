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

use crate::app::AppCtx;
use crate::bar_view::BarView;
use crate::panel_view::PanelView;

pub const USER_PANEL_SETTLED: u32 = 1;
pub const USER_CONFIG_CHANGED: u32 = 2;
pub const USER_THEMES_CHANGED: u32 = 3;
pub const USER_ISLAND: u32 = 4;
pub const TIMER_CLOCK: usize = 1;
pub const TIMER_DISPLAY: usize = 2;
pub const TIMER_RAISE: usize = 3;
/// Modo de medição (`ZEFIRO_CYCLES=N`): abre e fecha o calendário N vezes e sai.
pub const TIMER_CYCLE: usize = 4;
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
    menu: Option<MenuModel>,
    shown_month: (i32, u32),
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
        Ok(Loop { ctx, bars, panels, controller: PanelController::new(), menu: None, shown_month: (today.year, today.month), cycles: None })
    }

    fn bar_index(&self, hwnd: isize) -> Option<usize> { self.bars.iter().position(|b| b.hwnd().0 as isize == hwnd) }
    fn panel_index(&self, hwnd: isize) -> Option<usize> { self.panels.iter().position(|p| p.hwnd().0 as isize == hwnd) }

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
        }
    }

    fn run_effects(&mut self, fx: Vec<PanelEffect>) -> Result<()> {
        let opens_another = fx.iter().any(|e| matches!(e, PanelEffect::Open(_)));
        for e in fx {
            match e {
                PanelEffect::Close { monitor } => {
                    self.panels[monitor].close(&self.ctx, !opens_another)?;
                    for b in &mut self.bars {
                        b.set_active(&self.ctx, None)?;
                    }
                    for b in &self.bars {
                        for id in ["zefiro", "clock"] {
                            b.fade_chip(&self.ctx, id, true)?;
                        }
                    }
                    self.menu = None;
                }
                PanelEffect::Open(p) => {
                    if p.kind == PanelKind::Calendar {
                        let today = now_local().date;
                        self.shown_month = (today.year, today.month);
                    }
                    let scale = self.bars[p.monitor].scale();
                    let list = self.panel_list(p.kind, scale);
                    let Some(chip) = self.bars[p.monitor].chip_rect(&p.chip) else { continue };
                    let band_bottom = self.bars[p.monitor].band_bottom();
                    self.panels[p.monitor].open(&self.ctx, chip, band_bottom, &list)?;
                    self.bars[p.monitor].set_active(&self.ctx, Some(&p.chip))?;
                    self.bars[p.monitor].fade_chip(&self.ctx, &p.chip, false)?;
                }
            }
        }
        Ok(())
    }

    fn redraw_open_panel(&mut self) -> Result<()> {
        let Some(open) = self.controller.open().cloned() else { return Ok(()) };
        let scale = self.bars[open.monitor].scale();
        let list = self.panel_list(open.kind, scale);
        let Some(chip) = self.bars[open.monitor].chip_rect(&open.chip) else { return Ok(()) };
        let band_bottom = self.bars[open.monitor].band_bottom();
        self.panels[open.monitor].resize(&self.ctx, chip, band_bottom, &list)
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
        match e {
            WinEvent::Quit => return Ok(false),
            WinEvent::LeftUp { hwnd, x, y } => {
                if let Some(i) = self.bar_index(hwnd) {
                    if let Some(chip) = self.bars[i].chip_at(x, y) {
                        let fx = self.controller.handle(PanelInput::ChipClick { monitor: i, chip });
                        self.run_effects(fx)?;
                    }
                } else if let Some(i) = self.panel_index(hwnd) {
                    let (ox, oy) = self.panels[i].content_origin();
                    let scale = self.bars[i].scale();
                    let hit = self.menu.as_ref().and_then(|m| m.hit(Point { x: x - ox, y: y - oy }, scale));
                    if let Some(a) = hit {
                        self.menu_action(a)?;
                    }
                }
            }
            WinEvent::RightUp { hwnd, .. } => {
                if let Some(i) = self.bar_index(hwnd) {
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
                    self.bars[i].set_hover(&self.ctx, chip.as_deref())?;
                }
            }
            WinEvent::MouseLeave { hwnd } => {
                if let Some(i) = self.bar_index(hwnd) {
                    self.bars[i].set_hover(&self.ctx, None)?;
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
                match self.controller.open().map(|o| o.kind) {
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
                    _ => {}
                }
            }
            WinEvent::Wheel { hwnd, delta } => {
                if self.panel_index(hwnd).is_some() && self.controller.open().is_some_and(|o| o.kind == PanelKind::Calendar) {
                    let (y, m) = self.shown_month;
                    self.shown_month = shift_month(y, m, if delta > 0 { -1 } else { 1 });
                    self.redraw_open_panel()?;
                }
            }
            WinEvent::Timer { id: TIMER_CLOCK } => {
                self.apply_bars(true)?;
                if self.controller.open().is_some_and(|o| o.kind == PanelKind::Calendar) {
                    self.redraw_open_panel()?;
                }
                self.schedule_clock();
            }
            WinEvent::Timer { id: TIMER_CYCLE } => self.cycle_tick()?,
            WinEvent::User { code: USER_PANEL_SETTLED } => {
                for p in &mut self.panels {
                    p.hide_if_closed();
                }
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
                        self.ctx.config = new;
                        if theme_changed {
                            let (id, dir) = (self.ctx.config.theme.clone(), self.ctx.themes_dir.clone());
                            self.ctx.theme.reload(&id, &dir);
                            self.ctx.text.set_family(&self.ctx.theme.theme.font.family);
                        }
                        self.apply_bars(true)?;
                        crate::island::sync(&self.ctx);
                        self.schedule_clock();
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
    let mut l = Loop::new(ctx, bars)?;
    l.schedule_clock();
    if let Some(n) = std::env::var("ZEFIRO_CYCLES").ok().and_then(|v| v.parse::<u32>().ok()) {
        tracing::info!(ciclos = n, "modo de medição: abrindo e fechando o calendário");
        l.cycles = Some((n, 0));
        window::set_timer(TIMER_CYCLE, 2000);
    }
    crate::island::start(&l.ctx);
    while window::pump() {
        for e in window::take_events() {
            match l.handle(e) {
                Ok(true) => {}
                Ok(false) => {
                    crate::island::stop();
                    return Ok(());
                }
                Err(err) => tracing::error!(error = %err, ?e, "erro tratando evento"),
            }
        }
    }
    crate::island::stop();
    for b in l.bars.drain(..) {
        b.destroy();
    }
    Ok(())
}

