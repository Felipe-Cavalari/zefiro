//! Painel de Configurações (abre pelo menu do Z): o que se usa pouco. Cada item vale na hora, sem
//! rascunho; Esc ou clicar fora fecha.

use zefiro_platform::geometry::{Point, Rect, Size};
use zefiro_platform::text::{Align, Family, FontWeight, TextStyle};
use zefiro_resources::tokens::Tokens;
use zefiro_ui::paint::{DisplayList, DrawOp};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SettingsAction { Back, ToggleAutostart, ToggleHud, ExportDiagnostics }

/// A primeira linha volta ao menu do Z, como o "‹ Tema" da tela de temas.
const ITEMS: [(&str, SettingsAction); 4] = [
    ("‹ Configurações", SettingsAction::Back),
    ("Iniciar com o Windows", SettingsAction::ToggleAutostart),
    ("HUD de debug", SettingsAction::ToggleHud),
    ("Exportar diagnóstico", SettingsAction::ExportDiagnostics),
];

const W: f32 = 300.0;
const PAD: f32 = 16.0;
/// Espaço entre o "‹ Configurações" e os itens, como na tela de temas.
const BACK_GAP: f32 = 4.0;
const ROW_H: f32 = 32.0;
const FOOTER_H: f32 = 28.0;

#[derive(Debug, Clone, PartialEq)]
pub struct SettingsItemBox { pub rect: Rect, pub action: SettingsAction }

/// `autostart` e `hud` acendem as chaves; `version` vai no rodapé ("Zefiro 1.3.0").
pub struct SettingsModel { pub autostart: bool, pub hud: bool, version: String, focus: usize }

impl SettingsModel {
    /// O destaque começa no primeiro item, não no voltar.
    pub fn new(autostart: bool, hud: bool, version: &str) -> SettingsModel {
        SettingsModel { autostart, hud, version: version.to_string(), focus: 1 }
    }

    pub fn layout(&self, scale: f32) -> Vec<SettingsItemBox> {
        let s = |v: f32| v * scale;
        ITEMS.iter().enumerate().map(|(i, (_, action))| {
            let gap = if i == 0 { 0.0 } else { BACK_GAP };
            SettingsItemBox { rect: Rect::new(s(PAD), s(PAD + gap) + s(ROW_H) * i as f32, s(W - 2.0 * PAD), s(ROW_H)), action: *action }
        }).collect()
    }

    pub fn size(&self, scale: f32) -> Size {
        Size { w: W * scale, h: (2.0 * PAD + BACK_GAP + ROW_H * ITEMS.len() as f32 + FOOTER_H) * scale }
    }

    pub fn hit(&self, p: Point, scale: f32) -> Option<SettingsAction> {
        self.layout(scale).into_iter().find(|b| b.rect.contains(p)).map(|b| b.action)
    }

    /// O mouse leva o destaque para o item sob ele. `true` se mudou.
    pub fn hover(&mut self, p: Point, scale: f32) -> bool {
        let Some(i) = self.layout(scale).iter().position(|b| b.rect.contains(p)) else { return false };
        let changed = i != self.focus;
        self.focus = i;
        changed
    }

    /// Setas andam, Enter devolve a ação do item com o destaque.
    pub fn key(&mut self, down: Option<bool>) -> Option<SettingsAction> {
        match down {
            Some(true) => self.focus = (self.focus + 1).min(ITEMS.len() - 1),
            Some(false) => self.focus = self.focus.saturating_sub(1),
            None => return Some(ITEMS[self.focus].1),
        }
        None
    }

