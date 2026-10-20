//! Forma arredondada com preenchimento, profundidade, contorno, sombra e fundo do Windows,
//! animada no compositor (pílula → painel).
//!
//! Os valores x, y, w, h e raio ficam num `CompositionPropertySet`; as molas animam esses escalares e
//! `ExpressionAnimation`s ligam a geometria (e as geometrias de recorte dos filhos) a eles. Durante a
//! animação nada roda na CPU do Zefiro, e uma mola nova sempre parte do valor atual.
//!
//! A sombra é a da ilha (`DrawSoftShadow`): 7 retângulos arredondados concêntricos com queda quadrática,
//! como formas vetoriais ligadas às mesmas propriedades. (Um `DropShadow` num `LayerVisual` deixava a
//! janela inteira preta; ver o ledger da Task 20.)

use windows::UI::Composition::*;
use windows::core::{HSTRING, Interface, Result};
use windows_numerics::{Vector2, Vector3};
use zefiro_platform::color::Rgba;
use zefiro_ui::bar_layout::FilletSide;
use zefiro_ui::panel_layout::{CONTENT_ALPHA_EXPRESSION, Shape};
use zefiro_ui::spring::SpringParams;

use crate::compositor::{Comp, ui_color};

#[derive(Debug, Clone, PartialEq)]
pub struct MorphStyle {
    pub fill: Rgba,
    pub depth: bool,
    pub border: Option<Rgba>,
    pub border_px: f32,
    /// (cor, espalhamento em px). A ilha usa clamp(14 × escala, 6, 18).
    pub shadow: Option<(Rgba, f32)>,
    pub backdrop: bool,
}

const SHADOW_STEPS: usize = 7;

pub struct MorphShape {
    root: ContainerVisual,
    shape: CompositionSpriteShape,
    shadows: Vec<(CompositionSpriteShape, CompositionRoundedRectangleGeometry)>,
    props: CompositionPropertySet,
    backdrop: SpriteVisual,
}

const KEYS: [&str; 5] = ["x", "y", "w", "h", "r"];

fn bind(comp: &Comp, props: &CompositionPropertySet, g: &CompositionRoundedRectangleGeometry, exprs: [&str; 3], scalars: &[(&str, f32)]) -> Result<()> {
    for (prop, expr) in ["Offset", "Size", "CornerRadius"].into_iter().zip(exprs) {
        let e = comp.compositor.CreateExpressionAnimationWithExpression(&HSTRING::from(expr))?;
        e.SetReferenceParameter(&HSTRING::from("p"), &props.cast::<CompositionObject>()?)?;
        for (name, v) in scalars {
            e.SetScalarParameter(&HSTRING::from(*name), *v)?;
        }
        g.StartAnimation(&HSTRING::from(prop), &e)?;
    }
    Ok(())
}

fn bind_geometry(comp: &Comp, props: &CompositionPropertySet, g: &CompositionRoundedRectangleGeometry) -> Result<()> {
    bind(comp, props, g, ["Vector2(p.x, p.y)", "Vector2(p.w, p.h)", "Vector2(p.r, p.r)"], &[])
}

/// Anel da sombra (como em `DrawSoftShadow`): cresce `G` para os lados, `0,55·G` para cima e
/// `G` para baixo, deslocado `Y` para baixo.
fn bind_shadow(comp: &Comp, props: &CompositionPropertySet, g: &CompositionRoundedRectangleGeometry, grow: f32, y_offset: f32) -> Result<()> {
    bind(
        comp,
        props,
        g,
        ["Vector2(p.x - G, p.y - G * 0.55 + Y)", "Vector2(p.w + 2 * G, p.h + G * 1.55)", "Vector2(p.r + G, p.r + G)"],
        &[("G", grow), ("Y", y_offset)],
    )
}

