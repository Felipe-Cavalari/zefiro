use zefiro_bar::editor::EditorSession;
use zefiro_bar::editor_panel::{EditorPanel, EditorAction};
use zefiro_core::config::Config;

#[test]
fn keyboard_focus_scrolls_to_controls_and_footer_stays_accessible() {
    let session = EditorSession::new(Config::default(), String::new());
    let mut panel = EditorPanel::default();
    let rows = panel.rows(&session, &["Tela principal".into()]);
    assert!(rows.iter().any(|r| r.action == EditorAction::Save));
    assert!(rows.iter().any(|r| r.action == EditorAction::Cancel));
    for _ in 0..30 { panel.navigate(1, rows.len(), 4); }
    assert!(panel.focus < rows.len());
    let boxes = panel.boxes(&session, &["Tela principal".into()], 1.25, 500.0, 280.0);
    assert!(boxes.iter().all(|(_, r)| r.h >= 28.0 * 1.25));
    assert!(boxes.iter().any(|(a, _)| *a == EditorAction::Cancel));
    assert!(boxes.iter().any(|(a, _)| *a == EditorAction::Save));
    for (_, r) in boxes { assert!(r.y >= 0.0 && r.bottom() <= 280.0); }
}

#[test]
fn selected_item_focus_follows_identity_after_moving_between_sections() {
    let mut session = EditorSession::new(Config::default(), String::new());
    let mut panel = EditorPanel::default();
    let names = ["Principal".into()];
    let item = EditorAction::SelectItem("clock".into());
    panel.focus_action(&session, &names, &item, 4);
    assert_eq!(panel.rows(&session, &names)[panel.focus].action, item);
    session.move_item("clock", zefiro_bar::editor::Section::Start, 0).unwrap();
    panel.focus_action(&session, &names, &item, 4);
    assert_eq!(panel.rows(&session, &names)[panel.focus].action, item);
    assert!(panel.focus >= panel.scroll && panel.focus < panel.scroll + 4);
}
