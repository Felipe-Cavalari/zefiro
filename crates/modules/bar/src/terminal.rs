//! Painel "Terminal": os perfis do Windows Terminal (ícone e nome) e, pelo lápis, a edição que
//! esconde e ordena os perfis arrastando, como o "Editar barra".

use zefiro_platform::color::Rgba;
use zefiro_platform::geometry::{Point, Rect, Size};
use zefiro_platform::image::Image;
use zefiro_resources::tokens::Tokens;
use zefiro_platform::text::{Align, Family, FontWeight, TextStyle};
use zefiro_ui::paint::{DisplayList, DrawOp};

/// Perfis visíveis (na ordem escolhida) e ocultos (sempre na ordem do Terminal), por índice.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Arrangement { pub visible: Vec<usize>, pub hidden: Vec<usize> }

/// Onde um perfil arrastado cai: uma posição entre os visíveis (contada sem ele) ou "Ocultos".
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Slot { Visible(usize), Hidden }

impl Arrangement {
    /// Olho: um visível vai para os ocultos; um oculto vai para o fim dos visíveis.
    pub fn toggle(&mut self, profile: usize) {
        if self.visible.contains(&profile) {
            self.move_to(profile, Slot::Hidden);
        } else {
            self.move_to(profile, Slot::Visible(usize::MAX));
        }
    }

    /// Soltou o arraste.
    pub fn move_to(&mut self, profile: usize, slot: Slot) {
        self.visible.retain(|&p| p != profile);
        self.hidden.retain(|&p| p != profile);
        match slot {
            Slot::Visible(index) => self.visible.insert(index.min(self.visible.len()), profile),
            Slot::Hidden => {
                let at = self.hidden.partition_point(|&p| p < profile);
                self.hidden.insert(at, profile);
            }
        }
    }

    /// Ctrl+↑/↓: sobe ou desce entre os visíveis; um oculto não se move.
    pub fn nudge(&mut self, profile: usize, up: bool) {
        let Some(i) = self.visible.iter().position(|&p| p == profile) else { return };
        let j = if up { i.checked_sub(1) } else { (i + 1 < self.visible.len()).then_some(i + 1) };
        if let Some(j) = j {
            self.visible.swap(i, j);
        }
    }

    /// Linhas na ordem da tela (visíveis e depois ocultos), para as setas.
    pub fn rows(&self) -> Vec<usize> {
        self.visible.iter().chain(&self.hidden).copied().collect()
    }
}

/// Uma linha do painel.
pub struct ProfileRow { pub name: String, pub icon: Option<Image>, pub default: bool }

/// Arraste em andamento: o perfil, o mouse (no painel) e onde ele cairia.
pub struct DragView { pub profile: usize, pub point: Point, pub slot: Slot }

pub struct PanelView<'a> {
    pub rows: &'a [ProfileRow],
    pub arrangement: &'a Arrangement,
    pub editing: bool,
    /// No lugar da lista ("Carregando…", "Windows Terminal não encontrado"…).
    pub message: Option<&'a str>,
    /// Perfil escolhido pelo teclado.
    pub selected: Option<usize>,
    /// Perfil sob o mouse.
    pub hover: Option<usize>,
    pub drag: Option<&'a DragView>,
    /// Primeira linha visível (só no modo normal).
    pub scroll: usize,
}

/// Desenho e áreas de clique. As zonas só existem na edição.
pub struct PanelLayout {
    pub list: DisplayList,
    /// Lápis no modo normal; "Concluir" na edição.
    pub pencil: Rect,
    pub rows: Vec<(Rect, usize)>,
    pub eyes: Vec<(Rect, usize)>,
    pub visible_zone: Rect,
    pub hidden_zone: Rect,
}

pub const VISIBLE: usize = 12;

/// Primeira linha máxima da rolagem para `count` perfis visíveis.
pub fn max_scroll(count: usize) -> usize {
    count.saturating_sub(VISIBLE)
}

