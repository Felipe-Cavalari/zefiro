use zefiro_core::observability::install_panic_hook;

#[test]
fn panic_writes_a_crash_report() {
    let dir = tempfile::tempdir().unwrap();
    install_panic_hook(dir.path().to_path_buf(), "0.1.0-test");
    let r = std::panic::catch_unwind(|| panic!("falhou de propósito"));
    assert!(r.is_err());

    let reports: Vec<_> = std::fs::read_dir(dir.path()).unwrap().map(|e| e.unwrap().path()).collect();
    assert_eq!(reports.len(), 1, "{reports:?}");
    let name = reports[0].file_name().unwrap().to_string_lossy().to_string();
    assert!(name.starts_with("crash-") && name.ends_with(".txt"), "{name}");
    let body = std::fs::read_to_string(&reports[0]).unwrap();
    assert!(body.contains("0.1.0-test"));
    assert!(body.contains("falhou de propósito"));
    assert!(body.contains("backtrace"));
}
