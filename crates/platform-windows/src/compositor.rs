//! Windows.UI.Composition: fila de despacho, alvos de janela, superfícies de desenho e animações.

use windows::Foundation::{IReference, PropertyValue, TimeSpan, TypedEventHandler};
use windows::Graphics::DirectX::{DirectXAlphaMode, DirectXPixelFormat};
use windows::Graphics::SizeInt32;
use windows::System::DispatcherQueueController;
use windows::UI::Color;
use windows::UI::Composition::Desktop::DesktopWindowTarget;
use windows::UI::Composition::*;
use windows::Win32::Foundation::{HWND, POINT};
use windows::Win32::Graphics::Direct2D::Common::D2D1_COLOR_F;
use windows::Win32::Graphics::Direct2D::ID2D1DeviceContext;
use windows::Win32::System::WinRT::Composition::{ICompositionDrawingSurfaceInterop, ICompositorDesktopInterop, ICompositorInterop};
use windows::Win32::System::WinRT::{CreateDispatcherQueueController, DQTAT_COM_STA, DQTYPE_THREAD_CURRENT, DispatcherQueueOptions};
use windows::core::{HSTRING, Interface, Result};
use windows_numerics::{Matrix3x2, Vector2};
use zefiro_platform::color::Rgba;
use zefiro_ui::paint::DisplayList;
use zefiro_ui::spring::SpringParams;

use crate::gfx::Gfx;
use crate::painter::paint;
use crate::text::DWriteText;

thread_local! {
    /// O Windows só aceita uma fila de despacho por thread (bug achado no S1).
    static QUEUE: std::cell::RefCell<Option<DispatcherQueueController>> = const { std::cell::RefCell::new(None) };
}

pub fn thread_queue() -> Result<DispatcherQueueController> {
    QUEUE.with(|q| {
        if let Some(existing) = q.borrow().as_ref() {
            return Ok(existing.clone());
        }
        let options = DispatcherQueueOptions { dwSize: size_of::<DispatcherQueueOptions>() as u32, threadType: DQTYPE_THREAD_CURRENT, apartmentType: DQTAT_COM_STA };
        // SAFETY: cria a fila desta thread, exigida pelo Compositor.
        let created = unsafe { CreateDispatcherQueueController(options)? };
        *q.borrow_mut() = Some(created.clone());
        Ok(created)
    })
}

pub fn ui_color(c: Rgba) -> Color {
    let b = |v: f32| (v.clamp(0.0, 1.0) * 255.0).round() as u8;
    Color { A: b(c.a), R: b(c.r), G: b(c.g), B: b(c.b) }
}

pub struct Comp {
    _queue: DispatcherQueueController,
    pub compositor: Compositor,
    gdev: CompositionGraphicsDevice,
}

impl Comp {
    pub fn new(gfx: &Gfx) -> Result<Comp> {
        let queue = thread_queue()?;
        let compositor = Compositor::new()?;
        let interop: ICompositorInterop = compositor.cast()?;
        // SAFETY: dispositivo D2D válido.
        let gdev = unsafe { interop.CreateGraphicsDevice(&gfx.d2d_device)? };
        Ok(Comp { _queue: queue, compositor, gdev })
    }

    /// Alvo da janela (que precisa de WS_EX_NOREDIRECTIONBITMAP) e a raiz.
    pub fn target(&self, hwnd: HWND) -> Result<(DesktopWindowTarget, ContainerVisual)> {
        let desktop: ICompositorDesktopInterop = self.compositor.cast()?;
        // SAFETY: hwnd válido.
        let target = unsafe { desktop.CreateDesktopWindowTarget(hwnd, true)? };
        let root = self.compositor.CreateContainerVisual()?;
        target.SetRoot(&root)?;
        Ok((target, root))
    }

    pub fn surface(&self, w: f32, h: f32) -> Result<CompositionDrawingSurface> {
        self.gdev.CreateDrawingSurface(
            windows::Foundation::Size { Width: w.max(1.0), Height: h.max(1.0) },
            DirectXPixelFormat::B8G8R8A8UIntNormalized,
            DirectXAlphaMode::Premultiplied,
        )
    }

    pub fn draw(&self, surface: &CompositionDrawingSurface, w: f32, h: f32, list: &DisplayList, text: &DWriteText) -> Result<()> {
        let size = SizeInt32 { Width: w.ceil().max(1.0) as i32, Height: h.ceil().max(1.0) as i32 };
        if surface.SizeInt32()? != size {
            surface.Resize(size)?;
        }
        let interop: ICompositionDrawingSurfaceInterop = surface.cast()?;
        // SAFETY: BeginDraw/EndDraw pareados; o contexto só é usado entre eles.
        unsafe {
            let mut offset = POINT::default();
            let dc: ID2D1DeviceContext = interop.BeginDraw(None, &mut offset)?;
            dc.SetTransform(&Matrix3x2::translation(offset.x as f32, offset.y as f32));
            dc.Clear(Some(&D2D1_COLOR_F::default()));
            let result = paint(&dc, list, text);
            interop.EndDraw()?;
            result
        }
    }

    pub fn sprite_for(&self, surface: &CompositionDrawingSurface, w: f32, h: f32) -> Result<SpriteVisual> {
        let v = self.compositor.CreateSpriteVisual()?;
        v.SetBrush(&self.compositor.CreateSurfaceBrushWithSurface(surface)?)?;
        v.SetSize(Vector2 { X: w, Y: h })?;
        Ok(v)
    }