const WIDTH: f32 = 300.0;
const ROW_H: f32 = 36.0;
/// Topo da primeira linha: título e subtítulo acima.
const ROWS_Y: f32 = 62.0;
const ICON: f32 = 18.0;
const GLYPH_TERMINAL: &str = "\u{E756}";
const GLYPH_PENCIL: &str = "\u{E70F}";
const GLYPH_EYE: &str = "\u{E7B3}";
const GLYPH_EYE_OFF: &str = "\u{ED1A}";
const GLYPH_GRIP: &str = "\u{E76F}";
/// Opacidade dos ocultos e da linha que está sendo arrastada.
const HIDDEN_ALPHA: f32 = 0.4;
const DRAGGED_ALPHA: f32 = 0.35;

pub fn terminal_panel(view: &PanelView, scale: f32, tokens: &Tokens) -> PanelLayout {
    let px = |v: f32| (v * scale).round();
    let text = |size: f32, weight: FontWeight| TextStyle { size: size * scale, weight, family: Family::Text };
    let icons = |size: f32| TextStyle { size: size * scale, weight: FontWeight::Regular, family: Family::Icons };
    let arr = view.arrangement;
    let w = px(WIDTH);
    let row_rect = |y: f32| Rect::new(px(10.0), y, w - px(20.0), px(ROW_H));
    let mut ops = Vec::new();
    let mut rows = Vec::new();
    let mut eyes = Vec::new();

    ops.push(DrawOp::Text { rect: Rect::new(px(20.0), px(14.0), w - px(130.0), px(22.0)), text: "Terminal".into(), style: text(13.0, FontWeight::SemiBold), color: tokens.text, align: Align::Leading });
    let subtitle = if view.editing { "Arraste para ordenar · o olho esconde" } else { "Perfis do Windows Terminal" };
    ops.push(DrawOp::Text { rect: Rect::new(px(20.0), px(34.0), w - px(40.0), px(16.0)), text: subtitle.into(), style: text(11.0, FontWeight::Regular), color: tokens.text_tertiary, align: Align::Leading });
    let pencil = if view.editing {
        let r = Rect::new(w - px(20.0 + 84.0), px(12.0), px(84.0), px(28.0));
        ops.push(DrawOp::RoundRect { rect: r, radius: r.h / 2.0, color: tokens.accent_soft });
        ops.push(DrawOp::Text { rect: r, text: "Concluir".into(), style: text(12.0, FontWeight::SemiBold), color: tokens.accent, align: Align::Center });
        r
    } else {
        let r = Rect::new(w - px(20.0 + 28.0), px(12.0), px(28.0), px(28.0));
        ops.push(DrawOp::RoundRect { rect: r, radius: r.h / 2.0, color: tokens.raised });
        ops.push(DrawOp::Text { rect: r, text: GLYPH_PENCIL.into(), style: icons(12.0), color: tokens.text_secondary, align: Align::Center });
        r
    };

    let note = view.message.or((!view.editing && arr.visible.is_empty()).then_some("Nenhum perfil visível. Toque no lápis para escolher."));
    if let Some(note) = note {
        ops.push(DrawOp::Text { rect: Rect::new(px(20.0), px(ROWS_Y + 6.0), w - px(40.0), px(20.0)), text: note.into(), style: text(12.5, FontWeight::Regular), color: tokens.text, align: Align::Leading });
        let size = Size { w, h: px(ROWS_Y + 40.0 + 10.0) };
        return PanelLayout { list: DisplayList { size, ops }, pencil, rows, eyes, visible_zone: Rect::default(), hidden_zone: Rect::default() };
    }

    let dragged = view.drag.map(|d| d.profile);
    // Uma linha: fundo (escolhida ou sob o mouse), alça e olho na edição, ícone, nome e o selo.
    let mut draw_row = |ops: &mut Vec<DrawOp>, r: Rect, profile: usize, hidden: bool| {
        let row = &view.rows[profile];
        let alpha = if dragged == Some(profile) { DRAGGED_ALPHA } else if hidden { HIDDEN_ALPHA } else { 1.0 };
        let fade = |c: Rgba| c.with_alpha(c.a * alpha);
        if dragged.is_none() && (view.selected == Some(profile) || view.hover == Some(profile)) {
            ops.push(DrawOp::RoundRect { rect: r, radius: px(10.0), color: tokens.raised });
        }
        let mut x = r.x + px(10.0);
        if view.editing {
            ops.push(DrawOp::Text { rect: Rect::new(r.x + px(4.0), r.y, px(16.0), r.h), text: GLYPH_GRIP.into(), style: icons(11.0), color: fade(tokens.text_tertiary), align: Align::Center });
            x = r.x + px(24.0);
            let eye = Rect::new(r.right() - px(36.0), r.y, px(36.0), r.h);
            ops.push(DrawOp::Text { rect: eye, text: if hidden { GLYPH_EYE_OFF } else { GLYPH_EYE }.into(), style: icons(13.0), color: fade(tokens.text_secondary), align: Align::Center });
            eyes.push((eye, profile));
        }
        let icon = Rect::new(x, r.y + (r.h - px(ICON)) / 2.0, px(ICON), px(ICON));
        match &row.icon {
            Some(image) => ops.push(DrawOp::Image { rect: icon, image: image.clone(), radius: px(4.0), opacity: alpha }),
            None => ops.push(DrawOp::Text { rect: icon, text: GLYPH_TERMINAL.into(), style: icons(13.0), color: fade(tokens.text_secondary), align: Align::Center }),
        }
        let name_x = icon.right() + px(10.0);
        let mut name_right = r.right() - px(if view.editing { 40.0 } else { 10.0 });
        if row.default {
            let b = Rect::new(name_right - px(52.0), r.y + (r.h - px(16.0)) / 2.0, px(52.0), px(16.0));
            ops.push(DrawOp::RoundRect { rect: b, radius: b.h / 2.0, color: fade(tokens.raised_strong) });
            ops.push(DrawOp::Text { rect: b, text: "padrão".into(), style: text(10.0, FontWeight::SemiBold), color: fade(tokens.text_secondary), align: Align::Center });
            name_right = b.x - px(8.0);
        }
        ops.push(DrawOp::Text { rect: Rect::new(name_x, r.y, (name_right - name_x).max(0.0), r.h), text: row.name.clone(), style: text(12.5, FontWeight::Regular), color: fade(tokens.text), align: Align::Leading });
        rows.push((r, profile));
    };

    let (size, visible_zone, hidden_zone) = if view.editing {
        let target = view.drag.map(|d| d.slot);
        let visible_zone = Rect::new(px(6.0), px(ROWS_Y), w - px(12.0), px(arr.visible.len().max(1) as f32 * ROW_H));
        let label_y = visible_zone.bottom() + px(8.0);
        let hidden_zone = Rect::new(px(6.0), label_y + px(20.0), w - px(12.0), px(arr.hidden.len().max(1) as f32 * ROW_H));
        for (zone, lit) in [(visible_zone, matches!(target, Some(Slot::Visible(_)))), (hidden_zone, target == Some(Slot::Hidden))] {
            if lit {
                ops.push(DrawOp::RoundRect { rect: zone, radius: px(12.0), color: tokens.accent_soft });
            }
        }
        for (i, &profile) in arr.visible.iter().enumerate() {
            draw_row(&mut ops, row_rect(visible_zone.y + px(i as f32 * ROW_H)), profile, false);
        }
        ops.push(DrawOp::Text { rect: Rect::new(px(20.0), label_y, w - px(40.0), px(16.0)), text: "Ocultos".into(), style: text(10.5, FontWeight::SemiBold), color: tokens.text_tertiary, align: Align::Leading });
        for (i, &profile) in arr.hidden.iter().enumerate() {
            draw_row(&mut ops, row_rect(hidden_zone.y + px(i as f32 * ROW_H)), profile, true);
        }
        (Size { w, h: hidden_zone.bottom() + px(10.0) }, visible_zone, hidden_zone)
    } else {
        let scroll = view.scroll.min(max_scroll(arr.visible.len()));
        let shown: Vec<usize> = arr.visible.iter().skip(scroll).take(VISIBLE).copied().collect();
        for (i, &profile) in shown.iter().enumerate() {
            draw_row(&mut ops, row_rect(px(ROWS_Y + i as f32 * ROW_H)), profile, false);
        }
        (Size { w, h: px(ROWS_Y + shown.len() as f32 * ROW_H + 10.0) }, Rect::default(), Rect::default())
    };

    // Marcador de onde a linha cai e a cópia que segue o mouse.
    if let Some(d) = view.drag {
        if let Slot::Visible(index) = d.slot {
            let others: Vec<&(Rect, usize)> = rows.iter().filter(|(_, p)| *p != d.profile && arr.visible.contains(p)).collect();
            let y = match (others.get(index), others.last()) {
                (Some((r, _)), _) => r.y,
                (None, Some((r, _))) => r.bottom(),
                (None, None) => visible_zone.y,
            };
            ops.push(DrawOp::RoundRect { rect: Rect::new(px(20.0), y - px(1.25), w - px(40.0), px(2.5)), radius: px(1.25), color: tokens.accent });
        }
        let ghost = Rect::new(px(10.0), d.point.y - px(ROW_H / 2.0), w - px(20.0), px(ROW_H));
        ops.push(DrawOp::RoundRect { rect: ghost, radius: px(10.0), color: tokens.raised_strong });
        ops.push(DrawOp::RoundRectStroke { rect: ghost.inset(px(0.75), px(0.75)), radius: px(10.0), color: tokens.accent, width: px(1.5) });
        ops.push(DrawOp::Text { rect: Rect::new(ghost.x + px(24.0), ghost.y, ghost.w - px(34.0), ghost.h), text: view.rows[d.profile].name.clone(), style: text(12.5, FontWeight::Regular), color: tokens.text, align: Align::Leading });
    }

    PanelLayout { list: DisplayList { size, ops }, pencil, rows, eyes, visible_zone, hidden_zone }
}

