//! `%APPDATA%\zefiro\config.toml`: leitura com padrões, criação e edição sem perder comentários.

use serde::Deserialize;
use std::collections::{BTreeMap, HashSet};
use std::path::{Path, PathBuf};

pub const DEFAULT_CONFIG: &str = r#"# Zefiro — configuração da barra. Salvar aplica na hora.
theme = "obsidian"
# acrylic = true      # liga (true) ou desliga (false) o acrílico em qualquer tema; sem a chave, o tema decide

[bar]
height = "completa"   # completa | compacta
format = "faixa"      # faixa | pilulas | ilhas | fluent | vidro

[bar.sections]
start  = ["zefiro"]
center = ["island"]   # a ilha é um chip: tire do layout para liberar o centro
end    = ["clock"]

[island]
enabled = true
engine  = "embutida"  # embutida (desenhada pela barra) | externa (zefiro-island.exe, C++)
exe     = ""          # vazio = ao lado do zefiro.exe

[clock]
format  = ""          # vazio = formato do Windows; ou um padrão de data do Windows, ex.: "ddd d MMM"
seconds = false

[update]
check = true          # procura versão nova no GitHub Releases (1 vez por dia); instalar é pelo menu do Z

[chat]
origin = "auto"       # auto | windows | wsl | nome da distro: de onde vem o `claude` do chip "chat"
model  = ""           # vazio = o modelo que o Claude Code usaria; ou "sonnet", "opus"…
"#;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum BarHeight { Completa, Compacta }

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum BarFormat { Faixa, Pilulas, Ilhas, Fluent, Vidro }

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
            BarFormat::Ilhas => "ilhas",
            BarFormat::Fluent => "fluent",
            BarFormat::Vidro => "vidro",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(default)]
pub struct Config {
    pub theme: String,
    /// Acrílico escolhido no menu do Z, por cima do tema; `None` = o tema decide (`[material] backdrop`).
    pub acrylic: Option<bool>,
    pub bar: BarConfig,
    pub island: IslandConfig,
    pub clock: ClockConfig,
    pub update: UpdateConfig,
    pub terminal: TerminalConfig,
    pub chat: ChatConfig,
}

/// Chat com o Claude (chip `chat`). `origin`: `auto` (o Windows; sem ele, uma distro do WSL rodando
/// que tenha `claude`), `windows`, `wsl` (a distro padrão) ou o nome de uma distro. `model`: vazio
/// usa o modelo que o Claude Code usaria.
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(default)]
pub struct ChatConfig { pub origin: String, pub model: String }

impl Default for ChatConfig {
    fn default() -> Self {
        ChatConfig { origin: "auto".into(), model: String::new() }
    }
}

/// Perfis do Windows Terminal no painel do chip `terminal`, por `guid`: os visíveis nesta ordem (os
/// que faltam vão para o fim) e os ocultos. Escrita pelo próprio painel (lápis → Concluir).
#[derive(Debug, Clone, PartialEq, Deserialize, Default)]
#[serde(default)]
pub struct TerminalConfig { pub order: Vec<String>, pub hidden: Vec<String> }

/// Atualização pelo GitHub Releases (ADR-0014).
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(default)]
pub struct UpdateConfig { pub check: bool }

impl Default for UpdateConfig {
    fn default() -> Self {
        UpdateConfig { check: true }
    }
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(default)]
pub struct BarConfig {
    pub height: BarHeight,
    pub format: BarFormat,
    pub sections: Sections,
    pub layout_mode: LayoutMode,
    pub monitors: BTreeMap<String, MonitorConfig>,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LayoutMode { #[default] Shared, PerMonitor }

#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct MonitorConfig {
    #[serde(deserialize_with = "complete_sections")]
    pub sections: Sections,
}

fn complete_sections<'de, D: serde::Deserializer<'de>>(deserializer: D) -> Result<Sections, D::Error> {
    #[derive(Deserialize)]
    struct Complete { start: Vec<String>, center: Vec<String>, end: Vec<String> }
    let Complete { start, center, end } = Complete::deserialize(deserializer)?;
    Ok(Sections { start, center, end })
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(default)]
pub struct Sections { pub start: Vec<String>, pub center: Vec<String>, pub end: Vec<String> }

impl Sections {
    pub fn ids(&self) -> impl Iterator<Item = &str> {
        self.start.iter().chain(&self.center).chain(&self.end).map(String::as_str)
    }

    pub fn validate(&self, path: &str) -> Result<(), ConfigError> {
        let error = |message: String| ConfigError { line: None, message: format!("{path}: {message}") };
        if self.ids().count() > 64 {
            return Err(error("o limite é de 64 itens por layout".into()));
        }
        let mut ids = HashSet::new();
        for id in self.ids() {
            if id.is_empty() || id.len() > 256 {
                return Err(error("o ID do item deve ter de 1 a 256 bytes UTF-8".into()));
            }
            if !ids.insert(id) {
                return Err(error(format!("item repetido: `{id}`")));
            }
        }
        Ok(())
    }
}

/// Id do chip da ilha embutida (ADR-0017): ela só aparece onde o chip está.
pub const ISLAND_CHIP: &str = "island";

impl BarConfig {
    /// Algum layout (o compartilhado ou o de algum monitor) mostra o item `id`? Os serviços de cada
    /// chip só rodam quando ele está na barra.
    pub fn uses(&self, id: &str) -> bool {
        self.sections.ids().any(|i| i == id) || self.monitors.values().any(|m| m.sections.ids().any(|i| i == id))
    }

