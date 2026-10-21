//! Formatos Fluent (Win11 Fluent Onyx) e Vidro (Frosted Glass Revamp) do YASB, ADR-0018.

use zefiro_core::config::{BarFormat, BarHeight};
use zefiro_platform::geometry::Rect;
use zefiro_ui::bar_layout::{BarLayout, ChipBacking, ChipSpec, layout_sections, metrics};

fn chip(id: &str, width: f32) -> ChipSpec {
    ChipSpec { id: id.into(), content_w: width, wide: false }
}

fn layout(format: BarFormat, height: BarHeight, scale: f32) -> BarLayout {
    let start = [chip("zefiro", 12.0), chip("codex", 44.0)];
    let center = [chip("clock", 96.0)];
    let end = [chip("audio", 16.0), chip("power", 16.0)];
    layout_sections(1920.0, scale, height, format, [&start, &center, &end], None)
}

fn chip_rect(l: &BarLayout, id: &str) -> Rect {
    l.chips.iter().find(|c| c.id == id).unwrap().rect
}

#[test]
fn fluent_is_the_strip_with_a_card_per_chip() {
    let m = metrics(BarHeight::Completa, BarFormat::Fluent);
    assert_eq!((m.band, m.chip_h, m.chip_pad_x, m.chip_radius, m.margin), (36.0, 28.0, 12.0, 8.0, 0.0));
    assert_eq!(metrics(BarHeight::Compacta, BarFormat::Fluent).chip_h, 24.0);
    let l = layout(BarFormat::Fluent, BarHeight::Completa, 1.0);
    assert_eq!(l.strip, Some(l.band), "de ponta a ponta");
    assert!(l.divider, "com a linha embaixo, como o Win11");
    assert_eq!((l.strip_radius, l.chip_backing, l.chip_radius), (0.0, ChipBacking::Card, Some(8.0)));
    assert_eq!(chip_rect(&l, "zefiro"), Rect::new(8.0, 4.0, 36.0, 28.0), "12 de margem dentro do cartão");
    assert!(l.has_backing("codex") && l.has_backing("audio"));
    for bare in ["zefiro", "clock", "power", "island", "player"] {
        assert!(!l.has_backing(bare), "{bare} fica sem cartão, como no tema do YASB");
    }
}

#[test]
fn vidro_floats_the_whole_bar_away_from_the_edges() {
    let m = metrics(BarHeight::Completa, BarFormat::Vidro);
    assert_eq!((m.band, m.margin, m.strip_radius, m.chip_h, m.chip_radius), (46.0, 6.0, 16.0, 26.0, 10.0));
    let m = metrics(BarHeight::Compacta, BarFormat::Vidro);
    assert_eq!((m.band, m.chip_h), (42.0, 22.0));
    let l = layout(BarFormat::Vidro, BarHeight::Completa, 1.0);
    assert_eq!(l.band, Rect::new(0.0, 0.0, 1920.0, 46.0), "a AppBar reserva a barra e as margens");
    assert_eq!(l.strip, Some(Rect::new(6.0, 6.0, 1908.0, 34.0)));
    assert!(!l.divider);
    assert_eq!((l.strip_radius, l.chip_backing, l.chip_radius), (16.0, ChipBacking::Frost, Some(10.0)));
    // 6 de margem + 8 de borda; o chip fica centrado na barra flutuante: 6 + (34 − 26) / 2.
    assert_eq!(chip_rect(&l, "zefiro"), Rect::new(14.0, 10.0, 32.0, 26.0));
    assert_eq!(chip_rect(&l, "power").right(), 1906.0);
    assert!(l.has_backing("zefiro") && l.has_backing("clock"));
    assert!(!l.has_backing("island") && !l.has_backing("player"), "as pílulas da mídia não ganham fundo");
}

#[test]
fn the_island_pill_is_centered_in_the_content_area() {
    let island = [ChipSpec { id: "island".into(), content_w: 180.0, wide: true }];
    let vidro = layout_sections(1920.0, 1.0, BarHeight::Completa, BarFormat::Vidro, [&[], &island, &[]], None);
    // Barra flutuante de 6 a 40; pílula de 26 centrada: 6 + (34 − 26) / 2 = 10.
    assert_eq!(vidro.island_pill_y(), 10.0);
    let fluent = layout_sections(1920.0, 1.0, BarHeight::Completa, BarFormat::Fluent, [&[], &island, &[]], None);
    assert_eq!(fluent.island_pill_y(), 3.0);
}

#[test]
fn new_formats_scale_to_whole_pixels() {
    for format in [BarFormat::Fluent, BarFormat::Vidro] {
        for scale in [1.25, 1.5] {
            let l = layout(format, BarHeight::Compacta, scale);
            let strip = l.strip.unwrap();
            for r in std::iter::once(strip).chain(l.chips.iter().map(|c| c.rect)) {
                for v in [r.x, r.y, r.w, r.h] {
                    assert_eq!(v, v.round(), "{format:?} {scale}: {r:?}");
                }
            }
            for c in &l.chips {
                assert!(c.rect.y >= strip.y && c.rect.bottom() <= strip.bottom(), "{format:?} {scale}: {} fora da barra", c.id);
            }
        }
    }
}

#[test]
fn older_formats_keep_plain_chips() {
    for f in [BarFormat::Faixa, BarFormat::Pilulas, BarFormat::Ilhas] {
        let l = layout(f, BarHeight::Completa, 1.0);
        assert_eq!((l.chip_backing, l.chip_radius, l.strip_radius), (ChipBacking::None, None, 0.0), "{f:?}");
        assert!(!l.has_backing("codex"));
        assert_eq!(l.divider, f == BarFormat::Faixa);
    }
}
