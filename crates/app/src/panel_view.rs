//! Janela de painel de um monitor: a forma que nasce do chip e o conteúdo revelado por ela.

use windows::UI::Composition::Desktop::DesktopWindowTarget;
use windows::UI::Composition::{CompositionDrawingSurface, ContainerVisual, SpriteVisual, Visual};
use windows::Win32::Foundation::HWND;
use windows::core::{Interface, Result};
use windows_numerics::{Vector2, Vector3};
use zefiro_platform::geometry::Rect;
use zefiro_platform_windows::monitors::Monitor;
use zefiro_platform_windows::morph::MorphShape;
use zefiro_platform_windows::window;
use zefiro_ui::paint::DisplayList;
use zefiro_ui::panel_layout::{CHIP_FADE_S, Shape, panel_radius, panel_rect, pill_radius, to_local};
use zefiro_ui::spring::{NUDGE_DIP, SpringParams};
use zefiro_ui::panel_lifecycle::PanelLifecycle;

use crate::app::AppCtx;
use crate::theme_state::SurfaceKind;

/// Ao fechar, a forma some em fade quando já está perto do chip, em vez de ficar parada sobre
/// ele até a mola assentar de vez.
const CLOSE_FADE_DELAY_S: f32 = 0.15;

pub struct PanelView {
    hwnd: HWND,
    monitor: Monitor,
    scale: f32,
    _target: DesktopWindowTarget,
    root: ContainerVisual,
    shape: MorphShape,
    /// Camada do tamanho da janela: o recorte da forma é aplicado nela, no mesmo espaço da forma.
    layer: ContainerVisual,
    content: SpriteVisual,
    surface: CompositionDrawingSurface,
    window: Rect,
    chip: Rect,
    band_bottom: f32,
    panel: Rect,
    previous_foreground: isize,
    pub open: bool,
    /// A janela está na tela (aberta, ou ainda animando o fechamento).
    visible: bool,
    lifecycle: PanelLifecycle,
}

impl PanelView {
    pub fn create(ctx: &AppCtx, monitor: &Monitor) -> Result<PanelView> {
        let scale = monitor.dpi as f32 / 96.0;
        let hwnd = window::create(window::WindowKind::Panel, Rect::new(monitor.rect.x, monitor.rect.y, 10.0, 10.0))?;
        let (target, root) = ctx.comp.target(hwnd)?;
        let shape = MorphShape::new(&ctx.comp, &root, &ctx.theme.morph_style(scale, SurfaceKind::Panel))?;
        let surface = ctx.comp.surface(1.0, 1.0)?;
        let content = ctx.comp.sprite_for(&surface, 1.0, 1.0)?;
        let layer = ctx.comp.compositor.CreateContainerVisual()?;
        layer.Children()?.InsertAtTop(&content)?;
        // Dentro da raiz da forma: a "cutucada" move forma e conteúdo juntos.
        shape.root().Children()?.InsertAtTop(&layer)?;
        shape.clip_child(&ctx.comp, &layer.cast::<Visual>()?)?;
        Ok(PanelView { hwnd, monitor: monitor.clone(), scale, _target: target, root, shape, layer, content, surface, window: Rect::default(), chip: Rect::default(), band_bottom: 0.0, panel: Rect::default(), previous_foreground: 0, open: false, visible: false, lifecycle: PanelLifecycle::default() })
    }

    pub fn hwnd(&self) -> HWND {
        self.hwnd
    }

    /// Canto superior esquerdo do painel dentro da janela (para converter cliques).
    pub fn content_origin(&self) -> (f32, f32) {
        (self.panel.x - self.window.x, self.panel.y - self.window.y)
    }

    fn local(&self, r: Rect) -> Rect {
        to_local(Shape { rect: r, radius: 0.0 }, self.window).rect
    }

    fn draw(&mut self, ctx: &AppCtx, list: &DisplayList) -> Result<()> {
        ctx.comp.draw(&self.surface, list.size.w, list.size.h, list, &ctx.text)?;
        let p = self.local(self.panel);
        self.content.SetSize(Vector2 { X: list.size.w, Y: list.size.h })?;
        self.content.SetOffset(Vector3 { X: p.x, Y: p.y, Z: 0.0 })
    }

    pub fn open(&mut self, ctx: &AppCtx, chip: Rect, band_bottom: f32, list: &DisplayList) -> Result<()> {
        self.place(ctx, chip, band_bottom, list, true)
    }

