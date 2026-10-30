//! Painel "Comandos": dropdown da ferramenta, busca e a lista de comandos que se copia (ADR-0029).
//! Estado, teclado e desenho; quem lê a biblioteca de comandos e copia é o `app`.

use crate::vscode::edit_query;
use zefiro_platform::geometry::{Rect, Size};
use zefiro_platform::text::{Align, Family, FontWeight, TextMeasurer, TextStyle};
use zefiro_resources::tokens::Tokens;
use zefiro_ui::paint::{DisplayList, DrawOp};

const WIDTH: f32 = 400.0;
const ROW_H: f32 = 44.0;
/// Linhas de comando à vista.
pub const VISIBLE: usize = 10;
/// Itens do dropdown à vista (a janela acompanha o destaque).
const DROPDOWN_VISIBLE: usize = 8;
const ITEM_H: f32 = 28.0;
const HEADER_Y: f32 = 10.0;
const HEADER_H: f32 = 32.0;
const SEARCH_Y: f32 = 48.0;
const SEARCH_H: f32 = 30.0;
const HEADING_Y: f32 = 86.0;
const COPY_W: f32 = 44.0;
const FOOTER_H: f32 = 20.0;
const ALL_ICON: &str = "\u{E71D}";

/// Primeira linha máxima da rolagem para `count` comandos.
pub fn max_scroll(count: usize) -> usize {
    count.saturating_sub(VISIBLE)
}

/// Uma ferramenta do dropdown (sem "Todas", que é sempre o índice 0).
#[derive(Debug, Clone, PartialEq)]
pub struct ToolEntry { pub id: String, pub name: String, pub icon: char }

/// Uma linha de resultado: a ferramenta (selo em "Todas"), o comando e a explicação.
#[derive(Debug, Clone, PartialEq)]
pub struct CommandRow { pub tool: String, pub cmd: String, pub what: String }

/// O que o `app` faz depois de uma tecla ou clique.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Action {
    None,
    Redraw,
    /// Copiar o comando desta linha (índice na lista de resultados).
    Copy(usize),
    /// Gravar `[commands] last` (`None` = Todas).
    SaveTool(Option<String>),
    Close,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct PanelState {
    /// Id da ferramenta; `None` = Todas.
    pub tool: Option<String>,
    pub query: String,
    pub selected: usize,
    pub scroll: usize,
    /// A linha escolhida só aparece depois de buscar ou usar as setas.
    pub navigated: bool,
    /// Dropdown aberto: o item em destaque (0 = Todas).
    pub dropdown: Option<usize>,
}

impl PanelState {
    /// Estado ao abrir o painel; uma ferramenta que não existe mais volta para Todas.
    pub fn open(last: &str, tools: &[ToolEntry]) -> PanelState {
        PanelState { tool: tools.iter().find(|t| t.id == last).map(|t| t.id.clone()), ..PanelState::default() }
    }

    /// A linha escolhida aparece destacada?
    pub fn lit(&self) -> bool {
        self.navigated || !self.query.is_empty()
    }

    /// Posição da ferramenta atual no dropdown (0 = Todas).
    pub fn current_index(&self, tools: &[ToolEntry]) -> usize {
        self.tool.as_ref().and_then(|id| tools.iter().position(|t| &t.id == id)).map_or(0, |i| i + 1)
    }

    fn reset_list(&mut self) {
        self.selected = 0;
        self.scroll = 0;
        self.navigated = false;
    }

    /// Tecla digitada (`WM_CHAR`) na busca; a lista volta ao primeiro resultado. `true` se mudou.
    pub fn on_char(&mut self, code: u32, paste: Option<&str>) -> bool {
        let changed = edit_query(&mut self.query, code, paste);
        if changed {
            self.reset_list();
        }
        changed
    }

