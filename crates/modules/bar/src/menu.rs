//! Painel do Zefiro: altura, formato, a linha do tema e ações, com navegação por teclado. A linha do
//! tema troca o conteúdo do painel pela grade de temas ("‹ Tema" volta).

use zefiro_core::config::{BarFormat, BarHeight};
use zefiro_platform::color::Rgba;
use zefiro_platform::geometry::{Point, Rect, Size};
use zefiro_platform::text::{Align, Family, FontWeight, TextStyle};
use zefiro_resources::tokens::Tokens;
use zefiro_ui::paint::{DisplayList, DrawOp};

#[derive(Debug, Clone, PartialEq)]
pub enum MenuAction { SetHeight(BarHeight), SetFormat(BarFormat), SetTheme(String), OpenThemes, BackToMenu, ReloadTheme, OpenThemesFolder, RestartIsland, ToggleAcrylic, ToggleAutostart, ToggleHud, ExportDiagnostics, InstallUpdate, Exit, EditBar }

#[derive(Debug, Clone, PartialEq)]
pub struct ThemeSwatch { pub id: String, pub name: String, pub background: Rgba, pub text: Rgba, pub border: Option<Rgba> }

/// O que o painel mostra: o menu ou a grade de temas.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MenuPage { Main, Themes }

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

