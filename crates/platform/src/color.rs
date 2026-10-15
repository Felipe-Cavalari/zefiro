//! Cor RGBA em ponto flutuante, com o mesmo parser de hex da ilha.

#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Rgba { pub r: f32, pub g: f32, pub b: f32, pub a: f32 }

impl Rgba {
    pub const TRANSPARENT: Rgba = Rgba { r: 0.0, g: 0.0, b: 0.0, a: 0.0 };
    pub const WHITE: Rgba = Rgba { r: 1.0, g: 1.0, b: 1.0, a: 1.0 };
    pub const BLACK: Rgba = Rgba { r: 0.0, g: 0.0, b: 0.0, a: 1.0 };

    pub fn rgb(r: u8, g: u8, b: u8) -> Rgba {
        Rgba { r: r as f32 / 255.0, g: g as f32 / 255.0, b: b as f32 / 255.0, a: 1.0 }
    }

    /// Aceita 3, 4, 6 ou 8 dígitos (RGB, RGBA, RRGGBB, RRGGBBAA), com ou sem '#'.
    pub fn from_hex(s: &str) -> Option<Rgba> {
        let h = s.trim().trim_start_matches('#');
        if !h.chars().all(|c| c.is_ascii_hexdigit()) {
            return None;
        }
        let nib = |i: usize| u8::from_str_radix(&h[i..i + 1], 16).ok().map(|v| v * 17);
        let byte = |i: usize| u8::from_str_radix(&h[i..i + 2], 16).ok();
        let (r, g, b, a) = match h.len() {
            3 => (nib(0)?, nib(1)?, nib(2)?, 255),
            4 => (nib(0)?, nib(1)?, nib(2)?, nib(3)?),
            6 => (byte(0)?, byte(2)?, byte(4)?, 255),
            8 => (byte(0)?, byte(2)?, byte(4)?, byte(6)?),
            _ => return None,
        };
        Some(Rgba { a: a as f32 / 255.0, ..Rgba::rgb(r, g, b) })
    }

    pub fn with_alpha(self, a: f32) -> Rgba {
        Rgba { a: a.clamp(0.0, 1.0), ..self }
    }

    /// Interpolação linear de cada canal (t = 0 → self, t = 1 → other).
    pub fn mix(self, other: Rgba, t: f32) -> Rgba {
        let l = |a: f32, b: f32| a + (b - a) * t;
        Rgba { r: l(self.r, other.r), g: l(self.g, other.g), b: l(self.b, other.b), a: l(self.a, other.a) }
    }

    /// Luminância relativa (WCAG 2), igual à `RelativeLuminance` da ilha.
    pub fn relative_luminance(&self) -> f64 {
        let lin = |c: f32| {
            let v = c.clamp(0.0, 1.0) as f64;
            if v <= 0.04045 { v / 12.92 } else { ((v + 0.055) / 1.055).powf(2.4) }
        };
        0.2126 * lin(self.r) + 0.7152 * lin(self.g) + 0.0722 * lin(self.b)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_3_4_6_and_8_digit_hex_with_or_without_hash() {
        assert_eq!(Rgba::from_hex("#08080A"), Some(Rgba::rgb(8, 8, 10)));
        assert_eq!(Rgba::from_hex("08080A"), Some(Rgba::rgb(8, 8, 10)));
        assert_eq!(Rgba::from_hex("#fff"), Some(Rgba::rgb(255, 255, 255)));
        let half = Rgba::from_hex("#FFFFFF80").unwrap();
        assert!((half.a - 128.0 / 255.0).abs() < 1e-6);
        let short_alpha = Rgba::from_hex("#0008").unwrap();
        assert!((short_alpha.a - 136.0 / 255.0).abs() < 1e-6);
    }

    #[test]
    fn rejects_garbage() {
        for s in ["", "#", "#12", "#12345", "#GGGGGG", "azul", "#1234567"] {
            assert_eq!(Rgba::from_hex(s), None, "{s}");
        }
    }

    #[test]
    fn luminance_matches_wcag() {
        assert!((Rgba::WHITE.relative_luminance() - 1.0).abs() < 1e-9);
        assert!(Rgba::BLACK.relative_luminance().abs() < 1e-9);
        assert!((Rgba::rgb(8, 8, 10).relative_luminance() - 0.0025).abs() < 1e-3);
    }

    #[test]
    fn mix_and_alpha() {
        let m = Rgba::BLACK.mix(Rgba::WHITE, 0.25);
        assert!((m.r - 0.25).abs() < 1e-6 && (m.a - 1.0).abs() < 1e-6);
        assert_eq!(Rgba::WHITE.with_alpha(0.5).a, 0.5);
        assert_eq!(Rgba::WHITE.with_alpha(3.0).a, 1.0);
    }
}
