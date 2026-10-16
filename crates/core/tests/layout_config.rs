use zefiro_core::config::*;

#[test]
fn legacy_config_keeps_shared_layout_and_empty_center() {
    let config = parse_config(DEFAULT_CONFIG).unwrap();
    assert_eq!(config.bar.layout_mode, LayoutMode::Shared);
    assert!(config.bar.sections.center.is_empty());
    assert!(config.bar.monitors.is_empty());
}

#[test]
fn explicit_empty_sections_stay_empty() {
    let config = parse_config("[bar.sections]\nstart = []\ncenter = []\nend = []").unwrap();
    assert_eq!(config.bar.sections.ids().count(), 0);
}

#[test]
fn monitor_layout_is_complete_and_selected_by_identity() {
    let text = "[bar]\nlayout_mode = 'per_monitor'\n[bar.monitors.'display-path'.sections]\nstart = []\ncenter = ['clock']\nend = ['zefiro']";
    let config = parse_config(text).unwrap();
    assert_eq!(config.bar.sections_for(Some("display-path")).center, ["clock"]);
    assert_eq!(config.bar.sections_for(Some("other")), &config.bar.sections);
    assert_eq!(config.bar.sections_for(None), &config.bar.sections);
    assert!(parse_config("[bar.monitors.x.sections]\nstart = []\nend = []").is_err());
}

#[test]
fn shared_mode_ignores_saved_monitor_layout() {
    let config = parse_config("[bar.monitors.x.sections]\nstart = []\ncenter = ['clock']\nend = []").unwrap();
    assert_eq!(config.bar.sections_for(Some("x")), &config.bar.sections);
}

#[test]
fn duplicate_ids_are_rejected_across_sections_but_unknown_ids_survive() {
    let error = parse_config("[bar.sections]\ncenter = ['clock']").unwrap_err();
    assert!(error.message.contains("clock"));
    assert!(error.message.contains("bar.sections"));
    let config = parse_config("[bar.sections]\nstart = ['@autor/futuro']").unwrap();
    assert_eq!(config.bar.sections.start, ["@autor/futuro"]);
}

#[test]
fn layout_limits_reject_instead_of_truncating() {
    let ids = (0..65).map(|n| format!("'item-{n}'")).collect::<Vec<_>>().join(",");
    assert!(parse_config(&format!("[bar.sections]\nstart = [{ids}]")).is_err());
    assert!(parse_config(&format!("[bar.sections]\nstart = ['{}']", "é".repeat(129))).is_err());
    let monitors = (0..129).map(|n| format!("[bar.monitors.'m{n}'.sections]\nstart=[]\ncenter=[]\nend=[]\n")).collect::<String>();
    assert!(parse_config(&monitors).is_err());
}

#[test]
fn reading_a_legacy_file_does_not_rewrite_it() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("config.toml");
    let text = "# legado\n[bar.sections]\nstart = ['zefiro']\nend = ['clock']\n";
    std::fs::write(&path, text).unwrap();
    load_or_create(&path).unwrap();
    assert_eq!(std::fs::read_to_string(path).unwrap(), text);
}
