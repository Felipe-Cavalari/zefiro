//! De onde vem o `claude` do chat: o Windows ou uma distro do WSL (`[chat] origin`).

use std::path::PathBuf;

/// O que o `config.toml` pede.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Choice { Auto, Windows, WslDefault, Distro(String) }

impl Choice {
    /// Nome no seletor.
    pub fn label(&self) -> String {
        match self {
            Choice::Auto => "Automático".into(),
            Choice::Windows => "Windows".into(),
            Choice::WslDefault => "WSL".into(),
            Choice::Distro(d) => d.clone(),
        }
    }

    /// Valor gravado no `config.toml`.
    pub fn value(&self) -> String {
        match self {
            Choice::Auto => "auto".into(),
            Choice::Windows => "windows".into(),
            Choice::WslDefault => "wsl".into(),
            Choice::Distro(d) => d.clone(),
        }
    }

    /// A escolha que leva de volta à origem de uma conversa gravada ("Windows", "WSL", a distro).
    pub fn from_label(label: &str) -> Choice {
        match label {
            "" => Choice::Auto,
            "Windows" => Choice::Windows,
            "WSL" => Choice::WslDefault,
            d => Choice::Distro(d.to_string()),
        }
    }
}

/// As opções do seletor: automático, o Windows (se tem o executável) e as distros encontradas.
pub fn choices(windows: bool, distros: &[String]) -> Vec<Choice> {
    let mut out = vec![Choice::Auto];
    if windows {
        out.push(Choice::Windows);
    }
    out.extend(distros.iter().filter(|d| !ignored_distro(d)).map(|d| Choice::Distro(d.clone())));
    out
}

pub fn parse_choice(value: &str) -> Choice {
    match value.trim() {
        "" | "auto" => Choice::Auto,
        v if v.eq_ignore_ascii_case("windows") => Choice::Windows,
        v if v.eq_ignore_ascii_case("wsl") => Choice::WslDefault,
        v => Choice::Distro(v.to_string()),
    }
}

/// Onde o processo roda.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Origin {
    Windows(PathBuf),
    /// `None` = a distro padrão.
    Wsl(Option<String>),
}

impl Origin {
    /// Rótulo do painel e da conversa gravada ("Windows", "WSL" ou o nome da distro).
    pub fn label(&self) -> String {
        match self {
            Origin::Windows(_) => "Windows".into(),
            Origin::Wsl(None) => "WSL".into(),
            Origin::Wsl(Some(d)) => d.clone(),
        }
    }
}

/// Distros que nunca entram no automático.
pub fn ignored_distro(name: &str) -> bool {
    name.to_lowercase().starts_with("docker-desktop")
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PickError { NotFound, WindowsMissing }

/// Escolhe a origem. No automático, o Windows ganha; sem ele, a primeira distro rodando que tem
/// `claude` (`has_claude` só é chamado para distros rodando e fora as ignoradas, e nunca liga uma
/// distro parada). As escolhas explícitas do WSL não conferem nada: o usuário pediu aquela distro.
pub fn pick(choice: &Choice, windows_exe: Option<PathBuf>, running: &[String], has_claude: &mut dyn FnMut(&str) -> bool) -> Result<Origin, PickError> {
    match choice {
        Choice::Windows => windows_exe.map(Origin::Windows).ok_or(PickError::WindowsMissing),
        Choice::WslDefault => Ok(Origin::Wsl(None)),
        Choice::Distro(d) => Ok(Origin::Wsl(Some(d.clone()))),
        Choice::Auto => {
            if let Some(exe) = windows_exe {
                return Ok(Origin::Windows(exe));
            }
            running.iter().filter(|d| !ignored_distro(d)).find(|d| has_claude(d)).map(|d| Origin::Wsl(Some(d.clone()))).ok_or(PickError::NotFound)
        }
    }
}

pub fn pick_error_text(e: &PickError, agent: crate::agent::Agent) -> String {
    let bin = agent.binary();
    match e {
        PickError::NotFound => format!("Não achei o `{bin}` no Windows nem numa distro do WSL rodando. Instale o {} ou escolha outra origem.", agent.label()),
        PickError::WindowsMissing => format!("Não achei o {bin}.exe no Windows."),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn choices_from_the_config() {
        assert_eq!(parse_choice(""), Choice::Auto);
        assert_eq!(parse_choice("auto"), Choice::Auto);
        assert_eq!(parse_choice("Windows"), Choice::Windows);
        assert_eq!(parse_choice("wsl"), Choice::WslDefault);
        assert_eq!(parse_choice(" Ubuntu-24.04 "), Choice::Distro("Ubuntu-24.04".into()));
    }

    #[test]
    fn auto_prefers_windows_and_never_asks_about_it_when_found() {
        let mut asked = Vec::new();
        let exe = PathBuf::from("claude.exe");
        let got = pick(&Choice::Auto, Some(exe.clone()), &["Ubuntu".into()], &mut |d| {
            asked.push(d.to_string());
            true
        });
        assert_eq!(got, Ok(Origin::Windows(exe)));
        assert!(asked.is_empty());
    }

    #[test]
    fn auto_falls_back_to_a_running_distro_with_claude() {
        let running = ["docker-desktop".to_string(), "Debian".into(), "Ubuntu".into()];
        let mut asked = Vec::new();
        let got = pick(&Choice::Auto, None, &running, &mut |d| {
            asked.push(d.to_string());
            d == "Ubuntu"
        });
        assert_eq!(got, Ok(Origin::Wsl(Some("Ubuntu".into()))));
        assert_eq!(asked, ["Debian", "Ubuntu"], "docker-desktop fica de fora");
        assert_eq!(pick(&Choice::Auto, None, &[], &mut |_| true), Err(PickError::NotFound));
    }

    #[test]
    fn explicit_choices() {
        assert_eq!(pick(&Choice::Windows, None, &[], &mut |_| true), Err(PickError::WindowsMissing));
        assert_eq!(pick(&Choice::WslDefault, Some("c".into()), &[], &mut |_| false), Ok(Origin::Wsl(None)));
        assert_eq!(pick(&Choice::Distro("Arch".into()), None, &[], &mut |_| false), Ok(Origin::Wsl(Some("Arch".into()))), "a distro escolhida não precisa estar rodando");
    }

    #[test]
    fn choice_labels_values_and_round_trip() {
        for c in [Choice::Auto, Choice::Windows, Choice::WslDefault, Choice::Distro("Ubuntu".into())] {
            assert_eq!(parse_choice(&c.value()), c);
        }
        assert_eq!(Choice::Auto.label(), "Automático");
        assert_eq!(Choice::from_label(&Origin::Wsl(Some("Arch".into())).label()), Choice::Distro("Arch".into()));
        assert_eq!(Choice::from_label("Windows"), Choice::Windows);
        assert_eq!(choices(true, &["docker-desktop".into(), "Ubuntu".into()]), [Choice::Auto, Choice::Windows, Choice::Distro("Ubuntu".into())]);
        assert_eq!(choices(false, &[]), [Choice::Auto]);
        assert!(pick_error_text(&PickError::NotFound, crate::agent::Agent::Codex).contains("`codex`"));
    }

    #[test]
    fn labels() {
        assert_eq!(Origin::Windows("x".into()).label(), "Windows");
        assert_eq!(Origin::Wsl(None).label(), "WSL");
        assert_eq!(Origin::Wsl(Some("Ubuntu-24.04".into())).label(), "Ubuntu-24.04");
    }
}
