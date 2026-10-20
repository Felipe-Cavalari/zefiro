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
fn islands_format_parses_and_round_trips() {
    let c = parse_config("[bar]
format = \"ilhas\"
").unwrap();
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
fn only_terminal_changed() {
    let a = Config::default();
    let mut b = a.clone();
    b.terminal.hidden.push("{x}".into());
    assert!(a.same_except_terminal(&b));
    b.theme = "cobalto".into();
    assert!(!a.same_except_terminal(&b));
}
