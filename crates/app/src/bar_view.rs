//! Uma barra por monitor: faixa, pílulas ou ilhas, chips e transições.

use windows::UI::Composition::Desktop::DesktopWindowTarget;
use windows::UI::Composition::{CompositionDrawingSurface, ContainerVisual, SpriteVisual, Visual};
use windows::Win32::Foundation::HWND;
use windows::core::{Interface, Result};
use windows_numerics::{Vector2, Vector3};
use zefiro_bar::chips::{ChipContent, clock_chip, weather_chip, zefiro_chip};
use zefiro_bar::devices::{ICON_BRIGHTNESS, Level, level_chip, volume_level, with_svg_icon};
use zefiro_resources::icons;
use zefiro_core::config::BarHeight;
use zefiro_integration_clock::format::ClockText;
use zefiro_integration_clock::windows::now_local;
use zefiro_platform::geometry::{Point, Rect};
use zefiro_platform_windows::compositor::ui_color;
use zefiro_platform_windows::monitors::Monitor;
use zefiro_platform_windows::fillet::FilletShape;
use zefiro_platform_windows::morph::MorphShape;
use zefiro_platform_windows::{appbar, island_zorder, window};
use zefiro_ui::bar_layout::{BarLayout, ChipSpec, FilletSide, layout_sections, metrics};
use zefiro_ui::panel_layout::{CHIP_FADE_S, SHADOW_MARGIN_DIP, Shape, pill_radius};
use zefiro_ui::spring::SpringParams;
use zefiro_ui::paint::{DisplayList, DrawOp};
use zefiro_platform::geometry::Size;
use zefiro_platform::text::{Align, Family, FontWeight, TextStyle};

use crate::app::AppCtx;
use crate::island_view::IslandView;
use crate::theme_state::SurfaceKind;
use zefiro_island::view::{Collapsed, PillContent, RESERVE_DESIGN_W, activity_pill, idle_pill, media_pill, player_idle_pill, timer_pill, with_privacy};

/// Id do "chip" da ilha embutida (clique, painel e foco usam o mesmo caminho dos chips).
pub const ISLAND_CHIP: &str = "island";
/// Chip do player: a pílula de mídia da ilha num chip comum, em qualquer seção e monitor.
pub const PLAYER_CHIP: &str = "player";

struct ChipView { id: String, hover: MorphShape, content: SpriteVisual, surface: CompositionDrawingSurface, rect: Rect, pos: windows::UI::Composition::CompositionPropertySet }

/// Grupo de uma seção: pílula, ou ilha com as duas concordâncias (criadas no primeiro uso das ilhas).
struct GroupView { shape: MorphShape, fillets: Vec<FilletShape> }

