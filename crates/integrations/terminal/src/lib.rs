//! Perfis do Windows Terminal, portável: lê o `settings.json` e os fragmentos (texto → perfis),
//! aplica a ordem e os ocultos escolhidos no Zefiro e monta os argumentos do `wt.exe`. O Zefiro
//! nunca grava nos arquivos do Terminal.

pub mod jsonc;

use serde_json::{Map, Value};
use std::path::PathBuf;

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
}
