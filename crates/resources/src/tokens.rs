//! Tokens derivados do tema, com as regras da ilha (`BuildMaterialTokens`, `EnsureContrastAgainstBackground`).

use crate::theme::{Accent, Theme};
use zefiro_platform::color::Rgba;

#[derive(Debug, Clone, PartialEq)]
pub struct Tokens {
    pub dark: bool,
    pub fill: Rgba,
    pub raised: Rgba,
    pub raised_strong: Rgba,
    pub hairline: Rgba,
    pub border: Option<Rgba>,
    pub shadow: Rgba,
    pub text: Rgba,
    pub text_secondary: Rgba,
    pub text_tertiary: Rgba,
    pub accent: Rgba,
    pub accent_soft: Rgba,
}

pub fn derive_tokens(theme: &Theme, system_accent: Rgba, cover_accent: Option<Rgba>) -> Tokens {
    let bg = theme.colors.background;
    let dark = bg.relative_luminance() < 0.45;
    let lift = if dark { Rgba::WHITE } else { Rgba::BLACK };
    let raw_accent = match theme.colors.accent {
        Accent::Auto => cover_accent.unwrap_or(system_accent),
        Accent::System => system_accent,
        Accent::Color(c) => c,
    };
    let accent = ensure_contrast(raw_accent, bg);
    Tokens {
        dark,
        fill: bg.with_alpha(bg.a * theme.material.opacity),
        raised: lift.with_alpha(if dark { 0.085 } else { 0.062 }),
        raised_strong: lift.with_alpha(if dark { 0.150 } else { 0.105 }),
        hairline: lift.with_alpha(if dark { 0.100 } else { 0.085 }),
        border: theme.colors.border,
        shadow: Rgba::BLACK.with_alpha(0.55),
        text: theme.colors.text.with_alpha(0.98),
        text_secondary: theme.colors.text_secondary.with_alpha(0.88),
        text_tertiary: theme.colors.text_secondary.mix(bg, 0.35).with_alpha(0.72),
        accent,
        accent_soft: accent.with_alpha(0.18),
    }
}

pub fn contrast_ratio(a: Rgba, b: Rgba) -> f64 {
    let (la, lb) = (a.relative_luminance(), b.relative_luminance());
    (la.max(lb) + 0.05) / (la.min(lb) + 0.05)
}

/// Porte de `EnsureContrastAgainstBackground`: ajusta a luminosidade (HSL) em passos de 0,02
/// até 3:1, clareando em fundo escuro e escurecendo em fundo claro.
pub fn ensure_contrast(candidate: Rgba, background: Rgba) -> Rgba {
    let (h, mut s, mut l) = rgb_to_hsl(candidate);
    if s > 0.01 {
        s = s.max(0.35);
    }
    let on_dark = background.relative_luminance() < 0.45;
    l = if on_dark { l.min(0.85) } else { l.max(0.15) };
    let (step, limit) = if on_dark { (0.02, 0.95) } else { (-0.02, 0.06) };
    let mut c = hsl_to_rgb(h, s, l, candidate.a);
    while contrast_ratio(c, background) < 3.0 && (if on_dark { l < limit } else { l > limit }) {
        l += step;
        c = hsl_to_rgb(h, s, l, candidate.a);
    }
    c
}

pub(crate) fn rgb_to_hsl(c: Rgba) -> (f32, f32, f32) {
    let (r, g, b) = (c.r, c.g, c.b);
    let max = r.max(g).max(b);
    let min = r.min(g).min(b);
    let l = (max + min) / 2.0;
    if (max - min).abs() < f32::EPSILON {
        return (0.0, 0.0, l);
    }
    let d = max - min;
    let s = if l > 0.5 { d / (2.0 - max - min) } else { d / (max + min) };
    let mut h = if max == r {
        (g - b) / d + if g < b { 6.0 } else { 0.0 }
    } else if max == g {
        (b - r) / d + 2.0
    } else {
        (r - g) / d + 4.0
    };
    h *= 60.0;
    (h, s, l)
}

