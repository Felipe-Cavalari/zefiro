//! Quem abre e quem fecha: um painel por vez.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PanelKind { Menu, Settings, Calendar, Overflow, Island, Audio, Mic, Brightness, Claude, Codex, VsCode, Terminal, Chat, Power, Player, Weather, Hardware }

/// Configurações não são um chip: o menu do Z abre o painel como se fosse, preso ao chip `zefiro`.
pub const SETTINGS_CHIP: &str = "__settings";

impl PanelKind {
    /// Painel com abas da ilha: a própria ilha ou um dos chips que levam uma aba dela.
    pub fn tabbed(self) -> bool {
        matches!(self, PanelKind::Island | PanelKind::Player | PanelKind::Weather | PanelKind::Hardware)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OpenPanel { pub monitor: usize, pub chip: String, pub kind: PanelKind }

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PanelInput { ChipClick { monitor: usize, chip: String }, ClickOutside, Escape, FocusLost, ConfigChanged }

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PanelEffect { Open(OpenPanel), Close { monitor: usize } }

pub fn panel_kind_for(chip: &str) -> Option<PanelKind> {
    if chip.starts_with("__more_") { return Some(PanelKind::Overflow); }
    match chip {
        "zefiro" => Some(PanelKind::Menu),
        SETTINGS_CHIP => Some(PanelKind::Settings),
        "clock" => Some(PanelKind::Calendar),
        "island" => Some(PanelKind::Island),
        "audio" => Some(PanelKind::Audio),
        "mic" => Some(PanelKind::Mic),
        "brightness" => Some(PanelKind::Brightness),
        "claude" => Some(PanelKind::Claude),
        "codex" => Some(PanelKind::Codex),
        "vscode" => Some(PanelKind::VsCode),
        "terminal" => Some(PanelKind::Terminal),
        "chat" => Some(PanelKind::Chat),
        "power" => Some(PanelKind::Power),
        "player" => Some(PanelKind::Player),
        "weather" => Some(PanelKind::Weather),
        "hardware" => Some(PanelKind::Hardware),
        _ => None,
    }
}

/// Chips que agem direto no clique, sem painel (Notificações, Não perturbe).
pub fn action_chip(chip: &str) -> bool {
    matches!(chip, "notifications" | "dnd")
}

#[derive(Debug, Default)]
pub struct PanelController { open: Option<OpenPanel> }

impl PanelController {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn open(&self) -> Option<&OpenPanel> {
        self.open.as_ref()
    }

    pub fn handle(&mut self, input: PanelInput) -> Vec<PanelEffect> {
        let mut fx = Vec::new();
        match input {
            PanelInput::ChipClick { monitor, chip } => {
                let Some(kind) = panel_kind_for(&chip) else { return fx };
                if let Some(current) = self.open.take() {
                    fx.push(PanelEffect::Close { monitor: current.monitor });
                    if current.monitor == monitor && current.chip == chip {
                        return fx;
                    }
                }
                let p = OpenPanel { monitor, chip, kind };
                fx.push(PanelEffect::Open(p.clone()));
                self.open = Some(p);
            }
            PanelInput::ClickOutside | PanelInput::Escape | PanelInput::FocusLost | PanelInput::ConfigChanged => {
                if let Some(current) = self.open.take() {
                    fx.push(PanelEffect::Close { monitor: current.monitor });
                }
            }
        }
        fx
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn click(monitor: usize, chip: &str) -> PanelInput {
        PanelInput::ChipClick { monitor, chip: chip.into() }
    }

    #[test]
    fn new_chips_map_to_panels_or_actions() {
        assert_eq!(panel_kind_for("claude"), Some(PanelKind::Claude));
        assert_eq!(panel_kind_for("codex"), Some(PanelKind::Codex));
        assert_eq!(panel_kind_for("vscode"), Some(PanelKind::VsCode));
        assert_eq!(panel_kind_for("terminal"), Some(PanelKind::Terminal));
        assert_eq!(panel_kind_for("chat"), Some(PanelKind::Chat));
        assert_eq!(panel_kind_for("power"), Some(PanelKind::Power));
        for id in ["notifications", "dnd"] {
            assert_eq!(panel_kind_for(id), None);
            assert!(action_chip(id));
        }
        assert_eq!(panel_kind_for("clock"), Some(PanelKind::Calendar));
        assert!(!action_chip("clock"));
    }

    #[test]
    fn island_chips_open_tabbed_panels() {
        assert_eq!(panel_kind_for("player"), Some(PanelKind::Player));
        assert_eq!(panel_kind_for("weather"), Some(PanelKind::Weather));
        assert_eq!(panel_kind_for("hardware"), Some(PanelKind::Hardware));
        for kind in [PanelKind::Island, PanelKind::Player, PanelKind::Weather, PanelKind::Hardware] {
            assert!(kind.tabbed(), "{kind:?}");
        }
        assert!(!PanelKind::Calendar.tabbed());
    }

    #[test]
    fn settings_open_from_the_menu_in_its_place() {
        assert_eq!(panel_kind_for(SETTINGS_CHIP), Some(PanelKind::Settings));
        let mut c = PanelController::new();
        c.handle(click(1, "zefiro"));
        assert_eq!(
            c.handle(click(1, SETTINGS_CHIP)),
            vec![PanelEffect::Close { monitor: 1 }, PanelEffect::Open(OpenPanel { monitor: 1, chip: SETTINGS_CHIP.into(), kind: PanelKind::Settings })]
        );
    }

    #[test]
    fn click_opens_and_second_click_closes() {
        let mut c = PanelController::new();
        assert_eq!(c.handle(click(0, "clock")), vec![PanelEffect::Open(OpenPanel { monitor: 0, chip: "clock".into(), kind: PanelKind::Calendar })]);
        assert_eq!(c.handle(click(0, "clock")), vec![PanelEffect::Close { monitor: 0 }]);
        assert!(c.open().is_none());
    }

    #[test]
    fn opening_another_chip_closes_current_first() {
        let mut c = PanelController::new();
        c.handle(click(0, "clock"));
        assert_eq!(
            c.handle(click(1, "zefiro")),
            vec![PanelEffect::Close { monitor: 0 }, PanelEffect::Open(OpenPanel { monitor: 1, chip: "zefiro".into(), kind: PanelKind::Menu })]
        );
        assert_eq!(c.open().unwrap().chip, "zefiro");
    }

    #[test]
    fn same_chip_on_another_monitor_moves_the_panel() {
        let mut c = PanelController::new();
        c.handle(click(0, "clock"));
        assert_eq!(c.handle(click(1, "clock")).len(), 2);
        assert_eq!(c.open().unwrap().monitor, 1);
    }

    #[test]
    fn outside_escape_focus_lost_close() {
        for input in [PanelInput::ClickOutside, PanelInput::Escape, PanelInput::FocusLost] {
            let mut c = PanelController::new();
            c.handle(click(0, "zefiro"));
            assert_eq!(c.handle(input), vec![PanelEffect::Close { monitor: 0 }]);
        }
    }

    #[test]
    fn config_change_closes_open_panel() {
        let mut c = PanelController::new();
        c.handle(click(0, "zefiro"));
        assert_eq!(c.handle(PanelInput::ConfigChanged), vec![PanelEffect::Close { monitor: 0 }]);
        assert!(c.handle(PanelInput::ConfigChanged).is_empty());
    }

    #[test]
    fn unknown_chip_does_nothing() {
        let mut c = PanelController::new();
        assert!(c.handle(click(0, "desconhecido")).is_empty());
        assert_eq!(panel_kind_for("zefiro"), Some(PanelKind::Menu));
        assert_eq!(panel_kind_for("clock"), Some(PanelKind::Calendar));
    }
}
