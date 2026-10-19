//! Projetos recentes do VS Code, portável: lê `workspaceStorage\*\workspace.json` (um por pasta ou
//! workspace aberto), decodifica as URIs e ordena do mais recente ao mais antigo.

use serde_json::Value;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind { Local, Wsl, Remote, Workspace }

#[derive(Debug, Clone, PartialEq)]
pub struct Recent { pub uri: String, pub name: String, pub location: String, pub kind: Kind, pub modified: SystemTime }

/// `%20` → espaço, `%C3%A7` → "ç" (bytes UTF-8).
fn percent_decode(s: &str) -> String {
    let bytes = s.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%'
            && let Some(hex) = s.get(i + 1..i + 3)
            && let Ok(b) = u8::from_str_radix(hex, 16)
        {
            out.push(b);
            i += 3;
        } else {
            out.push(bytes[i]);
            i += 1;
        }
    }
    String::from_utf8_lossy(&out).into_owned()
}

/// Caminho Windows de uma URI `file:///c%3A/...`.
fn local_path(uri: &str) -> Option<PathBuf> {
    let rest = percent_decode(uri.strip_prefix("file:///")?);
    let mut chars = rest.chars();
    let drive = chars.next()?.to_ascii_uppercase();
    Some(PathBuf::from(format!("{drive}{}", chars.as_str().replace('/', "\\"))))
}

/// Caminho Windows de uma pasta do WSL: `vscode-remote://wsl+ubuntu/home/x` →
/// `\\wsl.localhost\ubuntu\home\x`. Outros remotos não têm caminho local.
pub fn wsl_path(uri: &str) -> Option<PathBuf> {
    let rest = uri.strip_prefix("vscode-remote://")?;
    let (authority, path) = rest.split_once('/').unwrap_or((rest, ""));
    let distro = percent_decode(authority).strip_prefix("wsl+")?.to_string();
    Some(PathBuf::from(format!(r"\\wsl.localhost\{distro}\{}", percent_decode(path).replace('/', "\\"))))
}

