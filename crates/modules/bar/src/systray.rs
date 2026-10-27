//! Painel "Bandeja": os ícones da bandeja do sistema numa grade de 5 colunas, como a setinha do
//! Windows 11. O nome do app aparece embaixo da grade com o mouse em cima (ou com a seleção do
//! teclado); o Zefiro não tem janela de dica.

use zefiro_platform::geometry::{Point, Rect, Size};
use zefiro_platform::image::Image;
use zefiro_platform::text::{Align, Family, FontWeight, TextStyle};
use zefiro_resources::tokens::Tokens;
use zefiro_ui::paint::{DisplayList, DrawOp};

pub const COLUMNS: usize = 5;
pub const EMPTY: &str = "Nenhum ícone na bandeja";

/// Margem do painel, lado da célula, lado do ícone e altura da linha do nome (DIP).
const PAD: f32 = 12.0;
const CELL: f32 = 36.0;
const ICON: f32 = 16.0;
const NAME_H: f32 = 24.0;
/// Ícone genérico para quem não mandou imagem (Segoe Fluent "AllApps").
const GLYPH_APP: &str = "\u{E71D}";

pub struct Cell<'a> {
    pub name: &'a str,
    pub image: Option<&'a Image>,
}

pub struct TrayView<'a> {
    pub cells: &'a [Cell<'a>],
    pub hover: Option<usize>,
    pub selected: Option<usize>,
    /// Primeira linha visível.
    pub scroll: usize,
    /// Altura máxima do painel (DIP): o que cabe no monitor abaixo da barra.
    pub max_h: f32,
}

