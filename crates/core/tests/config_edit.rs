use zefiro_core::config_edit::{ConfigPatch, PatchChoice};

const BASE: &str = "# pessoal\ntheme = 'obsidian' # tema\n[bar.sections]\nstart = ['zefiro'] # inicio\nend = ['clock']\n[future]\nvalue = 42 # preservar\n";

#[test]
fn patches_preserve_comments_and_unknown_fields() {
    let mut patch = ConfigPatch::default();
    patch.set_strings(&["bar", "sections", "start"], &["clock".into()]);
    patch.set_strings(&["bar", "sections", "end"], &["zefiro".into()]);
    let result = patch.apply(BASE, BASE, &[]).unwrap();
    assert!(result.contains("# inicio"));
    assert!(result.contains("value = 42 # preservar"));
    assert!(result.contains("theme = 'obsidian' # tema"));
    assert_eq!(zefiro_core::config::parse_config(&result).unwrap().bar.sections.start, ["clock"]);
}

#[test]
fn unrelated_external_changes_survive_reapply() {
    let mut patch = ConfigPatch::default();
    patch.set_bool(&["clock", "seconds"], true);
    let latest = BASE.replace("'obsidian'", "'plum'");
    let result = patch.apply(BASE, &latest, &[]).unwrap();
    assert!(result.contains("'plum'"));
    assert!(zefiro_core::config::parse_config(&result).unwrap().clock.seconds);
}

#[test]
fn same_field_conflicts_require_explicit_choice() {
    let mut patch = ConfigPatch::default();
    patch.set_strings(&["bar", "sections", "start"], &[]);
    let latest = BASE.replace("['zefiro']", "['@autor/custom']");
    let conflicts = patch.apply(BASE, &latest, &[]).unwrap_err();
    assert_eq!(conflicts.fields, [vec!["bar", "sections", "start"]]);
    let keep = patch.apply(BASE, &latest, &[(conflicts.fields[0].clone(), PatchChoice::File)]).unwrap();
    assert!(keep.contains("@autor/custom"));
    let own = patch.apply(BASE, &latest, &[(conflicts.fields[0].clone(), PatchChoice::Draft)]).unwrap();
    assert!(zefiro_core::config::parse_config(&own).unwrap().bar.sections.start.is_empty());
}

#[test]
fn invalid_latest_document_and_invalid_final_config_are_rejected() {
    let mut patch = ConfigPatch::default();
    patch.set_bool(&["clock", "seconds"], true);
    assert!(patch.apply(BASE, "[invalid", &[]).is_err());
    patch.set_strings(&["bar", "sections", "start"], &["clock".into()]);
    assert!(patch.apply(BASE, BASE, &[]).is_err());
}

#[test]
fn semantic_equality_does_not_create_conflict_from_formatting() {
    let mut patch = ConfigPatch::default();
    patch.set_strings(&["bar", "sections", "start"], &[]);
    assert!(patch.apply(BASE, &BASE.replace("['zefiro']", "[ \"zefiro\" ]"), &[]).is_ok());
}
