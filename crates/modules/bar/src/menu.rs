//! Painel do Zefiro: altura, formato, tema e ações, com navegação por teclado.

use zefiro_core::config::{BarFormat, BarHeight};
use zefiro_platform::color::Rgba;
use zefiro_platform::geometry::{Point, Rect, Size};
use zefiro_platform::text::{Align, Family, FontWeight, TextStyle};
use zefiro_resources::tokens::Tokens;
use zefiro_ui::paint::{DisplayList, DrawOp};

#[derive(Debug, Clone, PartialEq)]
pub enum MenuAction { SetHeight(BarHeight), SetFormat(BarFormat), SetTheme(String), ReloadTheme, OpenThemesFolder, RestartIsland, Exit, EditBar }

#[derive(Debug, Clone, PartialEq)]
pub struct ThemeSwatch { pub id: String, pub name: String, pub background: Rgba, pub text: Rgba, pub border: Option<Rgba> }

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NavKey { Up, Down, Left, Right, Enter }

#[derive(Debug, Clone, PartialEq)]
pub struct MenuItemBox { pub rect: Rect, pub row: usize, pub col: usize, pub action: MenuAction }

const W: f32 = 300.0;
const PAD: f32 = 16.0;
const TITLE_H: f32 = 20.0;
const ROW_H: f32 = 32.0;
const SWATCH_W: f32 = 67.0;
const SWATCH_H: f32 = 56.0;
const SECTION_GAP: f32 = 8.0;
const COLS: usize = 4;