pub(crate) fn hsl_to_rgb(h: f32, s: f32, l: f32, a: f32) -> Rgba {
    let l = l.clamp(0.0, 1.0);
    if s <= 0.0 {
        return Rgba { r: l, g: l, b: l, a };
    }
    let q = if l < 0.5 { l * (1.0 + s) } else { l + s - l * s };
    let p = 2.0 * l - q;
    let hue = |mut t: f32| {
        if t < 0.0 {
            t += 1.0
        }
        if t > 1.0 {
            t -= 1.0
        }
        if t < 1.0 / 6.0 {
            p + (q - p) * 6.0 * t
        } else if t < 0.5 {
            q
        } else if t < 2.0 / 3.0 {
            p + (q - p) * (2.0 / 3.0 - t) * 6.0
        } else {
            p
        }
    };
    let hn = h / 360.0;
    Rgba { r: hue(hn + 1.0 / 3.0), g: hue(hn), b: hue(hn - 1.0 / 3.0), a }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::theme::{BUILTIN, parse_theme};

    fn theme(id: &str) -> crate::theme::Theme {
        let (_, text) = BUILTIN.iter().find(|(i, _)| *i == id).unwrap();
        parse_theme(id, id, text).unwrap()
    }

    #[test]
    fn dark_theme_uses_white_lifts_with_island_alphas() {
        let t = derive_tokens(&theme("obsidian"), Rgba::rgb(0, 120, 215), None);
        assert!(t.dark);
        assert_eq!(t.raised, Rgba::WHITE.with_alpha(0.085));
        assert_eq!(t.raised_strong, Rgba::WHITE.with_alpha(0.150));
        assert_eq!(t.hairline, Rgba::WHITE.with_alpha(0.100));
        assert_eq!(t.shadow, Rgba::BLACK.with_alpha(0.55));
        assert!((t.text.a - 0.98).abs() < 1e-6);
        assert!((t.text_secondary.a - 0.88).abs() < 1e-6);
        assert!((t.fill.a - 0.96).abs() < 1e-6);
    }

    #[test]
    fn light_theme_uses_black_lifts() {
        let t = derive_tokens(&theme("porcelain"), Rgba::rgb(0, 120, 215), None);
        assert!(!t.dark);
        assert_eq!(t.raised, Rgba::BLACK.with_alpha(0.062));
        assert_eq!(t.raised_strong, Rgba::BLACK.with_alpha(0.105));
        assert_eq!(t.hairline, Rgba::BLACK.with_alpha(0.085));
    }

    #[test]
    fn tertiary_is_secondary_mixed_35_percent_into_background() {
        let th = theme("obsidian");
        let t = derive_tokens(&th, Rgba::BLACK, None);
        let expected = th.colors.text_secondary.mix(th.colors.background, 0.35).with_alpha(0.72);
        assert_eq!(t.text_tertiary, expected);
    }

    #[test]
    fn auto_accent_prefers_cover_then_system() {
        let th = theme("obsidian");
        let cover = Rgba::rgb(230, 80, 60);
        assert_eq!(derive_tokens(&th, Rgba::rgb(0, 120, 215), Some(cover)).accent, ensure_contrast(cover, th.colors.background));
        assert_eq!(
            derive_tokens(&th, Rgba::rgb(0, 120, 215), None).accent,
            ensure_contrast(Rgba::rgb(0, 120, 215), th.colors.background)
        );
    }

    #[test]
    fn every_accent_reaches_3_to_1_on_every_builtin_theme() {
        let accents = [Rgba::rgb(0, 0, 0), Rgba::rgb(255, 255, 255), Rgba::rgb(76, 201, 240), Rgba::rgb(20, 20, 60), Rgba::rgb(240, 240, 150)];
        for (id, _) in BUILTIN {
            let bg = theme(id).colors.background;
            for a in accents {
                let c = ensure_contrast(a, bg);
                assert!(contrast_ratio(c, bg) >= 3.0 - 1e-6 || is_at_limit(c, bg), "{id} {a:?} -> {c:?}");
            }
        }
    }

    // O laço da ilha para no limite de luminosidade; nesse caso o contraste máximo possível é aceito.
    fn is_at_limit(c: Rgba, bg: Rgba) -> bool {
        let (_, _, l) = rgb_to_hsl(c);
        if bg.relative_luminance() < 0.45 { l >= 0.95 - 0.021 } else { l <= 0.06 + 0.021 }
    }

    #[test]
    fn border_none_stays_none() {
        let mut th = theme("obsidian");
        th.colors.border = None;
        assert_eq!(derive_tokens(&th, Rgba::BLACK, None).border, None);
    }
}
