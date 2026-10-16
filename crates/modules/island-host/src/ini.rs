//! Grava no island.ini / island-state.ini só as chaves que a barra controla.

use zefiro_core::config::{BarFormat, BarHeight};
use zefiro_platform::color::Rgba;
use zefiro_resources::theme::{BUILTIN, SpringStyle, Theme, parse_theme};

#[derive(Debug, Clone, PartialEq)]
pub enum IslandTheme {
    Preset { index: usize, id: String },
    Custom { background: Rgba, text: Rgba, text_secondary: Rgba, border: Option<Rgba> },
}

#[derive(Debug, Clone, PartialEq)]
pub struct IslandSettings {
    pub bar_height: u32,
    pub pill_height: u32,
    pub theme: IslandTheme,
    pub spring: SpringStyle,
    pub speed: f32,
    /// Formato faixa: a ilha se funde à barra (sem contorno nem sombra). Nas pílulas ela é uma pílula como as outras.
    pub merged: bool,
    /// Sombra do tema (`material.shadow`), usada quando a ilha não está fundida.
    pub shadow: bool,
}

pub fn island_pill_height(height: BarHeight, format: BarFormat) -> u32 {
    match (height, format) {
        (BarHeight::Completa, BarFormat::Faixa) => 44,
        (BarHeight::Compacta, BarFormat::Faixa) => 28,
        (BarHeight::Completa, BarFormat::Pilulas) => 36,
        (BarHeight::Compacta, BarFormat::Pilulas) => 24,
    }
}

pub fn settings_for(theme: &Theme, height: BarHeight, format: BarFormat) -> IslandSettings {
    let builtin = BUILTIN.iter().position(|(id, _)| *id == theme.id);
    // Um tema embutido sobrescrito pelo usuário deixa de bater com a paleta da ilha: vira custom.
    let pristine = builtin.is_some_and(|i| parse_theme(BUILTIN[i].0, BUILTIN[i].0, BUILTIN[i].1).ok().is_some_and(|b| b.colors == theme.colors));
    let island_theme = match builtin {
        Some(index) if pristine => IslandTheme::Preset { index, id: theme.id.clone() },
        _ => IslandTheme::Custom {
            background: theme.colors.background,
            text: theme.colors.text,
            text_secondary: theme.colors.text_secondary,
            border: theme.colors.border,
        },
    };
    IslandSettings {
        bar_height: if height == BarHeight::Completa { 44 } else { 32 },
        pill_height: island_pill_height(height, format),
        theme: island_theme,
        spring: theme.animation.spring,
        speed: theme.animation.speed,
        merged: format == BarFormat::Faixa,
        shadow: theme.material.shadow,
    }
}

fn hex(c: Rgba) -> String {
    let b = |v: f32| (v.clamp(0.0, 1.0) * 255.0).round() as u8;
    format!("#{:02X}{:02X}{:02X}", b(c.r), b(c.g), b(c.b))
}

fn speed_name(v: f32) -> &'static str {
    const OPTS: [(f32, &str); 6] = [(0.5, "very-slow"), (0.75, "slow"), (1.0, "normal"), (1.35, "fast"), (1.65, "very-fast"), (2.0, "ultra-fast")];
    OPTS.iter().min_by(|a, b| (a.0 - v).abs().total_cmp(&(b.0 - v).abs())).map_or("normal", |o| o.1)
}

fn spring_name(s: SpringStyle) -> &'static str {
    match s {
        SpringStyle::Smooth => "smooth",
        SpringStyle::Default => "default",
        SpringStyle::Bouncy => "bouncy",
        SpringStyle::Snappy => "snappy",
    }
}

/// Troca/insere `key=value` dentro de `[section]`, preservando o resto (comentários, outras chaves, ordem).
fn set(lines: &mut Vec<String>, section: &str, key: &str, value: &str) {
    let header = format!("[{section}]");
    let start = match lines.iter().position(|l| l.trim().eq_ignore_ascii_case(&header)) {
        Some(i) => i,
        None => {
            if lines.last().is_some_and(|l| !l.trim().is_empty()) {
                lines.push(String::new());
            }
            lines.push(header);
            lines.len() - 1
        }
    };
    let end = lines[start + 1..].iter().position(|l| l.trim_start().starts_with('[')).map_or(lines.len(), |p| start + 1 + p);
    let line = format!("{key}={value}");
    for l in &mut lines[start + 1..end] {
        let t = l.trim_start();
        let is_key = !t.starts_with(';') && !t.starts_with('#') && t.split('=').next().is_some_and(|k| k.trim().eq_ignore_ascii_case(key));
        if is_key {
            *l = line;
            return;
        }
    }
    let mut at = end;
    while at > start + 1 && lines[at - 1].trim().is_empty() {
        at -= 1;
    }
    lines.insert(at, line);
}