fn actions() -> [(&'static str, MenuAction); 6] {
    [
        ("Editar barra", MenuAction::EditBar),
        ("Reiniciar a ilha", MenuAction::RestartIsland),
        ("Iniciar com o Windows", MenuAction::ToggleAutostart),
        ("HUD de debug", MenuAction::ToggleHud),
        ("Exportar diagnóstico", MenuAction::ExportDiagnostics),
        ("Sair", MenuAction::Exit),
    ]
}

/// Ações do fim da tela de temas.
fn theme_actions() -> [(&'static str, MenuAction); 2] {
    [("Recarregar tema", MenuAction::ReloadTheme), ("Abrir pasta de temas", MenuAction::OpenThemesFolder)]
}

/// Chave liga/desliga à direita da linha: trilho no destaque quando ligada, botão do lado dela.
fn switch(list: &mut DisplayList, row: Rect, on: bool, scale: f32, tokens: &Tokens) {
    let s = |v: f32| v * scale;
    let track = Rect::new(row.right() - s(46.0), row.y + (row.h - s(18.0)) / 2.0, s(34.0), s(18.0));
    list.push(DrawOp::RoundRect { rect: track, radius: track.h / 2.0, color: if on { tokens.accent } else { tokens.text_tertiary.with_alpha(0.4) } });
    let knob_x = if on { track.right() - s(9.0) } else { track.x + s(9.0) };
    list.push(DrawOp::Circle { center: Point { x: knob_x, y: track.y + track.h / 2.0 }, radius: s(7.0), color: Rgba::WHITE });
}

fn action_label(action: &MenuAction) -> &'static str {
    actions().into_iter().chain(theme_actions()).find(|(_, a)| a == action).map_or("", |(l, _)| l)
}

/// `acrylic`: (ligado agora, a transparência do Windows deixa).
pub struct MenuModel { height: BarHeight, format: BarFormat, theme_id: String, themes: Vec<ThemeSwatch>, focus: (usize, usize), autostart: bool, acrylic: (bool, bool), page: MenuPage, update: Option<String> }

/// Títulos de seção (texto e topo, em px), para o `paint`.
type Titles = Vec<(&'static str, f32)>;

/// Linha da tela de temas: o "‹ Tema" é a linha 0, a grade começa na 1.
const GRID_ROW: usize = 1;
/// Linha "Tema" no menu (depois de altura e formato).
const THEME_ROW: usize = 2;

impl MenuModel {
    pub fn new(height: BarHeight, format: BarFormat, theme_id: &str, themes: Vec<ThemeSwatch>) -> MenuModel {
        let col = if height == BarHeight::Compacta { 1 } else { 0 };
        MenuModel { height, format, theme_id: theme_id.to_string(), themes, focus: (0, col), autostart: false, acrylic: (false, true), page: MenuPage::Main, update: None }
    }

    pub fn page(&self) -> MenuPage {
        self.page
    }

    /// A linha "Tema": o painel passa a mostrar a grade, com o foco no tema atual.
    pub fn open_themes(&mut self) {
        self.page = MenuPage::Themes;
        let i = self.themes.iter().position(|t| t.id == self.theme_id).unwrap_or(0);
        self.focus = (GRID_ROW + i / COLS, i % COLS);
    }

    /// "‹ Tema" ou Esc: volta ao menu, com o foco na linha "Tema".
    pub fn back(&mut self) {
        self.page = MenuPage::Main;
        self.focus = (THEME_ROW, 0);
    }

    /// Altura, formato ou tema mudaram: o painel continua na mesma tela.
    pub fn set_choices(&mut self, height: BarHeight, format: BarFormat, theme_id: &str) {
        self.height = height;
        self.format = format;
        self.theme_id = theme_id.to_string();
    }

    /// A lista de temas mudou (recarregar tema, temas do usuário).
    pub fn set_themes(&mut self, themes: Vec<ThemeSwatch>) {
        self.themes = themes;
    }

    /// Texto da linha de atualização ("Atualizar para a 1.1.0", "Baixando…"); `None` esconde a linha.
    pub fn set_update(&mut self, label: Option<String>) {
        self.update = label;
    }

    /// Acrílico ligado agora e se o Windows deixa (sem "Efeitos de transparência" a chave some).
    pub fn set_acrylic(&mut self, on: bool, available: bool) {
        self.acrylic = (on, available);
    }

    pub fn set_autostart(&mut self, on: bool) {
        self.autostart = on;
    }

    /// "Iniciar com o Windows" ligado (a chave do item acende).
    pub fn with_autostart(mut self, on: bool) -> MenuModel {
        self.autostart = on;
        self
    }

    pub fn focus(&self) -> (usize, usize) {
        self.focus
    }

    fn build(&self, scale: f32) -> (Vec<MenuItemBox>, Titles, f32) {
        let s = |v: f32| v * scale;
        let inner = s(W - 2.0 * PAD);
        let mut out = Vec::new();
        let mut titles = Vec::new();
        let mut y = s(PAD);
        let mut row = 0;
        let line = |out: &mut Vec<MenuItemBox>, y: &mut f32, row: &mut usize, action: MenuAction| {
            out.push(MenuItemBox { rect: Rect::new(s(PAD), *y, inner, s(ROW_H)), row: *row, col: 0, action });
            *y += s(ROW_H);
            *row += 1;
        };
        match self.page {
            MenuPage::Main => {
                for (title, opts) in [
                    ("Altura", vec![MenuAction::SetHeight(BarHeight::Completa), MenuAction::SetHeight(BarHeight::Compacta)]),
                    ("Formato", [BarFormat::Faixa, BarFormat::Pilulas, BarFormat::Ilhas].map(MenuAction::SetFormat).to_vec()),
                ] {
                    titles.push((title, y));
                    y += s(TITLE_H);
                    let w = inner / opts.len() as f32;
                    for (col, action) in opts.into_iter().enumerate() {
                        out.push(MenuItemBox { rect: Rect::new(s(PAD) + w * col as f32, y, w, s(ROW_H)), row, col, action });
                    }
                    y += s(ROW_H) + s(SECTION_GAP);
                    row += 1;
                }
                line(&mut out, &mut y, &mut row, MenuAction::OpenThemes);
                line(&mut out, &mut y, &mut row, MenuAction::ToggleAcrylic);
                y += s(SECTION_GAP);
                if self.update.is_some() {
                    line(&mut out, &mut y, &mut row, MenuAction::InstallUpdate);
                }
                for (_, action) in actions() {
                    line(&mut out, &mut y, &mut row, action);
                }
            }
            MenuPage::Themes => {
                line(&mut out, &mut y, &mut row, MenuAction::BackToMenu);
                y += s(SECTION_GAP) / 2.0;
                for (i, t) in self.themes.iter().enumerate() {
                    let (r, c) = (i / COLS, i % COLS);
                    let rect = Rect::new(s(PAD) + s(SWATCH_W) * c as f32, y + s(SWATCH_H) * r as f32, s(SWATCH_W), s(SWATCH_H));
                    out.push(MenuItemBox { rect, row: row + r, col: c, action: MenuAction::SetTheme(t.id.clone()) });
                }
                let theme_rows = self.themes.len().div_ceil(COLS);
                y += s(SWATCH_H) * theme_rows as f32 + s(SECTION_GAP);
                row += theme_rows;
                for (_, action) in theme_actions() {
                    line(&mut out, &mut y, &mut row, action);
                }
            }
        }
        (out, titles, y + s(PAD))
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

    /// O mouse leva o destaque (o mesmo do teclado) para o item sob ele. `true` se mudou.
    pub fn hover(&mut self, p: Point, scale: f32) -> bool {
        let Some(b) = self.layout(scale).into_iter().find(|b| b.rect.contains(p)) else { return false };
        let changed = (b.row, b.col) != self.focus;
        self.focus = (b.row, b.col);
        changed
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

        for (text, y) in titles {
            list.push(DrawOp::Text { rect: Rect::new(s(PAD), y, s(W - 2.0 * PAD), s(TITLE_H)), text: text.into(), style: title, color: tokens.text_tertiary, align: Align::Leading });
        }
        let segments: &[usize] = if self.page == MenuPage::Main { &[0, 1] } else { &[] };
        for &row in segments {
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
                    } else if focused {
                        // Destaque do mouse/teclado: meio caminho até o selecionado.
                        list.push(DrawOp::RoundRect { rect: inner, radius: inner.h / 2.0, color: tokens.raised_strong.with_alpha(tokens.raised_strong.a * 0.5) });
                    }
                    let label = match &b.action {
                        MenuAction::SetHeight(BarHeight::Completa) => "Completa",
                        MenuAction::SetHeight(BarHeight::Compacta) => "Compacta",
                        MenuAction::SetFormat(BarFormat::Faixa) => "Faixa",
                        MenuAction::SetFormat(BarFormat::Ilhas) => "Ilhas",
                        _ => "Pílulas",
                    };
                    list.push(DrawOp::Text { rect: b.rect, text: label.into(), style: body, color: tokens.text, align: Align::Center });
                }
                MenuAction::SetTheme(id) => {
                    let t = self.themes.iter().find(|t| &t.id == id).expect("tema da grade");
                    let c = Point { x: b.rect.center_x(), y: b.rect.y + s(18.0) };
                    if focused {
                        list.push(DrawOp::RoundRect { rect: b.rect.inset(s(2.0), s(1.0)), radius: s(10.0), color: tokens.raised });
                    }
                    if self.is_selected(&b.action) {
                        list.push(DrawOp::Circle { center: c, radius: s(16.0), color: tokens.accent });
                    }
                    list.push(DrawOp::Circle { center: c, radius: s(14.0), color: t.border.unwrap_or(tokens.hairline) });
                    list.push(DrawOp::Circle { center: c, radius: s(13.0), color: t.background.with_alpha(1.0) });
                    list.push(DrawOp::Circle { center: c, radius: s(4.0), color: t.text });
                    list.push(DrawOp::Text { rect: Rect::new(b.rect.x, b.rect.y + s(36.0), b.rect.w, s(16.0)), text: t.name.clone(), style: small, color: if focused { tokens.text } else { tokens.text_secondary }, align: Align::Center });
                }
                MenuAction::OpenThemes | MenuAction::BackToMenu => {
                    if focused {
                        list.push(DrawOp::RoundRect { rect: b.rect, radius: s(10.0), color: tokens.raised });
                    }
                    if b.action == MenuAction::BackToMenu {
                        list.push(DrawOp::Text { rect: b.rect.inset(s(12.0), 0.0), text: "‹ Tema".into(), style: body, color: tokens.text_secondary, align: Align::Leading });
                        continue;
                    }
                    list.push(DrawOp::Text { rect: b.rect.inset(s(12.0), 0.0), text: "Tema".into(), style: body, color: tokens.text, align: Align::Leading });
                    // À direita: a bolinha do tema atual e o nome dele.
                    let current = self.themes.iter().find(|t| t.id == self.theme_id);
                    let name = format!("{} ›", current.map_or(self.theme_id.as_str(), |t| t.name.as_str()));
                    let value = Rect::new(b.rect.x, b.rect.y, b.rect.w - s(12.0), b.rect.h);
                    list.push(DrawOp::Text { rect: value, text: name, style: small, color: tokens.text_secondary, align: Align::Trailing });
                    if let Some(t) = current {
                        let c = Point { x: b.rect.right() - s(12.0) - s(78.0), y: b.rect.y + b.rect.h / 2.0 };
                        list.push(DrawOp::Circle { center: c, radius: s(6.5), color: t.border.unwrap_or(tokens.hairline) });
                        list.push(DrawOp::Circle { center: c, radius: s(5.5), color: t.background.with_alpha(1.0) });
                    }
                }
                MenuAction::ToggleAcrylic => {
                    if focused {
                        list.push(DrawOp::RoundRect { rect: b.rect, radius: s(10.0), color: tokens.raised });
                    }
                    list.push(DrawOp::Text { rect: b.rect.inset(s(12.0), 0.0), text: "Acrílico".into(), style: body, color: tokens.text, align: Align::Leading });
                    let (on, available) = self.acrylic;
                    if available {
                        switch(&mut list, b.rect, on, scale, tokens);
                    } else {
                        let value = Rect::new(b.rect.x, b.rect.y, b.rect.w - s(12.0), b.rect.h);
                        list.push(DrawOp::Text { rect: value, text: "desligado no Windows".into(), style: small, color: tokens.text_tertiary, align: Align::Trailing });
                    }
                }
                MenuAction::InstallUpdate => {
                    if focused {
                        list.push(DrawOp::RoundRect { rect: b.rect, radius: s(10.0), color: tokens.raised });
                    }
                    let label = self.update.clone().unwrap_or_default();
                    list.push(DrawOp::Text { rect: b.rect.inset(s(12.0), 0.0), text: label, style: body, color: tokens.accent, align: Align::Leading });
                }
                action => {
                    let label = action_label(action);
                    if focused {
                        list.push(DrawOp::RoundRect { rect: b.rect, radius: s(10.0), color: tokens.raised });
                    }
                    list.push(DrawOp::Text { rect: b.rect.inset(s(12.0), 0.0), text: label.into(), style: body, color: tokens.text, align: Align::Leading });
                    if *action == MenuAction::ToggleAutostart {
                        switch(&mut list, b.rect, self.autostart, scale, tokens);
                    }
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
    fn autostart_item_shows_a_switch_lit_only_when_on() {
        let tokens = zefiro_resources::tokens::derive_tokens(&zefiro_resources::theme::parse_theme("obsidian", "o", zefiro_resources::theme::BUILTIN[0].1).unwrap(), Rgba::rgb(0, 120, 215), None);
        let accent_pills = |m: &MenuModel| m.paint(1.0, &tokens).ops.iter().filter(|o| matches!(o, DrawOp::RoundRect { color, .. } if *color == tokens.accent)).count();
        let off = model();
        assert!(off.layout(1.0).iter().any(|b| b.action == MenuAction::ToggleAutostart), "o item existe");
        assert_eq!(accent_pills(&off), 0, "desligado: a chave fica apagada");
        assert_eq!(accent_pills(&model().with_autostart(true)), 1, "ligado: a chave acende");
    }

    #[test]
    fn acrylic_row_follows_the_theme_row_with_a_switch() {
        let tokens = zefiro_resources::tokens::derive_tokens(&zefiro_resources::theme::parse_theme("obsidian", "o", zefiro_resources::theme::BUILTIN[0].1).unwrap(), Rgba::rgb(0, 120, 215), None);
        let lit = |m: &MenuModel| m.paint(1.0, &tokens).ops.iter().filter(|o| matches!(o, DrawOp::RoundRect { color, .. } if *color == tokens.accent)).count();
        let mut m = model();
        let boxes = m.layout(1.0);
        let theme = boxes.iter().find(|b| b.action == MenuAction::OpenThemes).unwrap();
        let acrylic = boxes.iter().find(|b| b.action == MenuAction::ToggleAcrylic).expect("a linha existe");
        assert_eq!(acrylic.row, theme.row + 1, "logo abaixo de Tema");
        assert_eq!(lit(&m), 0, "desligado: a chave fica apagada");
        m.set_acrylic(true, true);
        assert_eq!(lit(&m), 1, "ligado: a chave acende");
        assert!(texts(&m.paint(1.0, &tokens)).contains(&"Acrílico".to_string()));
        m.set_acrylic(true, false);
        assert_eq!(lit(&m), 0, "sem a transparência do Windows, nada acende");
        assert!(texts(&m.paint(1.0, &tokens)).contains(&"desligado no Windows".to_string()));
    }

    #[test]
    fn hover_moves_the_highlight_and_reports_only_changes() {
        let mut m = model();
        let edit = m.layout(1.0).into_iter().find(|b| b.action == MenuAction::EditBar).unwrap();
        let p = Point { x: edit.rect.center_x(), y: edit.rect.y + 2.0 };
        assert!(m.hover(p, 1.0), "entrou em Editar barra");
        assert_eq!(m.focus(), (edit.row, edit.col));
        assert!(!m.hover(p, 1.0), "mesmo item: nada a redesenhar");
        assert!(!m.hover(Point { x: -5.0, y: -5.0 }, 1.0), "fora dos itens o destaque fica onde estava");
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
        m.key(NavKey::Down); // a linha "Tema", coluna volta a 0
        assert_eq!(m.key(NavKey::Enter), Some(MenuAction::OpenThemes));
        m.key(NavKey::Down); // Acrílico
        assert_eq!(m.key(NavKey::Enter), Some(MenuAction::ToggleAcrylic));
        m.key(NavKey::Down); // primeira ação
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
    fn format_row_offers_strip_pills_and_islands() {
        let mut m = MenuModel::new(BarHeight::Completa, BarFormat::Ilhas, "t2", swatches(8));
        let row: Vec<_> = m.layout(1.0).into_iter().filter(|b| matches!(b.action, MenuAction::SetFormat(_))).collect();
        assert_eq!(row.iter().map(|b| b.action.clone()).collect::<Vec<_>>(), [BarFormat::Faixa, BarFormat::Pilulas, BarFormat::Ilhas].map(MenuAction::SetFormat));
        assert!(row.windows(2).all(|w| w[0].rect.right() <= w[1].rect.x + 0.01 && w[0].rect.w == w[1].rect.w), "três colunas iguais");
        let texts = texts(&m.paint(1.0, &tokens()));
        assert!(["Faixa", "Pílulas", "Ilhas"].iter().all(|t| texts.contains(&t.to_string())), "{texts:?}");
        m.key(NavKey::Down);
        m.key(NavKey::Right);
        m.key(NavKey::Right);
        assert_eq!(m.key(NavKey::Enter), Some(MenuAction::SetFormat(BarFormat::Ilhas)));
    }

    #[test]
    fn main_menu_has_one_theme_row_and_no_grid() {
        let m = MenuModel::new(BarHeight::Completa, BarFormat::Faixa, "t2", swatches(9));
        let boxes = m.layout(1.0);
        assert!(boxes.iter().all(|b| !matches!(b.action, MenuAction::SetTheme(_) | MenuAction::ReloadTheme | MenuAction::OpenThemesFolder)));
        assert_eq!(boxes.iter().filter(|b| b.action == MenuAction::OpenThemes).count(), 1);
        assert_eq!(m.size(1.0), model().size(1.0), "o menu não cresce com o número de temas");
        let texts = texts(&m.paint(1.0, &tokens()));
        assert!(texts.contains(&"Tema".to_string()) && texts.contains(&"T2 ›".to_string()), "{texts:?}");
    }

    #[test]
    fn themes_page_opens_on_the_current_theme_and_goes_back() {
        let mut m = model();
        m.open_themes();
        assert_eq!(m.page(), MenuPage::Themes);
        assert_eq!(m.key(NavKey::Enter), Some(MenuAction::SetTheme("t2".into())), "começa no tema atual");
        m.key(NavKey::Down); // segunda linha de temas, mesma coluna
        assert_eq!(m.key(NavKey::Enter), Some(MenuAction::SetTheme("t6".into())));
        m.key(NavKey::Down); // Recarregar tema
        assert_eq!(m.key(NavKey::Enter), Some(MenuAction::ReloadTheme));
        m.key(NavKey::Down);
        assert_eq!(m.key(NavKey::Enter), Some(MenuAction::OpenThemesFolder));
        for _ in 0..10 {
            m.key(NavKey::Up);
        }
        assert_eq!(m.key(NavKey::Enter), Some(MenuAction::BackToMenu));
        m.back();
        assert_eq!(m.page(), MenuPage::Main);
        assert_eq!(m.key(NavKey::Enter), Some(MenuAction::OpenThemes), "volta com o foco na linha Tema");
        let themes = { let mut t = model(); t.open_themes(); t };
        assert!(texts(&themes.paint(1.0, &tokens())).contains(&"‹ Tema".to_string()));
    }

    #[test]
    fn partial_last_theme_row_clamps_column() {
        let mut m = MenuModel::new(BarHeight::Completa, BarFormat::Faixa, "t3", swatches(9));
        m.open_themes(); // foco em t3, coluna 3
        m.key(NavKey::Down);
        m.key(NavKey::Down); // terceira linha de temas só tem t8
        assert_eq!(m.key(NavKey::Enter), Some(MenuAction::SetTheme("t8".into())));
    }

    #[test]
    fn choices_update_in_place_and_keep_the_page() {
        let mut m = model();
        m.open_themes();
        m.set_choices(BarHeight::Compacta, BarFormat::Pilulas, "t5");
        assert_eq!(m.page(), MenuPage::Themes, "trocar o tema não volta ao menu");
        assert!(m.layout(1.0).iter().any(|b| b.action == MenuAction::SetTheme("t5".into())));
        m.back();
        assert!(texts(&m.paint(1.0, &tokens())).contains(&"T5 ›".to_string()));
    }

    #[test]
    fn debug_items_sit_before_exit() {
        let m = model();
        let labels: Vec<_> = m.layout(1.0).into_iter().filter_map(|b| match b.action {
            MenuAction::ToggleHud => Some("hud"),
            MenuAction::ExportDiagnostics => Some("diag"),
            MenuAction::Exit => Some("sair"),
            _ => None,
        }).collect();
        assert_eq!(labels, ["hud", "diag", "sair"]);
        let texts: Vec<String> = m.paint(1.0, &zefiro_resources::tokens::derive_tokens(&zefiro_resources::theme::parse_theme("obsidian", "o", zefiro_resources::theme::BUILTIN[0].1).unwrap(), Rgba::rgb(0, 120, 215), None))
            .ops.into_iter().filter_map(|o| if let DrawOp::Text { text, .. } = o { Some(text) } else { None }).collect();
        assert!(texts.contains(&"HUD de debug".to_string()) && texts.contains(&"Exportar diagnóstico".to_string()), "{texts:?}");
    }

    #[test]
    fn update_row_appears_above_the_actions_only_when_there_is_one() {
        let mut m = model();
        assert!(m.layout(1.0).iter().all(|b| b.action != MenuAction::InstallUpdate));
        let short = m.size(1.0).h;
        m.set_update(Some("Atualizar para a 1.1.0".into()));
        let actions: Vec<_> = m.layout(1.0).into_iter().filter(|b| matches!(b.action, MenuAction::InstallUpdate | MenuAction::EditBar)).map(|b| b.action).collect();
        assert_eq!(actions, [MenuAction::InstallUpdate, MenuAction::EditBar], "logo antes de Editar barra");
        assert_eq!(m.size(1.0).h - short, 32.0);
        let t = tokens();
        let row = m.paint(1.0, &t).ops.into_iter().find_map(|o| match o { DrawOp::Text { text, color, .. } if text == "Atualizar para a 1.1.0" => Some(color), _ => None });
        assert_eq!(row, Some(t.accent), "na cor de destaque");
        m.set_update(None);
        assert_eq!(m.size(1.0).h, short);
    }

    #[test]
    fn hit_test_matches_layout() {
        let mut m = model();
        for _ in 0..2 {
            for b in m.layout(1.0) {
                let center = Point { x: b.rect.x + b.rect.w / 2.0, y: b.rect.y + b.rect.h / 2.0 };
                assert_eq!(m.hit(center, 1.0), Some(b.action.clone()), "{b:?}");
            }
            assert_eq!(m.hit(Point { x: 1.0, y: 1.0 }, 1.0), None);
            m.open_themes();
        }
    }

    #[test]
    fn themes_page_grows_with_themes_and_scale() {
        let page = |n| { let mut m = MenuModel::new(BarHeight::Completa, BarFormat::Faixa, "t0", swatches(n)); m.open_themes(); m.size(1.0) };
        assert_eq!(page(8).w, 300.0);
        assert_eq!(page(9).h - page(8).h, 56.0);
        assert_eq!(MenuModel::new(BarHeight::Completa, BarFormat::Faixa, "t0", swatches(8)).size(1.5).w, 450.0);
    }

    fn tokens() -> zefiro_resources::tokens::Tokens {
        zefiro_resources::tokens::derive_tokens(&zefiro_resources::theme::parse_theme("obsidian", "o", zefiro_resources::theme::BUILTIN[0].1).unwrap(), Rgba::rgb(0, 120, 215), None)
    }

    fn texts(l: &DisplayList) -> Vec<String> {
        l.ops.iter().filter_map(|o| if let DrawOp::Text { text, .. } = o { Some(text.clone()) } else { None }).collect()
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