    /// Tecla com o painel aberto. `None` = não tratou. `rows` é a quantidade de resultados.
    pub fn on_key(&mut self, vk: u32, rows: usize, tools: &[ToolEntry]) -> Option<Action> {
        match vk {
            0x1B => Some(if self.dropdown.take().is_some() {
                Action::Redraw
            } else if !self.query.is_empty() {
                self.query.clear();
                self.reset_list();
                Action::Redraw
            } else {
                Action::Close
            }),
            0x26 | 0x28 => {
                let down = vk == 0x28;
                if let Some(i) = self.dropdown {
                    self.dropdown = Some(if down { (i + 1).min(tools.len()) } else { i.saturating_sub(1) });
                } else {
                    if self.lit() {
                        self.selected = if down { (self.selected + 1).min(rows.saturating_sub(1)) } else { self.selected.saturating_sub(1) };
                    }
                    self.navigated = true;
                    // A linha escolhida fica sempre à vista.
                    if self.selected < self.scroll {
                        self.scroll = self.selected;
                    } else if self.selected >= self.scroll + VISIBLE {
                        self.scroll = self.selected + 1 - VISIBLE;
                    }
                }
                Some(Action::Redraw)
            }
            0x0D => Some(match self.dropdown {
                Some(i) => self.pick(i, tools),
                None if rows > 0 => Action::Copy(self.selected.min(rows - 1)),
                None => Action::None,
            }),
            0x09 => Some(Action::None),
            _ => None,
        }
    }

    /// Roda sobre a lista de `count` resultados (o dropdown aberto não rola).
    pub fn wheel(&mut self, delta: i32, count: usize) {
        if self.dropdown.is_none() {
            self.scroll = if delta > 0 { self.scroll.saturating_sub(1) } else { (self.scroll + 1).min(max_scroll(count)) };
        }
    }

    /// Clique no cabeçalho: abre ou fecha o dropdown, já na ferramenta atual.
    pub fn click_header(&mut self, tools: &[ToolEntry]) {
        self.dropdown = if self.dropdown.is_some() { None } else { Some(self.current_index(tools)) };
    }

    /// Escolheu o item `index` do dropdown (0 = Todas).
    pub fn click_dropdown_item(&mut self, index: usize, tools: &[ToolEntry]) -> Action {
        self.pick(index, tools)
    }

    /// Clique fora do dropdown aberto fecha só ele. `true` se havia dropdown aberto.
    pub fn click_outside_dropdown(&mut self) -> bool {
        self.dropdown.take().is_some()
    }

    fn pick(&mut self, index: usize, tools: &[ToolEntry]) -> Action {
        self.dropdown = None;
        self.tool = index.checked_sub(1).and_then(|i| tools.get(i)).map(|t| t.id.clone());
        self.query.clear();
        self.reset_list();
        Action::SaveTool(self.tool.clone())
    }
}

pub struct PanelView<'a> {
    pub tools: &'a [ToolEntry],
    pub state: &'a PanelState,
    pub rows: &'a [CommandRow],
    /// "Mais usados" (ou "Comandos") quando a busca está vazia.
    pub heading: Option<&'a str>,
    pub measure: &'a dyn TextMeasurer,
}

/// Desenho e áreas de clique.
pub struct PanelLayout {
    pub list: DisplayList,
    /// Área de cada linha visível e o índice dela na lista de resultados.
    pub rows: Vec<(Rect, usize)>,
    pub header: Rect,
    /// Itens do dropdown aberto (retângulo e índice, 0 = Todas); vazio com ele fechado.
    pub dropdown_items: Vec<(Rect, usize)>,
    pub search: Rect,
}

