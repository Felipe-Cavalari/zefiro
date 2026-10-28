//! Chip `ia` no compositor (ADR-0028): cada slide é um visual desenhado uma vez, dentro de um
//! contêiner recortado no tamanho do chip e preso à posição dele por expressão. A troca é um
//! deslize de baixo para cima (o slide que sai sobe, o próximo entra por baixo) com quadros-chave
//! que rodam no Windows; o Zefiro só pede a troca a cada `[ia] interval`.

use windows::Foundation::TimeSpan;
use windows::UI::Composition::{CompositionDrawingSurface, CompositionObject, CompositionPropertySet, ContainerVisual, SpriteVisual, Visual};
use windows::core::{HSTRING, Interface, Result};
use windows_numerics::{Vector2, Vector3};
use zefiro_platform::geometry::Size;
use zefiro_ui::paint::DisplayList;
use zefiro_ui::panel_layout::CHIP_FADE_S;

use crate::app::AppCtx;

/// Duração do deslize (com a velocidade das animações do tema).
const SLIDE_S: f32 = 0.45;

struct SlideVisual { sprite: SpriteVisual, surface: CompositionDrawingSurface, list: DisplayList }

pub struct AiCarousel {
    root: ContainerVisual,
    slides: Vec<SlideVisual>,
    keys: Vec<String>,
    current: usize,
    size: Size,
}

impl AiCarousel {
    /// `chip_pos`: as propriedades `x`/`y` da posição do chip (`Comp::spring_offset`).
    pub fn new(ctx: &AppCtx, parent: &ContainerVisual, chip_pos: &CompositionPropertySet) -> Result<AiCarousel> {
        let root = ctx.comp.compositor.CreateContainerVisual()?;
        parent.Children()?.InsertAtTop(&root)?;
        let e = ctx.comp.compositor.CreateExpressionAnimationWithExpression(&HSTRING::from("Vector3(c.x, c.y, 0)"))?;
        e.SetReferenceParameter(&HSTRING::from("c"), &chip_pos.cast::<CompositionObject>()?)?;
        root.StartAnimation(&HSTRING::from("Offset"), &e)?;
        root.SetClip(&ctx.comp.compositor.CreateInsetClip()?)?;
        Ok(AiCarousel { root, slides: Vec::new(), keys: Vec::new(), current: 0, size: Size::default() })
    }

    pub fn root(&self) -> &ContainerVisual {
        &self.root
    }

    /// A chave do slide à vista.
    pub fn current_key(&self) -> Option<&str> {
        self.keys.get(self.current).map(String::as_str)
    }

    /// Mostra estes slides (`keys[i]` é o desenho `lists[i]`). Mudou a lista ou o tamanho: refaz e
    /// continua no mesmo slide, se ele ainda existe. Senão, só redesenha os que mudaram.
    pub fn sync(&mut self, ctx: &AppCtx, lists: &[DisplayList], keys: &[String], size: Size) -> Result<()> {
        self.root.SetSize(Vector2 { X: size.w, Y: size.h })?;
        if keys != self.keys || size != self.size || lists.len() != self.slides.len() {
            let keep = self.current_key().and_then(|k| keys.iter().position(|x| x == k)).unwrap_or(0);
            self.root.Children()?.RemoveAll()?;
            self.slides.clear();
            for (i, list) in lists.iter().enumerate() {
                let surface = ctx.comp.surface(size.w, size.h)?;
                ctx.comp.draw(&surface, size.w, size.h, list, &ctx.text)?;
                let sprite = ctx.comp.sprite_for(&surface, size.w, size.h)?;
                sprite.SetOffset(Vector3 { X: 0.0, Y: if i == keep { 0.0 } else { size.h }, Z: 0.0 })?;
                self.root.Children()?.InsertAtTop(&sprite)?;
                self.slides.push(SlideVisual { sprite, surface, list: list.clone() });
            }
            self.keys = keys.to_vec();
            self.size = size;
            self.current = keep.min(self.slides.len().saturating_sub(1));
            return Ok(());
        }
        for (s, list) in self.slides.iter_mut().zip(lists) {
            if s.list != *list {
                ctx.comp.draw(&s.surface, size.w, size.h, list, &ctx.text)?;
                s.list = list.clone();
            }
        }
        Ok(())
    }

    /// Próximo slide: o atual sobe e sai, o próximo entra por baixo. Com um slide só, nada.
    pub fn advance(&mut self, ctx: &AppCtx) -> Result<()> {
        let n = self.slides.len();
        if n < 2 {
            return Ok(());
        }
        let next = (self.current + 1) % n;
        let h = self.size.h;
        let secs = if ctx.theme.animations { SLIDE_S / ctx.theme.theme.animation.speed.max(0.1) } else { 0.001 };
        self.slide(ctx, self.current, 0.0, -h, secs)?;
        self.slide(ctx, next, h, 0.0, secs)?;
        self.current = next;
        Ok(())
    }

    /// Desliza um slide de `from` a `to` (em y). "Na hora" também é quadro-chave (1 ms): uma
    /// animação em andamento gravaria o valor dela por cima de um `SetOffset`.
    fn slide(&self, ctx: &AppCtx, index: usize, from: f32, to: f32, secs: f32) -> Result<()> {
        let a = ctx.comp.compositor.CreateVector3KeyFrameAnimation()?;
        a.InsertKeyFrame(0.0, Vector3 { X: 0.0, Y: from, Z: 0.0 })?;
        a.InsertKeyFrameWithEasingFunction(1.0, Vector3 { X: 0.0, Y: to, Z: 0.0 }, &crate::tab_strip::ease_in_out(ctx)?)?;
        a.SetDuration(TimeSpan { Duration: (secs.max(0.001) as f64 * 1e7) as i64 })?;
        self.slides[index].sprite.StartAnimation(&HSTRING::from("Offset"), &a)
    }

    /// Esconde (painel aberto, que nasce do chip) ou mostra o slide.
    pub fn fade(&self, ctx: &AppCtx, visible: bool) -> Result<()> {
        ctx.comp.fade(&self.root.cast::<Visual>()?, if visible { 1.0 } else { 0.0 }, CHIP_FADE_S)
    }
}
