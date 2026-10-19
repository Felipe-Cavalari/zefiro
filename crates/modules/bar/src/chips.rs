//! O que cada chip desenha.

use zefiro_platform::geometry::{Rect, Size};
use zefiro_platform::text::{Align, Family, FontWeight, TextMeasurer, TextStyle};
use zefiro_resources::tokens::Tokens;
use zefiro_ui::paint::{DisplayList, DrawOp};

#[derive(Debug, Clone, PartialEq)]
pub struct ChipContent { pub content_w: f32, pub list: DisplayList }

/// Monograma "Z" num quadrado arredondado (72 % da altura) + "Zefiro" (só na altura completa).
pub fn zefiro_chip(chip_h: f32, font_px: f32, compact: bool, scale: f32, tokens: &Tokens, m: &dyn TextMeasurer) -> ChipContent {
    let side = (chip_h * 0.72).round();
    let top = ((chip_h - side) / 2.0).round();
    let mut ops = vec![
        DrawOp::RoundRect { rect: Rect::new(0.0, top, side, side), radius: 6.0 * scale, color: tokens.raised_strong },
        DrawOp::Text {
            rect: Rect::new(0.0, top, side, side),
            text: "Z".into(),
            style: TextStyle { size: font_px, weight: FontWeight::Bold, family: Family::Text },
            color: tokens.text,
            align: Align::Center,
        },
    ];
    let mut w = side;
    if !compact {
        let style = TextStyle { size: font_px, weight: FontWeight::SemiBold, family: Family::Text };
        let label_w = m.measure("Zefiro", &style).w.ceil();
        let x = side + (8.0 * scale).round();
        ops.push(DrawOp::Text { rect: Rect::new(x, 0.0, label_w, chip_h), text: "Zefiro".into(), style, color: tokens.text_secondary, align: Align::Leading });
        w = x + label_w;
    }
    ChipContent { content_w: w, list: DisplayList { size: Size { w, h: chip_h }, ops } }
}

pub fn clock_chip(text: &str, chip_h: f32, font_px: f32, tokens: &Tokens, m: &dyn TextMeasurer) -> ChipContent {
    let style = TextStyle { size: font_px, weight: FontWeight::SemiBold, family: Family::Text };
    let w = m.measure(text, &style).w.ceil();
    let ops = vec![DrawOp::Text { rect: Rect::new(0.0, 0.0, w, chip_h), text: text.to_string(), style, color: tokens.text, align: Align::Leading }];
    ChipContent { content_w: w, list: DisplayList { size: Size { w, h: chip_h }, ops } }
}

pub const BELL: &str = "\u{EA8F}";

/// Sino de notificações: apagado sem nenhuma; com notificações, ponto de destaque e o número
/// (até "99+", com a largura de "99+" reservada).
pub fn bell_chip(count: usize, chip_h: f32, font_px: f32, scale: f32, tokens: &Tokens, m: &dyn TextMeasurer) -> ChipContent {
    let icon_w = (font_px * 1.25).ceil();
    let icon = TextStyle { size: font_px * 1.05, weight: FontWeight::Regular, family: Family::Icons };
    let color = if count == 0 { tokens.text_secondary } else { tokens.text };
    let mut ops = vec![DrawOp::Text { rect: Rect::new(0.0, 0.0, icon_w, chip_h), text: BELL.into(), style: icon, color, align: Align::Center }];
    if count == 0 {
        return ChipContent { content_w: icon_w, list: DisplayList { size: Size { w: icon_w, h: chip_h }, ops } };
    }
    ops.push(DrawOp::Circle { center: zefiro_platform::geometry::Point { x: icon_w - 2.0 * scale, y: chip_h * 0.32 }, radius: 3.0 * scale, color: tokens.accent });
    let style = TextStyle { size: font_px, weight: FontWeight::SemiBold, family: Family::Text };
    let text = if count > 99 { "99+".to_string() } else { count.to_string() };
    let gap = (5.0 * scale).round();
    let slot = m.measure("99+", &style).w.ceil().max(m.measure(&text, &style).w.ceil());
    ops.push(DrawOp::Text { rect: Rect::new(icon_w + gap, 0.0, slot, chip_h), text, style, color, align: Align::Leading });
    let w = icon_w + gap + slot;
    ChipContent { content_w: w, list: DisplayList { size: Size { w, h: chip_h }, ops } }
}

