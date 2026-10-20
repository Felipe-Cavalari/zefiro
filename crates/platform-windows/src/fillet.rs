//! Concordância do formato ilhas: o canto côncavo entre o trilho e a ilha (ADR-0016).
//!
//! A forma (um quadrado sem um quarto de círculo) é um caminho do Direct2D entregue ao compositor
//! (`CompositionPath`): preenche a cor do tema e recorta o fundo do Windows, então o acrílico também
//! aparece na concordância. (Um `CompositionMaskBrush` não aceita o fundo do Windows como fonte.) A
//! posição segue a ilha por expressão (`MorphShape::bind_offset`): durante a mola nada roda na CPU.

use windows::Graphics::IGeometrySource2D;
use windows::UI::Composition::*;
use windows::Win32::Graphics::Direct2D::Common::*;
use windows::Win32::Graphics::Direct2D::*;
use windows::Win32::System::WinRT::Graphics::Direct2D::{IGeometrySource2DInterop, IGeometrySource2DInterop_Impl};
use windows::core::{HSTRING, Interface, Ref, Result, implement};
use windows_numerics::Vector2;
use zefiro_ui::bar_layout::FilletSide;
use zefiro_ui::spring::SpringParams;

use crate::compositor::{Comp, ui_color};
use crate::morph::MorphStyle;

/// Entrega uma geometria do Direct2D ao compositor (`CompositionPath::Create`).
#[implement(IGeometrySource2D, IGeometrySource2DInterop)]
struct GeometrySource(ID2D1Geometry);

impl windows::Graphics::IGeometrySource2D_Impl for GeometrySource_Impl {}

impl IGeometrySource2DInterop_Impl for GeometrySource_Impl {
    fn GetGeometry(&self) -> Result<ID2D1Geometry> {
        Ok(self.0.clone())
    }

    fn TryGetGeometryUsingFactory(&self, _factory: Ref<'_, ID2D1Factory>) -> Result<ID2D1Geometry> {
        Err(windows::Win32::Foundation::E_NOTIMPL.into())
    }
}

/// O quadrado `size`×`size` cheio, menos o quarto de círculo de raio `size` com centro no canto de
/// baixo do lado oposto à ilha.
fn fillet_path(comp: &Comp, size: f32, side: FilletSide) -> Result<CompositionPathGeometry> {
    let p = |x: f32, y: f32| Vector2 { X: x, Y: y };
    // Left: a ilha fica à direita; o canto cheio é o de cima à direita e o arco vai de (s, s) a (0, 0).
    // Right: espelhado, de (0, s) a (s, 0).
    let (start, corner, arc_from, arc_to, sweep) = match side {
        FilletSide::Left => (p(0.0, 0.0), p(size, 0.0), p(size, size), p(0.0, 0.0), D2D1_SWEEP_DIRECTION_COUNTER_CLOCKWISE),
        FilletSide::Right => (p(size, 0.0), p(0.0, 0.0), p(0.0, size), p(size, 0.0), D2D1_SWEEP_DIRECTION_CLOCKWISE),
    };
    // SAFETY: fábrica e geometria locais; o sink é fechado antes de a geometria ser usada.
    let geometry: ID2D1Geometry = unsafe {
        let factory: ID2D1Factory = D2D1CreateFactory(D2D1_FACTORY_TYPE_SINGLE_THREADED, None)?;
        let path = factory.CreatePathGeometry()?;
        let sink = path.Open()?;
        sink.BeginFigure(start, D2D1_FIGURE_BEGIN_FILLED);
        sink.AddLine(corner);
        sink.AddLine(arc_from);
        sink.AddArc(&D2D1_ARC_SEGMENT { point: arc_to, size: D2D_SIZE_F { width: size, height: size }, rotationAngle: 0.0, sweepDirection: sweep, arcSize: D2D1_ARC_SIZE_SMALL });
        sink.EndFigure(D2D1_FIGURE_END_CLOSED);
        sink.Close()?;
        path.cast()?
    };
    let source: IGeometrySource2D = GeometrySource(geometry).into();
    comp.compositor.CreatePathGeometryWithPath(&CompositionPath::Create(&source)?)
}

pub struct FilletShape {
    root: ContainerVisual,
    shape: CompositionSpriteShape,
    backdrop: SpriteVisual,
    side: FilletSide,
    size: f32,
}

