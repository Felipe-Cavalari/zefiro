//! Menu do botão direito na ilha: fonte de mídia, saída de som do app, saída e microfone padrão.
//! Portável: monta as entradas e traduz o comando escolhido; a plataforma mostra o menu nativo.

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct MenuEntry {
    pub label: String,
    /// 0 = sem comando (título, separador ou submenu).
    pub id: u32,
    pub checked: bool,
    pub enabled: bool,
    pub separator: bool,
    pub children: Vec<MenuEntry>,
}

impl MenuEntry {
    fn item(label: impl Into<String>, id: u32, checked: bool) -> MenuEntry {
        MenuEntry { label: label.into(), id, checked, enabled: true, ..Default::default() }
    }
    fn note(label: impl Into<String>) -> MenuEntry {
        MenuEntry { label: label.into(), enabled: false, ..Default::default() }
    }
    fn separator() -> MenuEntry {
        MenuEntry { separator: true, ..Default::default() }
    }
    fn submenu(label: impl Into<String>, children: Vec<MenuEntry>) -> MenuEntry {
        MenuEntry { label: label.into(), enabled: !children.is_empty(), children, ..Default::default() }
    }
}

/// Estado da saída do app da fonte atual, como a plataforma informou.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AppOutput {
    /// Saída gravada para o app (`None` = padrão do sistema).
    Current(Option<String>),
    NoAudioYet,
    NotRunning,
    /// O Windows não tem a interface: o submenu não aparece.
    Unavailable,
}