fn join(lines: Vec<String>) -> String {
    let mut s = lines.join("\r\n");
    s.push_str("\r\n");
    s
}

pub fn apply_ini(text: &str, s: &IslandSettings) -> String {
    let mut l: Vec<String> = text.lines().map(str::to_string).collect();
    set(&mut l, "Zefiro", "Anchor", "bar");
    set(&mut l, "Zefiro", "BarHeight", &s.bar_height.to_string());
    set(&mut l, "Zefiro", "PillHeight", &s.pill_height.to_string());
    match &s.theme {
        IslandTheme::Preset { id, .. } => {
            set(&mut l, "Themes", "ThemePreset", id);
            set(&mut l, "Themes", "PillBgColor", "#08080A");
            set(&mut l, "Themes", "TextPrimaryColor", "#FFFFFF");
            set(&mut l, "Themes", "TextSecondaryColor", "#9B9BA5");
            set(&mut l, "Themes", "ContourBorderHex", "#1E1E22");
            set(&mut l, "Themes", "ContourBorderMode", "default");
        }
        IslandTheme::Custom { background, text, text_secondary, border } => {
            set(&mut l, "Themes", "ThemePreset", "custom");
            set(&mut l, "Themes", "PillBgColor", &hex(*background));
            set(&mut l, "Themes", "TextPrimaryColor", &hex(*text));
            set(&mut l, "Themes", "TextSecondaryColor", &hex(*text_secondary));
            match border {
                Some(b) => {
                    set(&mut l, "Themes", "ContourBorderHex", &hex(*b));
                    set(&mut l, "Themes", "ContourBorderMode", "default");
                }
                None => set(&mut l, "Themes", "ContourBorderMode", "borderless"),
            }
        }
    }
    if s.merged {
        set(&mut l, "Themes", "ContourBorderMode", "borderless");
    }
    set(&mut l, "Themes", "DropShadow", if s.shadow && !s.merged { "1" } else { "0" });
    // O tom da capa do álbum pinta o fundo da ilha e a destaca da faixa.
    set(&mut l, "Themes", "AccentBloom", if s.merged { "0" } else { "100" });
    set(&mut l, "Animations", "AnimationStyle", spring_name(s.spring));
    set(&mut l, "Animations", "AnimationSpeed", speed_name(s.speed));
    join(l)
}

pub fn apply_state(text: &str, s: &IslandSettings) -> String {
    let index = match &s.theme {
        IslandTheme::Preset { index, .. } => *index,
        IslandTheme::Custom { .. } => BUILTIN.len(),
    };
    let mut l: Vec<String> = text.lines().map(str::to_string).collect();
    set(&mut l, "Values", "ColorThemeV2", &index.to_string());
    join(l)
}