impl MorphShape {
    pub fn new(comp: &Comp, parent: &ContainerVisual, style: &MorphStyle) -> Result<MorphShape> {
        let c = &comp.compositor;
        let props = c.CreatePropertySet()?;
        for k in KEYS {
            props.InsertScalar(&HSTRING::from(k), 0.0)?;
        }
        let shape_visual = c.CreateShapeVisual()?;
        shape_visual.SetRelativeSizeAdjustment(Vector2 { X: 1.0, Y: 1.0 })?;
        let shapes = shape_visual.Shapes()?;

        // Sombra: do anel externo para o interno, por baixo da forma.
        let mut shadows = Vec::new();
        for _ in 0..SHADOW_STEPS {
            let g = c.CreateRoundedRectangleGeometry()?;
            let s = c.CreateSpriteShapeWithGeometry(&g)?;
            shapes.Append(&s.cast::<CompositionShape>()?)?;
            shadows.push((s, g));
        }

        let geometry = c.CreateRoundedRectangleGeometry()?;
        bind_geometry(comp, &props, &geometry)?;
        let shape = c.CreateSpriteShapeWithGeometry(&geometry)?;
        shapes.Append(&shape.cast::<CompositionShape>()?)?;

        // Fundo do Windows, recortado pela forma, por baixo de tudo.
        let backdrop = c.CreateSpriteVisual()?;
        backdrop.SetRelativeSizeAdjustment(Vector2 { X: 1.0, Y: 1.0 })?;
        backdrop.SetBrush(&c.CreateHostBackdropBrush()?)?;

        let root = c.CreateContainerVisual()?;
        root.SetRelativeSizeAdjustment(Vector2 { X: 1.0, Y: 1.0 })?;
        root.Children()?.InsertAtTop(&backdrop)?;
        root.Children()?.InsertAtTop(&shape_visual)?;
        parent.Children()?.InsertAtTop(&root)?;

        let m = MorphShape { root, shape, shadows, props, backdrop };
        m.clip_child(comp, &m.backdrop.cast()?)?;
        m.set_style(comp, style)?;
        Ok(m)
    }

    pub fn root(&self) -> &ContainerVisual {
        &self.root
    }

    pub fn set_style(&self, comp: &Comp, style: &MorphStyle) -> Result<()> {
        let c = &comp.compositor;
        let fill = style.fill;
        if style.depth {
            // Profundidade: escurece suavemente para baixo (6 % em tema claro, 14 % em escuro).
            let dark = fill.relative_luminance() < 0.45;
            let bottom = fill.mix(Rgba::BLACK.with_alpha(fill.a), if dark { 0.14 } else { 0.06 });
            let g = c.CreateLinearGradientBrush()?;
            g.SetStartPoint(Vector2 { X: 0.0, Y: 0.0 })?;
            g.SetEndPoint(Vector2 { X: 0.0, Y: 1.0 })?;
            g.ColorStops()?.Append(&c.CreateColorGradientStopWithOffsetAndColor(0.0, ui_color(fill))?)?;
            g.ColorStops()?.Append(&c.CreateColorGradientStopWithOffsetAndColor(1.0, ui_color(bottom))?)?;
            self.shape.SetFillBrush(&g)?;
        } else {
            self.shape.SetFillBrush(&c.CreateColorBrushWithColor(ui_color(fill))?)?;
        }
        match style.border {
            Some(b) => {
                self.shape.SetStrokeBrush(&c.CreateColorBrushWithColor(ui_color(b))?)?;
                self.shape.SetStrokeThickness(style.border_px)?;
            }
            None => self.shape.SetStrokeThickness(0.0)?,
        }
        // Sombra da ilha: anel i de 7, t = i/7, alfa = a · (1 − t)² · 0,55.
        for (idx, (s, g)) in self.shadows.iter().enumerate() {
            let i = SHADOW_STEPS - idx; // o primeiro (mais embaixo) é o anel externo
            let t = i as f32 / SHADOW_STEPS as f32;
            match style.shadow {
                Some((color, spread)) => {
                    let alpha = color.a * (1.0 - t) * (1.0 - t) * 0.55;
                    s.SetFillBrush(&c.CreateColorBrushWithColor(ui_color(color.with_alpha(alpha)))?)?;
                    bind_shadow(comp, &self.props, g, spread * t, spread * 0.35)?;
                }
                None => s.SetFillBrush(None::<&CompositionBrush>)?,
            }
        }
        self.backdrop.SetIsVisible(style.backdrop)?;
        Ok(())
    }

    pub fn set(&self, shape: Shape) -> Result<()> {
        let values = [shape.rect.x, shape.rect.y, shape.rect.w, shape.rect.h, shape.radius];
        for (k, v) in KEYS.iter().zip(values) {
            self.props.StopAnimation(&HSTRING::from(*k))?;
            self.props.InsertScalar(&HSTRING::from(*k), v)?;
        }
        Ok(())
    }

    pub fn animate(&self, comp: &Comp, shape: Shape, p: SpringParams) -> Result<()> {
        let values = [shape.rect.x, shape.rect.y, shape.rect.w, shape.rect.h, shape.radius];
        for (k, v) in KEYS.iter().zip(values) {
            self.props.StartAnimation(&HSTRING::from(*k), &comp.scalar_spring(v, p)?)?;
        }
        Ok(())
    }

