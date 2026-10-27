use zefiro_core::config::*;

#[test]
fn default_config_parses_to_the_spec_defaults() {
    let c = parse_config(DEFAULT_CONFIG).unwrap();
    assert_eq!(c.theme, "obsidian");
    assert_eq!(c.bar.height, BarHeight::Completa);
    assert_eq!(c.bar.format, BarFormat::Faixa);
    assert_eq!(c.bar.sections.start, vec!["zefiro".to_string()]);
    assert_eq!(c.bar.sections.end, vec!["clock".to_string()]);
    assert!(c.island.enabled);
    assert_eq!(c.island.exe, "");
    assert_eq!(c.island.engine, IslandEngine::Embutida);
    assert!(c.island.embedded() && !c.island.external());
    assert!(parse_config("[island]\nengine = \"externa\"\n").unwrap().island.external());
    assert_eq!(c.clock.format, "");
    assert!(!c.clock.seconds);
}

#[test]
fn the_island_is_a_chip_and_only_shows_where_it_is_placed() {
    let c = parse_config(DEFAULT_CONFIG).unwrap();
    assert_eq!(c.bar.sections.center, vec!["island".to_string()], "instalação nova: a ilha no centro");
    assert!(c.shows_embedded_island());
    let no_center_key = parse_config("[bar.sections]\nstart = [\"zefiro\"]\nend = [\"clock\"]\n").unwrap();
    assert!(no_center_key.shows_embedded_island(), "sem a chave center, o centro padrão tem a ilha");
    let free = parse_config("[bar.sections]\ncenter = [\"hardware\"]\n").unwrap();
    assert!(!free.shows_embedded_island(), "centro com outros chips: sem ilha");
    let external = parse_config("[island]\nengine = \"externa\"\n").unwrap();
    assert!(!external.shows_embedded_island(), "a externa não é o chip");
    let monitors = parse_config("[bar]\nlayout_mode = \"per_monitor\"\n[bar.sections]\ncenter = []\n[bar.monitors.a.sections]\nstart = [\"island\"]\ncenter = []\nend = []\n").unwrap();
    assert!(monitors.shows_embedded_island(), "em qualquer seção de qualquer monitor");
}

#[test]
fn acrylic_choice_is_optional_and_written_as_a_bool() {
    assert_eq!(parse_config(DEFAULT_CONFIG).unwrap().acrylic, None, "sem a chave, cada tema decide");
    assert_eq!(parse_config("acrylic = true\n").unwrap().acrylic, Some(true));
    assert_eq!(parse_config("acrylic = false\n").unwrap().acrylic, Some(false));
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("config.toml");
    std::fs::write(&path, "theme = \"cobalto\" # tema\n\n[bar]\nheight = \"compacta\"\n").unwrap();
    set_value(&path, &["acrylic"], false).unwrap();
    let text = std::fs::read_to_string(&path).unwrap();
    assert!(text.contains("theme = \"cobalto\" # tema"), "{text}");
    let c = parse_config(&text).unwrap();
    assert_eq!((c.acrylic, c.bar.height), (Some(false), BarHeight::Compacta), "a chave fica na raiz, antes de [bar]");
}

#[test]
fn island_engine_choice_needs_the_island_enabled() {
    let c = parse_config("[island]\nengine = \"embutida\"\n").unwrap();
    assert!(c.island.embedded() && !c.island.external());
    let off = parse_config("[island]\nenabled = false\nengine = \"embutida\"\n").unwrap();
    assert!(!off.island.embedded() && !off.island.external());
    assert!(parse_config("[island]\nengine = \"outra\"\n").is_err());
}

#[test]
fn missing_keys_take_defaults() {
    let c = parse_config("[bar]\nformat = \"pilulas\"\n").unwrap();
    assert_eq!(c.theme, "obsidian");
    assert_eq!(c.bar.height, BarHeight::Completa);
    assert_eq!(c.bar.format, BarFormat::Pilulas);
}