/// O que um botão solto no painel faz.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum UpAction { Nothing, Launch(usize), StartEdit, FinishEdit, Drop(usize, Slot), Toggle(usize) }

/// Estado do mouse no painel quando o botão é solto.
pub struct UpInput<'a> {
    pub editing: bool,
    /// Perfis lidos e `wt.exe` achado (o lápis só vale assim).
    pub ready: bool,
    /// Perfil em que o botão desceu na edição.
    pub press: Option<usize>,
    pub drag: Option<&'a DragView>,
    /// O gesto foi cancelado pelo teclado com o botão ainda apertado: este soltar não faz nada.
    pub swallow: bool,
}

pub fn up_action(layout: &PanelLayout, input: &UpInput, p: Point) -> UpAction {
    if input.swallow {
        return UpAction::Nothing;
    }
    if let Some(d) = input.drag {
        return UpAction::Drop(d.profile, d.slot);
    }
    if input.ready && layout.pencil.contains(p) {
        return if input.editing { UpAction::FinishEdit } else { UpAction::StartEdit };
    }
    if input.editing {
        let on_own_eye = |profile: usize| layout.eyes.iter().any(|(r, i)| *i == profile && r.contains(p));
        return match input.press {
            Some(profile) if on_own_eye(profile) => UpAction::Toggle(profile),
            _ => UpAction::Nothing,
        };
    }
    layout.rows.iter().find(|(r, _)| r.contains(p)).map_or(UpAction::Nothing, |(_, i)| UpAction::Launch(*i))
}

