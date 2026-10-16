//! A ilha embutida no centro da barra: a pílula (forma com mola), o conteúdo recolhido e as barras
//! do visualizador, tudo no compositor da própria barra.

use windows::UI::Composition::{CompositionColorBrush, CompositionDrawingSurface, ContainerVisual, SpriteVisual, Visual};
use windows::core::{HSTRING, Interface, Result};
use windows_numerics::{Vector2, Vector3};
use zefiro_island::view::{PillContent, visualizer_bars};
use zefiro_platform::color::Rgba;
use zefiro_platform::geometry::{Point, Rect};
use zefiro_platform_windows::compositor::ui_color;
use zefiro_platform_windows::morph::{MorphShape, MorphStyle};
use zefiro_ui::panel_layout::Shape;
use zefiro_ui::spring::SpringParams;

use crate::app::AppCtx;

pub struct IslandView {
    shape: MorphShape,
    /// Do tamanho da barra, recortado pela pílula: o conteúdo nunca sai dela durante a mola.
    holder: ContainerVisual,
    /// Conteúdo + visualizador; desloca junto com a pílula.
    inner: ContainerVisual,
    content: SpriteVisual,
    surface: CompositionDrawingSurface,
    bars: ContainerVisual,
    dots: ContainerVisual,
    bar_sprites: Vec<SpriteVisual>,
    brush: CompositionColorBrush,
    pub rect: Rect,
    vis: Option<Rect>,
    k: f32,
    drawn: Option<PillContent>,
    shown: bool,
}

impl IslandView {
    pub fn new(ctx: &AppCtx, parent: &ContainerVisual, style: &MorphStyle) -> Result<IslandView> {
        let c = &ctx.comp.compositor;
        let shape = MorphShape::new(&ctx.comp, parent, style)?;
        let holder = c.CreateContainerVisual()?;
        holder.SetRelativeSizeAdjustment(Vector2 { X: 1.0, Y: 1.0 })?;
        shape.root().Children()?.InsertAtTop(&holder)?;
        shape.clip_child(&ctx.comp, &holder.cast::<Visual>()?)?;
        let inner = c.CreateContainerVisual()?;
        holder.Children()?.InsertAtTop(&inner)?;
        let surface = ctx.comp.surface(1.0, 1.0)?;
        let content = ctx.comp.sprite_for(&surface, 1.0, 1.0)?;
        inner.Children()?.InsertAtTop(&content)?;
        let bars = c.CreateContainerVisual()?;
        inner.Children()?.InsertAtTop(&bars)?;
        let dots = c.CreateContainerVisual()?;
        inner.Children()?.InsertAtTop(&dots)?;
        let brush = c.CreateColorBrushWithColor(ui_color(ctx.theme.tokens.accent))?;
        Ok(IslandView { shape, holder, inner, content, surface, bars, dots, bar_sprites: Vec::new(), brush, rect: Rect::default(), vis: None, k: 1.0, drawn: None, shown: false })
    }

    pub fn root(&self) -> &ContainerVisual {
        self.shape.root()
    }

    /// Mostra `content` em `rect` (coordenadas da barra), animando a pílula até lá.
    pub fn update(&mut self, ctx: &AppCtx, content: PillContent, rect: Rect, radius: f32, style: &MorphStyle, animate: bool) -> Result<()> {
        self.shape.set_style(&ctx.comp, style)?;
        self.brush.SetColor(ui_color(ctx.theme.tokens.accent))?;
        let target = Shape { rect, radius };
        let first = !self.shown;
        if animate && !first {
            let spring = ctx.theme.spring(SpringParams::toward(self.rect.w, rect.w));
            self.shape.animate(&ctx.comp, target, spring)?;
            self.inner.StartAnimation(&HSTRING::from("Offset.X"), &ctx.comp.scalar_spring(rect.x, spring)?)?;
            self.inner.StartAnimation(&HSTRING::from("Offset.Y"), &ctx.comp.scalar_spring(rect.y, spring)?)?;
        } else {
            self.shape.set(target)?;
            self.inner.SetOffset(Vector3 { X: rect.x, Y: rect.y, Z: 0.0 })?;
        }
        self.shown = true;
        self.rect = rect;
        if self.drawn.as_ref() != Some(&content) {
            let size = content.size;
            ctx.comp.draw(&self.surface, size.w, size.h, &content.list, &ctx.text)?;
            self.content.SetSize(Vector2 { X: size.w, Y: size.h })?;
            self.k = rect.h / 44.0;
            self.place_bars(ctx, content.visualizer)?;
            if self.drawn.as_ref().map(|d| &d.dots) != Some(&content.dots) {
                self.place_dots(ctx, &content.dots)?;
            }
            self.drawn = Some(content);
        }
        Ok(())
    }