#[cfg(test)]
mod tests {
    use super::*;
    use zefiro_platform::color::Rgba;
    use zefiro_platform::text::FixedWidthMeasurer;
    use zefiro_resources::theme::{BUILTIN, parse_theme};
    use zefiro_resources::tokens::derive_tokens;
    use zefiro_ui::paint::DrawOp;

    fn tokens() -> zefiro_resources::tokens::Tokens {
        derive_tokens(&parse_theme("obsidian", "o", BUILTIN[0].1).unwrap(), Rgba::rgb(0, 120, 215), None)
    }

    #[test]
    fn zefiro_chip_has_monogram_and_label_when_full() {
        let c = zefiro_chip(32.0, 13.0, false, 1.0, &tokens(), &FixedWidthMeasurer);
        assert!(matches!(c.list.ops[0], DrawOp::RoundRect { .. }));
        let texts: Vec<&str> = c.list.ops.iter().filter_map(|o| if let DrawOp::Text { text, .. } = o { Some(text.as_str()) } else { None }).collect();
        assert_eq!(texts, vec!["Z", "Zefiro"]);
        assert!(c.content_w > 32.0 * 0.7);
    }

    #[test]
    fn zefiro_chip_is_monogram_only_when_compact() {
        let c = zefiro_chip(24.0, 12.0, true, 1.0, &tokens(), &FixedWidthMeasurer);
        let texts = c.list.ops.iter().filter(|o| matches!(o, DrawOp::Text { .. })).count();
        assert_eq!(texts, 1);
        assert_eq!(c.content_w, (24.0f32 * 0.72).round());
    }

    #[test]
    fn clock_chip_width_is_text_width_rounded_up() {
        let c = clock_chip("qui 15 15:47", 32.0, 13.0, &tokens(), &FixedWidthMeasurer);
        assert_eq!(c.content_w, (12.0f32 * 13.0 * 0.55).ceil());
        assert!(matches!(&c.list.ops[0], DrawOp::Text { text, .. } if text == "qui 15 15:47"));
    }

    fn texts(c: &ChipContent) -> Vec<String> {
        c.list.ops.iter().filter_map(|o| if let DrawOp::Text { text, .. } = o { Some(text.clone()) } else { None }).collect()
    }

    #[test]
    fn bell_without_notifications_is_dim() {
        let t = tokens();
        let c = bell_chip(0, 28.0, 12.0, 1.0, &t, &FixedWidthMeasurer);
        assert_eq!(texts(&c), vec![BELL.to_string()]);
        assert!(c.list.ops.iter().all(|o| !matches!(o, DrawOp::Circle { .. })), "sem ponto");
        assert!(matches!(&c.list.ops[0], DrawOp::Text { color, .. } if color == &t.text_secondary));
    }

    #[test]
    fn bell_shows_count_and_caps_at_99() {
        let t = tokens();
        let c = bell_chip(3, 28.0, 12.0, 1.0, &t, &FixedWidthMeasurer);
        assert_eq!(texts(&c), vec![BELL.to_string(), "3".into()]);
        assert!(c.list.ops.iter().any(|o| matches!(o, DrawOp::Circle { color, .. } if color == &t.accent)));
        assert_eq!(texts(&bell_chip(250, 28.0, 12.0, 1.0, &t, &FixedWidthMeasurer))[1], "99+");
    }

    #[test]
    fn bell_width_is_stable() {
        let t = tokens();
        let w = |n| bell_chip(n, 28.0, 12.0, 1.0, &t, &FixedWidthMeasurer).content_w;
        assert_eq!(w(1), w(250));
    }
}
