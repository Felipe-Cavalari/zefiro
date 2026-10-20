//! Perfis do Windows Terminal, portável: lê o `settings.json` e os fragmentos (texto → perfis),
//! aplica a ordem e os ocultos escolhidos no Zefiro e monta os argumentos do `wt.exe`. O Zefiro
//! nunca grava nos arquivos do Terminal.

pub mod jsonc;

use serde_json::{Map, Value};
use std::path::{Path, PathBuf};

/// De onde vem o ícone de um perfil.
#[derive(Debug, Clone, PartialEq)]
pub enum IconSource {
    /// Arquivo de imagem no disco.
    File(PathBuf),
    /// Caminho dentro do pacote do Terminal (`ms-appx:///ProfileIcons/pwsh.png` → `ProfileIcons/pwsh.png`).
    Package(String),
    /// Emoji, glifo ou nada: o Zefiro tenta o ícone do pacote pelo `guid` e depois o glifo.
    None,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Profile {
    /// Minúsculas, com chaves.
    pub guid: String,
    pub name: String,
    pub icon: IconSource,
    /// É o `defaultProfile` do Terminal.
    pub default: bool,
}

/// Perfis visíveis no Terminal, na ordem dele. `fragments` são pares (fonte, texto), em que a fonte é
/// o nome da pasta do fragmento (ex.: `Microsoft.WSL`). `env` expande `%VAR%` nos ícones.
pub fn parse_profiles(settings: &str, fragments: &[(&str, &str)], env: &dyn Fn(&str) -> Option<String>) -> Result<Vec<Profile>, String> {
    let root: Value = serde_json::from_str(&jsonc::strip(settings)).map_err(|e| e.to_string())?;
    let disabled: Vec<&str> = root["disabledProfileSources"].as_array().map(|a| a.iter().filter_map(Value::as_str).collect()).unwrap_or_default();
    let list = match &root["profiles"] {
        Value::Array(list) => list.as_slice(),
        other => other["list"].as_array().map_or(&[][..], Vec::as_slice),
    };
    let mut entries: Vec<(String, Map<String, Value>)> = list
        .iter()
        .filter_map(|p| Some((p["guid"].as_str()?.to_lowercase(), p.as_object()?.clone())))
        .collect();

    // Fragmentos completam o que o `settings.json` não diz; um fragmento ilegível é pulado.
    for (source, text) in fragments.iter().filter(|(source, _)| !disabled.contains(source)) {
        let Ok(fragment) = serde_json::from_str::<Value>(&jsonc::strip(text)) else { continue };
        for p in fragment["profiles"].as_array().into_iter().flatten() {
            let Some(fields) = p.as_object() else { continue };
            let target = p["updates"].as_str().map(str::to_lowercase);
            let key = target.clone().or_else(|| p["guid"].as_str().map(str::to_lowercase));
            let Some(key) = key else { continue };
            match entries.iter_mut().find(|(guid, _)| *guid == key) {
                Some((_, entry)) => {
                    for (k, v) in fields.iter().filter(|(k, _)| k.as_str() != "updates") {
                        entry.entry(k.clone()).or_insert_with(|| v.clone());
                    }
                }
                None if target.is_none() => {
                    let mut entry = fields.clone();
                    entry.entry("source").or_insert_with(|| Value::from(*source));
                    entries.push((key, entry));
                }
                None => {}
            }
        }
    }

    let default = root["defaultProfile"].as_str().unwrap_or_default();
    Ok(entries
        .into_iter()
        .filter(|(_, e)| e.get("hidden").and_then(Value::as_bool) != Some(true))
        .filter(|(_, e)| !e.get("source").and_then(Value::as_str).is_some_and(|s| disabled.contains(&s)))
        .filter_map(|(guid, e)| {
            let name = e.get("name")?.as_str()?.to_string();
            let icon = e.get("icon").and_then(Value::as_str).map_or(IconSource::None, |i| icon_source(&expand(i, env)));
            let default = guid == default.to_lowercase() || name == default;
            Some(Profile { guid, name, icon, default })
        })
        .collect())
}

/// (visíveis na ordem escolhida, ocultos na ordem do Terminal), índices em `profiles`. Perfis fora
/// de `order` vão para o fim dos visíveis; perfil novo aparece sozinho.
pub fn arrange(profiles: &[Profile], order: &[String], hidden: &[String]) -> (Vec<usize>, Vec<usize>) {
    let has = |list: &[String], guid: &str| list.iter().any(|g| g.eq_ignore_ascii_case(guid));
    let (mut visible, hidden): (Vec<usize>, Vec<usize>) = (0..profiles.len()).partition(|&i| !has(hidden, &profiles[i].guid));
    let rank = |i: &usize| order.iter().position(|g| g.eq_ignore_ascii_case(&profiles[*i].guid)).unwrap_or(usize::MAX);
    visible.sort_by_key(rank);
    (visible, hidden)
}

/// (`order`, `hidden`) para gravar. Os `guid`s de `previous_hidden` que `profiles` não conhece (distro
/// desinstalada) continuam ocultos, para a escolha voltar com o perfil.
pub fn saved_lists(profiles: &[Profile], visible: &[usize], hidden: &[usize], previous_hidden: &[String]) -> (Vec<String>, Vec<String>) {
    let guids = |list: &[usize]| list.iter().map(|&i| profiles[i].guid.clone()).collect::<Vec<_>>();
    let mut hidden = guids(hidden);
    let unknown = previous_hidden.iter().map(|g| g.to_lowercase()).filter(|g| !profiles.iter().any(|p| p.guid == *g));
    for guid in unknown {
        if !hidden.contains(&guid) {
            hidden.push(guid);
        }
    }
    (guids(visible), hidden)
}

/// Arquivos a tentar para o ícone, em ordem; vazio = glifo de terminal.
pub fn icon_candidates(profile: &Profile, package_dir: Option<&Path>) -> Vec<PathBuf> {
    // `ProfileIcons/pwsh.png` → as variantes de escala que o pacote traz e depois o nome puro.
    let scaled = |dir: &Path, relative: &str| -> Vec<PathBuf> {
        let (stem, ext) = relative.rsplit_once('.').unwrap_or((relative, "png"));
        vec![dir.join(format!("{stem}.scale-200.{ext}")), dir.join(format!("{stem}.scale-100.{ext}")), dir.join(relative)]
    };
    let mut out = match (&profile.icon, package_dir) {
        (IconSource::File(path), _) => vec![path.clone()],
        (IconSource::Package(relative), Some(dir)) => scaled(dir, relative),
        _ => Vec::new(),
    };
    if let Some(dir) = package_dir {
        out.extend(scaled(dir, &format!("ProfileIcons/{}.png", profile.guid)));
    }
    out
}

/// Argumentos do `wt.exe` para abrir o perfil.
pub fn open_args(profile: &Profile) -> Vec<String> {
    vec!["-p".into(), profile.guid.clone()]
}

#[derive(Debug, Clone, PartialEq)]
pub enum LoadError {
    NotInstalled,
    Unreadable(String),
}

/// Lê o primeiro `settings.json` que existir (Stable, Preview, instalação sem pacote) e os
/// fragmentos de `<local>` e `<program_data>`. Bloqueante: fora da thread de UI.
pub fn read_profiles(local_app_data: &Path, program_data: &Path) -> Result<Vec<Profile>, LoadError> {
    let packaged = |family: &str| local_app_data.join("Packages").join(family).join("LocalState").join("settings.json");
    let candidates = [packaged("Microsoft.WindowsTerminal_8wekyb3d8bbwe"), packaged("Microsoft.WindowsTerminalPreview_8wekyb3d8bbwe"), terminal_dir(local_app_data).join("settings.json")];
    let path = candidates.iter().find(|p| p.is_file()).ok_or(LoadError::NotInstalled)?;
    let settings = std::fs::read_to_string(path).map_err(|e| LoadError::Unreadable(e.to_string()))?;
    let fragments: Vec<(String, String)> = [local_app_data, program_data].iter().flat_map(|root| read_fragments(&terminal_dir(root).join("Fragments"))).collect();
    let borrowed: Vec<(&str, &str)> = fragments.iter().map(|(s, t)| (s.as_str(), t.as_str())).collect();
    parse_profiles(&settings, &borrowed, &|name| std::env::var(name).ok()).map_err(LoadError::Unreadable)
}

fn terminal_dir(root: &Path) -> PathBuf {
    root.join("Microsoft").join("Windows Terminal")
}

/// (nome da pasta, texto) de cada `Fragments\<fonte>\*.json`, em ordem de caminho.
fn read_fragments(root: &Path) -> Vec<(String, String)> {
    let sorted = |dir: &Path| -> Vec<PathBuf> {
        let mut paths: Vec<PathBuf> = std::fs::read_dir(dir).into_iter().flatten().flatten().map(|e| e.path()).collect();
        paths.sort();
        paths
    };
    let mut out = Vec::new();
    for source in sorted(root).into_iter().filter(|p| p.is_dir()) {
        let name = source.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
        for file in sorted(&source).into_iter().filter(|p| p.extension().is_some_and(|e| e.eq_ignore_ascii_case("json"))) {
            if let Ok(text) = std::fs::read_to_string(&file) {
                out.push((name.clone(), text));
            }
        }
    }
    out
}

/// Troca cada `%VAR%` conhecida pelo valor; as desconhecidas ficam como estão.
fn expand(text: &str, env: &dyn Fn(&str) -> Option<String>) -> String {
    let mut out = String::new();
    let mut rest = text;
    while let Some(start) = rest.find('%') {
        let Some(len) = rest[start + 1..].find('%') else { break };
        let name = &rest[start + 1..start + 1 + len];
        out.push_str(&rest[..start]);
        match env(name) {
            Some(value) => out.push_str(&value),
            None => out.push_str(&rest[start..start + len + 2]),
        }
        rest = &rest[start + len + 2..];
    }
    out.push_str(rest);
    out
}

fn icon_source(icon: &str) -> IconSource {
    let lower = icon.to_lowercase();
    const APPX: &str = "ms-appx:///";
    if lower.starts_with(APPX) {
        return IconSource::Package(icon[APPX.len()..].to_string());
    }
    let bytes = icon.as_bytes();
    let absolute = icon.starts_with(r"\\") || (bytes.len() > 2 && bytes[1] == b':' && matches!(bytes[2], b'\\' | b'/'));
    let image = [".png", ".ico", ".jpg", ".jpeg", ".bmp"].iter().any(|ext| lower.ends_with(ext));
    if absolute && image { IconSource::File(PathBuf::from(icon)) } else { IconSource::None }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SETTINGS: &str = include_str!("../tests/fixtures/settings.json");
    const WSL: &str = include_str!("../tests/fixtures/wsl-fragment.json");

    fn no_env(_: &str) -> Option<String> {
        None
    }

    fn names(p: &[Profile]) -> Vec<&str> {
        p.iter().map(|p| p.name.as_str()).collect()
    }

    #[test]
    fn user_settings_give_the_taskbar_profiles() {
        let p = parse_profiles(SETTINGS, &[("Microsoft.WSL", WSL)], &no_env).unwrap();
        assert_eq!(names(&p), ["PowerShell", "Windows PowerShell", "Prompt de comando", "Ubuntu-24.04"], "o Azure vem de uma fonte desativada");
        assert!(p.iter().all(|p| p.guid != "{963ff2f7-6aed-5ce3-9d91-90d99571f53a}"), "a cópia antiga do Ubuntu está oculta pelo fragmento");
        assert_eq!(p.iter().filter(|p| p.default).map(|p| p.name.as_str()).collect::<Vec<_>>(), ["Ubuntu-24.04"]);
        assert_eq!(p[3].icon, IconSource::File(r"C:\Users\teste\AppData\Local\wsl\{54e7dab8-3953-4a07-9e15-9c58dc528819}\shortcut.ico".into()));
    }

    #[test]
    fn disabled_sources_hide_their_profiles_and_fragments() {
        let s = r#"{"disabledProfileSources":["Windows.Terminal.Azure","Meu.Fragmento"],
            "profiles":[{"guid":"{a}","name":"Azure","source":"Windows.Terminal.Azure"},{"guid":"{b}","name":"Cmd"}]}"#;
        let f = r#"{"profiles":[{"guid":"{c}","name":"Do fragmento"}]}"#;
        let p = parse_profiles(s, &[("Meu.Fragmento", f)], &no_env).unwrap();
        assert_eq!(names(&p), ["Cmd"]);
    }