pub struct TrayLayout {
    pub list: DisplayList,
    pub cells: Vec<(Rect, usize)>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Nav {
    Left,
    Right,
    Up,
    Down,
}

/// Linhas que cabem em `max_h` (pelo menos uma).
fn rows_fitting(max_h: f32) -> usize {
    (((max_h - PAD * 2.0 - NAME_H) / CELL).floor() as usize).max(1)
}

/// Primeira linha máxima da rolagem.
pub fn max_scroll(count: usize, max_h: f32) -> usize {
    count.div_ceil(COLUMNS).saturating_sub(rows_fitting(max_h))
}

pub fn tray_panel(view: &TrayView, scale: f32, tokens: &Tokens) -> TrayLayout {
    let px = |v: f32| (v * scale).round();
    let text = |size: f32| TextStyle { size: size * scale, weight: FontWeight::Regular, family: Family::Text };
    let w = px(PAD * 2.0 + CELL * COLUMNS as f32);
    let mut ops = Vec::new();
    let mut cells = Vec::new();
    if view.cells.is_empty() {
        ops.push(DrawOp::Text { rect: Rect::new(px(PAD), px(PAD), w - px(PAD * 2.0), px(20.0)), text: EMPTY.into(), style: text(12.0), color: tokens.text_secondary, align: Align::Center });
        return TrayLayout { list: DisplayList { size: Size { w, h: px(PAD * 2.0 + 20.0) }, ops }, cells };
    }
    let total_rows = view.cells.len().div_ceil(COLUMNS);
    let rows = total_rows.min(rows_fitting(view.max_h));
    let scroll = view.scroll.min(max_scroll(view.cells.len(), view.max_h));
    let first = scroll * COLUMNS;
    let shown = view.cells.len().min(first + rows * COLUMNS);
    for i in first..shown {
        let (row, col) = ((i - first) / COLUMNS, (i - first) % COLUMNS);
        let r = Rect::new(px(PAD + col as f32 * CELL), px(PAD + row as f32 * CELL), px(CELL), px(CELL));
        if view.selected == Some(i) || view.hover == Some(i) {
            ops.push(DrawOp::RoundRect { rect: r.inset(px(2.0), px(2.0)), radius: px(8.0), color: tokens.raised });
        }
        let icon = Rect::new(r.x + (r.w - px(ICON)) / 2.0, r.y + (r.h - px(ICON)) / 2.0, px(ICON), px(ICON));
        match view.cells[i].image {
            Some(image) => ops.push(DrawOp::Image { rect: icon, image: image.clone(), radius: 0.0, opacity: 1.0 }),
            None => ops.push(DrawOp::Text { rect: icon, text: GLYPH_APP.into(), style: TextStyle { size: 13.0 * scale, weight: FontWeight::Regular, family: Family::Icons }, color: tokens.text_secondary, align: Align::Center }),
        }
        cells.push((r, i));
    }
    let name_y = px(PAD + rows as f32 * CELL);
    if let Some(cell) = view.hover.or(view.selected).and_then(|i| view.cells.get(i)) {
        ops.push(DrawOp::Text { rect: Rect::new(px(PAD), name_y, w - px(PAD * 2.0), px(NAME_H)), text: cell.name.to_string(), style: text(11.5), color: tokens.text_secondary, align: Align::Center });
    }
    TrayLayout { list: DisplayList { size: Size { w, h: name_y + px(NAME_H) + px(PAD) }, ops }, cells }
}

pub fn cell_at(layout: &TrayLayout, p: Point) -> Option<usize> {
    layout.cells.iter().find(|(r, _)| r.contains(p)).map(|(_, i)| *i)
}

/// O nome que o painel mostra: a primeira linha da dica ou, sem dica, o executável sem `.exe`.
pub fn display_name(tip: &str, exe: &str) -> String {
    match tip.lines().map(str::trim).find(|l| !l.is_empty()) {
        Some(line) => line.to_string(),
        None => exe.strip_suffix(".exe").or_else(|| exe.strip_suffix(".EXE")).unwrap_or(exe).to_string(),
    }
}

/// Setas na grade; para nas bordas. Sem seleção, qualquer seta escolhe o primeiro.
pub fn step(selected: Option<usize>, count: usize, key: Nav) -> Option<usize> {
    if count == 0 {
        return None;
    }
    let Some(i) = selected.filter(|&i| i < count) else { return Some(0) };
    let next = match key {
        Nav::Left if i % COLUMNS > 0 => i - 1,
        Nav::Right if i % COLUMNS < COLUMNS - 1 && i + 1 < count => i + 1,
        Nav::Up if i >= COLUMNS => i - COLUMNS,
        Nav::Down if i + COLUMNS < count => i + COLUMNS,
        _ => i,
    };
    Some(next)
}


#[cfg(test)]
mod tests {
    use super::*;
    use zefiro_platform::color::Rgba;
    use zefiro_resources::theme::{BUILTIN, parse_theme};
    use zefiro_resources::tokens::derive_tokens;

    fn tokens_for_tests() -> Tokens {
        derive_tokens(&parse_theme("obsidian", "o", BUILTIN[0].1).unwrap(), Rgba::rgb(0, 120, 215), None)
    }

