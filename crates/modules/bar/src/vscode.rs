//! Painel "VS Code · recentes": uma linha por projeto (nome, data, onde fica e um selo para WSL,
//! remoto ou workspace), com rolagem pela roda.

use zefiro_platform::geometry::{Rect, Size};
use zefiro_platform::text::{Align, Family, FontWeight, TextStyle};
use zefiro_resources::tokens::Tokens;
use zefiro_ui::paint::{DisplayList, DrawOp};

/// Uma linha do painel.
#[derive(Debug, Clone, PartialEq)]
pub struct RecentRow { pub name: String, pub location: String, pub badge: Option<&'static str>, pub date: String }

const WIDTH: f32 = 360.0;
const ROW_H: f32 = 44.0;
pub const VISIBLE: usize = 12;

/// Primeira linha máxima da rolagem para `count` projetos.
pub fn max_scroll(count: usize) -> usize {
    count.saturating_sub(VISIBLE)
}

/// Desenho e as áreas de clique (retângulo, índice do projeto). `message` aparece sem itens
/// ("Carregando…", "VS Code não encontrado").
pub fn vscode_panel(items: &[RecentRow], scroll: usize, message: Option<&str>, scale: f32, tokens: &Tokens) -> (DisplayList, Vec<(Rect, usize)>) {
    let px = |v: f32| (v * scale).round();
    let text = |size: f32, weight: FontWeight| TextStyle { size: size * scale, weight, family: Family::Text };
    let scroll = scroll.min(max_scroll(items.len()));
    let shown = items.len().saturating_sub(scroll).min(VISIBLE);
    let body = if shown == 0 { 40.0 } else { shown as f32 * ROW_H };
    let size = Size { w: px(WIDTH), h: px(44.0 + body + 10.0) };
    let mut list = DisplayList::new(size);
    let mut hits = Vec::new();
    list.push(DrawOp::Text { rect: Rect::new(px(20.0), px(14.0), size.w - px(40.0), px(22.0)), text: "VS Code · recentes".into(), style: text(13.0, FontWeight::SemiBold), color: tokens.text_secondary, align: Align::Leading });
    if shown == 0 {
        let note = message.unwrap_or("Nenhum projeto recente");
        list.push(DrawOp::Text { rect: Rect::new(px(20.0), px(48.0), size.w - px(40.0), px(20.0)), text: note.into(), style: text(12.5, FontWeight::Regular), color: tokens.text, align: Align::Leading });
        return (list, hits);
    }
    for (row, (index, item)) in items.iter().enumerate().skip(scroll).take(shown).enumerate() {
        let r = Rect::new(px(10.0), px(44.0 + row as f32 * ROW_H), size.w - px(20.0), px(ROW_H));
        let date_w = px(80.0);
        list.push(DrawOp::Text { rect: Rect::new(r.right() - px(10.0) - date_w, r.y + px(5.0), date_w, px(18.0)), text: item.date.clone(), style: text(11.0, FontWeight::Regular), color: tokens.text_tertiary, align: Align::Trailing });
        let mut name_w = r.w - px(20.0) - date_w - px(8.0);
        if let Some(badge) = item.badge {
            let bw = px(64.0);
            let b = Rect::new(r.right() - px(18.0) - date_w - bw, r.y + px(6.0), bw, px(16.0));
            list.push(DrawOp::RoundRect { rect: b, radius: b.h / 2.0, color: tokens.raised_strong });
            list.push(DrawOp::Text { rect: b, text: badge.into(), style: text(10.0, FontWeight::SemiBold), color: tokens.text_secondary, align: Align::Center });
            name_w -= bw + px(8.0);
        }
        list.push(DrawOp::Text { rect: Rect::new(r.x + px(10.0), r.y + px(4.0), name_w, px(20.0)), text: item.name.clone(), style: text(12.5, FontWeight::SemiBold), color: tokens.text, align: Align::Leading });
        list.push(DrawOp::Text { rect: Rect::new(r.x + px(10.0), r.y + px(23.0), r.w - px(20.0), px(16.0)), text: item.location.clone(), style: text(11.0, FontWeight::Regular), color: tokens.text_tertiary, align: Align::Leading });
        hits.push((r, index));
    }
    (list, hits)
}

#[cfg(test)]
mod tests {
    use super::*;
    use zefiro_platform::color::Rgba;
    use zefiro_resources::theme::{BUILTIN, parse_theme};
    use zefiro_resources::tokens::derive_tokens;

    fn tokens() -> Tokens {
        derive_tokens(&parse_theme("obsidian", "o", BUILTIN[0].1).unwrap(), Rgba::rgb(0, 120, 215), None)
    }

    fn row(i: usize) -> RecentRow {
        RecentRow { name: format!("projeto-{i}"), location: r"C:\Users\felip\Projects".into(), badge: if i == 1 { Some("WSL") } else { None }, date: "19/10 11:42".into() }
    }

    fn texts(l: &DisplayList) -> Vec<String> {
        l.ops.iter().filter_map(|o| if let DrawOp::Text { text, .. } = o { Some(text.clone()) } else { None }).collect()
    }

    #[test]
    fn panel_shows_name_date_location_and_badge() {
        let items: Vec<RecentRow> = (0..3).map(row).collect();
        let (list, hits) = vscode_panel(&items, 0, None, 1.0, &tokens());
        let t = texts(&list);
        for s in ["VS Code · recentes", "projeto-0", "19/10 11:42", r"C:\Users\felip\Projects", "WSL"] {
            assert!(t.contains(&s.to_string()), "falta {s}");
        }
        assert_eq!(hits.iter().map(|(_, i)| *i).collect::<Vec<_>>(), vec![0, 1, 2]);
        assert_eq!(list.size.w, 360.0);
    }

    #[test]
    fn panel_scrolls_after_twelve_rows() {
        let items: Vec<RecentRow> = (0..20).map(row).collect();
        let (list, hits) = vscode_panel(&items, 3, None, 1.0, &tokens());
        assert_eq!(hits.len(), 12);
        assert_eq!(hits[0].1, 3);
        assert_eq!(list.size.h, 44.0 + 12.0 * 44.0 + 10.0);
        assert!(!texts(&list).contains(&"projeto-0".to_string()));
        // A rolagem não passa do fim.
        assert_eq!(max_scroll(20), 8);
        assert_eq!(max_scroll(5), 0);
    }

    #[test]
    fn panel_message_without_items() {
        let (list, hits) = vscode_panel(&[], 0, Some("Carregando…"), 1.0, &tokens());
        assert!(texts(&list).contains(&"Carregando…".to_string()));
        assert!(hits.is_empty());
    }
}