/// Perfil que o Enter abre no modo normal: só o escolhido que está visível.
pub fn launch_target(selected: Option<usize>, arrangement: &Arrangement) -> Option<usize> {
    selected.filter(|p| arrangement.visible.contains(p))
}

/// Onde o perfil arrastado cairia com o mouse em `p`: abaixo do meio do vão entre as zonas é
/// "Ocultos"; acima, a posição entre os outros visíveis pelo meio de cada linha.
pub fn drop_slot(layout: &PanelLayout, arrangement: &Arrangement, dragged: usize, p: Point) -> Slot {
    if p.y >= (layout.visible_zone.bottom() + layout.hidden_zone.y) / 2.0 {
        return Slot::Hidden;
    }
    let above = layout
        .rows
        .iter()
        .filter(|(_, profile)| *profile != dragged && arrangement.visible.contains(profile))
        .filter(|(r, _)| r.y + r.h / 2.0 < p.y)
        .count();
    Slot::Visible(above)
}

#[cfg(test)]
mod tests {
    use super::*;
    use zefiro_resources::theme::{BUILTIN, parse_theme};
    use zefiro_resources::tokens::derive_tokens;

    fn tokens() -> Tokens {
        derive_tokens(&parse_theme("obsidian", "o", BUILTIN[0].1).unwrap(), Rgba::rgb(0, 120, 215), None)
    }

