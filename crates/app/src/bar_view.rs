//! Uma barra por monitor: faixa ou pílulas, chips e transições.

use windows::UI::Composition::Desktop::DesktopWindowTarget;
use windows::UI::Composition::{CompositionDrawingSurface, ContainerVisual, SpriteVisual, Visual};
use windows::Win32::Foundation::HWND;
use windows::core::{Interface, Result};
use windows_numerics::{Vector2, Vector3};
use zefiro_bar::chips::{ChipContent, clock_chip, zefiro_chip};
use zefiro_core::config::BarHeight;
use zefiro_integration_clock::format::ClockText;
use zefiro_integration_clock::windows::now_local;
use zefiro_platform::geometry::{Point, Rect};
use zefiro_platform_windows::compositor::ui_color;
use zefiro_platform_windows::monitors::Monitor;
use zefiro_platform_windows::morph::MorphShape;
use zefiro_platform_windows::{appbar, island_zorder, window};
use zefiro_ui::bar_layout::{BarLayout, ChipSpec, layout_bar, metrics};
use zefiro_ui::panel_layout::{CHIP_FADE_S, SHADOW_MARGIN_DIP, Shape, pill_radius};
use zefiro_ui::spring::SpringParams;

use crate::app::AppCtx;
use crate::theme_state::SurfaceKind;

struct ChipView { id: String, hover: MorphShape, content: SpriteVisual, surface: CompositionDrawingSurface, rect: Rect }

pub struct BarView {
    hwnd: HWND,
    monitor: Monitor,
    scale: f32,
    _target: DesktopWindowTarget,
    root: ContainerVisual,
    strip: MorphShape,
    divider: SpriteVisual,
    groups: Vec<MorphShape>,
    chips: Vec<ChipView>,
    layout: Option<BarLayout>,
    appbar: bool,
    hover: Option<String>,
    active: Option<String>,
}

fn chip_content(ctx: &AppCtx, id: &str, height: BarHeight, chip_h: f32, font_px: f32, scale: f32) -> ChipContent {
    let t = &ctx.theme.tokens;
    match id {
        "zefiro" => zefiro_chip(chip_h, font_px, height == BarHeight::Compacta, scale, t, &ctx.text),
        _ => {
            let now = now_local();
            let s = ctx.clock.chip_text(&now, &ctx.config.clock.format, ctx.config.clock.seconds);
            clock_chip(&s, chip_h, font_px, t, &ctx.text)
        }
    }
}

impl BarView {
    pub fn create(ctx: &AppCtx, monitor: &Monitor) -> Result<BarView> {
        let scale = monitor.dpi as f32 / 96.0;
        let h = ((44.0 + SHADOW_MARGIN_DIP) * scale).round();
        let bounds = Rect::new(monitor.rect.x, monitor.rect.y, monitor.rect.w, h);
        let hwnd = window::create(window::WindowKind::Bar, bounds)?;
        let (target, root) = ctx.comp.target(hwnd)?;
        root.SetSize(Vector2 { X: monitor.rect.w, Y: h })?;
        let strip = MorphShape::new(&ctx.comp, &root, &ctx.theme.morph_style(scale, SurfaceKind::Strip))?;
        let divider = ctx.comp.compositor.CreateSpriteVisual()?;
        root.Children()?.InsertAtTop(&divider)?;
        let mut groups = Vec::new();
        for _ in 0..2 {
            let g = MorphShape::new(&ctx.comp, &root, &ctx.theme.morph_style(scale, SurfaceKind::Pill))?;
            g.set_opacity(0.0)?;
            groups.push(g);
        }
        let mut view = BarView { hwnd, monitor: monitor.clone(), scale, _target: target, root, strip, divider, groups, chips: Vec::new(), layout: None, appbar: false, hover: None, active: None };
        view.apply(ctx, false)?;
        window::set_bounds(hwnd, bounds, true);
        Ok(view)
    }

    pub fn hwnd(&self) -> HWND {
        self.hwnd
    }

    pub fn monitor(&self) -> &Monitor {
        &self.monitor
    }

    pub fn scale(&self) -> f32 {
        self.scale
    }

    /// Base da faixa, em coordenadas da tela.
    pub fn band_bottom(&self) -> f32 {
        self.layout.as_ref().map_or(0.0, |l| l.band.bottom()) + self.monitor.rect.y
    }

