//! Uma barra por monitor: faixa ou pílulas, chips e transições.

use windows::UI::Composition::Desktop::DesktopWindowTarget;
use windows::UI::Composition::{CompositionDrawingSurface, ContainerVisual, SpriteVisual, Visual};
use windows::Win32::Foundation::HWND;
use windows::core::{Interface, Result};
use windows_numerics::{Vector2, Vector3};
use zefiro_bar::chips::{ChipContent, clock_chip, zefiro_chip};
use zefiro_bar::devices::{ICON_BRIGHTNESS, Level, level_chip, volume_level};
use zefiro_core::config::BarHeight;
use zefiro_integration_clock::format::ClockText;
use zefiro_integration_clock::windows::now_local;
use zefiro_platform::geometry::{Point, Rect};
use zefiro_platform_windows::compositor::ui_color;
use zefiro_platform_windows::monitors::Monitor;
use zefiro_platform_windows::morph::MorphShape;
use zefiro_platform_windows::{appbar, island_zorder, window};
use zefiro_ui::bar_layout::{BarLayout, ChipSpec, layout_sections, metrics};
use zefiro_ui::panel_layout::{CHIP_FADE_S, SHADOW_MARGIN_DIP, Shape, pill_radius};
use zefiro_ui::spring::SpringParams;
use zefiro_ui::paint::{DisplayList, DrawOp};
use zefiro_platform::geometry::Size;
use zefiro_platform::text::{Align, Family, FontWeight, TextStyle};

use crate::app::AppCtx;
use crate::island_view::IslandView;
use crate::theme_state::SurfaceKind;
use zefiro_island::view::{Collapsed, PillContent, RESERVE_DESIGN_W, activity_pill, idle_pill, media_pill, timer_pill, with_privacy};

/// Id do "chip" da ilha embutida (clique, painel e foco usam o mesmo caminho dos chips).
pub const ISLAND_CHIP: &str = "island";

struct ChipView { id: String, hover: MorphShape, content: SpriteVisual, surface: CompositionDrawingSurface, rect: Rect, pos: windows::UI::Composition::CompositionPropertySet }

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
    guides: Option<(CompositionDrawingSurface, SpriteVisual)>,
    drag_ghost: Option<SpriteVisual>,
    drag_marker: Option<SpriteVisual>,
    island: Option<IslandView>,
    hidden: bool,
    /// Áreas clicáveis sem a ilha (a ilha muda de tamanho sem refazer o layout inteiro).
    interactive_base: Vec<Rect>,
}

/// Conteúdo recolhido da ilha agora.
fn island_content(ctx: &AppCtx, pill_h: f32) -> PillContent {
    let font_scale = ctx.theme.theme.font.scale;
    let tokens = &ctx.theme.tokens;
    match (ctx.island.collapsed(ctx.now()), ctx.island.media()) {
        (Collapsed::Media, Some(s)) => with_privacy(media_pill(s, false, pill_h, font_scale, tokens), ctx.island.privacy),
        (Collapsed::Peek, Some(s)) => with_privacy(media_pill(s, true, pill_h, font_scale, tokens), ctx.island.privacy),
        (Collapsed::Activity(a), _) => activity_pill(&a, pill_h, font_scale, tokens),
        (Collapsed::Timer(t), _) => timer_pill(&t, ctx.now(), pill_h, font_scale, tokens),
        _ => {
            let clock = ctx.clock.chip_text(&now_local(), "", false);
            let weather = ctx.island.weather.as_ref().map(|w| (zefiro_island::weather::icon(w.code), w.temp_c));
            with_privacy(idle_pill(&clock, weather, pill_h, font_scale, tokens, &ctx.text), ctx.island.privacy)
        }
    }
}

/// Ícone dos chips de VS Code e Energia (só ícone).
fn icon_glyph(id: &str) -> &'static str {
    if id == "vscode" { "\u{E8B7}" } else { "\u{E7E8}" }
}

