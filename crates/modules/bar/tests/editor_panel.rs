use zefiro_bar::editor::{EditorSession, Section};
use zefiro_bar::editor_panel::{EditorAction, EditorContext, EditorPanel, PanelDrag, Target, Zone};
use zefiro_core::config::{Config, ISLAND_CHIP};
use zefiro_platform::geometry::Point;
use zefiro_platform::text::FixedWidthMeasurer;

fn session() -> EditorSession {
    let mut config = Config::default();
    config.bar.sections.end = vec!["claude".into(), "audio".into(), "clock".into()];
    EditorSession::new(config, String::new())
}

fn cx<'a>(session: &'a EditorSession, monitors: &'a [String]) -> EditorContext<'a> {
    EditorContext { session, monitors, monitor: 0, scale: 1.25, width: 560.0 * 1.25 }
}

fn chip_center(layout: &zefiro_bar::editor_panel::EditorLayout, id: &str) -> Point {
    let c = layout.chips.iter().find(|c| c.id == id).unwrap();
    Point { x: c.rect.center_x(), y: c.rect.y + c.rect.h / 2.0 }
}

#[test]
fn sections_sit_side_by_side_and_the_island_is_a_chip_like_any_other() {
    let s = session();
    let names = ["Principal".to_string(), "Lateral".to_string()];
    let panel = EditorPanel::default();
    let layout = panel.layout(&cx(&s, &names), &FixedWidthMeasurer);
    let zone = |z: Zone| layout.zones.iter().find(|(k, _)| *k == z).unwrap().1;
    let (start, center, end) = (zone(Zone::Section(Section::Start)), zone(Zone::Section(Section::Center)), zone(Zone::Section(Section::End)));
    assert!(start.right() <= center.x && center.right() <= end.x, "lado a lado, na ordem da barra");
    assert_eq!((start.y, start.h), (end.y, end.h), "mesma altura");
    // Sem a ilha externa (ADR-0026), nada fica travado no Centro.
    let island = layout.chips.iter().find(|c| c.id == ISLAND_CHIP).expect("a ilha aparece no Centro");
    assert_eq!(island.zone, Zone::Section(Section::Center));
    assert_eq!(EditorPanel::hit(&layout, Point { x: island.rect.center_x(), y: island.rect.y + 2.0 }), Some(Target::Chip(ISLAND_CHIP.into())), "dá para pegar a ilha");
    let middle = Point { x: center.center_x(), y: center.y + center.h / 2.0 };
    assert_eq!(EditorPanel::zone_at(&layout, middle), Some(Zone::Section(Section::Center)), "o Centro aceita soltura");
    for b in &layout.buttons {
        assert!(b.rect.bottom() <= layout.size.h && b.rect.h >= 28.0 * 1.25);
    }
    assert!(layout.buttons.iter().any(|b| b.action == EditorAction::Save && b.enabled));
    assert!(layout.buttons.iter().any(|b| b.action == EditorAction::Cancel && b.enabled));
}

#[test]
fn dragging_a_chip_from_the_end_lands_before_the_start_items() {
    let s = session();
    let names = ["Principal".to_string()];
    let mut panel = EditorPanel::default();
    let layout = panel.layout(&cx(&s, &names), &FixedWidthMeasurer);
    assert_eq!(EditorPanel::hit(&layout, chip_center(&layout, "claude")), Some(Target::Chip("claude".into())));
    // Solta à esquerda do Zefiro, no Início.
    let zefiro = layout.chips.iter().find(|c| c.id == "zefiro").unwrap().rect;
    let point = Point { x: zefiro.x + 2.0, y: zefiro.y + zefiro.h / 2.0 };
    panel.drag = Some(PanelDrag { id: "claude".into(), origin: chip_center(&layout, "claude"), point, active: true });
    assert_eq!(panel.drag_target(&layout), Some((Zone::Section(Section::Start), 0)));
    // À direita do Zefiro: depois dele.
    panel.drag.as_mut().unwrap().point = Point { x: zefiro.right() + 8.0, y: point.y };
    assert_eq!(panel.drag_target(&layout), Some((Zone::Section(Section::Start), 1)));
}

#[test]
fn shelf_lists_items_off_the_bar_and_accepts_removal() {
    let s = session();
    let names = ["Principal".to_string()];
    let panel = EditorPanel::default();
    let layout = panel.layout(&cx(&s, &names), &FixedWidthMeasurer);
    let shelf: Vec<&str> = layout.chips.iter().filter(|c| c.zone == Zone::Shelf).map(|c| c.id.as_str()).collect();
    assert!(shelf.contains(&"codex") && !shelf.contains(&"claude"), "só o que está fora da barra");
    let rect = layout.zones.iter().find(|(z, _)| *z == Zone::Shelf).unwrap().1;
    assert_eq!(EditorPanel::zone_at(&layout, Point { x: rect.right() - 4.0, y: rect.bottom() - 4.0 }), Some(Zone::Shelf));
    let center = layout.zones.iter().find(|(z, _)| *z == Zone::Section(Section::Center)).unwrap().1;
    assert_eq!(EditorPanel::zone_at(&layout, Point { x: center.center_x(), y: center.y + 4.0 }), Some(Zone::Section(Section::Center)), "sem ilha, o Centro aceita");
}