pub fn commands_panel(view: &PanelView, scale: f32, tokens: &Tokens) -> PanelLayout {
    let px = |v: f32| (v * scale).round();
    let text = |size: f32, weight: FontWeight| TextStyle { size: size * scale, weight, family: Family::Text };
    let icons = |size: f32| TextStyle { size: size * scale, weight: FontWeight::Regular, family: Family::Icons };
    let state = view.state;
    let w = px(WIDTH);
    let tool = state.tool.as_deref().and_then(|id| view.tools.iter().find(|t| t.id == id));

    let scroll = state.scroll.min(max_scroll(view.rows.len()));
    let shown = view.rows.len().saturating_sub(scroll).min(VISIBLE);
    let rows_y = if view.heading.is_some() { 108.0 } else { 88.0 };
    let body = if shown == 0 { 56.0 } else { shown as f32 * ROW_H };
    let dropdown_count = (view.tools.len() + 1).min(DROPDOWN_VISIBLE);
    // O painel sempre comporta o dropdown aberto: a altura não muda ao abrir.
    let dropdown_bottom = HEADER_Y + HEADER_H + 4.0 + dropdown_count as f32 * ITEM_H + 8.0;
    let footer_y = (rows_y + body).max(dropdown_bottom) + 6.0;
    let size = Size { w, h: px(footer_y + FOOTER_H + 8.0) };
    let mut list = DisplayList::new(size);

    // Cabeçalho: ícone, nome da ferramenta e a seta do dropdown.
    let header = Rect::new(px(10.0), px(HEADER_Y), w - px(20.0), px(HEADER_H));
    list.push(DrawOp::RoundRect { rect: header, radius: px(10.0), color: tokens.raised });
    let glyph = tool.map_or_else(|| ALL_ICON.to_string(), |t| t.icon.to_string());
    list.push(DrawOp::Text { rect: Rect::new(header.x + px(10.0), header.y, px(20.0), header.h), text: glyph, style: icons(14.0), color: tokens.text_secondary, align: Align::Center });
    list.push(DrawOp::Text { rect: Rect::new(header.x + px(38.0), header.y, header.w - px(76.0), header.h), text: tool.map_or("Todas", |t| t.name.as_str()).into(), style: text(13.0, FontWeight::SemiBold), color: tokens.text, align: Align::Leading });
    let chevron = if state.dropdown.is_some() { "\u{E70E}" } else { "\u{E70D}" };
    list.push(DrawOp::Text { rect: Rect::new(header.right() - px(30.0), header.y, px(20.0), header.h), text: chevron.into(), style: icons(11.0), color: tokens.text_tertiary, align: Align::Center });

    let search = Rect::new(px(10.0), px(SEARCH_Y), w - px(20.0), px(SEARCH_H));
    let mut hits = Vec::new();
    let mut dropdown_items = Vec::new();

    // O dropdown cobre a busca e os resultados; os tokens são translúcidos, então o que ficaria
    // por baixo simplesmente não é desenhado.
    if state.dropdown.is_none() {
        list.push(DrawOp::RoundRect { rect: search, radius: px(10.0), color: tokens.raised });
        list.push(DrawOp::Text { rect: Rect::new(search.x + px(10.0), search.y, px(16.0), search.h), text: "\u{E721}".into(), style: icons(12.0), color: tokens.text_tertiary, align: Align::Center });
        let field = Rect::new(search.x + px(32.0), search.y, search.w - px(42.0), search.h);
        let style = text(12.5, FontWeight::Regular);
        if state.query.is_empty() {
            let hint = format!("buscar em {}…", tool.map_or("todas", |t| t.name.as_str()));
            list.push(DrawOp::Text { rect: field, text: hint, style, color: tokens.text_tertiary, align: Align::Leading });
        } else {
            list.push(DrawOp::Text { rect: field, text: state.query.clone(), style, color: tokens.text, align: Align::Leading });
        }
        let caret_x = field.x + if state.query.is_empty() { 0.0 } else { view.measure.measure(&state.query, &style).w.min(field.w) };
        list.push(DrawOp::RoundRect { rect: Rect::new(caret_x, search.y + px(8.0), px(1.5), search.h - px(16.0)), radius: px(0.75), color: tokens.accent });

        if let Some(heading) = view.heading {
            list.push(DrawOp::Text { rect: Rect::new(px(20.0), px(HEADING_Y), w - px(40.0), px(18.0)), text: heading.into(), style: text(11.0, FontWeight::SemiBold), color: tokens.text_tertiary, align: Align::Leading });
        }

        if shown == 0 {
            let note = tool.map_or_else(|| "Nada encontrado".to_string(), |t| format!("Nada encontrado em {}", t.name));
            list.push(DrawOp::Text { rect: Rect::new(px(20.0), px(rows_y + 6.0), w - px(40.0), px(20.0)), text: note, style: text(12.5, FontWeight::Regular), color: tokens.text, align: Align::Leading });
            if tool.is_some() {
                list.push(DrawOp::Text { rect: Rect::new(px(20.0), px(rows_y + 28.0), w - px(40.0), px(18.0)), text: "Tente em Todas".into(), style: text(11.0, FontWeight::Regular), color: tokens.text_tertiary, align: Align::Leading });
            }
        }
        for (n, (index, item)) in view.rows.iter().enumerate().skip(scroll).take(shown).enumerate() {
            let r = Rect::new(px(10.0), px(rows_y + n as f32 * ROW_H), w - px(20.0), px(ROW_H));
            if state.lit() && state.selected == index {
                list.push(DrawOp::RoundRect { rect: r, radius: px(10.0), color: tokens.raised });
            }
            let copy_w = px(COPY_W);
            list.push(DrawOp::Text { rect: Rect::new(r.right() - px(10.0) - copy_w, r.y + px(4.0), copy_w, px(20.0)), text: "copiar".into(), style: text(11.0, FontWeight::Regular), color: tokens.text_tertiary, align: Align::Trailing });
            let mono = TextStyle { size: 12.5 * scale, weight: FontWeight::Regular, family: Family::Mono };
            list.push(DrawOp::Text { rect: Rect::new(r.x + px(10.0), r.y + px(4.0), r.w - px(20.0) - copy_w - px(8.0), px(20.0)), text: item.cmd.clone(), style: mono, color: tokens.text, align: Align::Leading });
            let mut what_w = r.w - px(20.0);
            if tool.is_none() {
                let badge_style = text(10.0, FontWeight::SemiBold);
                let bw = view.measure.measure(&item.tool, &badge_style).w + px(14.0);
                let badge = Rect::new(r.right() - px(10.0) - bw, r.y + px(24.0), bw, px(16.0));
                list.push(DrawOp::RoundRect { rect: badge, radius: badge.h / 2.0, color: tokens.raised_strong });
                list.push(DrawOp::Text { rect: badge, text: item.tool.clone(), style: badge_style, color: tokens.text_secondary, align: Align::Center });
                what_w -= bw + px(8.0);
            }
            list.push(DrawOp::Text { rect: Rect::new(r.x + px(10.0), r.y + px(23.0), what_w, px(16.0)), text: item.what.clone(), style: text(11.0, FontWeight::Regular), color: tokens.text_tertiary, align: Align::Leading });
            hits.push((r, index));
        }
    }

    list.push(DrawOp::Text { rect: Rect::new(px(20.0), px(footer_y), w - px(40.0), px(FOOTER_H)), text: "↵ copiar · ↑↓ navegar · Esc fechar".into(), style: text(11.0, FontWeight::Regular), color: tokens.text_tertiary, align: Align::Leading });

    if let Some(highlight) = state.dropdown {
        let overlay = Rect::new(px(10.0), header.bottom() + px(4.0), w - px(20.0), px(dropdown_count as f32 * ITEM_H + 8.0));
        list.push(DrawOp::RoundRect { rect: overlay, radius: px(10.0), color: tokens.raised });
        list.push(DrawOp::RoundRectStroke { rect: overlay, radius: px(10.0), color: tokens.hairline, width: 1.0 });
        let current = state.current_index(view.tools);
        let first = if highlight < dropdown_count { 0 } else { highlight + 1 - dropdown_count };
        for (slot, index) in (first..first + dropdown_count).enumerate() {
            let r = Rect::new(overlay.x + px(4.0), overlay.y + px(4.0) + slot as f32 * px(ITEM_H), overlay.w - px(8.0), px(ITEM_H));
            if index == highlight {
                list.push(DrawOp::RoundRect { rect: r, radius: px(8.0), color: tokens.raised_strong });
            }
            let (glyph, name) = match index.checked_sub(1).and_then(|i| view.tools.get(i)) {
                Some(t) => (t.icon.to_string(), t.name.as_str()),
                None => (ALL_ICON.to_string(), "Todas"),
            };
            list.push(DrawOp::Text { rect: Rect::new(r.x + px(8.0), r.y, px(20.0), r.h), text: glyph, style: icons(13.0), color: tokens.text_secondary, align: Align::Center });
            list.push(DrawOp::Text { rect: Rect::new(r.x + px(36.0), r.y, r.w - px(72.0), r.h), text: name.into(), style: text(12.5, FontWeight::Regular), color: tokens.text, align: Align::Leading });
            if index == current {
                list.push(DrawOp::Text { rect: Rect::new(r.right() - px(28.0), r.y, px(20.0), r.h), text: "\u{E73E}".into(), style: icons(12.0), color: tokens.accent, align: Align::Center });
            }
            dropdown_items.push((r, index));
        }
    }

    PanelLayout { list, rows: hits, header, dropdown_items, search }
}