/// `device`: monitor desta barra (o chip de brilho mostra o brilho dele).
fn chip_content(ctx: &AppCtx, id: &str, device: &str, height: BarHeight, chip_h: f32, font_px: f32, scale: f32) -> ChipContent {
    let t = &ctx.theme.tokens;
    match id {
        "zefiro" => zefiro_chip(chip_h, font_px, height == BarHeight::Compacta, scale, t, &ctx.text),
        "clock" => {
            let now = now_local();
            let mut s = ctx.clock.chip_text(&now, &ctx.config.clock.format, ctx.config.clock.seconds);
            while clock_chip(&s, chip_h, font_px, t, &ctx.text).content_w > 160.0 * scale && s.chars().count() > 1 {
                if s.ends_with('…') { s.pop(); }
                s.pop();
                s.push('…');
            }
            clock_chip(&s, chip_h, font_px, t, &ctx.text)
        }
        "audio" | "mic" => {
            let v = if id == "mic" { ctx.devices.mic } else { ctx.devices.speaker };
            let (glyph, dim) = volume_level(v.map(|v| ((v.level * 100.0).round() as u32, v.muted)), id == "mic");
            level_chip(Level { glyph, text: "", dim }, chip_h, font_px, scale, t, &ctx.text)
        }
        // Só o ícone: o nível aparece no painel, ao clicar.
        "brightness" => {
            let p = ctx.devices.brightness_of(device);
            level_chip(Level { glyph: ICON_BRIGHTNESS, text: "", dim: p.is_none() }, chip_h, font_px, scale, t, &ctx.text)
        }
        "codex" => zefiro_bar::usage::usage_chip(crate::usage_loop::CODEX_GLYPH, ctx.usage.codex.as_ref().and_then(|(_, u)| crate::usage_loop::codex_percent(u)), chip_h, font_px, scale, t, &ctx.text),
        "claude" => zefiro_bar::usage::usage_chip(crate::usage_loop::CLAUDE_GLYPH, ctx.usage.claude_chip(), chip_h, font_px, scale, t, &ctx.text),
        "notifications" => zefiro_bar::chips::bell_chip(ctx.system.notifications, chip_h, font_px, scale, t, &ctx.text),
        "dnd" => zefiro_bar::chips::dnd_chip(ctx.system.dnd, chip_h, font_px, t),
        "vscode" | "power" => level_chip(Level { glyph: icon_glyph(id), text: "", dim: false }, chip_h, font_px, scale, t, &ctx.text),
        _ => clock_chip("⋯", chip_h, font_px, t, &ctx.text),
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
        let groups = Vec::new();
        let mut view = BarView { hwnd, monitor: monitor.clone(), scale, _target: target, root, strip, divider, groups, chips: Vec::new(), layout: None, appbar: false, hover: None, active: None, guides: None, drag_ghost: None, drag_marker: None, island: None, hidden: false, interactive_base: Vec::new() };
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
        let sections = cfg.sections_for(self.monitor.identity.id.as_deref());
        let m = metrics(cfg.height, cfg.format);
        let chip_h = (m.chip_h * self.scale).round();
        let font_px = m.font * self.scale * ctx.theme.theme.font.scale;
        let mut contents: Vec<(String, ChipContent)> = sections
            .ids()
            .filter(|id| zefiro_bar::catalog::descriptor(id).is_some())
            .map(|id| (id.to_string(), chip_content(ctx, id, &self.monitor.device, cfg.height, chip_h, font_px, self.scale)))
            .collect();
        let spec = |ids: &[String]| {
            ids.iter().filter(|id| zefiro_bar::catalog::descriptor(id).is_some())
                .map(|id| ChipSpec { id: id.clone(), content_w: contents.iter().find(|(c, _)| c == id).map_or(0.0, |(_, c)| c.content_w) })
                .collect::<Vec<_>>()
        };
        let specs = [spec(&sections.start), spec(&sections.center), spec(&sections.end)];
        let hosts_island = self.hosts_island(ctx);
        let reservation = if hosts_island {
            Some((RESERVE_DESIGN_W * (m.island_pill * self.scale).round() / 44.0).round())
        } else {
            (self.monitor.primary && ctx.config.island.external()).then(|| window::island_reservation().unwrap_or(self.monitor.rect.w))
        };
        let layout = layout_sections(self.monitor.rect.w, self.scale, cfg.height, cfg.format, [&specs[0], &specs[1], &specs[2]], reservation);
        while self.groups.len() < layout.groups.len() {
            let group = MorphShape::new(&ctx.comp, &self.root, &ctx.theme.morph_style(self.scale, SurfaceKind::Pill))?;
            ctx.comp.place_background(&self.root, &group.root().cast()?, &self.divider.cast()?)?;
            group.set_opacity(0.0)?;
            self.groups.push(group);
        }
        for group in &layout.overflow {
            contents.push((group.id.clone(), chip_content(ctx, &group.id, &self.monitor.device, cfg.height, chip_h, font_px, self.scale)));
        }
        let mut index = 0;
        while index < self.chips.len() {
            if layout.chips.iter().any(|c| c.id == self.chips[index].id) {
                index += 1;
            } else {
                let chip = self.chips.remove(index);
                self.root.Children()?.Remove(&chip.content)?;
                self.root.Children()?.Remove(chip.hover.root())?;
            }
        }

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
            let is_new = !self.chips.iter().any(|c| &c.id == id);
            if is_new {
                let hover = MorphShape::new(&ctx.comp, &self.root, &ctx.theme.morph_style(self.scale, SurfaceKind::Hover))?;
                hover.set_opacity(0.0)?;
                let surface = ctx.comp.surface(content.list.size.w, content.list.size.h)?;
                let sprite = ctx.comp.sprite_for(&surface, content.list.size.w, content.list.size.h)?;
                self.root.Children()?.InsertAtTop(&sprite)?;
                let pos = ctx.comp.spring_offset(&sprite.cast::<Visual>()?, b.rect.x + pad, b.rect.y)?;
                self.chips.push(ChipView { id: id.clone(), hover, content: sprite, surface, rect: b.rect, pos });
            }
            let chip = self.chips.iter_mut().find(|c| &c.id == id).expect("criado acima");
            ctx.comp.draw(&chip.surface, content.list.size.w, content.list.size.h, &content.list, &ctx.text)?;
            chip.content.SetSize(Vector2 { X: content.list.size.w, Y: content.list.size.h })?;
            if animate && !is_new && chip.rect != b.rect {
                ctx.comp.move_offset(&chip.pos, b.rect.x + pad, b.rect.y, Some(spring))?;
            } else if is_new || !animate {
                ctx.comp.move_offset(&chip.pos, b.rect.x + pad, b.rect.y, None)?;
            }
            let r = Shape { rect: b.rect, radius: pill_radius(b.rect.h, radius_max, style, self.scale) };
            if animate { chip.hover.animate(&ctx.comp, r, spring)? } else { chip.hover.set(r)? }
            chip.rect = b.rect;
        }

        // Clique: chips + faixa (ou grupos) recebem clique; o resto passa para baixo.
        let mut interactive: Vec<Rect> = layout.chips.iter().map(|c| c.rect).collect();
        interactive.extend(layout.strip);
        interactive.extend(layout.groups.iter().copied());
        self.interactive_base = interactive;
        zefiro_platform_windows::bar_context::set_zone(self.hwnd, Rect::new(self.monitor.rect.x, self.monitor.rect.y, layout.band.w, layout.band.h));

        let height_px = layout.band.h as i32;
        let (hwnd, mon, reserve) = (self.hwnd, self.monitor.rect, self.appbar);
        self.layout = Some(layout);
        match (hosts_island, self.island.is_some()) {
            (true, false) => {
                let view = IslandView::new(ctx, &self.root, &ctx.theme.morph_style(self.scale, SurfaceKind::Pill))?;
                self.island = Some(view);
            }
            (false, true) => {
                if let Some(view) = self.island.take() {
                    self.root.Children()?.Remove(view.root())?;
                }
            }
            _ => {}
        }
        self.refresh_island(ctx, animate)?;
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

    /// A ilha embutida mora no centro da barra do monitor principal.
    pub fn hosts_island(&self, ctx: &AppCtx) -> bool {
        self.monitor.primary && ctx.config.island.embedded()
    }

    /// Redesenha só a ilha (mídia, espiada, visualizador), sem refazer o layout da barra.
    pub fn refresh_island(&mut self, ctx: &AppCtx, animate: bool) -> Result<()> {
        let (Some(layout), Some(view)) = (&self.layout, &mut self.island) else {
            window::set_interactive(self.hwnd, self.interactive_base.clone());
            return Ok(());
        };
        let pill_h = layout.island_pill_h;
        let content = island_content(ctx, pill_h);
        let rect = Rect::new(((self.monitor.rect.w - content.size.w) / 2.0).round(), ((layout.band.h - pill_h) / 2.0).round(), content.size.w, pill_h);
        // Na faixa, a ilha se funde à barra (sem contorno nem sombra); nas pílulas, é uma pílula.
        let kind = if layout.strip.is_some() { SurfaceKind::Strip } else { SurfaceKind::Pill };
        let style = ctx.theme.morph_style(self.scale, kind);
        let radius = pill_radius(pill_h, ctx.theme.theme.shape.radius_max * self.scale, ctx.theme.theme.shape.style, self.scale);
        view.update(ctx, content, rect, radius, &style, animate && ctx.theme.animations)?;
        let mut interactive = self.interactive_base.clone();
        interactive.push(rect);
        window::set_interactive(self.hwnd, interactive);
        Ok(())
    }

    /// Barras do visualizador (`None` = paradas).
    pub fn set_visualizer(&self, wave: Option<&zefiro_island::waveform::Waveform>) -> Result<()> {
        match &self.island {
            Some(view) => view.set_levels(&wave.map(|w| w.bars(view.bar_count())).unwrap_or_default()),
            None => Ok(()),
        }
    }

    pub fn has_island(&self) -> bool {
        self.island.is_some()
    }

    pub fn is_hidden(&self) -> bool {
        self.hidden
    }

    /// Chip sob o mouse agora (a roda ajusta som, microfone e brilho por ele).
    pub fn hovered(&self) -> Option<&str> {
        self.hover.as_deref()
    }

    pub fn chip_at(&self, x: f32, y: f32) -> Option<String> {
        if self.island.as_ref().is_some_and(|v| v.rect.contains(Point { x, y })) {
            return Some(ISLAND_CHIP.into());
        }
        self.chips.iter().find(|c| c.rect.contains(Point { x, y })).map(|c| c.id.clone())
    }

    pub fn overflow_items(&self, id: &str) -> Vec<String> {
        self.layout.as_ref().and_then(|layout| layout.overflow.iter().find(|g| g.id == id)).map(|g| g.ids.clone()).unwrap_or_default()
    }

    pub fn edit_regions(&self) -> [Rect; 3] {
        let height = self.layout.as_ref().map_or(44.0 * self.scale, |l| l.band.h);
        let third = self.monitor.rect.w / 3.0;
        std::array::from_fn(|i| Rect::new(i as f32 * third, 0.0, third, height))
    }

    pub fn show_edit_guides(&mut self, ctx: &AppCtx, show: bool) -> Result<()> {
        if !show {
            if let Some((_, sprite)) = self.guides.take() { self.root.Children()?.Remove(&sprite)?; }
            return Ok(());
        }
        if self.guides.is_some() { return Ok(()); }
        let height = self.layout.as_ref().map_or(44.0 * self.scale, |l| l.band.h);
        let size = Size { w: self.monitor.rect.w, h: 22.0 * self.scale };
        let mut list = DisplayList::new(size);
        for (section, rect) in zefiro_bar::editor::Section::ALL.into_iter().zip(self.edit_regions()) {
            let guide = Rect::new(rect.x + 4.0 * self.scale, 0.0, (rect.w - 8.0 * self.scale).max(0.0), size.h);
            list.push(DrawOp::RoundRect { rect: guide, radius: 5.0 * self.scale, color: ctx.theme.tokens.raised_strong });
            list.push(DrawOp::RoundRectStroke { rect: guide, radius: 5.0 * self.scale, color: ctx.theme.tokens.accent, width: self.scale });
            list.push(DrawOp::Text { rect: guide, text: format!("{}  +", section.label()), style: TextStyle { size: 11.0 * self.scale, weight: FontWeight::SemiBold, family: Family::Text }, color: ctx.theme.tokens.text, align: Align::Center });
        }
        if self.guides.is_none() {
            let surface = ctx.comp.surface(size.w, size.h)?;
            let sprite = ctx.comp.sprite_for(&surface, size.w, size.h)?;
            self.root.Children()?.InsertAtTop(&sprite)?;
            self.guides = Some((surface, sprite));
        }
        if let Some((surface, sprite)) = &self.guides {
            ctx.comp.draw(surface, size.w, size.h, &list, &ctx.text)?;
            sprite.SetOffset(Vector3 { X: 0.0, Y: height + self.scale, Z: 0.0 })?;
        }
        Ok(())
    }

    pub fn show_drag(&mut self, ctx: &AppCtx, id: &str, point: Point, marker: Option<Rect>) -> Result<()> {
        if self.drag_ghost.is_none() && let Some(chip) = self.chips.iter().find(|chip| chip.id == id) {
            let size = chip.content.Size()?;
            let sprite = ctx.comp.sprite_for(&chip.surface, size.X, size.Y)?;
            sprite.SetOpacity(0.85)?;
            self.root.Children()?.InsertAtTop(&sprite)?;
            self.drag_ghost = Some(sprite);
        }
        if let Some(sprite) = &self.drag_ghost { sprite.SetOffset(Vector3 { X: point.x + 8.0 * self.scale, Y: point.y, Z: 0.0 })?; }
        if let Some(rect) = marker {
            if self.drag_marker.is_none() {
                let sprite = ctx.comp.compositor.CreateSpriteVisual()?;
                sprite.SetBrush(&ctx.comp.compositor.CreateColorBrushWithColor(ui_color(ctx.theme.tokens.accent))?)?;
                self.root.Children()?.InsertAtTop(&sprite)?;
                self.drag_marker = Some(sprite);
            }
            if let Some(sprite) = &self.drag_marker {
                sprite.SetOpacity(1.0)?;
                sprite.SetSize(Vector2 { X: 2.0 * self.scale, Y: rect.h })?;
                sprite.SetOffset(Vector3 { X: rect.x, Y: rect.y, Z: 0.0 })?;
            }
        } else if let Some(sprite) = &self.drag_marker { sprite.SetOpacity(0.0)?; }
        Ok(())
    }

    pub fn clear_drag(&mut self) -> Result<()> {
        for sprite in [self.drag_ghost.take(), self.drag_marker.take()].into_iter().flatten() { self.root.Children()?.Remove(&sprite)?; }
        Ok(())
    }

    /// Retângulo do chip em coordenadas da tela (para o painel).
    pub fn chip_rect(&self, id: &str) -> Option<Rect> {
        if id == ISLAND_CHIP {
            let r = self.island.as_ref()?.rect;
            return Some(Rect::new(r.x + self.monitor.rect.x, r.y + self.monitor.rect.y, r.w, r.h));
        }
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
        if chip == ISLAND_CHIP {
            if let Some(view) = &self.island {
                view.fade_content(ctx, visible)?;
            }
            return Ok(());
        }
        if let Some(c) = self.chips.iter().find(|c| c.id == chip) {
            ctx.comp.fade(&c.content.cast::<Visual>()?, if visible { 1.0 } else { 0.0 }, CHIP_FADE_S)?;
        }
        Ok(())
    }

    /// Some (app em tela cheia neste monitor) ou volta. A reserva da AppBar continua.
    pub fn set_hidden(&mut self, hidden: bool) {
        self.hidden = hidden;
        if hidden {
            window::hide(self.hwnd);
        } else {
            window::show(self.hwnd);
        }
    }

    pub fn destroy(self) {
        if self.appbar {
            appbar::unregister(self.hwnd);
        }
        window::destroy(self.hwnd);
    }
}
