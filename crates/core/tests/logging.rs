use zefiro_core::observability::init_logging;

#[test]
fn writes_events_to_a_daily_file() {
    let dir = tempfile::tempdir().unwrap();
    let guard = init_logging(dir.path(), "info").unwrap();
    tracing::info!("evento de teste");
    drop(guard); // grava o que estava no buffer

    let mut content = String::new();
    for entry in std::fs::read_dir(dir.path()).unwrap() {
        let path = entry.unwrap().path();
        assert!(path.file_name().unwrap().to_string_lossy().starts_with("zefiro"), "{path:?}");
        content.push_str(&std::fs::read_to_string(path).unwrap());
    }
    assert!(content.contains("evento de teste"));
}
