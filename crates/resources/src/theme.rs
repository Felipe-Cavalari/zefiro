//! Arquivo de tema (TOML) e seus tipos.

use serde::Deserialize;
use zefiro_platform::color::Rgba;

pub const BUILTIN: [(&str, &str); 9] = [
    ("obsidian", include_str!("../themes/obsidian.toml")),
    ("graphite", include_str!("../themes/graphite.toml")),
    ("slate", include_str!("../themes/slate.toml")),
    ("nord", include_str!("../themes/nord.toml")),
    ("evergreen", include_str!("../themes/evergreen.toml")),
    ("espresso", include_str!("../themes/espresso.toml")),
    ("plum", include_str!("../themes/plum.toml")),
    ("porcelain", include_str!("../themes/porcelain.toml")),
    ("cobalto", include_str!("../themes/cobalto.toml")),
];

#[derive(Debug, Clone, PartialEq)]
pub struct Theme {
    pub id: String,
    pub name: String,
    pub colors: ThemeColors,
    pub material: Material,
    pub shape: ShapeSpec,
    pub font: FontSpec,
    pub animation: AnimationSpec,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ThemeColors {
    pub background: Rgba,
    pub text: Rgba,
    pub text_secondary: Rgba,
    pub border: Option<Rgba>,
    pub accent: Accent,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Accent { Auto, System, Color(Rgba) }

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Backdrop { None, Acrylic }

#[derive(Debug, Clone, PartialEq)]
pub struct Material { pub opacity: f32, pub depth: bool, pub shadow: bool, pub backdrop: Backdrop, pub backdrop_fill: f32 }

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ShapeStyle { Pill, W11 }

#[derive(Debug, Clone, PartialEq)]
pub struct ShapeSpec { pub style: ShapeStyle, pub radius_max: f32, pub panel_radius: f32 }

#[derive(Debug, Clone, PartialEq)]
pub struct FontSpec { pub family: String, pub scale: f32 }

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SpringStyle { Smooth, Default, Bouncy, Snappy }

#[derive(Debug, Clone, PartialEq)]
pub struct AnimationSpec { pub spring: SpringStyle, pub speed: f32 }

#[derive(Debug, Clone, PartialEq)]
pub struct ThemeError { pub source: String, pub line: Option<usize>, pub message: String }

// ── Formato do arquivo (serde) ──────────────────────────────────────────────

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct File {
    name: String,
    colors: FileColors,
    #[serde(default)]
    material: FileMaterial,
    #[serde(default)]
    shape: FileShape,
    #[serde(default)]
    font: FileFont,
    #[serde(default)]
    animation: FileAnimation,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct FileColors {
    background: toml::Spanned<String>,
    text: toml::Spanned<String>,
    text_secondary: toml::Spanned<String>,
    #[serde(default)]
    border: Option<toml::Spanned<String>>,
    #[serde(default)]
    accent: Option<toml::Spanned<String>>,
}

#[derive(Deserialize)]
#[serde(default, deny_unknown_fields)]
struct FileMaterial { opacity: f32, depth: bool, shadow: bool, backdrop: String, backdrop_fill: f32 }
impl Default for FileMaterial {
    fn default() -> Self { Self { opacity: 0.96, depth: true, shadow: true, backdrop: "none".into(), backdrop_fill: 0.45 } }
}

#[derive(Deserialize)]
#[serde(default, deny_unknown_fields)]
struct FileShape { style: String, radius_max: f32, panel_radius: f32 }
impl Default for FileShape {
    fn default() -> Self { Self { style: "pill".into(), radius_max: 44.0, panel_radius: 24.0 } }
}

#[derive(Deserialize)]
#[serde(default, deny_unknown_fields)]
struct FileFont { family: String, scale: f32 }
impl Default for FileFont {
    fn default() -> Self { Self { family: "Segoe UI Variable".into(), scale: 1.0 } }
}

#[derive(Deserialize)]
#[serde(default, deny_unknown_fields)]
struct FileAnimation { spring: String, speed: f32 }
impl Default for FileAnimation {
    fn default() -> Self { Self { spring: "default".into(), speed: 1.0 } }
}

fn line_of(text: &str, offset: usize) -> usize {
    text[..offset.min(text.len())].matches('\n').count() + 1
}

/// Lê um tema. `source_name` aparece nas mensagens de erro (nome do arquivo).
pub fn parse_theme(id: &str, source_name: &str, text: &str) -> Result<Theme, ThemeError> {
    let err = |line: Option<usize>, message: String| ThemeError { source: source_name.to_string(), line, message };
    let file: File = toml::from_str(text)
        .map_err(|e| err(e.span().map(|s| line_of(text, s.start)), e.message().to_string()))?;

    let color = |key: &str, v: &toml::Spanned<String>| {
        Rgba::from_hex(v.get_ref())
            .ok_or_else(|| err(Some(line_of(text, v.span().start)), format!("cor inválida em `{key}`: {:?}", v.get_ref())))
    };
    let border = match &file.colors.border {
        Some(v) if v.get_ref().eq_ignore_ascii_case("none") => None,
        Some(v) => Some(color("border", v)?),
        None => None,
    };
    let accent = match &file.colors.accent {
        None => Accent::Auto,
        Some(v) if v.get_ref().eq_ignore_ascii_case("auto") => Accent::Auto,
        Some(v) if v.get_ref().eq_ignore_ascii_case("system") => Accent::System,
        Some(v) => Accent::Color(color("accent", v)?),
    };
    let backdrop = match file.material.backdrop.as_str() {
        "none" => Backdrop::None,
        "acrylic" => Backdrop::Acrylic,
        other => return Err(err(None, format!("backdrop desconhecido: {other:?} (use none ou acrylic)"))),
    };
    let style = match file.shape.style.as_str() {
        "pill" => ShapeStyle::Pill,
        "w11" => ShapeStyle::W11,
        other => return Err(err(None, format!("shape.style desconhecido: {other:?} (use pill ou w11)"))),
    };
    let spring = match file.animation.spring.as_str() {
        "smooth" => SpringStyle::Smooth,
        "default" => SpringStyle::Default,
        "bouncy" => SpringStyle::Bouncy,
        "snappy" => SpringStyle::Snappy,
        other => return Err(err(None, format!("animation.spring desconhecido: {other:?}"))),
    };

    Ok(Theme {
        id: id.to_string(),
        name: file.name,
        colors: ThemeColors {
            background: color("background", &file.colors.background)?,
            text: color("text", &file.colors.text)?,
            text_secondary: color("text_secondary", &file.colors.text_secondary)?,
            border,
            accent,
        },
        material: Material {
            opacity: file.material.opacity.clamp(0.35, 1.0),
            depth: file.material.depth,
            shadow: file.material.shadow,
            backdrop,
            backdrop_fill: file.material.backdrop_fill.clamp(0.0, 1.0),
        },
        shape: ShapeSpec {
            style,
            radius_max: file.shape.radius_max.clamp(0.0, 64.0),
            panel_radius: file.shape.panel_radius.clamp(0.0, 48.0),
        },
        font: FontSpec { family: file.font.family, scale: file.font.scale.clamp(0.7, 1.6) },
        animation: AnimationSpec { spring, speed: file.animation.speed.clamp(0.5, 2.0) },
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use zefiro_platform::color::Rgba;

    #[test]
    fn all_builtin_themes_parse() {
        for (id, text) in BUILTIN {
            let t = parse_theme(id, id, text).unwrap_or_else(|e| panic!("{id}: {e:?}"));
            assert_eq!(t.id, id);
            assert_eq!(t.material.opacity, 0.96);
            assert_eq!(t.shape.radius_max, 44.0);
            assert_eq!(t.shape.panel_radius, 24.0);
        }
        assert_eq!(BUILTIN[0].0, "obsidian");
    }

    #[test]
    fn obsidian_matches_the_island_palette() {
        let t = parse_theme("obsidian", "obsidian", BUILTIN[0].1).unwrap();
        assert_eq!(t.name, "Obsidian");
        assert_eq!(t.colors.background, Rgba::rgb(0x08, 0x08, 0x0A));
        assert_eq!(t.colors.text_secondary, Rgba::rgb(0x9B, 0x9B, 0xA5));
        assert_eq!(t.colors.border, Some(Rgba::rgb(0x1E, 0x1E, 0x22)));
        assert_eq!(t.colors.accent, Accent::Auto);
        assert_eq!(t.material.backdrop, Backdrop::None);
        assert_eq!(t.animation.spring, SpringStyle::Default);
    }

    #[test]
    fn cobalto_uses_acrylic_and_a_fixed_blue_accent() {
        let (_, text) = BUILTIN.iter().find(|(id, _)| *id == "cobalto").expect("cobalto embutido");
        let t = parse_theme("cobalto", "cobalto", text).unwrap();
        assert_eq!(t.name, "Cobalto");
        assert_eq!(t.material.backdrop, Backdrop::Acrylic);
        assert_eq!(t.colors.accent, Accent::Color(Rgba::rgb(0x3B, 0x82, 0xF6)));
        // A capa do álbum não muda o destaque deste tema.
        let cover = Rgba::rgb(0xD4, 0x8F, 0x36);
        let blue = crate::tokens::ensure_contrast(Rgba::rgb(0x3B, 0x82, 0xF6), t.colors.background);
        assert_eq!(crate::tokens::derive_tokens(&t, Rgba::rgb(0, 120, 215), Some(cover)).accent, blue);
        assert_eq!(crate::tokens::derive_tokens(&t, Rgba::rgb(0, 120, 215), None).accent, blue);
    }

    #[test]
    fn missing_sections_take_defaults() {
        let t = parse_theme("mini", "mini.toml", "name = \"Mini\"\n[colors]\nbackground = \"#000\"\ntext = \"#fff\"\ntext_secondary = \"#888\"\n").unwrap();
        assert_eq!(t.colors.border, None);
        assert_eq!(t.material.opacity, 0.96);
        assert_eq!(t.shape.style, ShapeStyle::Pill);
        assert_eq!(t.font.family, "Segoe UI Variable");
        assert_eq!(t.animation.speed, 1.0);
    }

    #[test]
    fn invalid_color_reports_line_and_key() {
        let text = "name = \"X\"\n[colors]\nbackground = \"#000\"\ntext = \"azul\"\ntext_secondary = \"#888\"\n";
        let e = parse_theme("x", "x.toml", text).unwrap_err();
        assert_eq!(e.source, "x.toml");
        assert_eq!(e.line, Some(4));
        assert!(e.message.contains("text"), "{}", e.message);
    }

    #[test]
    fn syntax_error_reports_line() {
        let e = parse_theme("x", "x.toml", "name = \"X\"\n[colors\n").unwrap_err();
        assert_eq!(e.line, Some(2));
    }

    #[test]
    fn values_are_clamped_to_sane_ranges() {
        let text = "name = \"X\"\n[colors]\nbackground = \"#000\"\ntext = \"#fff\"\ntext_secondary = \"#888\"\n[material]\nopacity = 9\n[animation]\nspeed = 0.1\n[font]\nscale = 5\n";
        let t = parse_theme("x", "x.toml", text).unwrap();
        assert_eq!(t.material.opacity, 1.0);
        assert_eq!(t.animation.speed, 0.5);
        assert_eq!(t.font.scale, 1.6);
    }
}