#[test]
fn fluent_and_vidro_formats_parse_and_round_trip() {
    for (text, format) in [("fluent", BarFormat::Fluent), ("vidro", BarFormat::Vidro)] {
        let c = parse_config(&format!("[bar]\nformat = \"{text}\"\n")).unwrap();
        assert_eq!(c.bar.format, format);
        assert_eq!(format.as_str(), text);
    }
}

#[test]
fn islands_format_parses_and_round_trips() {
    let c = parse_config("[bar]\nformat = \"ilhas\"\n").unwrap();
    assert_eq!(c.bar.format, BarFormat::Ilhas);
    assert_eq!(BarFormat::Ilhas.as_str(), "ilhas");
}

#[test]
fn invalid_value_reports_line() {
    let e = parse_config("theme = \"nord\"\n[bar]\nheight = \"enorme\"\n").unwrap_err();
    assert_eq!(e.line, Some(3));
}

#[test]
fn load_or_create_writes_default_file_once() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("zefiro").join("config.toml");
    let c = load_or_create(&path).unwrap();
    assert_eq!(c.theme, "obsidian");
    assert_eq!(std::fs::read_to_string(&path).unwrap(), DEFAULT_CONFIG);
    std::fs::write(&path, "theme = \"plum\"\n").unwrap();
    assert_eq!(load_or_create(&path).unwrap().theme, "plum");
}

#[test]
fn set_value_keeps_comments_and_unknown_keys() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("config.toml");
    std::fs::write(&path, "# meu comentário\ntheme = \"obsidian\" # tema\n\n[bar]\nheight = \"completa\"\nextra = 1 # chave do futuro\n").unwrap();
    set_value(&path, &["bar", "height"], "compacta").unwrap();
    set_value(&path, &["theme"], "nord").unwrap();
    set_value(&path, &["bar", "format"], "pilulas").unwrap();
    let text = std::fs::read_to_string(&path).unwrap();
    assert!(text.contains("# meu comentário"));
    assert!(text.contains("extra = 1 # chave do futuro"));
    assert!(text.contains("theme = \"nord\" # tema"), "{text}");
    let c = parse_config(&text).unwrap();
    assert_eq!(c.bar.height, BarHeight::Compacta);
    assert_eq!(c.bar.format, BarFormat::Pilulas);
}

#[test]
fn bar_uses_reports_items_in_any_layout() {
    let c = parse_config("[bar.sections]\nstart = [\"zefiro\"]\nend = [\"claude\", \"clock\"]\n").unwrap();
    assert!(c.bar.uses("claude"));
    assert!(!c.bar.uses("codex"));
    let per_monitor = parse_config("[bar]\nlayout_mode = \"per_monitor\"\n[bar.sections]\nend = [\"clock\"]\n[bar.monitors.abc.sections]\nstart = []\ncenter = []\nend = [\"codex\"]\n").unwrap();
    assert!(per_monitor.bar.uses("codex"));
}

#[test]
fn update_check_is_on_by_default_and_can_be_turned_off() {
    assert!(parse_config(zefiro_core::config::DEFAULT_CONFIG).unwrap().update.check);
    assert!(parse_config("").unwrap().update.check, "configs antigas, sem a seção");
    assert!(!parse_config("[update]\ncheck = false\n").unwrap().update.check);
}

#[test]
fn terminal_section_is_optional_and_parsed() {
    assert_eq!(parse_config(DEFAULT_CONFIG).unwrap().terminal, TerminalConfig::default());
    let c = parse_config("[terminal]\norder = [\"{a}\"]\nhidden = [\"{b}\"]\n").unwrap();
    assert_eq!(c.terminal, TerminalConfig { order: vec!["{a}".into()], hidden: vec!["{b}".into()] });
}

#[test]
fn set_terminal_keeps_comments_and_other_keys() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("config.toml");
    std::fs::write(&path, DEFAULT_CONFIG).unwrap();
    set_terminal(&path, &TerminalConfig { order: vec!["{a}".into()], hidden: vec![] }).unwrap();
    let second = TerminalConfig { order: vec!["{b}".into(), "{a}".into()], hidden: vec!["{c}".into()] };
    set_terminal(&path, &second).unwrap();
    let text = std::fs::read_to_string(&path).unwrap();
    assert!(text.contains("# completa | compacta"), "{text}");
    let c = parse_config(&text).unwrap();
    assert_eq!(c.terminal, second);
    assert_eq!(c.theme, "obsidian");
}

