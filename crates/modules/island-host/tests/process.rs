use std::path::Path;
use zefiro_island_host::process::resolve_exe;

#[test]
fn resolves_configured_path_or_beside_the_bar() {
    let dir = tempfile::tempdir().unwrap();
    assert_eq!(resolve_exe("", dir.path()), None);
    let beside = dir.path().join("zefiro-island.exe");
    std::fs::write(&beside, b"").unwrap();
    assert_eq!(resolve_exe("", dir.path()), Some(beside.clone()));
    let other = dir.path().join("outra.exe");
    std::fs::write(&other, b"").unwrap();
    assert_eq!(resolve_exe(other.to_str().unwrap(), Path::new("C:\\nao-existe")), Some(other));
    assert_eq!(resolve_exe("C:\\nao-existe\\x.exe", dir.path()), None);
}
