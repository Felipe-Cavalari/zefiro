//! `%APPDATA%\zefiro\config.toml`: leitura com padrões, criação e edição sem perder comentários.

use serde::Deserialize;
use std::path::{Path, PathBuf};

pub const DEFAULT_CONFIG: &str = r#"# Zefiro — configuração da barra. Salvar aplica na hora.
theme = "obsidian"

[bar]
height = "completa"   # completa | compacta
format = "faixa"      # faixa | pilulas

[bar.sections]
start = ["zefiro"]
end   = ["clock"]
# o centro é a ilha (monitor principal)

[island]
enabled = true
exe     = ""          # vazio = ao lado do zefiro.exe

[clock]
format  = ""          # vazio = formato do Windows; ou um padrão de data do Windows, ex.: "ddd d MMM"
seconds = false
"#;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum BarHeight { Completa, Compacta }

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum BarFormat { Faixa, Pilulas }

impl BarHeight {
    pub fn as_str(self) -> &'static str {
        match self {
            BarHeight::Completa => "completa",
            BarHeight::Compacta => "compacta",
        }
    }
}

impl BarFormat {
    pub fn as_str(self) -> &'static str {
        match self {
            BarFormat::Faixa => "faixa",
            BarFormat::Pilulas => "pilulas",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(default)]
pub struct Config { pub theme: String, pub bar: BarConfig, pub island: IslandConfig, pub clock: ClockConfig }

#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(default)]
pub struct BarConfig { pub height: BarHeight, pub format: BarFormat, pub sections: Sections }

#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(default)]
pub struct Sections { pub start: Vec<String>, pub end: Vec<String> }

#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(default)]
pub struct IslandConfig { pub enabled: bool, pub exe: String }

#[derive(Debug, Clone, PartialEq, Deserialize, Default)]
#[serde(default)]
pub struct ClockConfig { pub format: String, pub seconds: bool }

impl Default for Config {
    fn default() -> Self {
        Config { theme: "obsidian".into(), bar: BarConfig::default(), island: IslandConfig::default(), clock: ClockConfig::default() }
    }
}

impl Default for BarConfig {
    fn default() -> Self {
        BarConfig { height: BarHeight::Completa, format: BarFormat::Faixa, sections: Sections::default() }
    }
}

impl Default for Sections {
    fn default() -> Self {
        Sections { start: vec!["zefiro".into()], end: vec!["clock".into()] }
    }
}

impl Default for IslandConfig {
    fn default() -> Self {
        IslandConfig { enabled: true, exe: String::new() }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct ConfigError { pub line: Option<usize>, pub message: String }

pub fn parse_config(text: &str) -> Result<Config, ConfigError> {
    toml::from_str(text).map_err(|e| ConfigError {
        line: e.span().map(|s| text[..s.start.min(text.len())].matches('\n').count() + 1),
        message: e.message().to_string(),
    })
}

/// Lê o arquivo; se não existir, cria com `DEFAULT_CONFIG`.
pub fn load_or_create(path: &Path) -> Result<Config, ConfigError> {
    let io = |e: std::io::Error| ConfigError { line: None, message: format!("{}: {e}", path.display()) };
    if !path.exists() {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(io)?;
        }
        std::fs::write(path, DEFAULT_CONFIG).map_err(io)?;
    }
    parse_config(&std::fs::read_to_string(path).map_err(io)?)
}

/// Grava `value` (string) em `keys` (ex.: `["bar", "height"]`), mantendo comentários e chaves desconhecidas.
pub fn set_value(path: &Path, keys: &[&str], value: &str) -> std::io::Result<()> {
    let text = std::fs::read_to_string(path).unwrap_or_default();
    let mut doc: toml_edit::DocumentMut = text.parse().map_err(std::io::Error::other)?;
    let (last, tables) = keys.split_last().expect("ao menos uma chave");
    let mut table = doc.as_table_mut();
    for k in tables {
        let entry = table.entry(k).or_insert(toml_edit::table());
        table = entry.as_table_mut().ok_or_else(|| std::io::Error::other(format!("`{k}` não é uma tabela")))?;
    }
    match table.get_mut(last).and_then(|i| i.as_value_mut()) {
        Some(existing) => {
            let decor = existing.decor().clone();
            *existing = toml_edit::Value::from(value);
            *existing.decor_mut() = decor;
        }
        None => {
            table.insert(last, toml_edit::value(value));
        }
    }
    let tmp = path.with_extension("toml.tmp");
    std::fs::write(&tmp, doc.to_string())?;
    std::fs::rename(&tmp, path)
}

/// `%APPDATA%\zefiro` (ou `$XDG_CONFIG_HOME/zefiro`, ou `~/.config/zefiro`).
pub fn default_config_dir() -> PathBuf {
    if let Some(appdata) = std::env::var_os("APPDATA") {
        return PathBuf::from(appdata).join("zefiro");
    }
    if let Some(xdg) = std::env::var_os("XDG_CONFIG_HOME") {
        return PathBuf::from(xdg).join("zefiro");
    }
    PathBuf::from(std::env::var_os("HOME").unwrap_or_else(|| ".".into())).join(".config").join("zefiro")
}