    fn place_bars(&mut self, ctx: &AppCtx, vis: Option<Rect>) -> Result<()> {
        if vis == self.vis {
            return Ok(());
        }
        self.vis = vis;
        let count = vis.map_or(0, |v| visualizer_bars(v, self.k));
        while self.bar_sprites.len() > count {
            if let Some(s) = self.bar_sprites.pop() {
                self.bars.Children()?.Remove(&s)?;
            }
        }
        while self.bar_sprites.len() < count {
            let s = ctx.comp.compositor.CreateSpriteVisual()?;
            s.SetBrush(&self.brush)?;
            self.bars.Children()?.InsertAtTop(&s)?;
            self.bar_sprites.push(s);
        }
        if let Some(v) = vis {
            self.bars.SetOffset(Vector3 { X: v.x, Y: v.y, Z: 0.0 })?;
        }
        self.set_levels(&[])
    }

    /// Pontos de privacidade pulsando (2 s por ciclo), só no compositor.
    fn place_dots(&mut self, ctx: &AppCtx, dots: &[(Point, f32, Rgba)]) -> Result<()> {
        self.dots.Children()?.RemoveAll()?;
        for (center, radius, color) in dots {
            let v = ctx.comp.dot(*radius, *color)?;
            v.SetOffset(Vector3 { X: center.x - radius, Y: center.y - radius, Z: 0.0 })?;
            self.dots.Children()?.InsertAtTop(&v)?;
            if ctx.theme.animations {
                ctx.comp.pulse(&v.cast::<Visual>()?, 0.35, 2.0)?;
            }
        }
        Ok(())
    }

    pub fn bar_count(&self) -> usize {
        self.bar_sprites.len()
    }

    /// Altura de cada barra (0 a 1). Vazio = barras paradas (pausado, sem som).
    pub fn set_levels(&self, levels: &[f32]) -> Result<()> {
        let Some(v) = self.vis else { return Ok(()) };
        let n = self.bar_sprites.len();
        if n == 0 {
            return Ok(());
        }
        let width = (2.0 * self.k).max(2.0).round();
        let pitch = v.w / n as f32;
        let min_h = (3.0 * self.k).max(2.0);
        for (i, s) in self.bar_sprites.iter().enumerate() {
            let level = levels.get(i).copied().unwrap_or(0.0).clamp(0.0, 1.0);
            // Raiz quadrada (desvio consciente da ilha C++, que é linear): fala baixa (0,1–0,3) ainda
            // mexe as barras, e música alta continua chegando ao topo.
            let h = (level.sqrt() * v.h * 0.86).max(min_h).round();
            s.SetSize(Vector2 { X: width, Y: h })?;
            s.SetOffset(Vector3 { X: (i as f32 * pitch + (pitch - width) / 2.0).round(), Y: ((v.h - h) / 2.0).round(), Z: 0.0 })?;
            s.SetOpacity(0.45 + 0.5 * level)?;
        }
        Ok(())
    }

    /// Esconde (painel da ilha aberto: a forma do painel nasce daqui) ou mostra o conteúdo.
    pub fn fade_content(&self, ctx: &AppCtx, visible: bool) -> Result<()> {
        ctx.comp.fade(&self.holder.cast::<Visual>()?, if visible { 1.0 } else { 0.0 }, zefiro_ui::panel_layout::CHIP_FADE_S)
    }
}