    fn profile_rows(n: usize) -> Vec<ProfileRow> {
        (0..n).map(|i| ProfileRow { name: format!("perfil-{i}"), icon: None, default: i == 0 }).collect()
    }

    fn view<'a>(rows: &'a [ProfileRow], arrangement: &'a Arrangement, editing: bool) -> PanelView<'a> {
        PanelView { rows, arrangement, editing, message: None, selected: None, hover: None, drag: None, scroll: 0 }
    }

    fn texts(l: &DisplayList) -> Vec<String> {
        l.ops.iter().filter_map(|o| if let DrawOp::Text { text, .. } = o { Some(text.clone()) } else { None }).collect()
    }

    fn center(r: Rect) -> Point {
        Point { x: r.x + r.w / 2.0, y: r.y + r.h / 2.0 }
    }

    #[test]
    fn normal_mode_lists_visible_rows_with_default_badge() {
        let rows = profile_rows(4);
        let arr = Arrangement { visible: vec![3, 0], hidden: vec![1, 2] };
        let l = terminal_panel(&view(&rows, &arr, false), 1.0, &tokens());
        let t = texts(&l.list);
        for s in ["Terminal", "Perfis do Windows Terminal", "perfil-3", "perfil-0", "padrão", "\u{E70F}"] {
            assert!(t.contains(&s.to_string()), "falta {s}");
        }
        assert!(!t.contains(&"perfil-1".to_string()) && !t.contains(&"Ocultos".to_string()));
        assert_eq!(l.rows.iter().map(|(_, i)| *i).collect::<Vec<_>>(), [3, 0]);
        assert!(l.eyes.is_empty());
        assert_eq!(l.list.size.w, 300.0);
    }

    #[test]
    fn editing_shows_hidden_rows_eyes_and_done() {
        let rows = profile_rows(4);
        let arr = Arrangement { visible: vec![3, 0], hidden: vec![1, 2] };
        let normal = terminal_panel(&view(&rows, &arr, false), 1.0, &tokens());
        let l = terminal_panel(&view(&rows, &arr, true), 1.0, &tokens());
        let t = texts(&l.list);
        for s in ["Concluir", "Arraste para ordenar · o olho esconde", "Ocultos", "perfil-1", "perfil-2"] {
            assert!(t.contains(&s.to_string()), "falta {s}");
        }
        assert_eq!(l.rows.iter().map(|(_, i)| *i).collect::<Vec<_>>(), [3, 0, 1, 2]);
        assert_eq!(l.eyes.len(), 4);
        assert!(l.list.size.h > normal.list.size.h);
        assert!(l.hidden_zone.y >= l.visible_zone.bottom());
    }

