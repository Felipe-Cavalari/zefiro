use zefiro_bar::editor::{EditorSession, Section};
use zefiro_core::config::{Config, LayoutMode};

#[test]
fn edits_change_only_draft_and_move_instead_of_duplicate() {
    let config = Config::default();
    let mut editor = EditorSession::new(config.clone(), "base".into());
    editor.move_item("clock", Section::Start, 0).unwrap();
    assert_eq!(editor.draft.bar.sections.start, ["clock", "zefiro"]);
    assert!(editor.draft.bar.sections.end.is_empty());
    assert_eq!(editor.confirmed, config);
    assert!(editor.add("clock", Section::End).is_err());
    editor.remove("clock").unwrap();
    editor.add("clock", Section::Center).unwrap();
    assert_eq!(editor.draft.bar.sections.center, ["clock"]);
}

#[test]
fn switching_modes_restores_own_layouts_and_preserves_disconnected_entries() {
    let mut editor = EditorSession::new(Config::default(), String::new());
    editor.select_monitor(Some("one".into()));
    editor.set_mode(LayoutMode::PerMonitor, &["one".into(), "two".into()]);
    editor.move_item("clock", Section::Start, 0).unwrap();
    editor.set_mode(LayoutMode::Shared, &[]);
    assert_eq!(editor.draft.bar.sections.start, ["clock", "zefiro"]);
    editor.set_mode(LayoutMode::PerMonitor, &[]);
    assert_eq!(editor.draft.bar.monitors["two"].sections.end, ["clock"]);
    assert_eq!(editor.sections().start, ["clock", "zefiro"]);
}

#[test]
fn unavailable_identity_cannot_create_persistent_layout() {
    let mut editor = EditorSession::new(Config::default(), String::new());
    editor.set_mode(LayoutMode::PerMonitor, &[]);
    assert!(editor.remove("clock").is_err());
    assert!(editor.draft.bar.monitors.is_empty());
}

#[test]
fn cancel_gesture_restores_arrays_and_updates_from_external_base_survive_cancel() {
    let mut editor = EditorSession::new(Config::default(), "base".into());
    editor.begin_move("clock").unwrap();
    editor.move_item("clock", Section::Start, 0).unwrap();
    editor.cancel_move();
    assert_eq!(editor.draft, Config::default());
    let external = Config { theme: "plum".into(), ..Config::default() };
    editor.external_change(external.clone(), "latest".into());
    assert!(editor.conflict);
    assert_eq!(editor.confirmed, external);
    assert_eq!(editor.draft.theme, "obsidian");
    editor.reload();
    assert_eq!(editor.draft.theme, "plum");
}

#[test]
fn patch_contains_only_fields_changed_by_the_editor() {
    let base = zefiro_core::config::DEFAULT_CONFIG;
    let mut editor = EditorSession::new(Config::default(), base.into());
    editor.move_item("clock", Section::Start, 1).unwrap();
    let latest = base.replace("obsidian", "nord");
    let result = editor.patch().apply(base, &latest, &[]).unwrap();
    let config = zefiro_core::config::parse_config(&result).unwrap();
    assert_eq!(config.theme, "nord");
    assert_eq!(config.bar.sections.start, ["zefiro", "clock"]);
    assert!(config.bar.sections.end.is_empty());
}
