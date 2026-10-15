//! Pílula → painel: onde o painel fica, raios, opacidade do conteúdo e a janela que contém a animação.

use zefiro_platform::geometry::{Rect, Size};
use zefiro_resources::theme::ShapeStyle;

pub const PANEL_GAP_DIP: f32 = 6.0;
pub const SCREEN_MARGIN_DIP: f32 = 8.0;
pub const SHADOW_MARGIN_DIP: f32 = 24.0;
/// O conteúdo do chip some neste tempo quando o painel abre.
pub const CHIP_FADE_S: f32 = 0.17;

/// Mesma regra de `content_alpha`, como expressão do compositor. `g` = geometria da forma, `H` = altura final.
pub const CONTENT_ALPHA_EXPRESSION: &str = "Clamp((g.Size.Y - 0.6 * H) / (0.4 * H), 0, 1)";

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Shape { pub rect: Rect, pub radius: f32 }

pub fn panel_rect(chip: Rect, band_bottom: f32, content: Size, monitor_w: f32, scale: f32) -> Rect {
    let margin = (SCREEN_MARGIN_DIP * scale).round();
    let x = (chip.center_x() - content.w / 2.0).round().clamp(margin, (monitor_w - margin - content.w).max(margin));
    let y = (band_bottom + PANEL_GAP_DIP * scale).round();
    Rect::new(x, y, content.w, content.h)
}

pub fn pill_radius(h: f32, radius_max_px: f32, style: ShapeStyle, scale: f32) -> f32 {
    match style {
        ShapeStyle::W11 => 8.0 * scale,
        ShapeStyle::Pill => (h / 2.0).min(radius_max_px),
    }
}

pub fn panel_radius(panel_radius_px: f32, style: ShapeStyle, scale: f32) -> f32 {
    match style {
        ShapeStyle::W11 => 8.0 * scale,
        ShapeStyle::Pill => panel_radius_px,
    }
}

pub fn content_alpha(h: f32, final_h: f32) -> f32 {
    if final_h <= 0.0 {
        return 0.0;
    }
    if h >= final_h {
        return 1.0; // na altura final o conteúdo fica totalmente opaco (sem erro de arredondamento)
    }
    ((h - 0.6 * final_h) / (0.4 * final_h)).clamp(0.0, 1.0)
}

/// Retângulo da janela do painel (coordenadas do monitor): chip + painel + margem de sombra, sem passar do topo.
pub fn window_bounds(chip: Rect, panel: Rect, scale: f32) -> Rect {
    let m = (SHADOW_MARGIN_DIP * scale).round();
    let u = chip.union(panel);
    let top = (u.y - m).max(0.0);
    Rect::new(u.x - m, top, u.w + 2.0 * m, u.bottom() + m - top)
}

pub fn to_local(shape: Shape, window: Rect) -> Shape {
    Shape { rect: Rect::new(shape.rect.x - window.x, shape.rect.y - window.y, shape.rect.w, shape.rect.h), radius: shape.radius }
}

#[cfg(test)]
mod tests {
    use super::*;
    use zefiro_resources::theme::ShapeStyle;

    #[test]
    fn panel_is_centered_under_chip_and_below_the_band() {
        let chip = Rect::new(900.0, 6.0, 120.0, 32.0);
        let p = panel_rect(chip, 44.0, Size { w: 352.0, h: 186.0 }, 1920.0, 1.0);
        assert_eq!(p, Rect::new(784.0, 50.0, 352.0, 186.0));
    }

    #[test]
    fn panel_stays_8_dip_inside_the_screen() {
        let right = panel_rect(Rect::new(1800.0, 6.0, 110.0, 32.0), 44.0, Size { w: 352.0, h: 100.0 }, 1920.0, 1.0);
        assert_eq!(right.right(), 1912.0);
        let left = panel_rect(Rect::new(8.0, 6.0, 80.0, 32.0), 44.0, Size { w: 300.0, h: 100.0 }, 1920.0, 1.0);
        assert_eq!(left.x, 8.0);
        let scaled = panel_rect(Rect::new(8.0, 6.0, 80.0, 32.0), 55.0, Size { w: 300.0, h: 100.0 }, 2400.0, 1.25);
        assert_eq!((scaled.x, scaled.y), (10.0, 63.0)); // 8 × 1,25 = 10; 55 + 6 × 1,25 = 62,5 → 63
    }

    #[test]
    fn radii_follow_the_island_rules() {
        assert_eq!(pill_radius(32.0, 44.0, ShapeStyle::Pill, 1.0), 16.0);
        assert_eq!(pill_radius(200.0, 44.0, ShapeStyle::Pill, 1.0), 44.0);
        assert_eq!(pill_radius(32.0, 44.0, ShapeStyle::W11, 1.5), 12.0); // 8 DIP
        assert_eq!(panel_radius(24.0, ShapeStyle::Pill, 1.0), 24.0);
        assert_eq!(panel_radius(30.0, ShapeStyle::W11, 1.0), 8.0);
    }

    #[test]
    fn content_alpha_starts_at_60_percent_of_final_height() {
        assert_eq!(content_alpha(0.0, 200.0), 0.0);
        assert_eq!(content_alpha(120.0, 200.0), 0.0);
        assert!((content_alpha(160.0, 200.0) - 0.5).abs() < 1e-6);
        assert_eq!(content_alpha(200.0, 200.0), 1.0);
        assert_eq!(content_alpha(230.0, 200.0), 1.0); // mola passando do alvo
        assert!(CONTENT_ALPHA_EXPRESSION.contains("0.6") && CONTENT_ALPHA_EXPRESSION.contains("0.4"));
    }

    #[test]
    fn window_bounds_cover_chip_panel_and_shadow() {
        let chip = Rect::new(900.0, 6.0, 120.0, 32.0);
        let panel = Rect::new(784.0, 50.0, 352.0, 186.0);
        let w = window_bounds(chip, panel, 1.0);
        assert_eq!(w, Rect::new(760.0, 0.0, 400.0, 260.0)); // ±24 de sombra, sem passar do topo
        let local = to_local(Shape { rect: chip, radius: 16.0 }, w);
        assert_eq!(local.rect, Rect::new(140.0, 6.0, 120.0, 32.0));
    }
}