    fn cells(names: &[&'static str]) -> Vec<Cell<'static>> {
        names.iter().map(|n| Cell { name: n, image: None }).collect()
    }

    fn view<'a>(cells: &'a [Cell<'a>]) -> TrayView<'a> {
        TrayView { cells, hover: None, selected: None, scroll: 0, max_h: 600.0 }
    }

    fn texts(layout: &TrayLayout) -> Vec<String> {
        layout.list.ops.iter().filter_map(|o| if let DrawOp::Text { text, .. } = o { Some(text.clone()) } else { None }).collect()
    }

    #[test]
    fn five_columns() {
        let c = cells(&["a", "b", "c", "d", "e", "f", "g"]);
        let l = tray_panel(&view(&c), 1.0, &tokens_for_tests());
        assert_eq!(l.cells.len(), 7);
        let rect = |i: usize| l.cells.iter().find(|(_, j)| *j == i).unwrap().0;
        assert!(rect(1).x > rect(0).x);
        assert_eq!(rect(5).x, rect(0).x, "a 6ª célula começa a linha 2");
        assert!(rect(5).y > rect(0).y);
        assert_eq!(rect(4).y, rect(0).y);
    }

    #[test]
    fn cell_at_hits_the_right_icon() {
        let c = cells(&["a", "b", "c", "d"]);
        let l = tray_panel(&view(&c), 2.0, &tokens_for_tests());
        let r = l.cells.iter().find(|(_, j)| *j == 3).unwrap().0;
        assert_eq!(cell_at(&l, Point { x: r.x + r.w / 2.0, y: r.y + r.h / 2.0 }), Some(3));
        assert_eq!(cell_at(&l, Point { x: -5.0, y: -5.0 }), None);
    }

    #[test]
    fn empty_shows_message() {
        let l = tray_panel(&view(&[]), 1.0, &tokens_for_tests());
        assert!(l.cells.is_empty());
        assert!(texts(&l).iter().any(|t| t == EMPTY));
    }

    #[test]
    fn hover_shows_name() {
        let c = cells(&["Discord", "Steam"]);
        let plain = tray_panel(&view(&c), 1.0, &tokens_for_tests());
        assert!(!texts(&plain).iter().any(|t| t == "Steam" || t == "Discord"));
        let hovered = tray_panel(&TrayView { hover: Some(1), ..view(&c) }, 1.0, &tokens_for_tests());
        assert!(texts(&hovered).iter().any(|t| t == "Steam"));
    }

    #[test]
    fn selection_shows_name() {
        let c = cells(&["Discord", "Steam"]);
        let l = tray_panel(&TrayView { selected: Some(0), ..view(&c) }, 1.0, &tokens_for_tests());
        assert!(texts(&l).iter().any(|t| t == "Discord"));
    }

    #[test]
    fn name_is_first_tip_line() {
        assert_eq!(display_name("Discord\n3 mensagens", "Discord.exe"), "Discord");
        assert_eq!(display_name("  Steam  ", "steam.exe"), "Steam");
    }

    #[test]
    fn name_falls_back_to_exe() {
        assert_eq!(display_name("", "steamwebhelper.exe"), "steamwebhelper");
        assert_eq!(display_name("\n", "Docker Desktop.exe"), "Docker Desktop");
        assert_eq!(display_name("", ""), "");
    }

    #[test]
    fn step_moves_in_grid() {
        assert_eq!(step(Some(0), 10, Nav::Right), Some(1));
        assert_eq!(step(Some(1), 10, Nav::Down), Some(6));
        assert_eq!(step(Some(7), 10, Nav::Down), Some(7), "não há linha abaixo");
        assert_eq!(step(Some(6), 10, Nav::Up), Some(1));
        assert_eq!(step(Some(0), 10, Nav::Left), Some(0));
        assert_eq!(step(Some(9), 10, Nav::Right), Some(9));
        assert_eq!(step(Some(4), 7, Nav::Down), Some(4), "a coluna 4 não existe na linha 2");
        assert_eq!(step(None, 10, Nav::Left), Some(0));
        assert_eq!(step(None, 0, Nav::Down), None);
    }

    #[test]
    fn many_icons_scroll_and_cap_height() {
        let names: Vec<&'static str> = (0..40).map(|_| "x").collect();
        let c = cells(&names);
        let v = TrayView { max_h: 200.0, ..view(&c) };
        let l = tray_panel(&v, 1.0, &tokens_for_tests());
        assert!(l.list.size.h <= 200.0, "{}", l.list.size.h);
        assert!(max_scroll(40, 200.0) > 0);
        let scrolled = tray_panel(&TrayView { scroll: 1, ..v }, 1.0, &tokens_for_tests());
        assert_eq!(scrolled.cells.iter().map(|(_, i)| *i).min(), Some(COLUMNS));
    }

    #[test]
    fn scroll_is_clamped() {
        let c = cells(&["a", "b"]);
        let l = tray_panel(&TrayView { scroll: 9, ..view(&c) }, 1.0, &tokens_for_tests());
        assert_eq!(l.cells.len(), 2);
    }
}