#[test]
fn clean_bar_switches_sit_above_the_buttons_on_both_pages() {
    let mut s = session();
    let two = ["Principal".to_string(), "Lateral".to_string()];
    let one = ["Principal".to_string()];
    for page in [zefiro_bar::editor_panel::EditorPage::Items, zefiro_bar::editor_panel::EditorPage::Options] {
        let panel = EditorPanel { page, ..Default::default() };
        let layout = panel.layout(&cx(&s, &two), &FixedWidthMeasurer);
        let find = |a: EditorAction| layout.buttons.iter().find(|b| b.action == a).unwrap_or_else(|| panic!("{a:?} em {page:?}")).clone();
        let (autohide, primary, save) = (find(EditorAction::ToggleAutohide), find(EditorAction::TogglePrimaryOnly), find(EditorAction::Save));
        assert!(autohide.enabled && primary.enabled);
        assert!(autohide.rect.bottom() <= save.rect.y && primary.rect.bottom() <= save.rect.y, "acima do Cancelar/Concluir");
    }
    // Um monitor só: "Só no monitor principal" não aparece.
    let layout = EditorPanel::default().layout(&cx(&s, &one), &FixedWidthMeasurer);
    assert!(!layout.buttons.iter().any(|b| b.action == EditorAction::TogglePrimaryOnly));
    // Só no principal ligado: a escolha de monitor e de layout fica apagada.
    s.toggle_primary_only();
    let layout = EditorPanel::default().layout(&cx(&s, &two), &FixedWidthMeasurer);
    for b in layout.buttons.iter().filter(|b| matches!(b.action, EditorAction::Monitor(_) | EditorAction::ToggleMode)) {
        assert!(!b.enabled, "{:?} fica apagado", b.action);
    }
    // Ligado, só sobra a barra do principal: a chave continua, senão não dá para desligar.
    let layout = EditorPanel::default().layout(&cx(&s, &one), &FixedWidthMeasurer);
    let primary = layout.buttons.iter().find(|b| b.action == EditorAction::TogglePrimaryOnly).expect("a chave continua com uma barra só");
    assert!(primary.enabled && primary.kind == zefiro_bar::editor_panel::ButtonKind::Switch { on: true });
    let autohide = layout.buttons.iter().find(|b| b.action == EditorAction::ToggleAutohide).unwrap();
    assert!(autohide.enabled, "ocultar automaticamente não depende mais da ilha");
}

#[test]
fn back_row_on_top_returns_to_the_menu_on_both_pages() {
    let s = session();
    let names = ["Principal".to_string(), "Lateral".to_string()];
    let tokens = zefiro_resources::tokens::derive_tokens(&zefiro_resources::theme::parse_theme("obsidian", "o", zefiro_resources::theme::BUILTIN[0].1).unwrap(), zefiro_platform::color::Rgba::rgb(0, 120, 215), None);
    for page in [zefiro_bar::editor_panel::EditorPage::Items, zefiro_bar::editor_panel::EditorPage::Options] {
        let panel = EditorPanel { page, ..Default::default() };
        let layout = panel.layout(&cx(&s, &names), &FixedWidthMeasurer);
        let back = layout.buttons.iter().find(|b| b.action == EditorAction::BackToMenu).unwrap_or_else(|| panic!("‹ Editar barra em {page:?}"));
        assert!(back.enabled && layout.buttons.iter().filter(|b| b.action != EditorAction::BackToMenu).all(|b| b.rect.y >= back.rect.bottom()), "no topo, como o ‹ Tema");
        assert_eq!(EditorPanel::hit(&layout, Point { x: back.rect.center_x(), y: back.rect.y + back.rect.h / 2.0 }), Some(Target::Button(EditorAction::BackToMenu)));
        let texts: Vec<String> = panel.paint(&layout, &s, &tokens).ops.into_iter().filter_map(|o| if let zefiro_ui::paint::DrawOp::Text { text, .. } = o { Some(text) } else { None }).collect();
        assert!(texts.contains(&"‹ Editar barra".to_string()) && !texts.contains(&"Editar barra".to_string()), "{texts:?}");
    }
}

#[test]
fn options_button_appears_for_the_selected_clock_and_tab_cycles_everything() {
    let s = session();
    let names = ["Principal".to_string()];
    let mut panel = EditorPanel { selected: Some("clock".into()), ..Default::default() };
    let layout = panel.layout(&cx(&s, &names), &FixedWidthMeasurer);
    assert!(layout.buttons.iter().any(|b| b.action == EditorAction::Options));
    let all = EditorPanel::focusables(&layout);
    for _ in 0..all.len() {
        panel.navigate(&layout, 1);
    }
    assert_eq!(panel.focus.as_ref(), all.last(), "Tab passa por todos e para no último");
    panel.navigate(&layout, 1);
    assert_eq!(panel.focus.as_ref(), all.first(), "e volta ao primeiro");
}
