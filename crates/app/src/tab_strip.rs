//! Abas do painel da ilha no compositor: cada aba é um visual desenhado uma vez (em camadas), e a
//! troca é um carrossel que roda dentro do Windows, na taxa do monitor, sem a CPU do Zefiro por quadro.
//!
//! Uma propriedade `pos` (a aba "em vista", contínua) é animada pela mola 250/31 da ilha C++; cada
//! aba liga posição, escala e opacidade a ela por expressão, com a fórmula do `DrawTabTransition`:
//! desliza `d × 34` px, encolhe até 96,5 % e some com `(1 − smoothstep(|d|))^1,4`, `d = i − pos`.

use windows::UI::Composition::{CompositionDrawingSurface, CompositionEasingFunction, CompositionObject, CompositionPropertySet, ContainerVisual, SpriteVisual, Visual};
use windows::Foundation::TimeSpan;
use windows::core::{HSTRING, Interface, Result};
use windows_numerics::{Vector2, Vector3};
use zefiro_island::tabs::{TAB_DAMPING, TAB_FADE_POWER, TAB_MIN_SCALE, TAB_STIFFNESS, TAB_TRAVEL};
use zefiro_platform::geometry::{Point, Size};
use zefiro_ui::paint::DisplayList;
use zefiro_ui::spring::SpringParams;

use crate::app::AppCtx;

/// Uma camada de uma aba: desenho, posição (canto superior esquerdo, coordenadas do painel) e,
/// se a posição mudar, quanto tempo leva deslizando até ela (o indicador da barra de fontes).
#[derive(Debug, Clone, PartialEq)]
pub struct LayerSpec { pub list: DisplayList, pub offset: Point, pub slide_s: Option<f32> }

impl LayerSpec {
    pub fn at_origin(list: DisplayList) -> LayerSpec {
        LayerSpec { list, offset: Point::default(), slide_s: None }
    }
}

struct LayerVisual { sprite: SpriteVisual, surface: CompositionDrawingSurface, list: DisplayList, offset: Point }

struct TabVisual { container: ContainerVisual, layers: Vec<LayerVisual> }

pub struct TabStrip {
    root: ContainerVisual,
    props: CompositionPropertySet,
    tabs: Vec<TabVisual>,
    index: usize,
    size: Size,
}

fn expr(ctx: &AppCtx, text: &str, props: &CompositionPropertySet, i: f32, travel: f32) -> Result<windows::UI::Composition::ExpressionAnimation> {
    let e = ctx.comp.compositor.CreateExpressionAnimationWithExpression(&HSTRING::from(text))?;
    e.SetReferenceParameter(&HSTRING::from("p"), &props.cast::<CompositionObject>()?)?;
    e.SetScalarParameter(&HSTRING::from("I"), i)?;
    e.SetScalarParameter(&HSTRING::from("T"), travel)?;
    Ok(e)
}

/// Curva de entrada e saída suave (o `SkipEaseInOut` da ilha) para o deslize do indicador.
pub fn ease_in_out(ctx: &AppCtx) -> Result<CompositionEasingFunction> {
    ctx.comp.compositor.CreateCubicBezierEasingFunction(Vector2 { X: 0.65, Y: 0.0 }, Vector2 { X: 0.35, Y: 1.0 })?.cast()
}

impl TabStrip {
    pub fn new(ctx: &AppCtx, parent: &ContainerVisual) -> Result<TabStrip> {
        let root = ctx.comp.compositor.CreateContainerVisual()?;
        parent.Children()?.InsertAtTop(&root)?;
        let props = ctx.comp.compositor.CreatePropertySet()?;
        props.InsertScalar(&HSTRING::from("pos"), 0.0)?;
        Ok(TabStrip { root, props, tabs: Vec::new(), index: 0, size: Size::default() })
    }

    pub fn root(&self) -> &ContainerVisual {
        &self.root
    }

