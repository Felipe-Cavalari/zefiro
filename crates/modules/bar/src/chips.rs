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

/// Clima: emoji da condição, temperatura (largura de "-00°" reservada, para o painel não andar
/// quando ela muda) e a cidade apagada, cortada com "…" para o chip caber em 160 DIP. Sem leitura,
/// só "--°".
pub fn weather_chip(reading: Option<(&str, i32, &str)>, chip_h: f32, font_px: f32, scale: f32, tokens: &Tokens, m: &dyn TextMeasurer) -> ChipContent {
    let strong = TextStyle { size: font_px, weight: FontWeight::SemiBold, family: Family::Text };
    let soft = TextStyle { size: font_px, weight: FontWeight::Regular, family: Family::Text };
    let reserved = m.measure("-00°", &strong).w.ceil();
    let Some((icon, temp, city)) = reading else {
        let ops = vec![DrawOp::Text { rect: Rect::new(0.0, 0.0, reserved, chip_h), text: "--°".into(), style: strong, color: tokens.text_secondary, align: Align::Leading }];
        return ChipContent { content_w: reserved, list: DisplayList { size: Size { w: reserved, h: chip_h }, ops } };
    };
    let gap = (5.0 * scale).round();
    let icon_w = m.measure(icon, &strong).w.ceil();
    let temp = format!("{temp}°");
    let slot = reserved.max(m.measure(&temp, &strong).w.ceil());
    let mut ops = vec![
        DrawOp::Text { rect: Rect::new(0.0, 0.0, icon_w, chip_h), text: icon.into(), style: strong, color: tokens.text, align: Align::Leading },
        DrawOp::Text { rect: Rect::new(icon_w + gap, 0.0, slot, chip_h), text: temp, style: strong, color: tokens.text, align: Align::Leading },
    ];
    let x = icon_w + gap + slot + gap;
    let max = 160.0 * scale;
    let mut name = city.trim().to_string();
    while !name.is_empty() && x + m.measure(&name, &soft).w.ceil() > max {
        if name.ends_with('…') { name.pop(); }
        name.pop();
        name = name.trim_end().to_string();
        if !name.is_empty() { name.push('…'); }
    }
    let mut w = x - gap;
    if !name.is_empty() {
        let name_w = m.measure(&name, &soft).w.ceil();
        ops.push(DrawOp::Text { rect: Rect::new(x, 0.0, name_w, chip_h), text: name, style: soft, color: tokens.text_secondary, align: Align::Leading });
        w = x + name_w;
    }
    ChipContent { content_w: w, list: DisplayList { size: Size { w, h: chip_h }, ops } }
}

pub const MOON: &str = "\u{E708}";

/// Não perturbe: lua na cor de destaque quando ativo, apagada quando não.
pub fn dnd_chip(active: bool, chip_h: f32, font_px: f32, tokens: &Tokens) -> ChipContent {
    let w = (font_px * 1.25).ceil();
    let style = TextStyle { size: font_px * 1.05, weight: FontWeight::Regular, family: Family::Icons };
    let color = if active { tokens.accent } else { tokens.text_secondary };
    let ops = vec![DrawOp::Text { rect: Rect::new(0.0, 0.0, w, chip_h), text: MOON.into(), style, color, align: Align::Center }];
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

    #[test]
    fn weather_chip_shows_icon_temperature_and_city() {
        let t = tokens();
        let c = weather_chip(Some(("☁️", 17, "Guarulhos")), 28.0, 12.0, 1.0, &t, &FixedWidthMeasurer);
        assert_eq!(texts(&c), vec!["☁️".to_string(), "17°".into(), "Guarulhos".into()]);
        assert!(matches!(&c.list.ops[2], DrawOp::Text { color, .. } if color == &t.text_secondary), "cidade apagada");
        assert!(matches!(&c.list.ops[1], DrawOp::Text { color, .. } if color == &t.text), "temperatura em destaque");
    }

    #[test]
    fn weather_chip_width_does_not_follow_the_temperature() {
        let t = tokens();
        let w = |temp| weather_chip(Some(("☀️", temp, "Itu")), 28.0, 12.0, 1.0, &t, &FixedWidthMeasurer).content_w;
        assert_eq!(w(7), w(-12));
        assert_eq!(w(7), w(35));
    }

    #[test]
    fn weather_chip_cuts_long_city_names_and_shows_dashes_without_reading() {
        let t = tokens();
        let long = weather_chip(Some(("☀️", 20, "Cidade muito comprida ".repeat(4).as_str())), 28.0, 12.0, 1.0, &t, &FixedWidthMeasurer);
        assert!(long.content_w <= 160.0, "{}", long.content_w);
        assert!(texts(&long)[2].ends_with('…'));
        let none = weather_chip(None, 28.0, 12.0, 1.0, &t, &FixedWidthMeasurer);
        assert_eq!(texts(&none), vec!["--°".to_string()]);
        assert!(matches!(&none.list.ops[0], DrawOp::Text { color, .. } if color == &t.text_secondary));
    }

    #[test]
    fn dnd_chip_lights_up_when_active() {
        let t = tokens();
        let on = dnd_chip(true, 28.0, 12.0, &t);
        let off = dnd_chip(false, 28.0, 12.0, &t);
        assert!(matches!(&on.list.ops[0], DrawOp::Text { color, .. } if color == &t.accent));
        assert!(matches!(&off.list.ops[0], DrawOp::Text { color, .. } if color == &t.text_secondary));
        assert_eq!(on.content_w, off.content_w);
    }
}