#[cfg(test)]
mod tests {
    use super::*;
    use zefiro_platform::color::Rgba;
    use zefiro_platform::geometry::Point;
    use zefiro_platform::text::FixedWidthMeasurer;
    use zefiro_resources::theme::{BUILTIN, parse_theme};
    use zefiro_resources::tokens::derive_tokens;

    fn tokens() -> Tokens {
        derive_tokens(&parse_theme("obsidian", "o", BUILTIN[0].1).unwrap(), Rgba::rgb(0, 120, 215), None)
    }

    fn tools() -> Vec<ToolEntry> {
        ["docker", "linux", "git"].iter().map(|id| ToolEntry { id: (*id).into(), name: id[..1].to_uppercase() + &id[1..], icon: '\u{E8A5}' }).collect()
    }

    fn row(i: usize) -> CommandRow {
        CommandRow { tool: "Docker".into(), cmd: format!("docker comando-{i}"), what: format!("Explicação {i}.") }
    }

    fn rows(n: usize) -> Vec<CommandRow> {
        (0..n).map(row).collect()
    }

    fn layout(state: &PanelState, rows: &[CommandRow], heading: Option<&str>) -> PanelLayout {
        let tools = tools();
        commands_panel(&PanelView { tools: &tools, state, rows, heading, measure: &FixedWidthMeasurer }, 1.0, &tokens())
    }

