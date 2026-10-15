//! Tema atual, tokens e as regras de estilo das formas.

use std::path::Path;
use zefiro_platform::color::Rgba;
use zefiro_platform_windows::morph::MorphStyle;
use zefiro_platform_windows::system::{animations_enabled, system_accent, transparency_enabled};
use zefiro_resources::registry::ThemeRegistry;
use zefiro_resources::theme::{Backdrop, Theme, ThemeError};
use zefiro_resources::tokens::{Tokens, derive_tokens};
use zefiro_ui::spring::SpringParams;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SurfaceKind { Strip, Pill, Panel, Hover }

pub struct ThemeState { pub registry: ThemeRegistry, pub theme: Theme, pub tokens: Tokens, pub animations: bool }

impl ThemeState {
    pub fn load(id: &str, user_dir: &Path) -> (ThemeState, Vec<ThemeError>) {
        let (registry, errors) = ThemeRegistry::load(Some(user_dir));
        let theme = registry.get(id).or_else(|| registry.get("obsidian")).cloned().expect("obsidian embutido");
        let tokens = derive_tokens(&theme, system_accent(), None);
        (ThemeState { registry, theme, tokens, animations: animations_enabled() }, errors)
    }

    /// Relê a pasta do usuário e aplica `id`. Erros mantêm a versão anterior do tema com erro.
    pub fn reload(&mut self, id: &str, user_dir: &Path) -> Vec<ThemeError> {
        let (registry, errors) = self.registry.reload_keeping_previous(Some(user_dir));
        if let Some(t) = registry.get(id).or_else(|| registry.get("obsidian")) {
            self.theme = t.clone();
        }
        self.registry = registry;
        self.tokens = derive_tokens(&self.theme, system_accent(), None);
        self.animations = animations_enabled();
        errors
    }

    pub fn spring(&self, base: SpringParams) -> SpringParams {
        base.styled(self.theme.animation.spring).with_speed(self.theme.animation.speed)
    }

    pub fn morph_style(&self, scale: f32, kind: SurfaceKind) -> MorphStyle {
        let backdrop = self.theme.material.backdrop == Backdrop::Acrylic && transparency_enabled();
        let fill = if backdrop { self.tokens.fill.with_alpha(self.tokens.fill.a * self.theme.material.backdrop_fill) } else { self.tokens.fill };
        // Espalhamento da sombra como na ilha: clamp(14 × escala, 6, 18).
        let shadow = self.theme.material.shadow.then_some((self.tokens.shadow, (14.0 * scale).clamp(6.0, 18.0)));
        match kind {
            SurfaceKind::Strip => MorphStyle { fill, depth: self.theme.material.depth, border: None, border_px: 0.0, shadow: None, backdrop },
            SurfaceKind::Pill | SurfaceKind::Panel => MorphStyle { fill, depth: self.theme.material.depth, border: self.tokens.border, border_px: scale, shadow, backdrop },
            SurfaceKind::Hover => MorphStyle { fill: self.tokens.raised, depth: false, border: None, border_px: 0.0, shadow: None, backdrop: false },
        }
    }

    /// Cor de fundo do chip quando o painel dele está aberto.
    pub fn active_fill(&self) -> Rgba {
        self.tokens.raised_strong
    }
}