/// Destaque que a ilha publica no `island-state.ini` (`[Values] ZefiroAccent`, 0xRRGGBB em decimal):
/// a capa do álbum tocando, ou o destaque do tema da ilha. A barra usa no lugar do `accent = "auto"`.
pub fn read_accent(state: &str) -> Option<Rgba> {
    let mut in_values = false;
    for line in state.lines().map(str::trim) {
        if line.starts_with('[') {
            in_values = line.eq_ignore_ascii_case("[Values]");
        } else if in_values && let Some((k, v)) = line.split_once('=') && k.trim().eq_ignore_ascii_case("ZefiroAccent") {
            let rgb: u32 = v.trim().parse().ok()?;
            return Some(Rgba::rgb((rgb >> 16) as u8, (rgb >> 8) as u8, rgb as u8));
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use zefiro_core::config::{BarFormat, BarHeight};
    use zefiro_resources::theme::{BUILTIN, parse_theme};

    fn theme(id: &str) -> zefiro_resources::theme::Theme {
        let (_, t) = BUILTIN.iter().find(|(i, _)| *i == id).unwrap();
        parse_theme(id, id, t).unwrap()
    }

    #[test]
    fn pill_heights_follow_the_spec_table() {
        assert_eq!(island_pill_height(BarHeight::Completa, BarFormat::Faixa), 44);
        assert_eq!(island_pill_height(BarHeight::Compacta, BarFormat::Faixa), 28);
        assert_eq!(island_pill_height(BarHeight::Completa, BarFormat::Pilulas), 36);
        assert_eq!(island_pill_height(BarHeight::Compacta, BarFormat::Pilulas), 24);
    }

    #[test]
    fn preset_theme_writes_preset_and_resets_hex_fields() {
        let s = settings_for(&theme("nord"), BarHeight::Compacta, BarFormat::Pilulas);
        let input = "; comentário do usuário\n[Zefiro]\nAnchor=mod\nRenderer=software\n\n[Themes]\nThemePreset=obsidian\nPillBgColor=#123456\nFontFamily=Aptos\n";
        let out = apply_ini(input, &s);
        assert!(out.contains("; comentário do usuário"));
        assert!(out.contains("Renderer=software"));
        assert!(out.contains("FontFamily=Aptos"));
        assert!(out.contains("Anchor=bar") && !out.contains("Anchor=mod"));
        assert!(out.contains("BarHeight=32"));
        assert!(out.contains("PillHeight=24"));
        assert!(out.contains("ThemePreset=nord"));
        assert!(out.contains("PillBgColor=#08080A"));
        assert!(out.contains("[Animations]\r\nAnimationStyle=default\r\nAnimationSpeed=normal"), "{out}");
    }

    #[test]
    fn custom_theme_writes_its_colors() {
        let mut t = theme("obsidian");
        t.id = "meu".into();
        t.colors.background = zefiro_platform::color::Rgba::rgb(0x10, 0x20, 0x30);
        t.colors.border = None;
        let out = apply_ini("", &settings_for(&t, BarHeight::Completa, BarFormat::Faixa));
        assert!(out.contains("ThemePreset=custom"));
        assert!(out.contains("PillBgColor=#102030"));
        assert!(out.contains("ContourBorderMode=borderless"));
    }

    #[test]
    fn strip_format_merges_the_island_into_the_bar_without_contour_or_shadow() {
        for height in [BarHeight::Completa, BarHeight::Compacta] {
            let out = apply_ini("", &settings_for(&theme("obsidian"), height, BarFormat::Faixa));
            assert!(out.contains("ContourBorderMode=borderless"), "{out}");
            assert!(out.contains("DropShadow=0"), "{out}");
            assert!(out.contains("AccentBloom=0"), "o tom da capa destacaria a ilha da faixa: {out}");
        }
    }

    #[test]
    fn pill_format_keeps_the_island_as_a_pill_with_contour_and_shadow() {
        let out = apply_ini("", &settings_for(&theme("obsidian"), BarHeight::Compacta, BarFormat::Pilulas));
        assert!(out.contains("ContourBorderMode=default"), "{out}");
        assert!(out.contains("DropShadow=1"), "{out}");
        assert!(out.contains("AccentBloom=100"), "{out}");
        // Voltando de faixa para pílulas, o arquivo antigo é corrigido.
        let back = apply_ini(&apply_ini("", &settings_for(&theme("obsidian"), BarHeight::Compacta, BarFormat::Faixa)), &settings_for(&theme("obsidian"), BarHeight::Compacta, BarFormat::Pilulas));
        assert!(back.contains("ContourBorderMode=default") && back.contains("DropShadow=1"), "{back}");
    }

    #[test]
    fn reads_the_accent_the_island_publishes() {
        // A ilha grava 0xRRGGBB como inteiro decimal (o shim só guarda inteiros).
        let state = "; Estado salvo pela ilha
[Values]
ColorThemeV2=0
ZefiroAccent=13930294
";
        assert_eq!(read_accent(state), Some(Rgba::rgb(0xD4, 0x8F, 0x36)));
        assert_eq!(read_accent("[Values]
ColorThemeV2=0
"), None);
        assert_eq!(read_accent("[Values]
ZefiroAccent=lixo
"), None);
        assert_eq!(read_accent("[Outra]
ZefiroAccent=255
"), None, "só vale na seção [Values]");
    }

    #[test]
    fn modified_builtin_is_sent_as_custom() {
        let mut t = theme("nord");
        t.colors.text = zefiro_platform::color::Rgba::rgb(1, 2, 3);
        assert!(matches!(settings_for(&t, BarHeight::Completa, BarFormat::Faixa).theme, IslandTheme::Custom { .. }));
    }

    #[test]
    fn state_file_gets_the_theme_index_and_keeps_other_values() {
        let s = settings_for(&theme("plum"), BarHeight::Completa, BarFormat::Faixa);
        let out = apply_state("[Values]\r\nColorThemeV2=0\r\nPinnedExpanded=1\r\n", &s);
        assert!(out.contains("ColorThemeV2=6"));
        assert!(out.contains("PinnedExpanded=1"));
        let empty = apply_state("", &s);
        assert!(empty.contains("[Values]") && empty.contains("ColorThemeV2=6"));
    }

    #[test]
    fn speed_maps_to_the_nearest_island_option() {
        let mut t = theme("obsidian");
        t.animation.speed = 1.6; // 1,5 empata entre fast (1,35) e very-fast (1,65)
        let out = apply_ini("", &settings_for(&t, BarHeight::Completa, BarFormat::Faixa));
        assert!(out.contains("AnimationSpeed=very-fast"), "{out}");
    }
}