    fn texts(l: &DisplayList) -> Vec<String> {
        l.ops.iter().filter_map(|o| if let DrawOp::Text { text, .. } = o { Some(text.clone()) } else { None }).collect()
    }

    fn docker() -> PanelState {
        PanelState::open("docker", &tools())
    }

    // ---- estado ----

    #[test]
    fn open_with_unknown_tool_falls_back_to_all() {
        assert_eq!(PanelState::open("sumiu", &tools()).tool, None);
        assert_eq!(PanelState::open("", &tools()).tool, None);
    }

    #[test]
    fn open_with_known_tool() {
        assert_eq!(docker().tool.as_deref(), Some("docker"));
    }

    #[test]
    fn typing_resets_selection_and_scroll() {
        let mut s = docker();
        (s.selected, s.scroll, s.navigated) = (5, 3, true);
        assert!(s.on_char('d' as u32, None));
        assert_eq!((s.query.as_str(), s.selected, s.scroll, s.navigated), ("d", 0, 0, false));
        assert!(!s.on_char(13, None), "Enter não é texto");
    }

    #[test]
    fn enter_copies_the_selected_row() {
        let mut s = docker();
        s.selected = 2;
        assert_eq!(s.on_key(0x0D, 5, &tools()), Some(Action::Copy(2)));
        assert_eq!(s.on_key(0x0D, 0, &tools()), Some(Action::None), "sem linhas, Enter não faz nada");
    }

    #[test]
    fn esc_closes_dropdown_then_clears_query_then_closes_panel() {
        let mut s = docker();
        s.query = "x".into();
        s.dropdown = Some(0);
        assert_eq!(s.on_key(0x1B, 3, &tools()), Some(Action::Redraw));
        assert_eq!((s.dropdown, s.query.as_str()), (None, "x"));
        assert_eq!(s.on_key(0x1B, 3, &tools()), Some(Action::Redraw));
        assert_eq!(s.query, "");
        assert_eq!(s.on_key(0x1B, 3, &tools()), Some(Action::Close));
    }

    #[test]
    fn arrows_move_the_dropdown_highlight_while_open() {
        let mut s = docker();
        s.selected = 1;
        s.dropdown = Some(0);
        s.on_key(0x28, 5, &tools());
        s.on_key(0x28, 5, &tools());
        assert_eq!(s.dropdown, Some(2));
        for _ in 0..10 {
            s.on_key(0x28, 5, &tools());
        }
        assert_eq!(s.dropdown, Some(3), "Todas + 3 ferramentas: o último índice é 3");
        s.on_key(0x26, 5, &tools());
        assert_eq!(s.dropdown, Some(2));
        assert_eq!(s.selected, 1, "a linha escolhida não mexe");
    }

    #[test]
    fn typing_still_edits_the_query_while_dropdown_is_open() {
        let mut s = docker();
        s.dropdown = Some(1);
        assert!(s.on_char('p' as u32, None));
        assert_eq!((s.query.as_str(), s.dropdown), ("p", Some(1)));
    }