#[test]
fn only_panel_sections_changed() {
    let a = Config::default();
    let mut b = a.clone();
    b.terminal.hidden.push("{x}".into());
    b.chat.agent = "codex".into();
    b.chat.codex.model = "gpt-5.6-sol".into();
    assert!(a.same_except_panels(&b));
    b.theme = "cobalto".into();
    assert!(!a.same_except_panels(&b));
}

#[test]
fn chat_section_defaults_to_auto_and_is_parsed() {
    let c = parse_config(DEFAULT_CONFIG).unwrap();
    assert_eq!(c.chat, ChatConfig::default());
    assert_eq!((c.chat.agent.as_str(), c.chat.claude.origin.as_str(), c.chat.codex.model.as_str()), ("claude", "auto", ""));
    let c = parse_config("[chat]\nagent = \"codex\"\n[chat.codex]\norigin = \"Ubuntu-24.04\"\nmodel = \"gpt-5.6-sol\"\n").unwrap();
    assert_eq!((c.chat.agent.as_str(), c.chat.of(true).origin.as_str(), c.chat.of(true).model.as_str()), ("codex", "Ubuntu-24.04", "gpt-5.6-sol"));
    assert_eq!(c.chat.of(false), &ChatAgentConfig::default(), "o Claude fica no padrão");
    assert_eq!(c.chat.of(true).prefer, "windows", "sem a chave, o automático prefere o Windows");
    let c = parse_config("[chat.claude]\nprefer = \"wsl\"\n").unwrap();
    assert_eq!((c.chat.claude.prefer.as_str(), c.chat.claude.origin.as_str()), ("wsl", "auto"));
}

#[test]
fn chat_choices_are_written_without_touching_the_rest() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("config.toml");
    std::fs::write(&path, DEFAULT_CONFIG).unwrap();
    set_value(&path, &["chat", "agent"], "codex").unwrap();
    set_value(&path, &["chat", "codex", "model"], "gpt-5.6-luna").unwrap();
    let text = std::fs::read_to_string(&path).unwrap();
    assert!(text.contains("# completa | compacta"));
    let c = parse_config(&text).unwrap();
    assert_eq!((c.chat.agent.as_str(), c.chat.codex.model.as_str(), c.chat.claude.origin.as_str()), ("codex", "gpt-5.6-luna", "auto"));
}

#[test]
fn spotify_section_parses_and_defaults_empty() {
    assert_eq!(parse_config(DEFAULT_CONFIG).unwrap().spotify, SpotifyConfig::default());
    assert_eq!(SpotifyConfig::default().client_id, "");
    let c = parse_config("[spotify]\nclient_id = \"abc123\"\n").unwrap();
    assert_eq!(c.spotify.client_id, "abc123");
}

#[test]
fn set_spotify_keeps_comments_and_other_sections() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("config.toml");
    std::fs::write(&path, DEFAULT_CONFIG).unwrap();
    set_spotify(&path, &SpotifyConfig { client_id: "abc".into() }).unwrap();
    let text = std::fs::read_to_string(&path).unwrap();
    assert!(text.contains("Redirect URI http://127.0.0.1:47823/callback"), "comentário na primeira vez: {text}");
    set_spotify(&path, &SpotifyConfig { client_id: "def".into() }).unwrap();
    let text = std::fs::read_to_string(&path).unwrap();
    assert!(text.contains("# completa | compacta"), "{text}");
    assert_eq!(text.matches("Redirect URI").count(), 1, "o comentário não se repete");
    let c = parse_config(&text).unwrap();
    assert_eq!(c.spotify.client_id, "def");
    assert_eq!(c.chat, ChatConfig::default());
}

#[test]
fn only_spotify_changed_is_same_except_panels() {
    let a = Config::default();
    let mut b = a.clone();
    b.spotify.client_id = "abc".into();
    assert!(a.same_except_panels(&b));
}