    #[test]
    fn row_without_icon_gets_the_terminal_glyph() {
        let mut rows = profile_rows(2);
        rows[1].icon = Image::new(1, 1, vec![0, 0, 0, 255]);
        let arr = Arrangement { visible: vec![0, 1], hidden: vec![] };
        let l = terminal_panel(&view(&rows, &arr, false), 1.0, &tokens());
        assert_eq!(texts(&l.list).iter().filter(|t| *t == "\u{E756}").count(), 1);
        assert_eq!(l.list.ops.iter().filter(|o| matches!(o, DrawOp::Image { .. })).count(), 1);
    }

    #[test]
    fn messages_replace_the_list() {
        let rows = profile_rows(2);
        let arr = Arrangement { visible: vec![0, 1], hidden: vec![] };
        let mut v = view(&rows, &arr, false);
        v.message = Some("Carregando…");
        let l = terminal_panel(&v, 1.0, &tokens());
        assert!(texts(&l.list).contains(&"Carregando…".to_string()));
        assert!(l.rows.is_empty());
        let none = Arrangement { visible: vec![], hidden: vec![0, 1] };
        let l = terminal_panel(&view(&rows, &none, false), 1.0, &tokens());
        assert!(texts(&l.list).contains(&"Nenhum perfil visível. Toque no lápis para escolher.".to_string()));
    }

    #[test]
    fn drop_slot_follows_the_mouse() {
        let rows = profile_rows(4);
        let arr = Arrangement { visible: vec![0, 1, 2], hidden: vec![3] };
        let l = terminal_panel(&view(&rows, &arr, true), 1.0, &tokens());
        let row = |p: usize| l.rows.iter().find(|(_, i)| *i == p).unwrap().0;
        assert_eq!(drop_slot(&l, &arr, 2, center(row(0))), Slot::Visible(0));
        let below = Point { x: 50.0, y: row(2).bottom() - 2.0 };
        assert_eq!(drop_slot(&l, &arr, 0, below), Slot::Visible(2));
        assert_eq!(drop_slot(&l, &arr, 0, center(l.hidden_zone)), Slot::Hidden);
    }

