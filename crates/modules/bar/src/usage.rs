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

/// Ícone + "42%". A partir de 80 % o texto fica na cor de destaque; sem dado, "—" apagado, na mesma
/// largura (o painel aberto nasce do chip e não pode andar).
pub fn usage_chip(glyph: &str, percent: Option<f32>, chip_h: f32, font_px: f32, scale: f32, tokens: &Tokens, m: &dyn TextMeasurer) -> ChipContent {
    let text = percent.map_or_else(|| "—".to_string(), |p| format!("{p:.0}%"));
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

/// Uma conta no painel: título ("Claude · Pro"), subtítulo apagado ("a@b.c · Windows", pode ser
/// vazio) e as linhas de uso, ou `message` (login expirado, sem dado…) no lugar delas.
#[derive(Debug, Clone, PartialEq)]
pub struct UsageBlock { pub title: String, pub subtitle: String, pub rows: Vec<UsageRow>, pub message: Option<String> }

/// "Atualizar" do Segoe Fluent Icons.
pub const GLYPH_REFRESH: &str = "\u{E72C}";

/// Estado do botão de atualizar: `enabled` (apagado enquanto atualiza ou em pausa) e `hovered`
/// (o mouse em cima, que acende o fundo como o dos chips).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RefreshButton { pub enabled: bool, pub hovered: bool }

/// Botão de atualizar, no canto do título do primeiro bloco.
pub fn refresh_button_rect(scale: f32) -> Rect {
    let px = |v: f32| (v * scale).round();
    Rect::new(px(WIDTH - 20.0 - 28.0), px(12.0), px(28.0), px(28.0))
}

/// Espaço entre dois blocos; a linha divisória fica no meio dele.
const BLOCK_GAP: f32 = 12.0;

/// `text` cortado com "…" para caber em `max_w`.
fn fit(text: &str, style: &TextStyle, max_w: f32, m: &dyn TextMeasurer) -> String {
    if m.measure(text, style).w <= max_w {
        return text.to_string();
    }
    let mut cut: String = text.to_string();
    while !cut.is_empty() && m.measure(&format!("{cut}…"), style).w > max_w {
        cut.pop();
    }
    format!("{}…", cut.trim_end())
}

/// Painel com um bloco por conta (título, subtítulo, uma linha por janela com rótulo, %, barra e
/// detalhe) e o rodapé. Com um bloco sem subtítulo, é o painel de uma conta só de antes. `refresh`:
/// botão de atualizar no título; `None`, sem botão.
pub fn usage_panel(blocks: &[UsageBlock], footer: &str, refresh: Option<RefreshButton>, scale: f32, tokens: &Tokens, m: &dyn TextMeasurer) -> DisplayList {
    let px = |v: f32| (v * scale).round();
    let text = |size: f32, weight: FontWeight| TextStyle { size: size * scale, weight, family: Family::Text };
    let lines = |b: &UsageBlock| -> Vec<String> { b.message.as_deref().map(|msg| msg.lines().map(String::from).collect()).unwrap_or_default() };
    let header_h = |b: &UsageBlock| if b.subtitle.is_empty() { 44.0 } else { 62.0 };
    let body_h = |b: &UsageBlock| if b.message.is_some() { 20.0 + lines(b).len().max(1) as f32 * 20.0 } else { b.rows.len().max(1) as f32 * ROW_H };
    let blocks_h: f32 = blocks.iter().map(|b| header_h(b) + body_h(b)).sum::<f32>() + blocks.len().saturating_sub(1) as f32 * BLOCK_GAP;
    let footer_h = if footer.is_empty() { 0.0 } else { 26.0 };
    let size = Size { w: px(WIDTH), h: px(blocks_h + footer_h + 10.0) };
    let mut list = DisplayList::new(size);
    let mut y = 0.0;
    for (index, b) in blocks.iter().enumerate() {
        if index > 0 {
            list.push(DrawOp::RoundRect { rect: Rect::new(px(20.0), px(y + BLOCK_GAP / 2.0), size.w - px(40.0), px(1.0)), radius: 0.0, color: tokens.hairline });
            y += BLOCK_GAP;
        }
        let beside_button = if index == 0 && refresh.is_some() { px(36.0) } else { 0.0 };
        list.push(DrawOp::Text { rect: Rect::new(px(20.0), px(y + 14.0), size.w - px(40.0) - beside_button, px(22.0)), text: b.title.clone(), style: text(13.0, FontWeight::SemiBold), color: tokens.text_secondary, align: Align::Leading });
        if !b.subtitle.is_empty() {
            let style = text(11.0, FontWeight::Regular);
            let shown = fit(&b.subtitle, &style, size.w - px(40.0), m);
            list.push(DrawOp::Text { rect: Rect::new(px(20.0), px(y + 36.0), size.w - px(40.0), px(16.0)), text: shown, style, color: tokens.text_tertiary, align: Align::Leading });
        }
        let top = y + header_h(b);
        if b.message.is_some() {
            // Uma linha por quebra de linha: o texto do painel não quebra sozinho.
            for (i, line) in lines(b).into_iter().enumerate() {
                list.push(DrawOp::Text { rect: Rect::new(px(20.0), px(top + 4.0 + i as f32 * 20.0), size.w - px(40.0), px(20.0)), text: line, style: text(12.5, FontWeight::Regular), color: tokens.text, align: Align::Leading });
            }
        } else {
            for (i, row) in b.rows.iter().enumerate() {
                let top = px(top + i as f32 * ROW_H);
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
        y = top + body_h(b);
    }
    if let Some(RefreshButton { enabled, hovered }) = refresh {
        let r = refresh_button_rect(scale);
        let icons = TextStyle { size: 12.0 * scale, weight: FontWeight::Regular, family: Family::Icons };
        let lit = enabled && hovered;
        list.push(DrawOp::RoundRect { rect: r, radius: r.h / 2.0, color: if lit { tokens.raised_strong } else { tokens.raised } });
        let color = if lit { tokens.text } else if enabled { tokens.text_secondary } else { tokens.text_tertiary };
        list.push(DrawOp::Text { rect: r, text: GLYPH_REFRESH.into(), style: icons, color, align: Align::Center });
    }
    if !footer.is_empty() {
        let top = px(y);
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
    fn chip_without_data_shows_dash_at_stable_width() {
        let c = chip(None);
        assert_eq!(texts(&c.list)[1], "—");
        assert!(colors(&c.list).iter().all(|c| *c == tokens().text_secondary));
        assert_eq!(c.content_w, chip(Some(100.0)).content_w);
    }

    #[test]
    fn chip_width_does_not_follow_percent() {
        assert_eq!(chip(Some(5.0)).content_w, chip(Some(100.0)).content_w);
    }

    fn rows() -> Vec<UsageRow> {
        vec![
            UsageRow { label: "5 horas".into(), percent: Some(80.0), detail: "reinicia em 2 h 14 min".into() },
            UsageRow { label: "7 dias".into(), percent: Some(19.0), detail: "reinicia qui 22 out, 11:08".into() },
        ]
    }

    fn block(title: &str, subtitle: &str, rows: Vec<UsageRow>, message: Option<&str>) -> UsageBlock {
        UsageBlock { title: title.into(), subtitle: subtitle.into(), rows, message: message.map(String::from) }
    }

    fn panel(blocks: &[UsageBlock], footer: &str) -> DisplayList {
        usage_panel(blocks, footer, None, 1.0, &tokens(), &FixedWidthMeasurer)
    }

    fn glyph(l: &DisplayList) -> Option<(Rect, Rgba)> {
        l.ops.iter().find_map(|o| if let DrawOp::Text { text, rect, color, .. } = o { (text == GLYPH_REFRESH).then_some((*rect, *color)) } else { None })
    }

    #[test]
    fn refresh_button_in_the_header() {
        let blocks = [block("Claude · Pro", "a@b.c · Windows", rows(), None)];
        let t = tokens();
        let on = usage_panel(&blocks, "", Some(RefreshButton { enabled: true, hovered: false }), 1.0, &t, &FixedWidthMeasurer);
        assert_eq!(glyph(&on), Some((refresh_button_rect(1.0), t.text_secondary)));
        // Apagado enquanto atualiza, logo depois de consultar ou com tudo em pausa.
        let off = usage_panel(&blocks, "", Some(RefreshButton { enabled: false, hovered: false }), 1.0, &t, &FixedWidthMeasurer);
        assert_eq!(glyph(&off).map(|g| g.1), Some(t.text_tertiary));
        assert_eq!(glyph(&panel(&blocks, "")), None);
        // O título não passa por baixo do botão.
        let title = text_rect(&on, "Claude · Pro");
        assert!(title.x + title.w <= refresh_button_rect(1.0).x);
        assert_eq!(refresh_button_rect(2.0), Rect::new(584.0, 24.0, 56.0, 56.0));
    }

    #[test]
    fn refresh_button_lights_up_under_the_mouse() {
        let blocks = [block("Claude · Pro", "a@b.c · Windows", rows(), None)];
        let t = tokens();
        let fill = |l: &DisplayList| l.ops.iter().find_map(|o| if let DrawOp::RoundRect { rect, color, .. } = o { (*rect == refresh_button_rect(1.0)).then_some(*color) } else { None });
        let rest = usage_panel(&blocks, "", Some(RefreshButton { enabled: true, hovered: false }), 1.0, &t, &FixedWidthMeasurer);
        let over = usage_panel(&blocks, "", Some(RefreshButton { enabled: true, hovered: true }), 1.0, &t, &FixedWidthMeasurer);
        assert_eq!(fill(&rest), Some(t.raised));
        assert_eq!(fill(&over), Some(t.raised_strong), "sob o mouse, o fundo acende como o dos chips");
        assert_eq!(glyph(&over).map(|g| g.1), Some(t.text), "e o ícone fica mais forte");
        // Apagado (atualizando, ou em pausa) não reage ao mouse.
        let off = usage_panel(&blocks, "", Some(RefreshButton { enabled: false, hovered: true }), 1.0, &t, &FixedWidthMeasurer);
        assert_eq!((fill(&off), glyph(&off).map(|g| g.1)), (Some(t.raised), Some(t.text_tertiary)));
    }

    fn text_rect(l: &DisplayList, wanted: &str) -> Rect {
        l.ops.iter().find_map(|o| if let DrawOp::Text { text, rect, .. } = o { (text == wanted).then_some(*rect) } else { None }).unwrap_or_else(|| panic!("falta {wanted}"))
    }

    #[test]
    fn panel_lists_rows_with_details() {
        let l = panel(&[block("Codex · plus", "", rows(), None)], "Último uso do Codex: 16/10 23:56");
        let t = texts(&l);
        for s in ["Codex · plus", "5 horas", "80%", "reinicia em 2 h 14 min", "7 dias", "19%", "Último uso do Codex: 16/10 23:56"] {
            assert!(t.contains(&s.to_string()), "falta {s}");
        }
        assert_eq!(l.size.w, 340.0);
    }

    #[test]
    fn one_block_without_subtitle_matches_old_layout() {
        // Valores do painel de antes dos blocos (uma conta, sem subtítulo).
        let l = panel(&[block("Claude", "", rows(), None)], "Atualizado agora");
        assert_eq!((l.size.w, l.size.h), (340.0, 196.0));
        assert_eq!(text_rect(&l, "Claude"), Rect::new(20.0, 14.0, 300.0, 22.0));
        assert_eq!(text_rect(&l, "5 horas"), Rect::new(20.0, 44.0, 230.0, 18.0));
        assert_eq!(text_rect(&l, "7 dias").y, 102.0);
        assert_eq!(text_rect(&l, "Atualizado agora").y, 166.0);
    }

    #[test]
    fn subtitle_is_cut_with_ellipsis() {
        let long = "a".repeat(200);
        let l = panel(&[block("Claude · Pro", &long, rows(), None)], "");
        let shown = texts(&l).into_iter().find(|t| t.starts_with('a')).expect("subtítulo");
        assert!(shown.ends_with('…'), "{shown}");
        let style = TextStyle { size: 11.0, weight: FontWeight::Regular, family: Family::Text };
        assert!(FixedWidthMeasurer.measure(&shown, &style).w <= 300.0);
        assert_eq!(text_rect(&l, "5 horas").y, 62.0);
    }

    #[test]
    fn two_blocks_stack_with_hairline() {
        let one = panel(&[block("Claude · Pro", "a@b.c · Windows", rows(), None)], "");
        let two = panel(&[block("Claude · Pro", "a@b.c · Windows", rows(), None), block("Claude · Max", "c@d.e · WSL", rows(), None)], "");
        assert_eq!(two.size.h, one.size.h * 2.0 - 10.0 + 12.0);
        let second = text_rect(&two, "Claude · Max");
        let last_of_first = two.ops.iter().filter_map(|o| if let DrawOp::Text { rect, .. } = o { Some(rect.y) } else { None }).filter(|y| *y < second.y).fold(0.0, f32::max);
        assert!(second.y > last_of_first);
        let hairline = tokens().hairline;
        assert!(two.ops.iter().any(|o| matches!(o, DrawOp::RoundRect { rect, color, .. } if *color == hairline && rect.y < second.y && rect.y > last_of_first)));
    }

    #[test]
    fn panel_message_replaces_rows() {
        let l = panel(&[block("Codex", "", rows(), Some("Nenhum uso do Codex encontrado"))], "");
        let t = texts(&l);
        assert!(t.contains(&"Nenhum uso do Codex encontrado".to_string()));
        assert!(!t.contains(&"5 horas".to_string()));
    }

    #[test]
    fn message_block_breaks_lines() {
        let l = panel(&[block("Claude", "", Vec::new(), Some("WSL desligado.
Abra o WSL para ver o uso de lá."))], "");
        let (a, b) = (text_rect(&l, "WSL desligado."), text_rect(&l, "Abra o WSL para ver o uso de lá."));
        assert!(a.y < b.y);
    }
}