    /// Refaz o layout e anima as formas até ele.
    pub fn apply(&mut self, ctx: &AppCtx, animate: bool) -> Result<()> {
        let cfg = &ctx.config.bar;
        let m = metrics(cfg.height, cfg.format);
        let chip_h = (m.chip_h * self.scale).round();
        let font_px = m.font * self.scale * ctx.theme.theme.font.scale;
        let contents: Vec<(String, ChipContent)> = cfg
            .sections
            .start
            .iter()
            .chain(cfg.sections.end.iter())
            .map(|id| (id.clone(), chip_content(ctx, id, cfg.height, chip_h, font_px, self.scale)))
            .collect();
        let spec = |ids: &[String]| {
            ids.iter()
                .map(|id| ChipSpec { id: id.clone(), content_w: contents.iter().find(|(c, _)| c == id).map_or(0.0, |(_, c)| c.content_w) })
                .collect::<Vec<_>>()
        };
        let layout = layout_bar(self.monitor.rect.w, self.scale, cfg.height, cfg.format, &spec(&cfg.sections.start), &spec(&cfg.sections.end));

        self.strip.set_style(&ctx.comp, &ctx.theme.morph_style(self.scale, SurfaceKind::Strip))?;
        for g in &self.groups {
            g.set_style(&ctx.comp, &ctx.theme.morph_style(self.scale, SurfaceKind::Pill))?;
        }
        self.divider.SetBrush(&ctx.comp.compositor.CreateColorBrushWithColor(ui_color(ctx.theme.tokens.hairline))?)?;

        let animate = animate && ctx.theme.animations;
        let spring = ctx.theme.spring(SpringParams::SETTLE);
        let place = |shape: &MorphShape, s: Shape| if animate { shape.animate(&ctx.comp, s, spring) } else { shape.set(s) };
        let fade = |shape: &MorphShape, v: f32| if animate { shape.animate_opacity(&ctx.comp, v, spring) } else { shape.set_opacity(v) };
        let style = ctx.theme.theme.shape.style;
        let radius_max = ctx.theme.theme.shape.radius_max * self.scale;

        // Faixa, ou o "resto" dela encolhido no centro quando o formato é pílulas.
        let pill_h = layout.island_pill_h;
        let center = Rect::new(((self.monitor.rect.w - pill_h * 4.0) / 2.0).round(), layout.band.y + ((layout.band.h - pill_h) / 2.0).round(), pill_h * 4.0, pill_h);
        match layout.strip {
            Some(band) => {
                place(&self.strip, Shape { rect: band, radius: 0.0 })?;
                fade(&self.strip, 1.0)?;
            }
            None => {
                place(&self.strip, Shape { rect: center, radius: pill_h / 2.0 })?;
                fade(&self.strip, 0.0)?;
            }
        }
        let line = self.scale.max(1.0);
        self.divider.SetOffset(Vector3 { X: 0.0, Y: layout.band.bottom() - line, Z: 0.0 })?;
        self.divider.SetSize(Vector2 { X: self.monitor.rect.w, Y: line })?;
        self.divider.SetOpacity(if layout.strip.is_some() { 1.0 } else { 0.0 })?;

        // Grupos: nascem do retângulo da faixa nas pontas.
        let was_strip = self.layout.as_ref().is_none_or(|l| l.groups.is_empty());
        for (i, g) in self.groups.iter().enumerate() {
            match layout.groups.get(i) {
                Some(r) => {
                    if was_strip {
                        g.set(Shape { rect: Rect::new(r.x, layout.band.y, r.w, layout.band.h), radius: 0.0 })?;
                    }
                    place(g, Shape { rect: *r, radius: pill_radius(r.h, radius_max, style, self.scale) })?;
                    fade(g, 1.0)?;
                }
                None => fade(g, 0.0)?,
            }
        }

        // Chips: cria os que faltam, redesenha o conteúdo e move.
        let pad = (10.0 * self.scale).round();
        for (id, content) in &contents {
            let Some(b) = layout.chips.iter().find(|c| &c.id == id) else { continue };
            if !self.chips.iter().any(|c| &c.id == id) {
                let hover = MorphShape::new(&ctx.comp, &self.root, &ctx.theme.morph_style(self.scale, SurfaceKind::Hover))?;
                hover.set_opacity(0.0)?;
                let surface = ctx.comp.surface(content.list.size.w, content.list.size.h)?;
                let sprite = ctx.comp.sprite_for(&surface, content.list.size.w, content.list.size.h)?;
                self.root.Children()?.InsertAtTop(&sprite)?;
                self.chips.push(ChipView { id: id.clone(), hover, content: sprite, surface, rect: b.rect });
            }
            let chip = self.chips.iter_mut().find(|c| &c.id == id).expect("criado acima");
            ctx.comp.draw(&chip.surface, content.list.size.w, content.list.size.h, &content.list, &ctx.text)?;
            chip.content.SetSize(Vector2 { X: content.list.size.w, Y: content.list.size.h })?;
            chip.content.SetOffset(Vector3 { X: b.rect.x + pad, Y: b.rect.y, Z: 0.0 })?;
            let r = Shape { rect: b.rect, radius: pill_radius(b.rect.h, radius_max, style, self.scale) };
            if animate { chip.hover.animate(&ctx.comp, r, spring)? } else { chip.hover.set(r)? }
            chip.rect = b.rect;
        }

        // Clique: chips + faixa (ou grupos) recebem clique; o resto passa para baixo.
        let mut interactive: Vec<Rect> = layout.chips.iter().map(|c| c.rect).collect();
        interactive.extend(layout.strip);
        interactive.extend(layout.groups.iter().copied());
        window::set_interactive(self.hwnd, interactive);

        let height_px = layout.band.h as i32;
        let (hwnd, mon, reserve) = (self.hwnd, self.monitor.rect, self.appbar);
        self.layout = Some(layout);
        self.refresh_chip_fills(ctx)?;
        if reserve {
            if animate {
                let raw = hwnd.0 as isize; // HWND não é Send
                let batch = ctx.comp.on_settled(move || {
                    appbar::set_pos(HWND(raw as *mut _), mon, height_px);
                    island_zorder::raise_island();
                })?;
                batch.End()?;
            } else {
                appbar::set_pos(hwnd, mon, height_px);
                island_zorder::raise_island();
            }
        }
        Ok(())
    }

