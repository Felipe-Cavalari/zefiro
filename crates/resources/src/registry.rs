//! Temas embutidos + temas do usuário (`%APPDATA%\zefiro\themes\*.toml`).

use std::path::{Path, PathBuf};

use crate::theme::{BUILTIN, Theme, ThemeError, parse_theme};

#[derive(Debug, Clone, PartialEq)]
pub struct ThemeEntry { pub theme: Theme, pub user: bool }

#[derive(Debug, Clone, Default)]
pub struct ThemeRegistry { entries: Vec<ThemeEntry> }

fn builtin_entries() -> Vec<ThemeEntry> {
    BUILTIN
        .iter()
        .map(|(id, text)| ThemeEntry { theme: parse_theme(id, id, text).expect("tema embutido válido"), user: false })
        .collect()
}

fn read_user_dir(dir: &Path) -> Vec<(String, Result<Theme, ThemeError>)> {
    let mut files: Vec<PathBuf> = std::fs::read_dir(dir)
        .map(|rd| rd.filter_map(|e| e.ok().map(|e| e.path())).collect())
        .unwrap_or_default();
    files.retain(|p| p.extension().is_some_and(|e| e.eq_ignore_ascii_case("toml")));
    files.sort();
    files
        .into_iter()
        .map(|p| {
            let id = p.file_stem().unwrap_or_default().to_string_lossy().to_lowercase();
            let source = p.display().to_string();
            let parsed = std::fs::read_to_string(&p)
                .map_err(|e| ThemeError { source: source.clone(), line: None, message: e.to_string() })
                .and_then(|text| parse_theme(&id, &source, &text));
            (id, parsed)
        })
        .collect()
}

impl ThemeRegistry {
    pub fn load(user_dir: Option<&Path>) -> (ThemeRegistry, Vec<ThemeError>) {
        ThemeRegistry::default().reload_keeping_previous(user_dir)
    }

    /// Relê a pasta do usuário. Um arquivo com erro mantém a versão anterior daquele tema, se havia.
    pub fn reload_keeping_previous(&self, user_dir: Option<&Path>) -> (ThemeRegistry, Vec<ThemeError>) {
        let mut entries = builtin_entries();
        let mut errors = Vec::new();
        let mut user: Vec<ThemeEntry> = Vec::new();
        if let Some(dir) = user_dir {
            for (id, parsed) in read_user_dir(dir) {
                match parsed {
                    Ok(theme) => user.push(ThemeEntry { theme, user: true }),
                    Err(e) => {
                        errors.push(e);
                        if let Some(prev) = self.entries.iter().find(|x| x.user && x.theme.id == id) {
                            user.push(prev.clone());
                        }
                    }
                }
            }
        }
        for u in user {
            if let Some(slot) = entries.iter_mut().find(|e| e.theme.id == u.theme.id) {
                *slot = u;
            } else {
                entries.push(u);
            }
        }
        entries[BUILTIN.len()..].sort_by(|a, b| a.theme.id.cmp(&b.theme.id));
        (ThemeRegistry { entries }, errors)
    }

    pub fn get(&self, id: &str) -> Option<&Theme> {
        self.entries.iter().find(|e| e.theme.id == id).map(|e| &e.theme)
    }

    pub fn list(&self) -> &[ThemeEntry] {
        &self.entries
    }
}

/// Copia o arquivo de um tema embutido para `dir` (cria a pasta). Não sobrescreve.
pub fn export_builtin(id: &str, dir: &Path) -> std::io::Result<PathBuf> {
    let (_, text) = BUILTIN
        .iter()
        .find(|(i, _)| *i == id)
        .ok_or_else(|| std::io::Error::new(std::io::ErrorKind::NotFound, format!("tema embutido {id} não existe")))?;
    std::fs::create_dir_all(dir)?;
    let path = dir.join(format!("{id}.toml"));
    if !path.exists() {
        std::fs::write(&path, text)?;
    }
    Ok(path)
}

#[cfg(test)]
mod tests {
    use super::*;

    const MINI: &str = "name = \"Meu\"\n[colors]\nbackground = \"#101010\"\ntext = \"#fff\"\ntext_secondary = \"#888\"\n";

    #[test]
    fn builtins_only_without_user_dir() {
        let (r, errors) = ThemeRegistry::load(None);
        assert!(errors.is_empty());
        assert_eq!(r.list().len(), 8);
        assert_eq!(r.list()[0].theme.id, "obsidian");
        assert!(r.get("porcelain").is_some());
        assert!(r.get("nao-existe").is_none());
    }

    #[test]
    fn user_theme_is_added_and_can_override_builtin() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("meu.toml"), MINI).unwrap();
        std::fs::write(dir.path().join("obsidian.toml"), MINI.replace("Meu", "Obsidian do usuário")).unwrap();
        std::fs::write(dir.path().join("leia-me.txt"), "ignorado").unwrap();
        let (r, errors) = ThemeRegistry::load(Some(dir.path()));
        assert!(errors.is_empty(), "{errors:?}");
        assert_eq!(r.list().len(), 9);
        assert_eq!(r.get("obsidian").unwrap().name, "Obsidian do usuário");
        assert!(r.list().iter().any(|e| e.user && e.theme.id == "meu"));
    }

    #[test]
    fn broken_user_theme_keeps_previous_and_reports_line() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("meu.toml");
        std::fs::write(&path, MINI).unwrap();
        let (first, _) = ThemeRegistry::load(Some(dir.path()));
        std::fs::write(&path, MINI.replace("#101010", "preto")).unwrap();
        let (second, errors) = first.reload_keeping_previous(Some(dir.path()));
        assert_eq!(errors.len(), 1);
        assert_eq!(errors[0].line, Some(3));
        assert!(errors[0].source.ends_with("meu.toml"), "{}", errors[0].source);
        assert_eq!(second.get("meu").unwrap().colors.background, first.get("meu").unwrap().colors.background);
    }

    #[test]
    fn export_copies_builtin_without_overwriting() {
        let dir = tempfile::tempdir().unwrap();
        let p = export_builtin("nord", dir.path()).unwrap();
        assert!(std::fs::read_to_string(&p).unwrap().contains("name = \"Nord\""));
        std::fs::write(&p, "editado").unwrap();
        export_builtin("nord", dir.path()).unwrap();
        assert_eq!(std::fs::read_to_string(&p).unwrap(), "editado");
        assert!(export_builtin("nao-existe", dir.path()).is_err());
    }
}
