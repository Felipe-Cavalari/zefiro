use zefiro_integration_update::{Version, parse_latest, sha256_hex, trusted_url, update_for, verify};

const LATEST: &str = include_str!("fixtures/latest-v1.0.1.json");

#[test]
fn versions_parse_with_or_without_v_and_compare_numerically() {
    assert_eq!(Version::parse("v1.0.1"), Some(Version(1, 0, 1)));
    assert_eq!(Version::parse("1.10.0"), Some(Version(1, 10, 0)));
    assert_eq!(Version::parse("2.0"), Some(Version(2, 0, 0)));
    assert_eq!(Version::parse("v1.2.3-beta"), None, "pré-release não conta");
    assert_eq!(Version::parse("abc"), None);
    assert!(Version(1, 10, 0) > Version(1, 9, 9));
    assert_eq!(Version(1, 1, 0).to_string(), "1.1.0");
}

#[test]
fn reads_the_real_latest_release() {
    let r = parse_latest(LATEST).unwrap();
    assert_eq!(r.version, Version(1, 0, 1));
    assert_eq!(r.asset.name, "Zefiro-1.0.1-setup.exe");
    assert_eq!(r.asset.size, 3_629_679);
    assert_eq!(r.asset.sha256.as_deref(), Some("052905f828886c2b05fedcb8a7cf14fe28577617cb7ab267406114c7b561f9e1"));
    assert!(trusted_url(&r.asset.url));
}

#[test]
fn offers_only_newer_versions() {
    assert!(update_for("1.0.0", LATEST).is_some());
    assert!(update_for("1.0.1", LATEST).is_none(), "mesma versão");
    assert!(update_for("1.1.0", LATEST).is_none(), "o build local é mais novo");
}

#[test]
fn releases_without_setup_drafts_or_bad_tags_are_ignored() {
    let no_setup = LATEST.replace("Zefiro-1.0.1-setup.exe\",", "notas.txt\",");
    assert!(parse_latest(&no_setup).is_err());
    let draft = LATEST.replace("\"draft\": false", "\"draft\": true");
    assert!(parse_latest(&draft).is_err());
    let beta = LATEST.replace("\"tag_name\": \"v1.0.1\"", "\"tag_name\": \"v9.0.0-beta\"");
    assert!(parse_latest(&beta).is_err());
    assert!(parse_latest("<html>limite</html>").is_err());
}

#[test]
fn only_this_repository_release_downloads_are_trusted() {
    assert!(trusted_url("https://github.com/Felipe-Cavalari/zefiro/releases/download/v1.1.0/Zefiro-1.1.0-setup.exe"));
    assert!(!trusted_url("http://github.com/Felipe-Cavalari/zefiro/releases/download/v1.1.0/x.exe"), "sem https");
    assert!(!trusted_url("https://github.com/outra/zefiro/releases/download/v1.1.0/x.exe"));
    assert!(!trusted_url("https://evil.example/Felipe-Cavalari/zefiro/releases/download/x.exe"));
    let moved = LATEST.replace("https://github.com/Felipe-Cavalari/zefiro/releases/download", "https://example.com/dl");
    assert!(parse_latest(&moved).is_err(), "asset fora do repositório não vira atualização");
}

#[test]
fn sha256_matches_the_standard_vectors() {
    assert_eq!(sha256_hex(b""), "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855");
    assert_eq!(sha256_hex(b"abc"), "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad");
    let long = vec![b'a'; 1_000_000];
    assert_eq!(sha256_hex(&long), "cdc76e5c9914fb9281a1c7e284d73e67f1809a48a497200e046d39ccc7112cd0");
}

#[test]
fn verify_checks_size_and_hash() {
    let mut r = parse_latest(LATEST).unwrap();
    r.asset.size = 3;
    r.asset.sha256 = Some(sha256_hex(b"abc"));
    assert!(verify(b"abc", &r.asset).is_ok());
    assert!(verify(b"abd", &r.asset).is_err(), "hash diferente");
    assert!(verify(b"abcd", &r.asset).is_err(), "tamanho diferente");
    r.asset.sha256 = None;
    assert!(verify(b"abc", &r.asset).is_ok(), "sem digest, só o tamanho");
}