    pub fn register_appbar(&mut self, _ctx: &AppCtx) {
        if let Some(l) = &self.layout {
            appbar::register(self.hwnd, self.monitor.rect, l.band.h as i32);
            self.appbar = true;
            island_zorder::raise_island();
        }
    }

    /// Outra AppBar mudou de lugar: só reposiciona (sem registrar de novo).
    pub fn reposition_appbar(&self) {
        if let (true, Some(l)) = (self.appbar, &self.layout) {
            appbar::set_pos(self.hwnd, self.monitor.rect, l.band.h as i32);
        }
    }

    pub fn chip_at(&self, x: f32, y: f32) -> Option<String> {
        self.chips.iter().find(|c| c.rect.contains(Point { x, y })).map(|c| c.id.clone())
    }

    /// Retângulo do chip em coordenadas da tela (para o painel).
    pub fn chip_rect(&self, id: &str) -> Option<Rect> {
        self.chips.iter().find(|c| c.id == id).map(|c| Rect::new(c.rect.x + self.monitor.rect.x, c.rect.y + self.monitor.rect.y, c.rect.w, c.rect.h))
    }

    pub fn set_hover(&mut self, ctx: &AppCtx, chip: Option<&str>) -> Result<()> {
        if self.hover.as_deref() == chip {
            return Ok(());
        }
        self.hover = chip.map(str::to_string);
        self.refresh_chip_fills(ctx)
    }

    pub fn set_active(&mut self, ctx: &AppCtx, chip: Option<&str>) -> Result<()> {
        self.active = chip.map(str::to_string);
        self.refresh_chip_fills(ctx)
    }

    fn refresh_chip_fills(&self, ctx: &AppCtx) -> Result<()> {
        let spring = ctx.theme.spring(SpringParams::SETTLE);
        for c in &self.chips {
            let active = self.active.as_deref() == Some(&c.id);
            let hovered = self.hover.as_deref() == Some(&c.id);
            let mut style = ctx.theme.morph_style(self.scale, SurfaceKind::Hover);
            if active {
                style.fill = ctx.theme.active_fill();
            }
            c.hover.set_style(&ctx.comp, &style)?;
            c.hover.animate_opacity(&ctx.comp, if active || hovered { 1.0 } else { 0.0 }, spring)?;
        }
        Ok(())
    }

    /// Esconde (ao abrir o painel) ou mostra (ao fechar) o conteúdo do chip em 0,17 s.
    pub fn fade_chip(&self, ctx: &AppCtx, chip: &str, visible: bool) -> Result<()> {
        if let Some(c) = self.chips.iter().find(|c| c.id == chip) {
            ctx.comp.fade(&c.content.cast::<Visual>()?, if visible { 1.0 } else { 0.0 }, CHIP_FADE_S)?;
        }
        Ok(())
    }

    pub fn destroy(self) {
        if self.appbar {
            appbar::unregister(self.hwnd);
        }
        window::destroy(self.hwnd);
    }
}