#[derive(Debug, Clone, Default)]
pub struct MenuInput {
    /// (id, nome) das sessões de mídia e a atual.
    pub sources: Vec<(String, String)>,
    pub current_source: Option<String>,
    /// (id, nome) das saídas e entradas ativas, com o padrão de cada uma.
    pub outputs: Vec<(String, String)>,
    pub default_output: Option<String>,
    pub inputs: Vec<(String, String)>,
    pub default_input: Option<String>,
    pub app_output: Option<AppOutput>,
    pub mic_muted: Option<bool>,
    /// Timer de foco: `None` = parado; `Some(true)` = rodando; `Some(false)` = pausado.
    pub timer_running: Option<bool>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MenuCommand {
    Source(String),
    /// `None` = padrão do sistema.
    AppOutput(Option<String>),
    DefaultOutput(String),
    DefaultInput(String),
    ToggleMicMute,
    /// (foco?, minutos)
    StartTimer(bool, u32),
    PauseResumeTimer,
    StopTimer,
}

const SOURCE: u32 = 100;
const APP_OUTPUT: u32 = 200;
const OUTPUT: u32 = 300;
const INPUT: u32 = 400;
const MIC_MUTE: u32 = 500;
const TIMER_25: u32 = 600;
const TIMER_50: u32 = 601;
const TIMER_BREAK: u32 = 602;
const TIMER_PAUSE: u32 = 603;
const TIMER_STOP: u32 = 604;

pub fn build(input: &MenuInput) -> Vec<MenuEntry> {
    let mut menu = Vec::new();
    if !input.sources.is_empty() {
        let items = input.sources.iter().enumerate().map(|(i, (id, name))| MenuEntry::item(name, SOURCE + i as u32, input.current_source.as_ref() == Some(id))).collect();
        menu.push(MenuEntry::submenu("Fonte de mídia", items));
    }
    if let (Some(state), Some(source)) = (&input.app_output, input.current_source.as_ref()) {
        let app = input.sources.iter().find(|(id, _)| id == source).map_or("App", |(_, n)| n.as_str());
        let mut items = vec![MenuEntry::note(app), MenuEntry::separator()];
        match state {
            AppOutput::Current(current) => {
                let default_name = input.default_output.as_ref().and_then(|d| input.outputs.iter().find(|(id, _)| id == d)).map(|(_, n)| n.as_str());
                let label = match default_name {
                    Some(n) => format!("Padrão do sistema ({n})"),
                    None => "Padrão do sistema".into(),
                };
                items.push(MenuEntry::item(label, APP_OUTPUT, current.is_none()));
                items.push(MenuEntry::separator());
                for (i, (id, name)) in input.outputs.iter().enumerate() {
                    let checked = current.as_ref().is_some_and(|c| c.eq_ignore_ascii_case(id));
                    items.push(MenuEntry::item(name, APP_OUTPUT + 1 + i as u32, checked));
                }
            }
            AppOutput::NoAudioYet => items.push(MenuEntry::note(format!("{app} ainda não abriu o áudio"))),
            AppOutput::NotRunning => items.push(MenuEntry::note(format!("{app} não está aberto"))),
            AppOutput::Unavailable => items.clear(),
        }
        if !items.is_empty() {
            menu.push(MenuEntry::submenu("Saída de som do app", items));
        }
    }
    if !menu.is_empty() {
        menu.push(MenuEntry::separator());
    }
    let devices = |list: &[(String, String)], default: &Option<String>, base: u32| -> Vec<MenuEntry> {
        list.iter().enumerate().map(|(i, (id, name))| MenuEntry::item(name, base + i as u32, default.as_ref() == Some(id))).collect()
    };
    menu.push(MenuEntry::submenu("Saída de som", devices(&input.outputs, &input.default_output, OUTPUT)));
    menu.push(MenuEntry::submenu("Microfone", devices(&input.inputs, &input.default_input, INPUT)));
    if let Some(muted) = input.mic_muted {
        menu.push(MenuEntry::item("Microfone mudo", MIC_MUTE, muted));
    }
    menu.push(MenuEntry::separator());
    let mut timer = vec![MenuEntry::item("Foco de 25 min", TIMER_25, false), MenuEntry::item("Foco de 50 min", TIMER_50, false), MenuEntry::item("Pausa de 5 min", TIMER_BREAK, false)];
    if let Some(running) = input.timer_running {
        timer.push(MenuEntry::separator());
        timer.push(MenuEntry::item(if running { "Pausar" } else { "Retomar" }, TIMER_PAUSE, false));
        timer.push(MenuEntry::item("Parar", TIMER_STOP, false));
    }
    menu.push(MenuEntry::submenu("Timer de foco", timer));
    menu
}

/// Comando do id escolhido no menu montado com o mesmo `input`.
pub fn command(input: &MenuInput, id: u32) -> Option<MenuCommand> {
    let at = |list: &[(String, String)], base: u32| list.get(id.checked_sub(base)? as usize).map(|(id, _)| id.clone());
    match id {
        TIMER_25 => Some(MenuCommand::StartTimer(true, 25)),
        TIMER_50 => Some(MenuCommand::StartTimer(true, 50)),
        TIMER_BREAK => Some(MenuCommand::StartTimer(false, 5)),
        TIMER_PAUSE => Some(MenuCommand::PauseResumeTimer),
        TIMER_STOP => Some(MenuCommand::StopTimer),
        MIC_MUTE => Some(MenuCommand::ToggleMicMute),
        INPUT.. if id < MIC_MUTE => at(&input.inputs, INPUT).map(MenuCommand::DefaultInput),
        OUTPUT.. if id < INPUT => at(&input.outputs, OUTPUT).map(MenuCommand::DefaultOutput),
        APP_OUTPUT => Some(MenuCommand::AppOutput(None)),
        APP_OUTPUT.. if id < OUTPUT => at(&input.outputs, APP_OUTPUT + 1).map(|d| MenuCommand::AppOutput(Some(d))),
        SOURCE.. if id < APP_OUTPUT => at(&input.sources, SOURCE).map(MenuCommand::Source),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pairs(v: &[(&str, &str)]) -> Vec<(String, String)> {
        v.iter().map(|(a, b)| (a.to_string(), b.to_string())).collect()
    }

    fn input() -> MenuInput {
        MenuInput {
            sources: pairs(&[("spotify", "Spotify"), ("brave", "Brave")]),
            current_source: Some("brave".into()),
            outputs: pairs(&[("o1", "Alto-falantes"), ("o2", "Fones")]),
            default_output: Some("o1".into()),
            inputs: pairs(&[("i1", "Microfone USB")]),
            default_input: Some("i1".into()),
            app_output: Some(AppOutput::Current(Some("O2".into()))),
            mic_muted: Some(false),
            timer_running: None,
        }
    }

    fn find<'a>(menu: &'a [MenuEntry], label: &str) -> &'a MenuEntry {
        menu.iter().find(|e| e.label == label).unwrap_or_else(|| panic!("{label} ausente"))
    }