    fn up(editing: bool, press: Option<usize>, drag: Option<&DragView>, swallow: bool) -> UpInput<'_> {
        UpInput { editing, ready: true, press, drag, swallow }
    }

    #[test]
    fn release_after_a_keyboard_cancel_opens_nothing() {
        let rows = profile_rows(2);
        let arr = Arrangement { visible: vec![0, 1], hidden: vec![] };
        let l = terminal_panel(&view(&rows, &arr, false), 1.0, &tokens());
        let on_row = center(l.rows[1].0);
        assert_eq!(up_action(&l, &up(false, None, None, true), on_row), UpAction::Nothing, "Esc ou Enter no meio do arraste não abre perfil ao soltar");
        assert_eq!(up_action(&l, &up(false, None, None, false), on_row), UpAction::Launch(1));
    }

    #[test]
    fn release_in_normal_mode_pencil_needs_a_loaded_list() {
        let rows = profile_rows(2);
        let arr = Arrangement { visible: vec![0, 1], hidden: vec![] };
        let l = terminal_panel(&view(&rows, &arr, false), 1.0, &tokens());
        assert_eq!(up_action(&l, &up(false, None, None, false), center(l.pencil)), UpAction::StartEdit);
        let not_ready = UpInput { ready: false, ..up(false, None, None, false) };
        assert_eq!(up_action(&l, &not_ready, center(l.pencil)), UpAction::Nothing);
    }

    #[test]
    fn release_in_edit_mode_never_launches() {
        let rows = profile_rows(3);
        let arr = Arrangement { visible: vec![0, 1], hidden: vec![2] };
        let l = terminal_panel(&view(&rows, &arr, true), 1.0, &tokens());
        let row = |p: usize| l.rows.iter().find(|(_, i)| *i == p).unwrap().0;
        let eye = |p: usize| l.eyes.iter().find(|(_, i)| *i == p).unwrap().0;
        assert_eq!(up_action(&l, &up(true, Some(1), None, false), center(row(1))), UpAction::Nothing, "clique na linha não abre na edição");
        assert_eq!(up_action(&l, &up(true, Some(2), None, false), center(eye(2))), UpAction::Toggle(2));
        assert_eq!(up_action(&l, &up(true, Some(1), None, false), center(eye(0))), UpAction::Nothing, "o olho é do perfil em que o botão desceu");
        let drag = DragView { profile: 0, point: center(l.hidden_zone), slot: Slot::Hidden };
        assert_eq!(up_action(&l, &up(true, Some(0), Some(&drag), false), center(l.hidden_zone)), UpAction::Drop(0, Slot::Hidden));
        assert_eq!(up_action(&l, &up(true, None, None, false), center(l.pencil)), UpAction::FinishEdit);
    }

    #[test]
    fn enter_opens_only_a_visible_choice() {
        let arr = Arrangement { visible: vec![0, 1], hidden: vec![2] };
        assert_eq!(launch_target(Some(1), &arr), Some(1));
        assert_eq!(launch_target(Some(2), &arr), None, "escondido pelo teclado na edição");
        assert_eq!(launch_target(None, &arr), None);
    }

    #[test]
    fn scrolls_after_twelve_in_normal_mode() {
        let rows = profile_rows(20);
        let arr = Arrangement { visible: (0..20).rev().collect(), hidden: vec![] };
        let mut v = view(&rows, &arr, false);
        v.scroll = 3;
        let l = terminal_panel(&v, 1.0, &tokens());
        assert_eq!(l.rows.len(), 12);
        assert_eq!(l.rows[0].1, arr.visible[3]);
        assert_eq!(max_scroll(20), 8);
    }

    fn a() -> Arrangement {
        Arrangement { visible: vec![0, 1, 3], hidden: vec![2] }
    }

    #[test]
    fn eye_hides_and_shows() {
        let mut x = a();
        x.toggle(1);
        assert_eq!(x, Arrangement { visible: vec![0, 3], hidden: vec![1, 2] });
        x.toggle(2);
        assert_eq!(x, Arrangement { visible: vec![0, 3, 2], hidden: vec![1] });
    }

    #[test]
    fn drag_reorders_hides_and_shows() {
        let mut x = a();
        x.move_to(3, Slot::Visible(0));
        assert_eq!(x.visible, [3, 0, 1]);
        x.move_to(0, Slot::Hidden);
        assert_eq!((x.visible.clone(), x.hidden.clone()), (vec![3, 1], vec![0, 2]));
        x.move_to(2, Slot::Visible(1));
        assert_eq!((x.visible, x.hidden), (vec![3, 2, 1], vec![0]));
    }

    #[test]
    fn nudge_moves_within_visible_only() {
        let mut x = a();
        x.nudge(3, true);
        assert_eq!(x.visible, [0, 3, 1]);
        x.nudge(0, true);
        x.nudge(2, false);
        assert_eq!((x.visible.clone(), x.hidden.clone()), (vec![0, 3, 1], vec![2]));
    }

    #[test]
    fn hiding_everything_is_allowed() {
        let mut x = a();
        for i in [0, 1, 3] {
            x.toggle(i);
        }
        assert_eq!((x.visible.len(), x.hidden), (0, vec![0, 1, 2, 3]));
    }

    #[test]
    fn rows_list_visible_then_hidden() {
        assert_eq!(a().rows(), [0, 1, 3, 2]);
    }
}
