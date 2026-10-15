use std::sync::mpsc;
use std::time::Duration;
use zefiro_core::watch::watch_paths;

#[test]
fn several_quick_writes_become_one_notification() {
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("config.toml");
    std::fs::write(&file, "a").unwrap();
    let (tx, rx) = mpsc::channel();
    let _w = watch_paths(&[dir.path().to_path_buf()], Duration::from_millis(150), move |paths| {
        tx.send(paths).unwrap();
    })
    .unwrap();
    std::thread::sleep(Duration::from_millis(100));
    for i in 0..5 {
        std::fs::write(&file, format!("b{i}")).unwrap();
        std::thread::sleep(Duration::from_millis(20));
    }
    let first = rx.recv_timeout(Duration::from_secs(3)).expect("nenhuma notificação");
    assert!(first.iter().any(|p| p.ends_with("config.toml")), "{first:?}");
    assert!(rx.recv_timeout(Duration::from_millis(400)).is_err(), "notificou mais de uma vez");
}