    #[test]
    fn object_form_and_uppercase_guids() {
        let s = r#"{"defaultProfile":"Cmd","profiles":{"defaults":{},"list":[{"guid":"{AB-CD}","name":"Cmd"}]}}"#;
        let p = parse_profiles(s, &[], &no_env).unwrap();
        assert_eq!(p[0].guid, "{ab-cd}");
        assert!(p[0].default, "defaultProfile pelo nome");
    }

    #[test]
    fn settings_win_over_fragment_and_fragment_only_goes_last() {
        let s = r#"{"profiles":[{"guid":"{a}","name":"Meu"}]}"#;
        let f = r#"{"profiles":[{"updates":"{A}","name":"Do fragmento","icon":"C:\\i.png"},{"guid":"{b}","name":"Novo"},{"name":"Sem guid"}]}"#;
        let p = parse_profiles(s, &[("X", f)], &no_env).unwrap();
        assert_eq!((p[0].name.as_str(), &p[0].icon), ("Meu", &IconSource::File(r"C:\i.png".into())));
        assert_eq!(names(&p), ["Meu", "Novo"]);
    }

    #[test]
    fn icon_sources() {
        let env = |k: &str| (k == "USERPROFILE").then(|| r"C:\Users\t".to_string());
        let s = r#"{"profiles":[{"guid":"{1}","name":"a","icon":"ms-appx:///ProfileIcons/pwsh.png"},
            {"guid":"{2}","name":"b","icon":"%USERPROFILE%\\x.ico"},{"guid":"{3}","name":"c","icon":"🐧"},{"guid":"{4}","name":"d"}]}"#;
        let p = parse_profiles(s, &[], &env).unwrap();
        assert_eq!(p[0].icon, IconSource::Package("ProfileIcons/pwsh.png".into()));
        assert_eq!(p[1].icon, IconSource::File(r"C:\Users\t\x.ico".into()));
        assert_eq!((&p[2].icon, &p[3].icon), (&IconSource::None, &IconSource::None));
    }

    #[test]
    fn broken_json_is_an_error() {
        assert!(parse_profiles("{", &[], &no_env).is_err());
    }

    fn p(guid: &str) -> Profile {
        Profile { guid: guid.into(), name: guid.into(), icon: IconSource::None, default: false }
    }

    fn strings(v: &[&str]) -> Vec<String> {
        v.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn new_profiles_go_last_and_hidden_follow_terminal_order() {
        let ps = [p("{a}"), p("{b}"), p("{c}"), p("{d}")];
        let arranged = arrange(&ps, &strings(&["{C}", "{a}"]), &strings(&["{d}", "{b}"]));
        assert_eq!(arranged, (vec![2, 0], vec![1, 3]));
        let ps = [p("{a}"), p("{b}"), p("{c}")];
        assert_eq!(arrange(&ps, &strings(&["{c}"]), &[]), (vec![2, 0, 1], vec![]), "perfis novos no fim, na ordem do Terminal");
    }

    #[test]
    fn no_section_shows_everything() {
        assert_eq!(arrange(&[p("{a}"), p("{b}")], &[], &[]), (vec![0, 1], vec![]));
    }

    #[test]
    fn unknown_hidden_guids_survive_saving() {
        let ps = [p("{a}"), p("{b}")];
        let (order, hidden) = saved_lists(&ps, &[1], &[0], &strings(&["{gone}", "{A}"]));
        assert_eq!(order, ["{b}"]);
        assert_eq!(hidden, ["{a}", "{gone}"]);
    }

    #[test]
    fn icon_candidates_in_order() {
        let dir = Path::new(r"C:\WT");
        let mut q = p("{x}");
        q.icon = IconSource::Package("ProfileIcons/pwsh.png".into());
        let expected: Vec<PathBuf> = ["ProfileIcons/pwsh.scale-200.png", "ProfileIcons/pwsh.scale-100.png", "ProfileIcons/pwsh.png", "ProfileIcons/{x}.scale-200.png", "ProfileIcons/{x}.scale-100.png", "ProfileIcons/{x}.png"]
            .iter()
            .map(|f| dir.join(f))
            .collect();
        assert_eq!(icon_candidates(&q, Some(dir)), expected);
        let mut f = p("{x}");
        f.icon = IconSource::File(r"C:\i.ico".into());
        assert_eq!(icon_candidates(&f, None), [PathBuf::from(r"C:\i.ico")]);
        assert!(icon_candidates(&p("{x}"), None).is_empty(), "emoji ou nada, sem pacote: glifo");
    }

    #[test]
    fn opens_by_guid() {
        assert_eq!(open_args(&p("{a}")), ["-p", "{a}"]);
    }

    #[test]
    fn reads_stable_settings_and_fragments_from_disk() {
        let local = tempfile::tempdir().unwrap();
        let program = tempfile::tempdir().unwrap();
        let state = local.path().join("Packages").join("Microsoft.WindowsTerminal_8wekyb3d8bbwe").join("LocalState");
        std::fs::create_dir_all(&state).unwrap();
        std::fs::write(state.join("settings.json"), SETTINGS).unwrap();
        let wsl = terminal_dir(local.path()).join("Fragments").join("Microsoft.WSL");
        std::fs::create_dir_all(&wsl).unwrap();
        std::fs::write(wsl.join("ubuntu.json"), WSL).unwrap();
        let p = read_profiles(local.path(), program.path()).unwrap();
        assert_eq!(names(&p), ["PowerShell", "Windows PowerShell", "Prompt de comando", "Ubuntu-24.04"]);
        assert!(matches!(p[3].icon, IconSource::File(_)), "o ícone do Ubuntu vem do fragmento");
    }

    #[test]
    fn missing_terminal_and_unreadable_settings() {
        let local = tempfile::tempdir().unwrap();
        assert_eq!(read_profiles(local.path(), local.path()), Err(LoadError::NotInstalled));
        let plain = terminal_dir(local.path());
        std::fs::create_dir_all(&plain).unwrap();
        std::fs::write(plain.join("settings.json"), "{").unwrap();
        assert!(matches!(read_profiles(local.path(), local.path()), Err(LoadError::Unreadable(_))));
    }
}