impl FilletShape {
    pub fn new(comp: &Comp, parent: &ContainerVisual, side: FilletSide, size: f32, style: &MorphStyle) -> Result<FilletShape> {
        let c = &comp.compositor;
        let backdrop = c.CreateSpriteVisual()?;
        backdrop.SetRelativeSizeAdjustment(Vector2 { X: 1.0, Y: 1.0 })?;
        backdrop.SetBrush(&c.CreateHostBackdropBrush()?)?;
        let shape = c.CreateSpriteShape()?;
        let shape_visual = c.CreateShapeVisual()?;
        shape_visual.SetRelativeSizeAdjustment(Vector2 { X: 1.0, Y: 1.0 })?;
        shape_visual.Shapes()?.Append(&shape.cast::<CompositionShape>()?)?;
        let root = c.CreateContainerVisual()?;
        root.Children()?.InsertAtTop(&backdrop)?;
        root.Children()?.InsertAtTop(&shape_visual)?;
        parent.Children()?.InsertAtTop(&root)?;
        let f = FilletShape { root, shape, backdrop, side, size: 0.0 };
        f.reshape(comp, size)?;
        f.set_style(comp, style)?;
        Ok(FilletShape { size, ..f })
    }

    fn reshape(&self, comp: &Comp, size: f32) -> Result<()> {
        let path = fillet_path(comp, size, self.side)?;
        self.shape.SetGeometry(&path)?;
        self.backdrop.SetClip(&comp.compositor.CreateGeometricClipWithGeometry(&path)?)?;
        self.root.SetSize(Vector2 { X: size, Y: size })
    }

    pub fn root(&self) -> &ContainerVisual {
        &self.root
    }

    pub fn side(&self) -> FilletSide {
        self.side
    }

    /// Cor e fundo do tema. Sem profundidade: num quadrado de 8 px ela só faria emenda com a ilha.
    pub fn set_style(&self, comp: &Comp, style: &MorphStyle) -> Result<()> {
        self.shape.SetFillBrush(&comp.compositor.CreateColorBrushWithColor(ui_color(style.fill))?)?;
        self.backdrop.SetIsVisible(style.backdrop)
    }

    /// Outra escala (monitor com outro DPI): refaz o caminho no tamanho novo.
    pub fn resize(&mut self, comp: &Comp, size: f32) -> Result<()> {
        if size != self.size {
            self.reshape(comp, size)?;
            self.size = size;
        }
        Ok(())
    }

    pub fn set_opacity(&self, v: f32) -> Result<()> {
        self.root.SetOpacity(v)
    }

    pub fn animate_opacity(&self, comp: &Comp, v: f32, p: SpringParams) -> Result<()> {
        self.root.StartAnimation(&HSTRING::from("Opacity"), &comp.scalar_spring(v, p)?)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::gfx::Gfx;
    use crate::morph::MorphShape;
    use zefiro_platform::color::Rgba;
    use zefiro_platform::geometry::Rect;
    use zefiro_ui::panel_layout::Shape;

    #[test]
    fn builds_both_sides_and_follows_the_island_without_a_window() {
        let gfx = Gfx::new().unwrap();
        let comp = Comp::new(&gfx).unwrap();
        let parent = comp.compositor.CreateContainerVisual().unwrap();
        let style = MorphStyle { fill: Rgba::rgb(8, 8, 10), depth: false, border: None, border_px: 0.0, shadow: None, backdrop: true };
        let island = MorphShape::new(&comp, &parent, &style).unwrap();
        island.set(Shape { rect: Rect::new(100.0, -16.0, 200.0, 52.0), radius: 16.0 }).unwrap();
        for side in [FilletSide::Left, FilletSide::Right] {
            let mut f = FilletShape::new(&comp, &parent, side, 8.0, &style).unwrap();
            island.bind_offset(&comp, &f.root().cast().unwrap(), side, 8.0, 4.0).unwrap();
            f.resize(&comp, 12.0).unwrap();
            assert_eq!(f.root().Size().unwrap(), Vector2 { X: 12.0, Y: 12.0 });
            f.set_style(&comp, &MorphStyle { backdrop: false, ..style.clone() }).unwrap();
            f.animate_opacity(&comp, 0.0, SpringParams::SETTLE).unwrap();
        }
        assert_eq!(parent.Children().unwrap().Count().unwrap(), 3);
    }
}