    #[test]
    fn picking_a_tool_saves_it_and_resets() {
        let mut s = docker();
        (s.query, s.selected, s.scroll, s.navigated, s.dropdown) = ("x".into(), 4, 2, true, Some(3));
        assert_eq!(s.click_dropdown_item(3, &tools()), Action::SaveTool(Some("git".into())));
        assert_eq!((s.tool.as_deref(), s.query.as_str(), s.selected, s.scroll, s.navigated, s.dropdown), (Some("git"), "", 0, 0, false, None));
        s.dropdown = Some(0);
        assert_eq!(s.on_key(0x0D, 5, &tools()), Some(Action::SaveTool(None)), "Enter no índice 0 escolhe Todas");
        assert_eq!(s.tool, None);
    }

    #[test]
    fn header_click_toggles_the_dropdown_on_the_current_tool() {
        let mut s = docker();
        s.click_header(&tools());
        assert_eq!(s.dropdown, Some(1), "Docker é o primeiro, logo o índice 1");
        s.click_header(&tools());
        assert_eq!(s.dropdown, None);
    }

    #[test]
    fn click_outside_closes_only_the_dropdown() {
        let mut s = docker();
        assert!(!s.click_outside_dropdown());
        s.dropdown = Some(1);
        assert!(s.click_outside_dropdown());
        assert_eq!(s.dropdown, None);
    }

    #[test]
    fn selection_stays_visible_when_moving_past_visible_rows() {
        let mut s = docker();
        s.navigated = true;
        for _ in 0..12 {
            s.on_key(0x28, 25, &tools());
        }
        assert_eq!((s.selected, s.scroll), (12, 12 + 1 - VISIBLE));
        for _ in 0..12 {
            s.on_key(0x26, 25, &tools());
        }
        assert_eq!((s.selected, s.scroll), (0, 0));
        assert_eq!(max_scroll(25), 25 - VISIBLE);
    }

    #[test]
    fn first_arrow_only_lights_the_selection() {
        let mut s = docker();
        s.on_key(0x28, 5, &tools());
        assert_eq!((s.selected, s.navigated), (0, true));
        assert!(s.lit());
    }

    // ---- desenho ----

    #[test]
    fn header_shows_tool_name_and_search_hint() {
        let t = texts(&layout(&docker(), &rows(2), None).list);
        assert!(t.contains(&"Docker".to_string()) && t.contains(&"buscar em Docker…".to_string()), "{t:?}");
        let t = texts(&layout(&PanelState::open("", &tools()), &rows(2), None).list);
        assert!(t.contains(&"Todas".to_string()) && t.contains(&"buscar em todas…".to_string()), "{t:?}");
    }

    #[test]
    fn rows_show_cmd_what_and_copy_hint() {
        let t = texts(&layout(&docker(), &rows(2), None).list);
        assert!(t.contains(&"docker comando-0".to_string()) && t.contains(&"Explicação 1.".to_string()));
        assert_eq!(t.iter().filter(|x| *x == "copiar").count(), 2);
    }

    #[test]
    fn command_uses_the_mono_family() {
        let l = layout(&docker(), &rows(1), None).list;
        let mono = l.ops.iter().any(|o| matches!(o, DrawOp::Text { text, style, .. } if text == "docker comando-0" && style.family == Family::Mono));
        assert!(mono);
    }

    #[test]
    fn badge_only_in_all() {
        let in_all = texts(&layout(&PanelState::open("", &tools()), &rows(1), None).list);
        assert!(in_all.iter().filter(|x| *x == "Docker").count() == 1, "selo com a ferramenta da linha: {in_all:?}");
        let in_tool = texts(&layout(&PanelState::open("linux", &tools()), &rows(1), None).list);
        assert!(!in_tool.contains(&"Docker".to_string()), "com ferramenta escolhida não há selo: {in_tool:?}");
    }

    #[test]
    fn heading_shown_only_when_given() {
        assert!(texts(&layout(&docker(), &rows(1), Some("Mais usados")).list).contains(&"Mais usados".to_string()));
        assert!(!texts(&layout(&docker(), &rows(1), None).list).contains(&"Mais usados".to_string()));
    }

    #[test]
    fn footer_is_always_drawn() {
        for r in [rows(0), rows(3)] {
            assert!(texts(&layout(&docker(), &r, None).list).contains(&"↵ copiar · ↑↓ navegar · Esc fechar".to_string()));
        }
    }

