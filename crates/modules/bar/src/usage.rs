//! Uso do Claude e do Codex na barra: o chip (ícone + % da janela principal) e o painel com uma
//! linha por janela de uso.

use zefiro_integration_usage::format::is_high;
use zefiro_platform::geometry::{Rect, Size};
use zefiro_platform::text::{Align, Family, FontWeight, TextMeasurer, TextStyle};
use zefiro_resources::tokens::Tokens;
use zefiro_ui::paint::{DisplayList, DrawOp};

use crate::chips::ChipContent;
use crate::devices::{Level, level_chip};

/// Uma janela de uso no painel: "5 horas", 80 %, "reinicia em 2 h 14 min".
#[derive(Debug, Clone, PartialEq)]
pub struct UsageRow { pub label: String, pub percent: Option<f32>, pub detail: String }

/// Ícone + "42%". A partir de 80 % o texto fica na cor de destaque; sem dado, só o ícone apagado.
pub fn usage_chip(glyph: &str, percent: Option<f32>, chip_h: f32, font_px: f32, scale: f32, tokens: &Tokens, m: &dyn TextMeasurer) -> ChipContent {
    let text = percent.map(|p| format!("{p:.0}%")).unwrap_or_default();
    let mut chip = level_chip(Level { glyph, text: &text, dim: percent.is_none() }, chip_h, font_px, scale, tokens, m);
    if percent.is_some_and(is_high) {
        for op in &mut chip.list.ops {
            if let DrawOp::Text { color, .. } = op {
                *color = tokens.accent;
            }
        }
    }
    chip
}

const WIDTH: f32 = 340.0;
const ROW_H: f32 = 58.0;

/// Painel com título, uma linha por janela (rótulo, %, barra e detalhe) e o rodapé. `message`
/// (login expirado, sem dado…) substitui as linhas.
pub fn usage_panel(title: &str, rows: &[UsageRow], message: Option<&str>, footer: &str, scale: f32, tokens: &Tokens) -> DisplayList {
    let px = |v: f32| (v * scale).round();
    let text = |size: f32, weight: FontWeight| TextStyle { size: size * scale, weight, family: Family::Text };
    let body_h = if message.is_some() { 44.0 } else { rows.len().max(1) as f32 * ROW_H };
    let footer_h = if footer.is_empty() { 0.0 } else { 26.0 };
    let size = Size { w: px(WIDTH), h: px(44.0 + body_h + footer_h + 10.0) };
    let mut list = DisplayList::new(size);
    list.push(DrawOp::Text { rect: Rect::new(px(20.0), px(14.0), size.w - px(40.0), px(22.0)), text: title.into(), style: text(13.0, FontWeight::SemiBold), color: tokens.text_secondary, align: Align::Leading });
    if let Some(message) = message {
        list.push(DrawOp::Text { rect: Rect::new(px(20.0), px(48.0), size.w - px(40.0), px(36.0)), text: message.into(), style: text(12.5, FontWeight::Regular), color: tokens.text, align: Align::Leading });
    } else {
        for (i, row) in rows.iter().enumerate() {
            let top = px(44.0 + i as f32 * ROW_H);
            list.push(DrawOp::Text { rect: Rect::new(px(20.0), top, size.w - px(110.0), px(18.0)), text: row.label.clone(), style: text(12.0, FontWeight::Regular), color: tokens.text_secondary, align: Align::Leading });
            let percent = row.percent.map(|p| format!("{p:.0}%")).unwrap_or_else(|| "—".into());
            list.push(DrawOp::Text { rect: Rect::new(size.w - px(80.0), top, px(60.0), px(18.0)), text: percent, style: text(13.0, FontWeight::SemiBold), color: tokens.text, align: Align::Trailing });
            let track = Rect::new(px(20.0), top + px(24.0), size.w - px(40.0), px(5.0));
            list.push(DrawOp::RoundRect { rect: track, radius: track.h / 2.0, color: tokens.raised_strong });
            if let Some(p) = row.percent {
                let fill = (p.clamp(0.0, 100.0) / 100.0 * track.w).max(track.h);
                let color = if is_high(p) { tokens.accent } else { tokens.text_secondary };
                list.push(DrawOp::RoundRect { rect: Rect::new(track.x, track.y, fill, track.h), radius: track.h / 2.0, color });
            }
            if !row.detail.is_empty() {
                list.push(DrawOp::Text { rect: Rect::new(px(20.0), top + px(34.0), size.w - px(40.0), px(16.0)), text: row.detail.clone(), style: text(11.5, FontWeight::Regular), color: tokens.text_tertiary, align: Align::Leading });
            }
        }
    }
    if !footer.is_empty() {
        let top = px(44.0 + body_h);
        list.push(DrawOp::RoundRect { rect: Rect::new(px(20.0), top, size.w - px(40.0), px(1.0)), radius: 0.0, color: tokens.hairline });
        list.push(DrawOp::Text { rect: Rect::new(px(20.0), top + px(6.0), size.w - px(40.0), px(18.0)), text: footer.into(), style: text(11.0, FontWeight::Regular), color: tokens.text_tertiary, align: Align::Leading });
    }
    list
}

