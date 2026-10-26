//! Painel "VS Code · recentes": uma linha por projeto (nome, data, onde fica e um selo para WSL,
//! remoto ou workspace), com busca no topo e rolagem pela roda.

use zefiro_platform::geometry::{Point, Rect, Size};
use zefiro_platform::text::{Align, Family, FontWeight, TextMeasurer, TextStyle};
use zefiro_resources::tokens::Tokens;
use zefiro_ui::paint::{DisplayList, DrawOp};

/// Uma linha do painel.
#[derive(Debug, Clone, PartialEq)]
pub struct RecentRow { pub name: String, pub location: String, pub badge: Option<&'static str>, pub date: String }

const WIDTH: f32 = 360.0;
const ROW_H: f32 = 44.0;
/// Topo da primeira linha: título e caixa de busca acima.
const ROWS_Y: f32 = 82.0;
pub const VISIBLE: usize = 12;

/// Primeira linha máxima da rolagem para `count` projetos.
pub fn max_scroll(count: usize) -> usize {
    count.saturating_sub(VISIBLE)
}

/// Minúsculas e sem acento, para a busca.
fn fold(s: &str) -> String {
    s.chars()
        .flat_map(char::to_lowercase)
        .map(|c| match c {
            'á' | 'à' | 'â' | 'ã' | 'ä' => 'a',
            'é' | 'è' | 'ê' | 'ë' => 'e',
            'í' | 'ì' | 'î' | 'ï' => 'i',
            'ó' | 'ò' | 'ô' | 'õ' | 'ö' => 'o',
            'ú' | 'ù' | 'û' | 'ü' => 'u',
            'ç' => 'c',
            'ñ' => 'n',
            c => c,
        })
        .collect()
}

/// O projeto aparece na busca? Cada palavra precisa estar no nome ou no caminho, sem diferenciar
/// maiúsculas nem acentos. Busca vazia mostra tudo.
pub fn matches(query: &str, name: &str, location: &str) -> bool {
    let haystack = format!("{} {}", fold(name), fold(location));
    fold(query).split_whitespace().all(|word| haystack.contains(word))
}

/// Uma tecla digitada (`WM_CHAR`) na busca: letras entram, Backspace apaga uma, Ctrl+Backspace
/// limpa e Ctrl+V cola a primeira linha de `paste`. `true` se o texto mudou.
pub fn edit_query(query: &mut String, code: u32, paste: Option<&str>) -> bool {
    const MAX: usize = 120;
    match code {
        8 => query.pop().is_some(),
        127 => {
            let had = !query.is_empty();
            query.clear();
            had
        }
        22 => {
            let line = paste.and_then(|p| p.lines().map(str::trim).find(|l| !l.is_empty())).unwrap_or("");
            let room = MAX.saturating_sub(query.chars().count());
            query.extend(line.chars().take(room));
            !line.is_empty() && room > 0
        }
        c if c >= 32 && query.chars().count() < MAX => match char::from_u32(c) {
            Some(ch) => {
                query.push(ch);
                true
            }
            None => false,
        },
        _ => false,
    }
}

/// Linha sob o mouse (o índice em `items` que o `vscode_panel` devolveu nas áreas de clique). O
/// mouse leva o mesmo destaque das setas, como no menu do Z: sem isso o painel parecia parado.
pub fn hovered_row(hits: &[(Rect, usize)], p: Point) -> Option<usize> {
    hits.iter().find(|(r, _)| r.contains(p)).map(|(_, i)| *i)
}

/// Desenho, as áreas de clique (retângulo, índice do projeto) e a caixa de busca. `message` aparece
/// sem itens ("Carregando…", "VS Code não encontrado"); `selected` é a linha escolhida pelo teclado.
/// Estado da busca para o desenho: o texto, a linha escolhida e o medidor (para o cursor).
pub struct Search<'a> { pub query: &'a str, pub selected: Option<usize>, pub measure: &'a dyn TextMeasurer }

pub fn vscode_panel(items: &[RecentRow], scroll: usize, message: Option<&str>, search_state: &Search, scale: f32, tokens: &Tokens) -> (DisplayList, Vec<(Rect, usize)>, Rect) {
    let (query, selected) = (search_state.query, search_state.selected);
    let px = |v: f32| (v * scale).round();
    let text = |size: f32, weight: FontWeight| TextStyle { size: size * scale, weight, family: Family::Text };
    let scroll = scroll.min(max_scroll(items.len()));
    let shown = items.len().saturating_sub(scroll).min(VISIBLE);
    let body = if shown == 0 { 40.0 } else { shown as f32 * ROW_H };
    let size = Size { w: px(WIDTH), h: px(ROWS_Y + body + 10.0) };
    let mut list = DisplayList::new(size);
    let mut hits = Vec::new();
    list.push(DrawOp::Text { rect: Rect::new(px(20.0), px(14.0), size.w - px(40.0), px(22.0)), text: "VS Code · recentes".into(), style: text(13.0, FontWeight::SemiBold), color: tokens.text_secondary, align: Align::Leading });
    let search = Rect::new(px(10.0), px(44.0), size.w - px(20.0), px(30.0));
    list.push(DrawOp::RoundRect { rect: search, radius: px(10.0), color: tokens.raised });
    // Lupa, o texto (ou a dica) e o cursor logo depois do texto.
    list.push(DrawOp::Text { rect: Rect::new(search.x + px(10.0), search.y, px(16.0), search.h), text: "\u{E721}".into(), style: TextStyle { size: 12.0 * scale, weight: FontWeight::Regular, family: Family::Icons }, color: tokens.text_tertiary, align: Align::Center });
    let field = Rect::new(search.x + px(32.0), search.y, search.w - px(42.0), search.h);
    let style = text(12.5, FontWeight::Regular);
    if query.is_empty() {
        list.push(DrawOp::Text { rect: field, text: "Buscar projetos".into(), style, color: tokens.text_tertiary, align: Align::Leading });
    } else {
        list.push(DrawOp::Text { rect: field, text: query.into(), style, color: tokens.text, align: Align::Leading });
    }
    let caret_x = field.x + if query.is_empty() { 0.0 } else { search_state.measure.measure(query, &style).w.min(field.w) };
    list.push(DrawOp::RoundRect { rect: Rect::new(caret_x, search.y + px(8.0), px(1.5), search.h - px(16.0)), radius: px(0.75), color: tokens.accent });
    if shown == 0 {
        let note = message.unwrap_or("Nenhum projeto encontrado");
        list.push(DrawOp::Text { rect: Rect::new(px(20.0), px(ROWS_Y + 6.0), size.w - px(40.0), px(20.0)), text: note.into(), style: text(12.5, FontWeight::Regular), color: tokens.text, align: Align::Leading });
        return (list, hits, search);
    }
    for (row, (index, item)) in items.iter().enumerate().skip(scroll).take(shown).enumerate() {
        let r = Rect::new(px(10.0), px(ROWS_Y + row as f32 * ROW_H), size.w - px(20.0), px(ROW_H));
        if selected == Some(index) {
            list.push(DrawOp::RoundRect { rect: r, radius: px(10.0), color: tokens.raised });
        }
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
    (list, hits, search)
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

    fn search(query: &str, selected: Option<usize>) -> Search<'_> {
        Search { query, selected, measure: &zefiro_platform::text::FixedWidthMeasurer }
    }

    fn texts(l: &DisplayList) -> Vec<String> {
        l.ops.iter().filter_map(|o| if let DrawOp::Text { text, .. } = o { Some(text.clone()) } else { None }).collect()
    }

    #[test]
    fn search_ignores_case_and_accents_and_needs_every_word() {
        let (name, location) = ("Portal-Mizuho", r"C:\Users\felip\Projetos Ação");
        assert!(matches("", name, location), "busca vazia mostra tudo");
        assert!(matches("mizu", name, location));
        assert!(matches("PORTAL", name, location));
        assert!(matches("acao", name, location), "sem acento acha com acento");
        assert!(matches("portal açã", name, location), "cada palavra pode estar no nome ou no caminho");
        assert!(!matches("portal zefiro", name, location), "todas as palavras precisam aparecer");
    }

    #[test]
    fn typing_edits_the_query() {
        let mut q = String::new();
        for c in "zéf".chars() {
            assert!(edit_query(&mut q, c as u32, None));
        }
        assert_eq!(q, "zéf");
        assert!(edit_query(&mut q, 8, None), "Backspace apaga uma letra");
        assert_eq!(q, "zé");
        assert!(edit_query(&mut q, 22, Some("  portal\nmizuho ")), "Ctrl+V cola a primeira linha");
        assert_eq!(q, "zéportal");
        assert!(edit_query(&mut q, 127, None), "Ctrl+Backspace limpa");
        assert_eq!(q, "");
        assert!(!edit_query(&mut q, 8, None), "nada a apagar");
        assert!(!edit_query(&mut q, 13, None), "Enter não é texto");
    }

    #[test]
    fn search_box_shows_the_hint_or_the_query() {
        let items: Vec<RecentRow> = (0..3).map(row).collect();
        let (list, _, _) = vscode_panel(&items, 0, None, &search("", None), 1.0, &tokens());
        assert!(texts(&list).contains(&"Buscar projetos".to_string()));
        let (list, _, _) = vscode_panel(&items, 0, None, &search("proj", None), 1.0, &tokens());
        assert!(texts(&list).contains(&"proj".to_string()) && !texts(&list).contains(&"Buscar projetos".to_string()));
    }

    #[test]
    fn search_box_sits_above_the_rows_and_the_selected_row_is_lit() {
        let items: Vec<RecentRow> = (0..3).map(row).collect();
        let (list, hits, search) = vscode_panel(&items, 0, None, &search("", Some(1)), 1.0, &tokens());
        assert!(search.bottom() <= hits[0].0.y, "a busca fica acima da primeira linha");
        let lit = list.ops.iter().filter(|o| matches!(o, DrawOp::RoundRect { rect, .. } if rect.x == hits[1].0.x && rect.y == hits[1].0.y && rect.w == hits[1].0.w)).count();
        assert_eq!(lit, 1, "a linha escolhida pelo teclado tem fundo");
    }

    #[test]
    fn panel_shows_name_date_location_and_badge() {
        let items: Vec<RecentRow> = (0..3).map(row).collect();
        let (list, hits, _) = vscode_panel(&items, 0, None, &search("", None), 1.0, &tokens());
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
        let (list, hits, _) = vscode_panel(&items, 3, None, &search("", None), 1.0, &tokens());
        assert_eq!(hits.len(), 12);
        assert_eq!(hits[0].1, 3);
        assert_eq!(list.size.h, 82.0 + 12.0 * 44.0 + 10.0);
        assert!(!texts(&list).contains(&"projeto-0".to_string()));
        // A rolagem não passa do fim.
        assert_eq!(max_scroll(20), 8);
        assert_eq!(max_scroll(5), 0);
    }

    #[test]
    fn the_row_under_the_mouse_takes_the_highlight() {
        let items: Vec<RecentRow> = (0..20).map(row).collect();
        // Rolado em 1: as áreas de clique dizem a posição na lista mostrada.
        let (_, hits, search_box) = vscode_panel(&items, 1, None, &search("", None), 1.0, &tokens());
        let over = |r: Rect| Point { x: r.center_x(), y: r.y + r.h / 2.0 };
        assert_eq!(hovered_row(&hits, over(hits[2].0)), Some(3), "terceira linha visível = projeto 3");
        assert_eq!(hovered_row(&hits, over(search_box)), None, "a busca não é linha");
        let (list, _, _) = vscode_panel(&items, 1, None, &search("", hovered_row(&hits, over(hits[0].0))), 1.0, &tokens());
        assert!(list.ops.iter().any(|o| matches!(o, DrawOp::RoundRect { rect, .. } if *rect == hits[0].0)), "a linha sob o mouse ganha o fundo");
    }

    #[test]
    fn panel_message_without_items() {
        let (list, hits, _) = vscode_panel(&[], 0, Some("Carregando…"), &search("", None), 1.0, &tokens());
        assert!(texts(&list).contains(&"Carregando…".to_string()));
        assert!(hits.is_empty());
    }
}
