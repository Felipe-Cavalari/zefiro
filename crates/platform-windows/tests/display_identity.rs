use zefiro_platform_windows::monitors;

#[test]
fn attached_displays_have_distinct_persistent_paths_when_available() {
    let displays = monitors::list();
    assert!(!displays.is_empty());
    let ids: Vec<_> = displays.iter().filter_map(|d| d.identity.id.as_deref()).collect();
    let unique: std::collections::HashSet<_> = ids.iter().copied().collect();
    assert_eq!(unique.len(), ids.len());
    for display in displays {
        assert!(!display.identity.name.is_empty());
        if let Some(id) = display.identity.id {
            assert!(!id.starts_with(r"\\.\DISPLAY"));
            assert!(id.contains('#'), "caminho de interface esperado: {id}");
        }
    }
}