    /// "Cutucada": desloca a forma inteira `from_y` px e volta a 0 com a mola.
    pub fn nudge(&self, comp: &Comp, from_y: f32, p: SpringParams) -> Result<()> {
        self.root.SetOffset(Vector3 { X: 0.0, Y: from_y, Z: 0.0 })?;
        self.root.StartAnimation(&HSTRING::from("Offset.Y"), &comp.scalar_spring(0.0, p)?)
    }

    pub fn set_opacity(&self, v: f32) -> Result<()> {
        self.root.SetOpacity(v)
    }

    pub fn animate_opacity(&self, comp: &Comp, v: f32, p: SpringParams) -> Result<()> {
        self.root.StartAnimation(&HSTRING::from("Opacity"), &comp.scalar_spring(v, p)?)
    }

    /// Recorta `child` pela forma atual (ligado por expressão: acompanha a animação).
    pub fn clip_child(&self, comp: &Comp, child: &Visual) -> Result<()> {
        let g = comp.compositor.CreateRoundedRectangleGeometry()?;
        bind_geometry(comp, &self.props, &g)?;
        child.SetClip(&comp.compositor.CreateGeometricClipWithGeometry(&g)?)
    }

    /// Prende uma concordância (`fillet`, de lado `size`) na lateral da forma, logo abaixo do trilho
    /// (`rail`): segue `x` e `w` por expressão, então acompanha a mola da ilha sem a CPU.
    pub fn bind_offset(&self, comp: &Comp, fillet: &Visual, side: FilletSide, size: f32, rail: f32) -> Result<()> {
        let expr = match side {
            FilletSide::Left => "Vector3(p.x - S, R, 0)",
            FilletSide::Right => "Vector3(p.x + p.w, R, 0)",
        };
        let e = comp.compositor.CreateExpressionAnimationWithExpression(&HSTRING::from(expr))?;
        e.SetReferenceParameter(&HSTRING::from("p"), &self.props.cast::<CompositionObject>()?)?;
        e.SetScalarParameter(&HSTRING::from("S"), size)?;
        e.SetScalarParameter(&HSTRING::from("R"), rail)?;
        fillet.StartAnimation(&HSTRING::from("Offset"), &e)
    }

    /// Opacidade de `child` = regra da ilha sobre a altura atual da forma.
    pub fn bind_alpha(&self, comp: &Comp, child: &Visual, final_h: f32) -> Result<()> {
        let expr = CONTENT_ALPHA_EXPRESSION.replace("g.Size.Y", "p.h");
        let e = comp.compositor.CreateExpressionAnimationWithExpression(&HSTRING::from(expr))?;
        e.SetReferenceParameter(&HSTRING::from("p"), &self.props.cast::<CompositionObject>()?)?;
        e.SetScalarParameter(&HSTRING::from("H"), final_h)?;
        child.StartAnimation(&HSTRING::from("Opacity"), &e)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::compositor::Comp;
    use crate::gfx::Gfx;
    use zefiro_platform::geometry::Rect;
    use zefiro_ui::panel_layout::Shape;
    use zefiro_ui::spring::SpringParams;

    #[test]
    fn creates_sets_and_animates_without_a_window() {
        let gfx = Gfx::new().unwrap();
        let comp = Comp::new(&gfx).unwrap();
        let parent = comp.compositor.CreateContainerVisual().unwrap();
        let style = MorphStyle { fill: Rgba::rgb(8, 8, 10), depth: true, border: Some(Rgba::rgb(30, 30, 34)), border_px: 1.0, shadow: Some((Rgba::BLACK.with_alpha(0.55), 14.0)), backdrop: false };
        let m = MorphShape::new(&comp, &parent, &style).unwrap();
        m.set(Shape { rect: Rect::new(10.0, 6.0, 120.0, 32.0), radius: 16.0 }).unwrap();
        m.animate(&comp, Shape { rect: Rect::new(0.0, 50.0, 352.0, 186.0), radius: 24.0 }, SpringParams::GROW).unwrap();
        let child = comp.compositor.CreateSpriteVisual().unwrap();
        m.clip_child(&comp, &child.cast().unwrap()).unwrap();
        m.bind_alpha(&comp, &child.cast().unwrap(), 186.0).unwrap();
        assert_eq!(parent.Children().unwrap().Count().unwrap(), 1);
    }
}