    fn place(&mut self, ctx: &AppCtx, chip: Rect, band_bottom: f32, list: &DisplayList, opening: bool) -> Result<()> {
        let mon = self.monitor.rect;
        let chip_local_mon = Rect::new(chip.x - mon.x, chip.y - mon.y, chip.w, chip.h);
        let panel_mon = panel_rect(chip_local_mon, band_bottom - mon.y, list.size, mon.w, self.scale);
        let was_open = self.open;
        let was_visible = self.visible;
        self.chip = chip;
        self.band_bottom = band_bottom;
        self.panel = Rect::new(panel_mon.x + mon.x, panel_mon.y + mon.y, panel_mon.w, panel_mon.h);
        // A janela de desenho cobre o monitor inteiro e deixa todo clique passar (só a janela de
        // entrada, recortada no painel, recebe clique). Assim a forma vive sempre no mesmo espaço
        // e trocar de painel no meio de uma animação só redireciona a mola, sem salto.
        self.window = mon;
        window::set_bounds(self.hwnd, self.window, true);
        self.root.SetSize(Vector2 { X: self.window.w, Y: self.window.h })?;
        self.layer.SetSize(Vector2 { X: self.window.w, Y: self.window.h })?;
        self.shape.set_style(&ctx.comp, &ctx.theme.morph_style(self.scale, SurfaceKind::Panel))?;
        self.draw(ctx, list)?;

        let style = ctx.theme.theme.shape.style;
        let chip_shape = Shape { rect: self.local(self.chip), radius: pill_radius(self.chip.h, ctx.theme.theme.shape.radius_max * self.scale, style, self.scale) };
        let panel_shape = Shape { rect: self.local(self.panel), radius: panel_radius(ctx.theme.theme.shape.panel_radius * self.scale, style, self.scale) };
        if !self.visible {
            // Escondido: nasce do chip. Visível (fechando, ou outro painel): a mola parte do valor atual.
            self.shape.set(chip_shape)?;
            self.shape.set_opacity(1.0)?;
        } else {
            self.shape.animate_opacity(&ctx.comp, 1.0, ctx.theme.spring(SpringParams::SETTLE))?;
        }
        self.visible = true;
        self.lifecycle.open();
        self.shape.bind_alpha(&ctx.comp, &self.layer.cast::<Visual>()?, panel_shape.rect.h)?;
        if ctx.theme.animations {
            // Abrir: mola de crescer + cutucada. Conteúdo mudando de tamanho: mola de ajuste.
            let p = if opening && !was_open { SpringParams::GROW } else { SpringParams::SETTLE };
            self.shape.animate(&ctx.comp, panel_shape, ctx.theme.spring(p))?;
            if opening {
                self.shape.nudge(&ctx.comp, NUDGE_DIP * self.scale, ctx.theme.spring(SpringParams::SETTLE))?;
            }
        } else {
            self.shape.set(panel_shape)?;
        }
        window::set_interactive(self.hwnd, vec![panel_shape.rect]);
        if !was_visible {
            // Trocando de painel no meio da animação, a janela de antes continua a mesma.
            self.previous_foreground = window::foreground();
        }
        window::focus(self.hwnd);
        self.open = true;
        Ok(())
    }

    /// O conteúdo mudou (menu redesenhado, outro mês no calendário) ou a barra mudou embaixo
    /// dele (altura/formato): o painel segue o chip com a mola de ajuste.
    pub fn resize(&mut self, ctx: &AppCtx, chip: Rect, band_bottom: f32, list: &DisplayList) -> Result<()> {
        let same_size = (list.size.h - self.panel.h).abs() < 0.5 && (list.size.w - self.panel.w).abs() < 0.5;
        if same_size && chip == self.chip && (band_bottom - self.band_bottom).abs() < 0.5 {
            // O tema pode ter mudado: a forma pega as cores novas junto com o conteúdo.
            self.shape.set_style(&ctx.comp, &ctx.theme.morph_style(self.scale, SurfaceKind::Panel))?;
            return self.draw(ctx, list);
        }
        self.place(ctx, chip, band_bottom, list, false)
    }

    /// `restore_focus`: devolve o foco à janela de antes. Falso quando outro painel abre em
    /// seguida (o foco vai direto para ele).
    pub fn close(&mut self, ctx: &AppCtx, restore_focus: bool) -> Result<()> {
        if !self.open {
            return Ok(());
        }
        self.open = false;
        let ticket = self.lifecycle.close();
        window::set_interactive(self.hwnd, Vec::new());
        // Esc / clique no chip: o foco volta para a janela de antes (spec §6.1, item 5).
        if restore_focus {
            window::restore_foreground(self.previous_foreground, self.hwnd);
        }
        let style = ctx.theme.theme.shape.style;
        let chip_shape = Shape { rect: self.local(self.chip), radius: pill_radius(self.chip.h, ctx.theme.theme.shape.radius_max * self.scale, style, self.scale) };
        if ctx.theme.animations {
            let raw = self.hwnd.0 as isize;
            let batch = ctx.comp.on_settled(move || window::post_panel_settled(raw, ticket))?;
            self.shape.animate(&ctx.comp, chip_shape, ctx.theme.spring(SpringParams::SHRINK))?;
            ctx.comp.fade_after(&self.shape.root().cast::<Visual>()?, 0.0, CHIP_FADE_S, CLOSE_FADE_DELAY_S)?;
            batch.End()?;
        } else {
            self.shape.set(chip_shape)?;
            window::hide(self.hwnd);
            self.visible = false;
            self.lifecycle.settle(ticket);
            self.release_surface(ctx)?;
        }
        Ok(())
    }

    /// Chamado quando a animação de fechar terminou (se nada reabriu o painel nesse meio-tempo).
    pub fn hide_if_closed(&mut self, ctx: &AppCtx, ticket: usize) -> Result<()> {
        if !self.open && self.visible && self.lifecycle.settle(ticket) {
            window::hide(self.hwnd);
            self.visible = false;
            self.release_surface(ctx)?;
        }
        Ok(())
    }

    fn release_surface(&mut self, ctx: &AppCtx) -> Result<()> {
        let surface = ctx.comp.surface(1.0, 1.0)?;
        self.content.SetBrush(&ctx.comp.compositor.CreateSurfaceBrushWithSurface(&surface)?)?;
        self.surface = surface;
        self.content.SetSize(Vector2 { X: 1.0, Y: 1.0 })?;
        window::set_timer(crate::event_loop::TIMER_GRAPHICS_TRIM, 1000);
        Ok(())
    }

    /// O teclado está neste painel agora.
    pub fn has_focus(&self) -> bool {
        window::input_window(self.hwnd).is_some_and(|h| h.0 as isize == window::foreground())
    }

    pub fn restore_focus(&self) { window::restore_foreground(self.previous_foreground, self.hwnd); }

    pub fn destroy(self) {
        window::destroy(self.hwnd);
    }
}