    pub fn paint(&self, scale: f32, tokens: &Tokens) -> DisplayList {
        let s = |v: f32| v * scale;
        let size = self.size(scale);
        let mut list = DisplayList::new(size);
        let body = TextStyle { size: s(13.0), weight: FontWeight::Regular, family: Family::Text };
        let small = TextStyle { size: s(11.0), weight: FontWeight::Regular, family: Family::Text };
        for (i, b) in self.layout(scale).iter().enumerate() {
            if i == self.focus {
                list.push(DrawOp::RoundRect { rect: b.rect, radius: s(10.0), color: tokens.raised });
            }
            let color = if b.action == SettingsAction::Back { tokens.text_secondary } else { tokens.text };
            list.push(DrawOp::Text { rect: b.rect.inset(s(12.0), 0.0), text: ITEMS[i].0.into(), style: body, color, align: Align::Leading });
            match b.action {
                SettingsAction::ToggleAutostart => crate::menu::switch(&mut list, b.rect, self.autostart, scale, tokens),
                SettingsAction::ToggleHud => crate::menu::switch(&mut list, b.rect, self.hud, scale, tokens),
                _ => {}
            }
        }
        let footer = Rect::new(s(PAD + 12.0), size.h - s(PAD + FOOTER_H) + s(8.0), s(W - 2.0 * PAD - 12.0), s(20.0));
        list.push(DrawOp::Text { rect: footer, text: format!("Zefiro {}", self.version), style: small, color: tokens.text_tertiary, align: Align::Leading });
        list
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use zefiro_platform::color::Rgba;

    fn tokens() -> Tokens {
        zefiro_resources::tokens::derive_tokens(&zefiro_resources::theme::parse_theme("obsidian", "o", zefiro_resources::theme::BUILTIN[0].1).unwrap(), Rgba::rgb(0, 120, 215), None)
    }

    fn texts(list: &DisplayList) -> Vec<String> {
        list.ops.iter().filter_map(|o| if let DrawOp::Text { text, .. } = o { Some(text.clone()) } else { None }).collect()
    }

    #[test]
    fn lists_the_rare_items_in_order_with_the_version() {
        let m = SettingsModel::new(false, false, "1.3.0");
        assert_eq!(m.layout(1.0).iter().map(|b| b.action).collect::<Vec<_>>(), [SettingsAction::Back, SettingsAction::ToggleAutostart, SettingsAction::ToggleHud, SettingsAction::ExportDiagnostics]);
        let t = texts(&m.paint(1.0, &tokens()));
        assert!(t.contains(&"‹ Configurações".to_string()) && t.contains(&"Zefiro 1.3.0".to_string()), "como o ‹ Tema: {t:?}");
        for b in m.layout(1.0) {
            assert!(b.rect.bottom() <= m.size(1.0).h, "cabe no painel");
            assert_eq!(m.hit(Point { x: b.rect.center_x(), y: b.rect.y + b.rect.h / 2.0 }, 1.0), Some(b.action));
        }
    }

    #[test]
    fn switches_light_up_with_their_state() {
        let t = tokens();
        let lit = |m: &SettingsModel| m.paint(1.0, &t).ops.iter().filter(|o| matches!(o, DrawOp::RoundRect { color, .. } if *color == t.accent)).count();
        assert_eq!(lit(&SettingsModel::new(false, false, "x")), 0);
        assert_eq!(lit(&SettingsModel::new(true, false, "x")), 1);
        assert_eq!(lit(&SettingsModel::new(true, true, "x")), 2);
    }

    #[test]
    fn keyboard_walks_and_clamps() {
        let mut m = SettingsModel::new(false, false, "x");
        assert_eq!(m.key(None), Some(SettingsAction::ToggleAutostart), "começa no primeiro item, não no voltar");
        for _ in 0..9 {
            m.key(Some(true));
        }
        assert_eq!(m.key(None), Some(SettingsAction::ExportDiagnostics));
        m.key(Some(false));
        assert_eq!(m.key(None), Some(SettingsAction::ToggleHud));
        for _ in 0..9 {
            m.key(Some(false));
        }
        assert_eq!(m.key(None), Some(SettingsAction::Back), "↑ no topo chega ao ‹ Configurações");
        let p = m.layout(1.0)[2].rect;
        assert!(m.hover(Point { x: p.center_x(), y: p.y + 1.0 }, 1.0));
        assert_eq!(m.key(None), Some(SettingsAction::ToggleHud));
    }
}