    pub fn sections_for(&self, monitor_id: Option<&str>) -> &Sections {
        if self.layout_mode == LayoutMode::PerMonitor
            && let Some(entry) = monitor_id.and_then(|id| self.monitors.get(id)) {
            return &entry.sections;
        }
        &self.sections
    }
}

impl Config {
    /// Só a seção `[terminal]` mudou? O painel do Terminal grava nela e não deve se fechar.
    pub fn same_except_terminal(&self, other: &Config) -> bool {
        Config { terminal: other.terminal.clone(), ..self.clone() } == *other
    }

    pub fn validate(&self) -> Result<(), ConfigError> {
        self.bar.sections.validate("bar.sections")?;
        if self.bar.monitors.len() > 128 {
            return Err(ConfigError { line: None, message: "bar.monitors: o limite é de 128 monitores".into() });
        }
        for (id, monitor) in &self.bar.monitors {
            monitor.sections.validate(&format!("bar.monitors.{id}.sections"))?;
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(default)]
pub struct IslandConfig { pub enabled: bool, pub exe: String, pub engine: IslandEngine }

/// `embutida` (padrão desde a 1c) = a ilha desenhada pela própria barra (Rust); `externa` = o
/// `zefiro-island.exe` (C++), que continua disponível para quem precisar de algo que só ele tem.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum IslandEngine { Externa, #[default] Embutida }

impl Config {
    /// A ilha embutida aparece em algum layout: engine embutida e o chip `island` na barra. Os
    /// serviços que só servem a ela (avisos, privacidade, timer) seguem isto.
    pub fn shows_embedded_island(&self) -> bool {
        self.island.embedded() && self.bar.uses(ISLAND_CHIP)
    }
}

impl IslandConfig {
    pub fn embedded(&self) -> bool {
        self.enabled && self.engine == IslandEngine::Embutida
    }

    pub fn external(&self) -> bool {
        self.enabled && self.engine == IslandEngine::Externa
    }
}

#[derive(Debug, Clone, PartialEq, Deserialize, Default)]
#[serde(default)]
pub struct ClockConfig { pub format: String, pub seconds: bool }

impl Default for Config {
    fn default() -> Self {
        Config { theme: "obsidian".into(), acrylic: None, bar: BarConfig::default(), island: IslandConfig::default(), clock: ClockConfig::default(), update: UpdateConfig::default(), terminal: TerminalConfig::default(), chat: ChatConfig::default() }
    }
}

impl Default for BarConfig {
    fn default() -> Self {
        BarConfig { height: BarHeight::Completa, format: BarFormat::Faixa, sections: Sections::default(), layout_mode: LayoutMode::Shared, monitors: BTreeMap::new() }
    }
}

impl Default for Sections {
    fn default() -> Self {
        Sections { start: vec!["zefiro".into()], center: vec![ISLAND_CHIP.into()], end: vec!["clock".into()] }
    }
}

impl Default for IslandConfig {
    fn default() -> Self {
        IslandConfig { enabled: true, exe: String::new(), engine: IslandEngine::Embutida }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct ConfigError { pub line: Option<usize>, pub message: String }

pub fn parse_config(text: &str) -> Result<Config, ConfigError> {
    let config: Config = toml::from_str(text).map_err(|e| ConfigError {
        line: e.span().map(|s| text[..s.start.min(text.len())].matches('\n').count() + 1),
        message: e.message().to_string(),
    })?;
    config.validate()?;
    Ok(config)
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
pub fn set_value(path: &Path, keys: &[&str], value: impl Into<toml_edit::Value>) -> std::io::Result<()> {
    let value: toml_edit::Value = value.into();
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
            *existing = value;
            *existing.decor_mut() = decor;
        }
        None => {
            table.insert(last, toml_edit::Item::Value(value));
        }
    }
    let tmp = path.with_extension("toml.tmp");
    std::fs::write(&tmp, doc.to_string())?;
    std::fs::rename(&tmp, path)
}

/// Grava `[terminal] order` e `hidden` de uma vez, mantendo comentários e o resto do arquivo.
pub fn set_terminal(path: &Path, terminal: &TerminalConfig) -> std::io::Result<()> {
    let text = std::fs::read_to_string(path).unwrap_or_default();
    let mut doc: toml_edit::DocumentMut = text.parse().map_err(std::io::Error::other)?;
    let table = doc.entry("terminal").or_insert(toml_edit::table()).as_table_mut().ok_or_else(|| std::io::Error::other("`terminal` não é uma tabela"))?;
    for (key, values) in [("order", &terminal.order), ("hidden", &terminal.hidden)] {
        table.insert(key, toml_edit::value(values.iter().map(String::as_str).collect::<toml_edit::Array>()));
    }
    let tmp = path.with_extension("toml.tmp");
    std::fs::write(&tmp, doc.to_string())?;
    std::fs::rename(&tmp, path)
}

/// `%APPDATA%\zefiro` (ou `$XDG_CONFIG_HOME/zefiro`, ou `~/.config/zefiro`).
pub fn default_config_dir() -> PathBuf {
    if let Some(dir) = std::env::var_os("ZEFIRO_CONFIG_DIR") {
        return PathBuf::from(dir);
    }
    if let Some(appdata) = std::env::var_os("APPDATA") {
        return PathBuf::from(appdata).join("zefiro");
    }
    if let Some(xdg) = std::env::var_os("XDG_CONFIG_HOME") {
        return PathBuf::from(xdg).join("zefiro");
    }
    PathBuf::from(std::env::var_os("HOME").unwrap_or_else(|| ".".into())).join(".config").join("zefiro")
}
