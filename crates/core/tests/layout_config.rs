use zefiro_core::config::*;

#[test]
fn legacy_config_keeps_shared_layout_and_the_island_in_the_center() {
    let config = parse_config(DEFAULT_CONFIG).unwrap();
    assert_eq!(config.bar.layout_mode, LayoutMode::Shared);
    assert_eq!(config.bar.sections.center, [ISLAND_CHIP]);
    assert!(config.bar.monitors.is_empty());
    // Config de antes do chip, sem a chave `center`: a ilha continua onde estava (ADR-0017).
    let old = parse_config("[bar.sections]\nstart = [\"zefiro\"]\nend = [\"clock\"]\n").unwrap();
    assert_eq!(old.bar.sections.center, [ISLAND_CHIP]);
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
fn clean_bar_options_default_to_todays_behavior() {
    let config = parse_config(DEFAULT_CONFIG).unwrap();
    assert!(!config.bar.autohide);
    assert_eq!(config.bar.display, BarDisplay::Todos);
    assert!(config.bar.shows_on(false), "sem a chave, todo monitor tem barra");

    let text = "[bar]\nautohide = true\ndisplay = 'principal'\nlayout_mode = 'per_monitor'\n[bar.monitors.x.sections]\nstart = []\ncenter = ['clock']\nend = []";
    let config = parse_config(text).unwrap();
    assert!(config.bar.shows_on(true) && !config.bar.shows_on(false), "só o principal");
    assert_eq!(config.bar.sections_for(Some("x")).center, ["clock"], "o layout do secundário continua guardado");
    assert!(config.autohide());
    assert!(parse_config("[bar]\ndisplay = 'segundo'").is_err(), "valor errado é erro, como height e format");

    // Ilha externa: outro processo, não acompanha a barra; ocultar automaticamente não vale.
    let external = parse_config("[bar]\nautohide = true\n[island]\nengine = 'externa'").unwrap();
    assert!(!external.autohide());
}

#[test]
fn primary_only_keeps_one_bar_even_without_a_primary_flag() {
    let mut bar = BarConfig::default();
    assert_eq!(bar.monitors_with_bar(&[false, true, false]), [0, 1, 2]);
    bar.display = BarDisplay::Principal;
    assert_eq!(bar.monitors_with_bar(&[false, true, false]), [1]);
    // Nenhum marcado como principal (troca de monitor no meio): fica o primeiro, nunca zero barras.
    assert_eq!(bar.monitors_with_bar(&[false, false]), [0]);
    assert!(bar.monitors_with_bar(&[]).is_empty());
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