    #[test]
    fn empty_state_has_tool_hint() {
        let t = texts(&layout(&docker(), &[], None).list);
        assert!(t.contains(&"Nada encontrado em Docker".to_string()) && t.contains(&"Tente em Todas".to_string()), "{t:?}");
        let t = texts(&layout(&PanelState::open("", &tools()), &[], None).list);
        assert!(t.contains(&"Nada encontrado".to_string()) && !t.iter().any(|x| x.starts_with("Tente")), "{t:?}");
    }

    #[test]
    fn dropdown_overlay_does_not_change_panel_height() {
        let closed = layout(&docker(), &rows(2), None);
        let mut s = docker();
        s.dropdown = Some(0);
        let open = layout(&s, &rows(2), None);
        assert_eq!(closed.list.size, open.list.size);
        let few_closed = layout(&docker(), &[], None);
        let few_open = layout(&PanelState { dropdown: Some(0), ..docker() }, &[], None);
        assert_eq!(few_closed.list.size, few_open.list.size);
        let last = few_open.dropdown_items.last().unwrap().0;
        assert!(last.bottom() <= few_open.list.size.h, "poucas linhas: o painel ainda comporta o dropdown");
    }

    #[test]
    fn dropdown_lists_all_then_tools_and_marks_the_current() {
        let mut s = docker();
        s.dropdown = Some(2);
        let l = layout(&s, &rows(2), None);
        let t = texts(&l.list);
        let pos = |name: &str| t.iter().rposition(|x| x == name).unwrap_or_else(|| panic!("falta {name}: {t:?}"));
        assert!(pos("Todas") < pos("Docker") && pos("Docker") < pos("Linux") && pos("Linux") < pos("Git"), "{t:?}");
        assert_eq!(l.dropdown_items.iter().map(|(_, i)| *i).collect::<Vec<_>>(), [0, 1, 2, 3]);
        assert_eq!(t.iter().filter(|x| *x == "\u{E73E}").count(), 1, "só a ferramenta atual leva o visto");
    }

    #[test]
    fn open_dropdown_hides_the_content_underneath() {
        let mut s = docker();
        s.dropdown = Some(0);
        let t = texts(&layout(&s, &rows(2), Some("Mais usados")).list);
        assert!(!t.contains(&"docker comando-0".to_string()) && !t.contains(&"Mais usados".to_string()), "{t:?}");
    }

    #[test]
    fn long_command_is_clipped_to_the_row() {
        let long = vec![CommandRow { tool: "Docker".into(), cmd: "docker run -d --name x ".repeat(10), what: "w".into() }];
        let l = layout(&docker(), &long, None);
        let (row_rect, _) = l.rows[0];
        let cmd_rect = l.list.ops.iter().find_map(|o| if let DrawOp::Text { rect, style, .. } = o { (style.family == Family::Mono).then_some(*rect) } else { None }).unwrap();
        assert!(cmd_rect.right() <= row_rect.right() - 10.0 - 40.0, "o comando não invade o 'copiar': {cmd_rect:?} / {row_rect:?}");
    }

    #[test]
    fn hits_match_the_visible_rows() {
        let mut s = docker();
        s.scroll = 5;
        let l = layout(&s, &rows(25), None);
        assert_eq!(l.rows.len(), VISIBLE);
        assert_eq!(l.rows[0].1, 5, "o índice é o da linha na lista, não na tela");
        assert!(l.rows.windows(2).all(|w| w[0].0.bottom() <= w[1].0.y + 0.01));
        let past = layout(&PanelState { scroll: 99, ..docker() }, &rows(25), None);
        assert_eq!(past.rows[0].1, 25 - VISIBLE, "a rolagem é limitada");
    }

    #[test]
    fn selected_row_is_highlighted_only_when_lit() {
        let mut s = docker();
        let count = |l: &PanelLayout| l.list.ops.iter().filter(|o| matches!(o, DrawOp::RoundRect { .. })).count();
        let plain = count(&layout(&s, &rows(2), None));
        s.navigated = true;
        assert_eq!(count(&layout(&s, &rows(2), None)), plain + 1);
    }

    #[test]
    fn areas_stay_inside_the_panel() {
        let l = layout(&docker(), &rows(3), None);
        assert!(l.rows.iter().all(|(r, _)| r.right() <= l.list.size.w && r.bottom() <= l.list.size.h));
        assert!(l.header.contains(Point { x: l.header.x + 1.0, y: l.header.y + 1.0 }));
        assert!(l.search.y >= l.header.bottom());
    }
}
