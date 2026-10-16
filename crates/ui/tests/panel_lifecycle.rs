use zefiro_ui::panel_lifecycle::PanelLifecycle;

#[test]
fn old_close_completion_cannot_hide_a_reopened_and_closed_again_panel() {
    let mut panel = PanelLifecycle::default();
    panel.open();
    let first_close = panel.close();
    panel.open();
    let second_close = panel.close();
    assert!(!panel.settle(first_close));
    assert!(panel.visible());
    assert!(panel.settle(second_close));
    assert!(!panel.visible());
}

#[test]
fn completion_from_a_destroyed_panel_is_not_valid_for_its_replacement() {
    let mut first = PanelLifecycle::default();
    first.open();
    let ticket = first.close();
    let mut replacement = PanelLifecycle::default();
    replacement.open();
    replacement.close();
    assert!(!replacement.settle(ticket));
    assert!(replacement.visible());
}