fn actions() -> [(&'static str, MenuAction); 5] {
    [
        ("Editar barra", MenuAction::EditBar),
        ("Recarregar tema", MenuAction::ReloadTheme),
        ("Abrir pasta de temas", MenuAction::OpenThemesFolder),
        ("Reiniciar a ilha", MenuAction::RestartIsland),
        ("Sair", MenuAction::Exit),
    ]
}

pub struct MenuModel { height: BarHeight, format: BarFormat, theme_id: String, themes: Vec<ThemeSwatch>, focus: (usize, usize) }

/// Onde ficam os títulos de seção (topo, em px), para o `paint`.
struct Titles { altura: f32, formato: f32, tema: f32 }

impl MenuModel {
    pub fn new(height: BarHeight, format: BarFormat, theme_id: &str, themes: Vec<ThemeSwatch>) -> MenuModel {
        let col = if height == BarHeight::Compacta { 1 } else { 0 };
        MenuModel { height, format, theme_id: theme_id.to_string(), themes, focus: (0, col) }
    }

    pub fn focus(&self) -> (usize, usize) {
        self.focus
    }

    fn build(&self, scale: f32) -> (Vec<MenuItemBox>, Titles, f32) {
        let s = |v: f32| v * scale;
        let inner = s(W - 2.0 * PAD);
        let mut out = Vec::new();
        let mut y = s(PAD);
        let mut row = 0;

        let mut segment = |y: &mut f32, row: &mut usize, opts: [MenuAction; 2]| {
            let w = inner / 2.0;
            for (col, action) in opts.into_iter().enumerate() {
                out.push(MenuItemBox { rect: Rect::new(s(PAD) + w * col as f32, *y, w, s(ROW_H)), row: *row, col, action });
            }
            *y += s(ROW_H);
            *row += 1;
        };

        let altura = y;
        y += s(TITLE_H);
        segment(&mut y, &mut row, [MenuAction::SetHeight(BarHeight::Completa), MenuAction::SetHeight(BarHeight::Compacta)]);
        y += s(SECTION_GAP);
        let formato = y;
        y += s(TITLE_H);
        segment(&mut y, &mut row, [MenuAction::SetFormat(BarFormat::Faixa), MenuAction::SetFormat(BarFormat::Pilulas)]);
        y += s(SECTION_GAP);
        let tema = y;
        y += s(TITLE_H);
        for (i, t) in self.themes.iter().enumerate() {
            let (r, c) = (i / COLS, i % COLS);
            let rect = Rect::new(s(PAD) + s(SWATCH_W) * c as f32, y + s(SWATCH_H) * r as f32, s(SWATCH_W), s(SWATCH_H));
            out.push(MenuItemBox { rect, row: row + r, col: c, action: MenuAction::SetTheme(t.id.clone()) });
        }
        let theme_rows = self.themes.len().div_ceil(COLS);
        y += s(SWATCH_H) * theme_rows as f32;
        row += theme_rows;
        y += s(SECTION_GAP);
        for (_, action) in actions() {
            out.push(MenuItemBox { rect: Rect::new(s(PAD), y, inner, s(ROW_H)), row, col: 0, action });
            y += s(ROW_H);
            row += 1;
        }
        (out, Titles { altura, formato, tema }, y + s(PAD))
    }

    pub fn layout(&self, scale: f32) -> Vec<MenuItemBox> {
        self.build(scale).0
    }

    pub fn size(&self, scale: f32) -> Size {
        Size { w: W * scale, h: self.build(scale).2 }
    }

    pub fn hit(&self, p: Point, scale: f32) -> Option<MenuAction> {
        self.layout(scale).into_iter().find(|b| b.rect.contains(p)).map(|b| b.action)
    }

    pub fn key(&mut self, k: NavKey) -> Option<MenuAction> {
        let boxes = self.layout(1.0);
        let rows = boxes.iter().map(|b| b.row).max().map_or(0, |r| r + 1);
        let (mut r, mut c) = self.focus;
        match k {
            NavKey::Up => r = r.saturating_sub(1),
            NavKey::Down => r = (r + 1).min(rows.saturating_sub(1)),
            NavKey::Left => c = c.saturating_sub(1),
            NavKey::Right => c += 1,
            NavKey::Enter => return boxes.iter().find(|b| b.row == r && b.col == c).map(|b| b.action.clone()),
        }
        let len = boxes.iter().filter(|b| b.row == r).count();
        self.focus = (r, c.min(len.saturating_sub(1)));
        None
    }

    fn is_selected(&self, a: &MenuAction) -> bool {
        match a {
            MenuAction::SetHeight(h) => *h == self.height,
            MenuAction::SetFormat(f) => *f == self.format,
            MenuAction::SetTheme(id) => *id == self.theme_id,
            _ => false,
        }
    }

    pub fn paint(&self, scale: f32, tokens: &Tokens) -> DisplayList {
        let s = |v: f32| v * scale;
        let (boxes, titles, height) = self.build(scale);
        let mut list = DisplayList::new(Size { w: s(W), h: height });
        let title = TextStyle { size: s(11.0), weight: FontWeight::SemiBold, family: Family::Text };
        let body = TextStyle { size: s(13.0), weight: FontWeight::Regular, family: Family::Text };
        let small = TextStyle { size: s(11.0), weight: FontWeight::Regular, family: Family::Text };

        for (text, y) in [("Altura", titles.altura), ("Formato", titles.formato), ("Tema", titles.tema)] {
            list.push(DrawOp::Text { rect: Rect::new(s(PAD), y, s(W - 2.0 * PAD), s(TITLE_H)), text: text.into(), style: title, color: tokens.text_tertiary, align: Align::Leading });
        }
        for row in [0usize, 1] {
            let cells: Vec<&MenuItemBox> = boxes.iter().filter(|b| b.row == row).collect();
            let r = cells[0].rect.union(cells[cells.len() - 1].rect);
            list.push(DrawOp::RoundRect { rect: r, radius: r.h / 2.0, color: tokens.raised });
        }
        for b in &boxes {
            let focused = (b.row, b.col) == self.focus;
            match &b.action {
                MenuAction::SetHeight(_) | MenuAction::SetFormat(_) => {
                    let inner = b.rect.inset(s(3.0), s(3.0));
                    if self.is_selected(&b.action) {
                        list.push(DrawOp::RoundRect { rect: inner, radius: inner.h / 2.0, color: tokens.raised_strong });
                    }
                    let label = match &b.action {
                        MenuAction::SetHeight(BarHeight::Completa) => "Completa",
                        MenuAction::SetHeight(BarHeight::Compacta) => "Compacta",
                        MenuAction::SetFormat(BarFormat::Faixa) => "Faixa",
                        _ => "Pílulas",
                    };
                    list.push(DrawOp::Text { rect: b.rect, text: label.into(), style: body, color: tokens.text, align: Align::Center });
                    if focused {
                        list.push(DrawOp::RoundRectStroke { rect: inner, radius: inner.h / 2.0, color: tokens.accent, width: s(1.5) });
                    }
                }
                MenuAction::SetTheme(id) => {
                    let t = self.themes.iter().find(|t| &t.id == id).expect("tema da grade");
                    let c = Point { x: b.rect.center_x(), y: b.rect.y + s(18.0) };
                    if self.is_selected(&b.action) {
                        list.push(DrawOp::Circle { center: c, radius: s(16.0), color: tokens.accent });
                    }
                    list.push(DrawOp::Circle { center: c, radius: s(14.0), color: t.border.unwrap_or(tokens.hairline) });
                    list.push(DrawOp::Circle { center: c, radius: s(13.0), color: t.background.with_alpha(1.0) });
                    list.push(DrawOp::Circle { center: c, radius: s(4.0), color: t.text });
                    list.push(DrawOp::Text { rect: Rect::new(b.rect.x, b.rect.y + s(36.0), b.rect.w, s(16.0)), text: t.name.clone(), style: small, color: tokens.text_secondary, align: Align::Center });
                    if focused {
                        list.push(DrawOp::RoundRectStroke { rect: b.rect.inset(s(2.0), s(2.0)), radius: s(10.0), color: tokens.accent, width: s(1.5) });
                    }
                }
                action => {
                    let label = actions().into_iter().find(|(_, a)| a == action).map_or("", |(l, _)| l);
                    if focused {
                        list.push(DrawOp::RoundRect { rect: b.rect, radius: s(10.0), color: tokens.raised });
                    }
                    list.push(DrawOp::Text { rect: b.rect.inset(s(12.0), 0.0), text: label.into(), style: body, color: tokens.text, align: Align::Leading });
                }
            }
        }
        list
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use zefiro_core::config::{BarFormat, BarHeight};
    use zefiro_platform::color::Rgba;

    fn swatches(n: usize) -> Vec<ThemeSwatch> {
        (0..n).map(|i| ThemeSwatch { id: format!("t{i}"), name: format!("T{i}"), background: Rgba::BLACK, text: Rgba::WHITE, border: None }).collect()
    }

    fn model() -> MenuModel {
        MenuModel::new(BarHeight::Completa, BarFormat::Faixa, "t2", swatches(8))
    }

    #[test]
    fn starts_focused_on_the_selected_height() {
        assert_eq!(model().focus(), (0, 0));
        assert_eq!(MenuModel::new(BarHeight::Compacta, BarFormat::Faixa, "t2", swatches(8)).focus(), (0, 1));
    }

    #[test]
    fn right_then_enter_picks_compact() {
        let mut m = model();
        assert_eq!(m.key(NavKey::Right), None);
        assert_eq!(m.key(NavKey::Enter), Some(MenuAction::SetHeight(BarHeight::Compacta)));
    }

    #[test]
    fn down_walks_rows_and_clamps_columns() {
        let mut m = model();
        m.key(NavKey::Down); // formato
        assert_eq!(m.key(NavKey::Enter), Some(MenuAction::SetFormat(BarFormat::Faixa)));
        m.key(NavKey::Down); // primeira linha de temas
        for _ in 0..4 {
            m.key(NavKey::Right); // a quarta passa do fim e fica na coluna 3
        }
        assert_eq!(m.key(NavKey::Enter), Some(MenuAction::SetTheme("t3".into())));
        m.key(NavKey::Down); // segunda linha de temas, coluna 3
        assert_eq!(m.key(NavKey::Enter), Some(MenuAction::SetTheme("t7".into())));
        m.key(NavKey::Down); // primeira ação, coluna volta a 0
        assert_eq!(m.key(NavKey::Enter), Some(MenuAction::EditBar));
        for _ in 0..10 {
            m.key(NavKey::Down);
        }
        assert_eq!(m.key(NavKey::Enter), Some(MenuAction::Exit));
        for _ in 0..20 {
            m.key(NavKey::Up);
        }
        assert_eq!(m.focus().0, 0);
    }

    #[test]
    fn partial_last_theme_row_clamps_column() {
        let mut m = MenuModel::new(BarHeight::Completa, BarFormat::Faixa, "t0", swatches(9));
        m.key(NavKey::Down);
        m.key(NavKey::Down);
        for _ in 0..3 {
            m.key(NavKey::Right);
        }
        m.key(NavKey::Down);
        m.key(NavKey::Down); // terceira linha de temas só tem t8
        assert_eq!(m.key(NavKey::Enter), Some(MenuAction::SetTheme("t8".into())));
    }

    #[test]
    fn hit_test_matches_layout() {
        let m = model();
        for b in m.layout(1.0) {
            let center = Point { x: b.rect.x + b.rect.w / 2.0, y: b.rect.y + b.rect.h / 2.0 };
            assert_eq!(m.hit(center, 1.0), Some(b.action.clone()), "{b:?}");
        }
        assert_eq!(m.hit(Point { x: 1.0, y: 1.0 }, 1.0), None);
    }

    #[test]
    fn size_grows_with_themes_and_scale() {
        let a = MenuModel::new(BarHeight::Completa, BarFormat::Faixa, "t0", swatches(8)).size(1.0);
        let b = MenuModel::new(BarHeight::Completa, BarFormat::Faixa, "t0", swatches(9)).size(1.0);
        assert_eq!(a.w, 300.0);
        assert_eq!(b.h - a.h, 56.0);
        assert_eq!(MenuModel::new(BarHeight::Completa, BarFormat::Faixa, "t0", swatches(8)).size(1.5).w, 450.0);
    }

    #[test]
    fn paint_marks_current_choices() {
        let o = zefiro_resources::theme::parse_theme("obsidian", "o", zefiro_resources::theme::BUILTIN[0].1).unwrap();
        let t = zefiro_resources::tokens::derive_tokens(&o, Rgba::rgb(0, 120, 215), None);
        let list = model().paint(1.0, &t);
        let strong = list.ops.iter().filter(|op| matches!(op, zefiro_ui::paint::DrawOp::RoundRect { color, .. } if *color == t.raised_strong)).count();
        assert_eq!(strong, 2); // altura e formato atuais
        assert_eq!(list.size, model().size(1.0));
    }
}
