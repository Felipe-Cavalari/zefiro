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
    assert_eq!(editor.draft.bar.sections.center, ["island", "clock"], "a ilha é o chip padrão do centro");
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

#[test]
fn invalid_external_text_blocks_reload_and_keeps_last_valid_cancel_base() {
    let mut editor = EditorSession::new(Config::default(), "theme = 'obsidian'".into());
    editor.move_item("clock", Section::Start, 0).unwrap();
    editor.external_invalid("[broken".into(), "Erro de sintaxe".into());
    assert!(editor.conflict);
    assert!(editor.file_error.is_some());
    assert_eq!(editor.latest_text, "[broken");
    editor.reload();
    assert_eq!(editor.draft.bar.sections.start, ["clock", "zefiro"]);
    assert_eq!(editor.confirmed, Config::default());
    editor.external_change(Config::default(), "theme = 'obsidian' # corrigido".into());
    assert!(editor.file_error.is_none());
    editor.reload();
    assert_eq!(editor.draft, Config::default());
}

#[test]
fn gallery_rejects_capacity_overflow_without_mutating_existing_unknown_items() {
    let config = Config { bar: zefiro_core::config::BarConfig {
        sections: zefiro_core::config::Sections { start: (0..64).map(|i| format!("@autor/item-{i}")).collect(), center: Vec::new(), end: Vec::new() },
        ..Default::default()
    }, ..Default::default() };
    let mut editor = EditorSession::new(config.clone(), String::new());
    assert!(editor.add("clock", Section::End).is_err());
    assert_eq!(editor.draft, config);
}

/// Reaplica como o app faz (`reapply_editor`) e depois conclui como `EditorAction::Save`.
fn reapply_then_save(editor: &mut EditorSession) -> String {
    let merged = editor.patch().apply(&editor.original_text, &editor.latest_text, &[]).unwrap();
    editor.rebase_onto_latest(zefiro_core::config::parse_config(&merged).unwrap());
    assert!(!editor.conflict);
    editor.patch().apply(&editor.original_text, &editor.latest_text, &[]).unwrap()
}

#[test]
fn save_after_reapply_does_not_conflict_on_fields_changed_only_outside() {
    use zefiro_core::config::parse_config;
    let base = zefiro_core::config::DEFAULT_CONFIG.replace("center = [\"island\"]", "center = []");
    let mut editor = EditorSession::new(parse_config(&base).unwrap(), base.clone());
    editor.remove("zefiro").unwrap();
    let latest = base.replace("end    = [\"clock\"]", "end    = []");
    editor.external_change(parse_config(&latest).unwrap(), latest.clone());
    assert!(editor.conflict);
    let saved = parse_config(&reapply_then_save(&mut editor)).unwrap();
    assert!(saved.bar.sections.start.is_empty());
    assert!(saved.bar.sections.end.is_empty());
    assert_eq!(editor.original_text, latest);
}

#[test]
fn save_after_reapply_keeps_clock_seconds_changed_outside() {
    use zefiro_core::config::parse_config;
    let base = zefiro_core::config::DEFAULT_CONFIG;
    let mut editor = EditorSession::new(parse_config(base).unwrap(), base.into());
    editor.remove("zefiro").unwrap();
    let latest = base.replace("seconds = false", "seconds = true");
    editor.external_change(parse_config(&latest).unwrap(), latest.clone());
    let saved = reapply_then_save(&mut editor);
    let config = parse_config(&saved).unwrap();
    assert!(config.clock.seconds);
    assert!(config.bar.sections.start.is_empty());
    assert!(saved.contains("# vazio = formato do Windows"));
}