/// A distro de um caminho `\\wsl.localhost\<distro>\...`, para saber se dá para conferir sem ligar o WSL.
pub fn wsl_distro(path: &Path) -> Option<&str> {
    path.to_str()?.strip_prefix(r"\\wsl.localhost\")?.split('\\').next().filter(|d| !d.is_empty())
}

fn capitalized(s: &str) -> String {
    let mut c = s.chars();
    c.next().map(|f| f.to_uppercase().chain(c).collect()).unwrap_or_default()
}

/// Nome curto, onde fica e o tipo. `None` para esquemas desconhecidos.
pub fn describe(uri: &str, is_workspace: bool) -> Option<(String, String, Kind)> {
    if let Some(path) = local_path(uri) {
        let parent = path.parent().map(|p| p.display().to_string()).unwrap_or_default();
        let file = path.file_name()?.to_string_lossy().into_owned();
        if is_workspace {
            // Workspaces sem título ficam em `Code\Workspaces\<id>\workspace.json`.
            let name = if file == "workspace.json" { "Workspace sem título".to_string() } else { file.trim_end_matches(".code-workspace").to_string() };
            return Some((name, parent, Kind::Workspace));
        }
        return Some((file, parent, Kind::Local));
    }
    let rest = uri.strip_prefix("vscode-remote://")?;
    let (authority, path) = rest.split_once('/').unwrap_or((rest, ""));
    let authority = percent_decode(authority);
    let path = format!("/{}", percent_decode(path));
    let (remote, host) = authority.split_once('+').unwrap_or((authority.as_str(), ""));
    let (parent, name) = path.rsplit_once('/').map(|(p, n)| (if p.is_empty() { "/" } else { p }, n)).unwrap_or(("/", path.as_str()));
    let (label, kind) = if remote == "wsl" { (format!("WSL: {}", capitalized(host)), Kind::Wsl) } else { (format!("{remote}: {host}"), Kind::Remote) };
    let kind = if is_workspace { Kind::Workspace } else { kind };
    let name = if is_workspace { name.trim_end_matches(".code-workspace").to_string() } else { name.to_string() };
    Some((name, format!("{label} · {parent}"), kind))
}

/// `{"folder": uri}` ou `{"workspace": uri}` → (uri, é workspace).
pub fn parse_workspace_json(text: &str) -> Option<(String, bool)> {
    let v: Value = serde_json::from_str(text).ok()?;
    if let Some(folder) = v.get("folder").and_then(Value::as_str) {
        return Some((folder.to_string(), false));
    }
    v.get("workspace").and_then(Value::as_str).map(|w| (w.to_string(), true))
}

/// Quando a entrada foi usada: a pasta dela ou o `state.vscdb` de dentro, o que for mais novo.
fn modified(dir: &Path) -> Option<SystemTime> {
    let folder = std::fs::metadata(dir).and_then(|m| m.modified()).ok();
    let state = std::fs::metadata(dir.join("state.vscdb")).and_then(|m| m.modified()).ok();
    folder.max(state)
}

/// Até `max` projetos de `storage_dir`, do mais recente ao mais antigo. Pastas locais e do WSL
/// (pelo caminho `\\wsl.localhost\...`) para as quais `exists` é falso (apagadas) ficam fora;
/// entradas ilegíveis também. Quem chama decide não acessar o WSL desligado (`wsl_distro`).
pub fn read_recents(storage_dir: &Path, exists: &dyn Fn(&Path) -> bool, max: usize) -> Vec<Recent> {
    let Ok(entries) = std::fs::read_dir(storage_dir) else { return Vec::new() };
    let mut recents: Vec<Recent> = entries
        .flatten()
        .filter_map(|entry| {
            let dir = entry.path();
            let (uri, is_workspace) = parse_workspace_json(&std::fs::read_to_string(dir.join("workspace.json")).ok()?)?;
            if let Some(path) = local_path(&uri).or_else(|| wsl_path(&uri))
                && !exists(&path)
            {
                return None;
            }
            let (name, location, kind) = describe(&uri, is_workspace)?;
            Some(Recent { uri, name, location, kind, modified: modified(&dir)? })
        })
        .collect();
    recents.sort_by_key(|r| std::cmp::Reverse(r.modified));
    recents.dedup_by(|a, b| a.uri == b.uri);
    recents.truncate(max);
    recents
}

/// Argumentos do VS Code para abrir o projeto.
pub fn open_args(r: &Recent) -> Vec<String> {
    let flag = if r.kind == Kind::Workspace { "--file-uri" } else { "--folder-uri" };
    vec![flag.to_string(), r.uri.clone()]
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    #[test]
    fn local_folder_name_and_location() {
        let (name, location, kind) = describe("file:///c%3A/Users/felip/Projects/Gbpa/lumi", false).unwrap();
        assert_eq!((name.as_str(), location.as_str(), kind), ("lumi", r"C:\Users\felip\Projects\Gbpa", Kind::Local));
    }

    #[test]
    fn wsl_folder() {
        let (name, location, kind) = describe("vscode-remote://wsl%2Bubuntu-24.04/home/felip/Projects/garimpo-bot/garimpo-bot-frontend", false).unwrap();
        assert_eq!((name.as_str(), location.as_str(), kind), ("garimpo-bot-frontend", "WSL: Ubuntu-24.04 · /home/felip/Projects/garimpo-bot", Kind::Wsl));
    }

    #[test]
    fn other_remote_folder() {
        let (name, location, kind) = describe("vscode-remote://ssh-remote%2Bdevbox/srv/app", false).unwrap();
        assert_eq!((name.as_str(), location.as_str(), kind), ("app", "ssh-remote: devbox · /srv", Kind::Remote));
    }

    #[test]
    fn workspace_file() {
        let (name, _, kind) = describe("file:///c%3A/Users/felip/Projects/zefiro.code-workspace", true).unwrap();
        assert_eq!((name.as_str(), kind), ("zefiro", Kind::Workspace));
        let (untitled, _, _) = describe("file:///c%3A/Users/felip/AppData/Roaming/Code/Workspaces/1791484386102/workspace.json", true).unwrap();
        assert_eq!(untitled, "Workspace sem título");
    }

    #[test]
    fn decodes_spaces_and_accents() {
        let (name, location, _) = describe("file:///c%3A/Users/felip/Meus%20Projetos/Constru%C3%A7%C3%A3o", false).unwrap();
        assert_eq!(name, "Construção");
        assert_eq!(location, r"C:\Users\felip\Meus Projetos");
    }

    #[test]
    fn parses_workspace_json() {
        assert_eq!(parse_workspace_json(r#"{"folder": "file:///c%3A/x"}"#), Some(("file:///c%3A/x".to_string(), false)));
        assert_eq!(parse_workspace_json(r#"{"workspace": "file:///c%3A/w.code-workspace"}"#), Some(("file:///c%3A/w.code-workspace".to_string(), true)));
        assert_eq!(parse_workspace_json("{}"), None);
        assert_eq!(parse_workspace_json("lixo"), None);
    }

    fn storage(entries: &[(&str, &str)]) -> std::path::PathBuf {
        // Uma pasta por chamada: os testes rodam em paralelo.
        static NEXT: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
        let n = NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let dir = std::env::temp_dir().join(format!("zefiro-vscode-{}-{n}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        for (hash, json) in entries {
            std::fs::create_dir_all(dir.join(hash)).unwrap();
            std::fs::write(dir.join(hash).join("workspace.json"), json).unwrap();
            std::thread::sleep(std::time::Duration::from_millis(30));
        }
        dir
    }

    #[test]
    fn deleted_local_folder_is_skipped() {
        let dir = storage(&[("a", r#"{"folder": "file:///c%3A/existe"}"#), ("b", r#"{"folder": "file:///c%3A/apagada"}"#)]);
        let exists = |p: &Path| p.ends_with("existe");
        let names: Vec<String> = read_recents(&dir, &exists, 20).into_iter().map(|r| r.name).collect();
        assert_eq!(names, vec!["existe"]);
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn wsl_folder_is_checked_through_its_windows_path() {
        assert_eq!(wsl_path("vscode-remote://wsl%2Bubuntu-24.04/home/felip/Projects/a%20b"), Some(PathBuf::from(r"\\wsl.localhost\ubuntu-24.04\home\felip\Projects\a b")));
        assert_eq!(wsl_path("vscode-remote://ssh-remote%2Bdevbox/srv"), None, "só WSL tem caminho no Windows");
        assert_eq!(wsl_distro(Path::new(r"\\wsl.localhost\ubuntu-24.04\home\x")), Some("ubuntu-24.04"));
        assert_eq!(wsl_distro(Path::new(r"C:\x")), None);
        let dir = storage(&[("a", r#"{"folder": "vscode-remote://wsl%2Bubuntu-24.04/home/existe"}"#), ("b", r#"{"folder": "vscode-remote://wsl%2Bubuntu-24.04/home/apagada"}"#)]);
        let exists = |p: &Path| p.ends_with("existe");
        let names: Vec<String> = read_recents(&dir, &exists, 20).into_iter().map(|r| r.name).collect();
        assert_eq!(names, vec!["existe"]);
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn orders_by_modified_and_caps_at_max() {
        let dir = storage(&[
            ("a", r#"{"folder": "file:///c%3A/um"}"#),
            ("b", r#"{"folder": "vscode-remote://wsl%2Bubuntu-24.04/home/dois"}"#),
            ("c", r#"{"folder": "file:///c%3A/tres"}"#),
            ("d", "quebrado"),
        ]);
        let names: Vec<String> = read_recents(&dir, &|_: &Path| true, 2).into_iter().map(|r| r.name).collect();
        assert_eq!(names, vec!["tres", "dois"]);
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn open_args_for_folder_and_workspace() {
        let folder = Recent { uri: "file:///c%3A/x".into(), name: "x".into(), location: String::new(), kind: Kind::Local, modified: std::time::UNIX_EPOCH };
        assert_eq!(open_args(&folder), vec!["--folder-uri".to_string(), "file:///c%3A/x".into()]);
        let ws = Recent { kind: Kind::Workspace, ..folder };
        assert_eq!(open_args(&ws)[0], "--file-uri");
    }
}