    #[test]
    fn builds_sources_app_output_devices_and_mute() {
        let i = input();
        let menu = build(&i);
        let sources = find(&menu, "Fonte de mídia");
        assert!(sources.children[1].checked && !sources.children[0].checked);
        let app = find(&menu, "Saída de som do app");
        assert_eq!(app.children[0].label, "Brave");
        assert_eq!(app.children[2].label, "Padrão do sistema (Alto-falantes)");
        assert!(!app.children[2].checked);
        assert!(app.children[5].checked, "Fones (id comparado sem maiúsculas)");
        assert!(find(&menu, "Saída de som").children[0].checked);
        assert!(find(&menu, "Microfone").children[0].checked);
        assert!(!find(&menu, "Microfone mudo").checked);
    }

    #[test]
    fn ids_map_back_to_commands() {
        let i = input();
        let menu = build(&i);
        let id = |label: &str, child: usize| find(&menu, label).children[child].id;
        assert_eq!(command(&i, id("Fonte de mídia", 0)), Some(MenuCommand::Source("spotify".into())));
        assert_eq!(command(&i, id("Saída de som do app", 2)), Some(MenuCommand::AppOutput(None)));
        assert_eq!(command(&i, id("Saída de som do app", 5)), Some(MenuCommand::AppOutput(Some("o2".into()))));
        assert_eq!(command(&i, id("Saída de som", 1)), Some(MenuCommand::DefaultOutput("o2".into())));
        assert_eq!(command(&i, id("Microfone", 0)), Some(MenuCommand::DefaultInput("i1".into())));
        assert_eq!(command(&i, find(&menu, "Microfone mudo").id), Some(MenuCommand::ToggleMicMute));
        assert_eq!(command(&i, 0), None);
        assert_eq!(command(&i, 399), None);
    }

    #[test]
    fn app_output_states_and_no_media() {
        let mut i = input();
        i.app_output = Some(AppOutput::NoAudioYet);
        assert_eq!(find(&build(&i), "Saída de som do app").children[2].label, "Brave ainda não abriu o áudio");
        i.app_output = Some(AppOutput::Unavailable);
        assert!(!build(&i).iter().any(|e| e.label == "Saída de som do app"));
        let empty = MenuInput { outputs: pairs(&[("o1", "A")]), ..Default::default() };
        let menu = build(&empty);
        assert_eq!(menu[0].label, "Saída de som", "sem mídia, começa pelos dispositivos");
    }
}

#[cfg(test)]
mod timer_menu_tests {
    use super::*;

    #[test]
    fn timer_submenu_offers_start_and_controls_only_when_running() {
        let stopped = MenuInput::default();
        let menu = build(&stopped);
        let timer = menu.iter().find(|e| e.label == "Timer de foco").unwrap();
        assert_eq!(timer.children.len(), 3);
        assert_eq!(command(&stopped, timer.children[1].id), Some(MenuCommand::StartTimer(true, 50)));
        let running = MenuInput { timer_running: Some(true), ..Default::default() };
        let menu = build(&running);
        let timer = menu.iter().find(|e| e.label == "Timer de foco").unwrap();
        assert_eq!(timer.children[4].label, "Pausar");
        assert_eq!(command(&running, timer.children[5].id), Some(MenuCommand::StopTimer));
    }
}