#[cfg(test)]
mod tests {
    use super::*;
    use zefiro_platform::color::Rgba;
    use zefiro_platform::text::FixedWidthMeasurer;
    use zefiro_resources::theme::{BUILTIN, parse_theme};
    use zefiro_resources::tokens::derive_tokens;

    fn tokens() -> Tokens {
        derive_tokens(&parse_theme("obsidian", "o", BUILTIN[0].1).unwrap(), Rgba::rgb(0, 120, 215), None)
    }

    fn chip(percent: Option<f32>) -> ChipContent {
        usage_chip("\u{E943}", percent, 28.0, 12.0, 1.0, &tokens(), &FixedWidthMeasurer)
    }

    fn colors(l: &DisplayList) -> Vec<Rgba> {
        l.ops.iter().filter_map(|o| if let DrawOp::Text { color, .. } = o { Some(*color) } else { None }).collect()
    }

    fn texts(l: &DisplayList) -> Vec<String> {
        l.ops.iter().filter_map(|o| if let DrawOp::Text { text, .. } = o { Some(text.clone()) } else { None }).collect()
    }

    #[test]
    fn chip_turns_accent_at_80() {
        let t = tokens();
        assert!(colors(&chip(Some(85.0)).list).iter().all(|c| *c == t.accent));
        assert!(colors(&chip(Some(50.0)).list).iter().all(|c| *c == t.text));
        assert_eq!(texts(&chip(Some(85.4)).list)[1], "85%");
    }

    #[test]
    fn chip_without_data_is_dim_and_empty() {
        let c = chip(None);
        assert_eq!(c.list.ops.len(), 1);
        assert_eq!(colors(&c.list), vec![tokens().text_secondary]);
    }

    #[test]
    fn chip_width_does_not_follow_percent() {
        assert_eq!(chip(Some(5.0)).content_w, chip(Some(100.0)).content_w);
    }

    #[test]
    fn panel_lists_rows_with_details() {
        let rows = [
            UsageRow { label: "5 horas".into(), percent: Some(80.0), detail: "reinicia em 2 h 14 min".into() },
            UsageRow { label: "7 dias".into(), percent: Some(19.0), detail: "reinicia qui 22 out, 11:08".into() },
        ];
        let l = usage_panel("Codex · plus", &rows, None, "Último uso do Codex: 16/10 23:56", 1.0, &tokens());
        let t = texts(&l);
        for s in ["Codex · plus", "5 horas", "80%", "reinicia em 2 h 14 min", "7 dias", "19%", "Último uso do Codex: 16/10 23:56"] {
            assert!(t.contains(&s.to_string()), "falta {s}");
        }
        assert_eq!(l.size.w, 340.0);
    }

    #[test]
    fn panel_message_replaces_rows() {
        let rows = [UsageRow { label: "5 horas".into(), percent: Some(80.0), detail: String::new() }];
        let l = usage_panel("Codex", &rows, Some("Nenhum uso do Codex encontrado"), "", 1.0, &tokens());
        let t = texts(&l);
        assert!(t.contains(&"Nenhum uso do Codex encontrado".to_string()));
        assert!(!t.contains(&"5 horas".to_string()));
    }
}