    /// Um fundo criado depois dos chips continua abaixo do conteúdo já existente.
    pub fn place_background(&self, parent: &ContainerVisual, background: &Visual, anchor: &Visual) -> Result<()> {
        parent.Children()?.Remove(background)?;
        parent.Children()?.InsertAbove(background, anchor)
    }

    pub fn scalar_spring(&self, value: f32, p: SpringParams) -> Result<SpringScalarNaturalMotionAnimation> {
        let c = p.compositor();
        let a = self.compositor.CreateSpringScalarAnimation()?;
        a.SetDampingRatio(c.damping_ratio)?;
        a.SetPeriod(TimeSpan { Duration: (c.period_s as f64 * 1e7) as i64 })?;
        a.SetFinalValue(&PropertyValue::CreateSingle(value)?.cast::<IReference<f32>>()?)?;
        Ok(a)
    }

    /// Posição animável por mola: `Offset` do visual ligado por expressão a `x`/`y` de um conjunto de
    /// propriedades (a mola escalar em `Offset.X` não move o visual; nas propriedades, sim).
    pub fn spring_offset(&self, visual: &Visual, x: f32, y: f32) -> Result<CompositionPropertySet> {
        let props = self.compositor.CreatePropertySet()?;
        props.InsertScalar(&HSTRING::from("x"), x)?;
        props.InsertScalar(&HSTRING::from("y"), y)?;
        let e = self.compositor.CreateExpressionAnimationWithExpression(&HSTRING::from("Vector3(p.x, p.y, 0)"))?;
        e.SetReferenceParameter(&HSTRING::from("p"), &props.cast::<CompositionObject>()?)?;
        visual.StartAnimation(&HSTRING::from("Offset"), &e)?;
        Ok(props)
    }

    /// Move uma posição de `spring_offset`: com mola (parte do valor atual) ou na hora.
    pub fn move_offset(&self, props: &CompositionPropertySet, x: f32, y: f32, spring: Option<SpringParams>) -> Result<()> {
        for (k, v) in [("x", x), ("y", y)] {
            match spring {
                Some(p) => props.StartAnimation(&HSTRING::from(k), &self.scalar_spring(v, p)?)?,
                None => {
                    props.StopAnimation(&HSTRING::from(k))?;
                    props.InsertScalar(&HSTRING::from(k), v)?;
                }
            }
        }
        Ok(())
    }

    pub fn fade(&self, visual: &Visual, to: f32, seconds: f32) -> Result<()> {
        self.fade_after(visual, to, seconds, 0.0)
    }

    /// Como `fade`, mas começa depois de `delay_s` segundos.
    pub fn fade_after(&self, visual: &Visual, to: f32, seconds: f32, delay_s: f32) -> Result<()> {
        let a = self.compositor.CreateScalarKeyFrameAnimation()?;
        a.InsertKeyFrame(1.0, to)?;
        a.SetDuration(TimeSpan { Duration: (seconds as f64 * 1e7) as i64 })?;
        a.SetDelayTime(TimeSpan { Duration: (delay_s as f64 * 1e7) as i64 })?;
        visual.StartAnimation(&HSTRING::from("Opacity"), &a)
    }

    /// Pulso de opacidade sem fim (1 → `low` → 1 a cada `period_s`), rodando só no compositor.
    pub fn pulse(&self, visual: &Visual, low: f32, period_s: f32) -> Result<()> {
        let a = self.compositor.CreateScalarKeyFrameAnimation()?;
        a.InsertKeyFrame(0.0, 1.0)?;
        a.InsertKeyFrame(0.5, low)?;
        a.InsertKeyFrame(1.0, 1.0)?;
        a.SetDuration(TimeSpan { Duration: (period_s as f64 * 1e7) as i64 })?;
        a.SetIterationBehavior(AnimationIterationBehavior::Forever)?;
        visual.StartAnimation(&HSTRING::from("Opacity"), &a)
    }

    /// Círculo cheio (ponto de privacidade) centrado em (0, 0) do visual devolvido.
    pub fn dot(&self, radius: f32, color: Rgba) -> Result<ShapeVisual> {
        let c = &self.compositor;
        let g = c.CreateEllipseGeometry()?;
        g.SetRadius(Vector2 { X: radius, Y: radius })?;
        g.SetCenter(Vector2 { X: radius, Y: radius })?;
        let s = c.CreateSpriteShapeWithGeometry(&g)?;
        s.SetFillBrush(&c.CreateColorBrushWithColor(ui_color(color))?)?;
        let v = c.CreateShapeVisual()?;
        v.Shapes()?.Append(&s.cast::<CompositionShape>()?)?;
        v.SetSize(Vector2 { X: 2.0 * radius, Y: 2.0 * radius })?;
        Ok(v)
    }

    /// Lote que avisa quando as animações iniciadas até `End()` terminarem. O aviso chega na thread de UI.
    pub fn on_settled(&self, f: impl FnOnce() + Send + 'static) -> Result<CompositionScopedBatch> {
        let batch = self.compositor.CreateScopedBatch(CompositionBatchTypes::Animation)?;
        let cell = std::sync::Mutex::new(Some(f));
        batch.Completed(&TypedEventHandler::new(move |_, _| {
            if let Some(f) = cell.lock().ok().and_then(|mut g| g.take()) {
                f();
            }
            Ok(())
        }))?;
        Ok(batch)
    }
}