    /// Mostra as abas (`tabs[i]` = camadas da aba `i`) com a aba `index` em vista. Só redesenha as
    /// camadas cujo desenho mudou. `animate`: a troca de aba anda com a mola; senão, pula.
    pub fn sync(&mut self, ctx: &AppCtx, origin: Point, size: Size, tabs: &[Vec<LayerSpec>], index: usize, animate: bool) -> Result<()> {
        self.root.SetOffset(Vector3 { X: origin.x, Y: origin.y, Z: 0.0 })?;
        let scale = size.w / zefiro_island::view::EXPANDED_DESIGN.w;
        let rebuilt = tabs.len() != self.tabs.len() || size != self.size;
        if rebuilt {
            self.root.Children()?.RemoveAll()?;
            self.tabs.clear();
            self.size = size;
            let a = "Clamp(Abs(I - p.pos), 0, 1)";
            let e = format!("({a} * {a} * (3 - 2 * {a}))");
            let s = format!("(1 - {:.4} * {e})", 1.0 - TAB_MIN_SCALE);
            for i in 0..tabs.len() {
                let container = ctx.comp.compositor.CreateContainerVisual()?;
                container.SetSize(Vector2 { X: size.w, Y: size.h })?;
                container.SetCenterPoint(Vector3 { X: size.w / 2.0, Y: size.h / 2.0, Z: 0.0 })?;
                let travel = TAB_TRAVEL * scale;
                container.StartAnimation(&HSTRING::from("Offset"), &expr(ctx, "Vector3((I - p.pos) * T, 0, 0)", &self.props, i as f32, travel)?)?;
                container.StartAnimation(&HSTRING::from("Scale"), &expr(ctx, &format!("Vector3({s}, {s}, 1)"), &self.props, i as f32, travel)?)?;
                container.StartAnimation(&HSTRING::from("Opacity"), &expr(ctx, &format!("Pow(1 - {e}, {TAB_FADE_POWER})"), &self.props, i as f32, travel)?)?;
                self.root.Children()?.InsertAtTop(&container)?;
                self.tabs.push(TabVisual { container, layers: Vec::new() });
            }
        }
        for (tab, specs) in self.tabs.iter_mut().zip(tabs) {
            while tab.layers.len() > specs.len() {
                if let Some(l) = tab.layers.pop() {
                    tab.container.Children()?.Remove(&l.sprite)?;
                }
            }
            for (k, spec) in specs.iter().enumerate() {
                if k == tab.layers.len() {
                    let surface = ctx.comp.surface(spec.list.size.w, spec.list.size.h)?;
                    let sprite = ctx.comp.sprite_for(&surface, spec.list.size.w, spec.list.size.h)?;
                    sprite.SetOffset(Vector3 { X: spec.offset.x, Y: spec.offset.y, Z: 0.0 })?;
                    tab.container.Children()?.InsertAtTop(&sprite)?;
                    ctx.comp.draw(&surface, spec.list.size.w, spec.list.size.h, &spec.list, &ctx.text)?;
                    tab.layers.push(LayerVisual { sprite, surface, list: spec.list.clone(), offset: spec.offset });
                    continue;
                }
                let layer = &mut tab.layers[k];
                if layer.list != spec.list {
                    ctx.comp.draw(&layer.surface, spec.list.size.w, spec.list.size.h, &spec.list, &ctx.text)?;
                    layer.sprite.SetSize(Vector2 { X: spec.list.size.w, Y: spec.list.size.h })?;
                    layer.list = spec.list.clone();
                }
                if layer.offset != spec.offset {
                    match spec.slide_s.filter(|_| animate && ctx.theme.animations) {
                        Some(secs) => {
                            let a = ctx.comp.compositor.CreateVector3KeyFrameAnimation()?;
                            a.InsertKeyFrameWithEasingFunction(1.0, Vector3 { X: spec.offset.x, Y: spec.offset.y, Z: 0.0 }, &ease_in_out(ctx)?)?;
                            a.SetDuration(TimeSpan { Duration: (secs as f64 * 1e7) as i64 })?;
                            layer.sprite.StartAnimation(&HSTRING::from("Offset"), &a)?;
                        }
                        None => layer.sprite.SetOffset(Vector3 { X: spec.offset.x, Y: spec.offset.y, Z: 0.0 })?,
                    }
                    layer.offset = spec.offset;
                }
            }
        }
        if rebuilt || !animate || !ctx.theme.animations {
            self.props.StopAnimation(&HSTRING::from("pos"))?;
            self.props.InsertScalar(&HSTRING::from("pos"), index as f32)?;
        } else if index != self.index {
            let spring = SpringParams { stiffness: TAB_STIFFNESS, damping: TAB_DAMPING }.with_speed(ctx.theme.theme.animation.speed);
            self.props.StartAnimation(&HSTRING::from("pos"), &ctx.comp.scalar_spring(index as f32, spring)?)?;
        }
        self.index = index;
        Ok(())
    }

    /// Visual de uma camada (para a troca de fonte animar a capa e os textos).
    pub fn layer(&self, tab: usize, layer: usize) -> Option<Visual> {
        self.tabs.get(tab)?.layers.get(layer)?.sprite.cast().ok()
    }
}