pub struct BarView {
    hwnd: HWND,
    monitor: Monitor,
    scale: f32,
    _target: DesktopWindowTarget,
    root: ContainerVisual,
    strip: MorphShape,
    divider: SpriteVisual,
    /// Início, centro e fim, nesta ordem (`BarLayout::group_slot`).
    groups: Vec<GroupView>,
    /// Pedaços do trilho do formato ilhas, da esquerda para a direita.
    rails: Vec<MorphShape>,
    chips: Vec<ChipView>,
    layout: Option<BarLayout>,
    appbar: bool,
    hover: Option<String>,
    active: Option<String>,
    guides: Option<(CompositionDrawingSurface, SpriteVisual)>,
    drag_ghost: Option<SpriteVisual>,
    drag_marker: Option<SpriteVisual>,
    island: Option<IslandView>,
    /// Pílula do chip do player, quando ele está nesta barra.
    player: Option<IslandView>,
    /// Mouse em cima do player desta barra (espia só ele).
    player_hover: bool,
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

/// Pílula do player agora: só mídia, sem avisos (`hover` = o mouse em cima deste chip).
fn player_content(ctx: &AppCtx, pill_h: f32, hover: bool) -> PillContent {
    let font_scale = ctx.theme.theme.font.scale;
    let tokens = &ctx.theme.tokens;
    match (ctx.island.player(hover, ctx.now()), ctx.island.media()) {
        (Collapsed::Peek, Some(s)) => media_pill(s, true, pill_h, font_scale, tokens),
        (Collapsed::Media, Some(s)) => media_pill(s, false, pill_h, font_scale, tokens),
        _ => player_idle_pill(pill_h, tokens),
    }
}

/// Conteúdo do chip do player: a pílula fica numa `IslandView` própria; a superfície do chip só
/// guarda a nota, que aparece no fantasma do arraste do modo de edição.
fn player_chip(pill: &PillContent, pad: f32, chip_h: f32, font_px: f32, tokens: &zefiro_resources::tokens::Tokens) -> ChipContent {
    let w = (pill.size.w - 2.0 * pad).max(1.0);
    let style = TextStyle { size: font_px * 1.05, weight: FontWeight::Regular, family: Family::Icons };
    let ops = vec![DrawOp::Text { rect: Rect::new(0.0, 0.0, w, chip_h), text: "\u{E8D6}".into(), style, color: tokens.text_secondary, align: Align::Center }];
    ChipContent { content_w: w, list: DisplayList { size: Size { w, h: chip_h }, ops } }
}

/// Ícone dos chips de VS Code, Monitor do PC, Terminal e Energia (só ícone).
fn icon_glyph(id: &str) -> &'static str {
    match id {
        "vscode" => "\u{E8B7}",
        "hardware" => "\u{E950}",
        "terminal" => "\u{E756}",
        _ => "\u{E7E8}",
    }
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
        // Claude, Codex e VS Code: o ícone da marca (SVG) no lugar do glifo.
        "codex" => with_svg_icon(zefiro_bar::usage::usage_chip(crate::usage_loop::CODEX_GLYPH, ctx.usage.codex.as_ref().and_then(|(_, u)| crate::usage_loop::codex_percent(u)), chip_h, font_px, scale, t, &ctx.text), icons::CODEX),
        "claude" => with_svg_icon(zefiro_bar::usage::usage_chip(crate::usage_loop::CLAUDE_GLYPH, ctx.usage.claude_chip(), chip_h, font_px, scale, t, &ctx.text), icons::CLAUDE_CODE),
        "vscode" => with_svg_icon(level_chip(Level { glyph: icon_glyph(id), text: "", dim: false }, chip_h, font_px, scale, t, &ctx.text), icons::VSCODE),
        "notifications" => zefiro_bar::chips::bell_chip(ctx.system.notifications, chip_h, font_px, scale, t, &ctx.text),
        "dnd" => zefiro_bar::chips::dnd_chip(ctx.system.dnd, chip_h, font_px, t),
        "weather" => weather_chip(ctx.island.weather.as_ref().map(|w| (zefiro_island::weather::icon(w.code), w.temp_c, w.city.as_str())), chip_h, font_px, scale, t, &ctx.text),
        "hardware" | "power" | "terminal" => level_chip(Level { glyph: icon_glyph(id), text: "", dim: false }, chip_h, font_px, scale, t, &ctx.text),
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
        let mut view = BarView { hwnd, monitor: monitor.clone(), scale, _target: target, root, strip, divider, groups, rails: Vec::new(), chips: Vec::new(), layout: None, appbar: false, hover: None, active: None, guides: None, drag_ghost: None, drag_marker: None, island: None, player: None, player_hover: false, hidden: false, interactive_base: Vec::new() };
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
        let pad = (m.chip_pad_x * self.scale).round();
        let mut contents: Vec<(String, ChipContent)> = sections
            .ids()
            .filter(|id| zefiro_bar::catalog::descriptor(id).is_some())
            .map(|id| {
                let content = if id == PLAYER_CHIP {
                    player_chip(&player_content(ctx, chip_h, self.player_hover), pad, chip_h, font_px, &ctx.theme.tokens)
                } else {
                    chip_content(ctx, id, &self.monitor.device, cfg.height, chip_h, font_px, self.scale)
                };
                (id.to_string(), content)
            })
            .collect();
        let spec = |ids: &[String]| {
            ids.iter().filter(|id| zefiro_bar::catalog::descriptor(id).is_some())
                .map(|id| ChipSpec { id: id.clone(), content_w: contents.iter().find(|(c, _)| c == id).map_or(0.0, |(_, c)| c.content_w), wide: id == PLAYER_CHIP })
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
        // Um grupo por seção (início, centro, fim) e um pedaço do trilho por vão entre as ilhas.
        while self.groups.len() < 3 {
            let shape = MorphShape::new(&ctx.comp, &self.root, &ctx.theme.morph_style(self.scale, SurfaceKind::Pill))?;
            ctx.comp.place_background(&self.root, &shape.root().cast()?, &self.divider.cast()?)?;
            shape.set_opacity(0.0)?;
            self.groups.push(GroupView { shape, fillets: Vec::new() });
        }
        let island_style = ctx.theme.morph_style(self.scale, SurfaceKind::Island);
        if let Some(islands) = &layout.islands {
            while self.rails.len() < islands.rails.len() {
                let rail = MorphShape::new(&ctx.comp, &self.root, &island_style)?;
                ctx.comp.place_background(&self.root, &rail.root().cast()?, &self.divider.cast()?)?;
                rail.set_opacity(0.0)?;
                self.rails.push(rail);
            }
            for g in self.groups.iter_mut().filter(|g| g.fillets.is_empty()) {
                for side in [FilletSide::Left, FilletSide::Right] {
                    let fillet = FilletShape::new(&ctx.comp, &self.root, side, islands.fillet, &island_style)?;
                    ctx.comp.place_background(&self.root, &fillet.root().cast()?, &self.divider.cast()?)?;
                    fillet.set_opacity(0.0)?;
                    g.fillets.push(fillet);
                }
            }
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
        let group_style = if layout.islands.is_some() { island_style.clone() } else { ctx.theme.morph_style(self.scale, SurfaceKind::Pill) };
        for g in &self.groups {
            g.shape.set_style(&ctx.comp, &group_style)?;
            for f in &g.fillets {
                f.set_style(&ctx.comp, &island_style)?;
            }
        }
        for rail in &self.rails {
            rail.set_style(&ctx.comp, &island_style)?;
        }
        self.divider.SetBrush(&ctx.comp.compositor.CreateColorBrushWithColor(ui_color(ctx.theme.tokens.hairline))?)?;

        let animate = animate && ctx.theme.animations;
        let spring = ctx.theme.spring(SpringParams::SETTLE);
        let place = |shape: &MorphShape, s: Shape| if animate { shape.animate(&ctx.comp, s, spring) } else { shape.set(s) };
        let fade = |shape: &MorphShape, v: f32| if animate { shape.animate_opacity(&ctx.comp, v, spring) } else { shape.set_opacity(v) };
        // "Na hora" também é animação (1 ms), senão uma mola de opacidade em andamento grava por cima.
        let at_once = |visual: &ContainerVisual, v: f32| ctx.comp.fade(&visual.cast::<Visual>()?, v, 0.001);
        let style = ctx.theme.theme.shape.style;
        let radius_max = ctx.theme.theme.shape.radius_max * self.scale;
        // Da faixa para as pílulas ou ilhas, os grupos e o trilho nascem do retângulo da faixa.
        let was_strip = self.layout.as_ref().is_none_or(|l| l.groups.is_empty());
        let band_rect = |r: Rect| Shape { rect: Rect::new(r.x, layout.band.y, r.w, layout.band.h), radius: 0.0 };

        // Faixa; nas pílulas, o "resto" dela encolhe no centro; nas ilhas, ilhas e trilho nascem
        // cobrindo a faixa inteira, então ela some na hora.
        let pill_h = layout.island_pill_h;
        let center = Rect::new(((self.monitor.rect.w - pill_h * 4.0) / 2.0).round(), layout.band.y + ((layout.band.h - pill_h) / 2.0).round(), pill_h * 4.0, pill_h);
        match (layout.strip, &layout.islands) {
            (Some(band), _) => {
                place(&self.strip, Shape { rect: band, radius: 0.0 })?;
                fade(&self.strip, 1.0)?;
            }
            (None, Some(_)) => at_once(self.strip.root(), 0.0)?,
            (None, None) => {
                place(&self.strip, Shape { rect: center, radius: pill_h / 2.0 })?;
                fade(&self.strip, 0.0)?;
            }
        }
        let line = self.scale.max(1.0);
        self.divider.SetOffset(Vector3 { X: 0.0, Y: layout.band.bottom() - line, Z: 0.0 })?;
        self.divider.SetSize(Vector2 { X: self.monitor.rect.w, Y: line })?;
        self.divider.SetOpacity(if layout.strip.is_some() { 1.0 } else { 0.0 })?;

        // Grupos, cada um no lugar da sua seção. Um grupo que não existia nasce no lugar final.
        let mut targets: [Option<Rect>; 3] = [None; 3];
        for r in &layout.groups {
            targets[layout.group_slot(*r)] = Some(*r);
        }
        let mut before: [bool; 3] = [false; 3];
        if let Some(prev) = &self.layout {
            for r in &prev.groups {
                before[prev.group_slot(*r)] = true;
            }
        }
        for (slot, g) in self.groups.iter_mut().enumerate() {
            match targets[slot] {
                Some(r) => {
                    let target = layout.group_shape(r, pill_radius(r.h, radius_max, style, self.scale));
                    if was_strip {
                        g.shape.set(band_rect(r))?;
                    } else if !before[slot] {
                        g.shape.set(target)?;
                    }
                    place(&g.shape, target)?;
                    if was_strip && layout.islands.is_some() { at_once(g.shape.root(), 1.0)? } else { fade(&g.shape, 1.0)? }
                }
                None => fade(&g.shape, 0.0)?,
            }
            for f in &mut g.fillets {
                let shown = match (&layout.islands, targets[slot]) {
                    (Some(islands), Some(r)) => {
                        f.resize(&ctx.comp, islands.fillet)?;
                        g.shape.bind_offset(&ctx.comp, &f.root().cast()?, f.side(), islands.fillet, islands.rail_h)?;
                        islands.fillets.iter().any(|x| x.side == f.side() && if x.side == FilletSide::Left { x.rect.right() == r.x } else { x.rect.x == r.right() })
                    }
                    _ => false,
                };
                let v = if shown { 1.0 } else { 0.0 };
                if animate { f.animate_opacity(&ctx.comp, v, spring)? } else { f.set_opacity(v)? }
            }
        }

        // Trilho: um pedaço por vão; vindo da faixa, cada pedaço nasce da altura dela e encolhe.
        let rails = layout.islands.as_ref().map_or(&[][..], |i| &i.rails[..]);
        let rails_before = self.layout.as_ref().and_then(|l| l.islands.as_ref()).map_or(0, |i| i.rails.len());
        for (i, rail) in self.rails.iter().enumerate() {
            match rails.get(i) {
                Some(r) => {
                    let target = Shape { rect: *r, radius: 0.0 };
                    if was_strip {
                        rail.set(band_rect(*r))?;
                        at_once(rail.root(), 1.0)?;
                    } else if i >= rails_before {
                        rail.set(target)?;
                    }
                    place(rail, target)?;
                    fade(rail, 1.0)?;
                }
                None => fade(rail, 0.0)?,
            }
        }

        // Chips: cria os que faltam, redesenha o conteúdo e move.
        for (id, content) in &contents {
            let Some(b) = layout.chips.iter().find(|c| &c.id == id) else { continue };
            let is_new = !self.chips.iter().any(|c| &c.id == id);
            if is_new {
                let hover = MorphShape::new(&ctx.comp, &self.root, &ctx.theme.morph_style(self.scale, SurfaceKind::Hover))?;
                hover.set_opacity(0.0)?;
                let surface = ctx.comp.surface(content.list.size.w, content.list.size.h)?;
                let sprite = ctx.comp.sprite_for(&surface, content.list.size.w, content.list.size.h)?;
                if id == PLAYER_CHIP {
                    // Quem aparece é a pílula; a nota da superfície é só para o arraste.
                    sprite.SetOpacity(0.0)?;
                }
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
        interactive.extend(layout.islands.iter().flat_map(|i| i.rails.iter().copied()));
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
        let has_player = self.layout.as_ref().is_some_and(|l| l.chips.iter().any(|c| c.id == PLAYER_CHIP));
        match (has_player, self.player.is_some()) {
            (true, false) => self.player = Some(IslandView::new(ctx, &self.root, &ctx.theme.morph_style(self.scale, SurfaceKind::Strip))?),
            (false, true) => {
                if let Some(view) = self.player.take() {
                    self.root.Children()?.Remove(view.root())?;
                }
            }
            _ => {}
        }
        self.refresh_island(ctx, animate)?;
        self.refresh_player(ctx, animate)?;
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
        let rect = Rect::new(((self.monitor.rect.w - content.size.w) / 2.0).round(), layout.island_pill_y(), content.size.w, pill_h);
        // Na faixa e nas ilhas, a ilha se funde à barra (sem contorno nem sombra); nas pílulas, é uma pílula.
        let kind = if layout.islands.is_some() { SurfaceKind::Island } else if layout.merges_island() { SurfaceKind::Strip } else { SurfaceKind::Pill };
        let style = ctx.theme.morph_style(self.scale, kind);
        let radius = pill_radius(pill_h, ctx.theme.theme.shape.radius_max * self.scale, ctx.theme.theme.shape.style, self.scale);
        view.update(ctx, content, rect, radius, &style, animate && ctx.theme.animations)?;
        let mut interactive = self.interactive_base.clone();
        interactive.push(rect);
        window::set_interactive(self.hwnd, interactive);
        Ok(())
    }

    /// Redesenha a pílula do player no retângulo do chip. Na faixa ou nas pílulas, ela se funde à
    /// barra, como a ilha na faixa.
    pub fn refresh_player(&mut self, ctx: &AppCtx, animate: bool) -> Result<()> {
        let (Some(layout), Some(view)) = (&self.layout, &mut self.player) else { return Ok(()) };
        let Some(chip) = layout.chips.iter().find(|c| c.id == PLAYER_CHIP) else { return Ok(()) };
        let content = player_content(ctx, chip.rect.h, self.player_hover);
        let rect = Rect::new(chip.rect.x, chip.rect.y, content.size.w, chip.rect.h);
        let style = ctx.theme.morph_style(self.scale, if layout.islands.is_some() { SurfaceKind::Island } else { SurfaceKind::Strip });
        let radius = pill_radius(rect.h, ctx.theme.theme.shape.radius_max * self.scale, ctx.theme.theme.shape.style, self.scale);
        view.update(ctx, content, rect, radius, &style, animate && ctx.theme.animations)
    }

    /// A pílula do player mudou de largura (parado, mídia, espiando): o chip precisa de outro layout.
    fn player_resized(&self, ctx: &AppCtx) -> bool {
        let Some(chip) = self.layout.as_ref().and_then(|l| l.chips.iter().find(|c| c.id == PLAYER_CHIP)) else { return false };
        player_content(ctx, chip.rect.h, self.player_hover).size.w != chip.rect.w
    }

    /// Mídia, espiada ou aviso mudaram: redesenha a ilha e o player. Se o player mudou de largura,
    /// refaz o layout (os vizinhos deslizam com a mola).
    pub fn refresh_media(&mut self, ctx: &AppCtx, animate: bool) -> Result<()> {
        if self.player_resized(ctx) {
            return self.apply(ctx, animate);
        }
        self.refresh_island(ctx, animate)?;
        self.refresh_player(ctx, animate)
    }

    /// Mouse entrou ou saiu do player desta barra. `true` se mudou.
    pub fn set_player_hover(&mut self, on: bool) -> bool {
        let changed = self.player_hover != on;
        self.player_hover = on;
        changed
    }

    /// Barras do visualizador (`None` = paradas), na ilha e no player.
    pub fn set_visualizer(&self, wave: Option<&zefiro_island::waveform::Waveform>) -> Result<()> {
        for view in [&self.island, &self.player].into_iter().flatten() {
            view.set_levels(&wave.map(|w| w.bars(view.bar_count())).unwrap_or_default())?;
        }
        Ok(())
    }

    pub fn has_island(&self) -> bool {
        self.island.is_some()
    }

    /// Esta barra mostra a mídia (ilha ou player): a captura do som serve a ela.
    pub fn shows_media(&self) -> bool {
        self.island.is_some() || self.player.is_some()
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
            // O player é uma pílula, como a ilha: sem fundo de hover nem de ativo.
            let player = c.id == PLAYER_CHIP;
            let active = !player && self.active.as_deref() == Some(&c.id);
            let hovered = !player && self.hover.as_deref() == Some(&c.id);
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
        if chip == PLAYER_CHIP {
            if let Some(view) = &self.player {
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
