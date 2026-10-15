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
}
