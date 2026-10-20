//! Formato ilhas (Adaptive Islands do YASB): trilho no topo, ilhas penduradas nele.

use zefiro_core::config::{BarFormat, BarHeight};
use zefiro_platform::geometry::Rect;
use zefiro_ui::bar_layout::{BarLayout, ChipSpec, FilletSide, layout_sections, metrics};
use zefiro_ui::panel_layout::Shape;

fn chip(id: &str, width: f32) -> ChipSpec {
    ChipSpec { id: id.into(), content_w: width, wide: false }
}

fn ilhas(scale: f32, sections: [&[ChipSpec]; 3], island: Option<f32>) -> BarLayout {
    layout_sections(1920.0, scale, BarHeight::Completa, BarFormat::Ilhas, sections, island)
}

fn chip_rect(l: &BarLayout, id: &str) -> Rect {
    l.chips.iter().find(|c| c.id == id).unwrap().rect
}

#[test]
fn metrics_follow_the_yasb_theme() {
    let m = metrics(BarHeight::Completa, BarFormat::Ilhas);
    assert_eq!((m.band, m.chip_h, m.island_pill, m.group_pad, m.edge), (36.0, 28.0, 28.0, 12.0, 0.0));
    assert_eq!((m.rail, m.island_radius, m.fillet), (4.0, 16.0, 8.0));
    let m = metrics(BarHeight::Compacta, BarFormat::Ilhas);
    assert_eq!((m.band, m.chip_h, m.island_pill, m.rail), (32.0, 24.0, 24.0, 4.0));
    for f in [BarFormat::Faixa, BarFormat::Pilulas] {
        assert_eq!(metrics(BarHeight::Completa, f).rail, 0.0, "{f:?} não tem trilho");
    }
}

#[test]
fn edge_islands_touch_the_monitor_and_wrap_their_chips() {
    let start = [chip("zefiro", 12.0)];
    let end = [chip("a", 16.0), chip("b", 16.0)];
    let l = ilhas(1.0, [&start, &[], &end], None);
    assert_eq!(l.strip, None);
    assert_eq!(l.groups, vec![Rect::new(0.0, 0.0, 56.0, 36.0), Rect::new(1820.0, 0.0, 100.0, 36.0)]);
    // 12 de margem; o chip fica centrado abaixo do trilho: 4 + (32 − 28) / 2
    assert_eq!(chip_rect(&l, "zefiro"), Rect::new(12.0, 6.0, 32.0, 28.0));
    assert_eq!(chip_rect(&l, "a"), Rect::new(1832.0, 6.0, 36.0, 28.0));
    assert_eq!(chip_rect(&l, "b"), Rect::new(1872.0, 6.0, 36.0, 28.0));
}

#[test]
fn center_island_wraps_the_media_island_reservation() {
    let l = ilhas(1.0, [&[chip("zefiro", 12.0)], &[chip("clock", 50.0)], &[]], Some(200.0));
    assert_eq!(l.groups.len(), 2);
    assert_eq!(l.groups[1], Rect::new(848.0, 0.0, 224.0, 36.0), "reserva + 12 de cada lado, centrada");
    assert_eq!(l.regions[1].x, 848.0);
    assert_eq!(l.regions[1].w, 224.0);
    assert!(!l.chips.iter().any(|c| c.id == "clock"), "o centro é da ilha de mídia");
    assert_eq!(l.island_pill_y(), 6.0, "a pílula fica centrada abaixo do trilho");
    assert!(l.merges_island());
}

#[test]
fn empty_sections_have_no_island_and_the_rail_fills_the_gaps() {
    let l = ilhas(1.0, [&[chip("zefiro", 12.0)], &[], &[]], None);
    let islands = l.islands.as_ref().unwrap();
    assert_eq!(l.groups, vec![Rect::new(0.0, 0.0, 56.0, 36.0)]);
    assert_eq!(islands.rails, vec![Rect::new(56.0, 0.0, 1864.0, 4.0)]);

    let none = ilhas(1.0, [&[], &[], &[]], None);
    assert!(none.groups.is_empty());
    assert_eq!(none.islands.as_ref().unwrap().rails, vec![Rect::new(0.0, 0.0, 1920.0, 4.0)]);
}

#[test]
fn rail_and_islands_tile_the_top_without_overlapping() {
    let s = [chip("zefiro", 12.0), chip("codex", 46.0)];
    let c = [chip("clock", 60.0)];
    let e = [chip("audio", 16.0)];
    for scale in [1.0, 1.25, 1.5] {
        for (sections, island) in [([&s[..], &c[..], &e[..]], None), ([&s[..], &[][..], &e[..]], Some(190.0)), ([&[][..], &c[..], &[][..]], None)] {
            let l = ilhas(scale, sections, island);
            let mut spans: Vec<(f32, f32)> = l.groups.iter().chain(&l.islands.as_ref().unwrap().rails).map(|r| (r.x, r.right())).collect();
            spans.sort_by(|a, b| a.0.total_cmp(&b.0));
            assert_eq!(spans.first().unwrap().0, 0.0);
            assert_eq!(spans.last().unwrap().1, 1920.0);
            for pair in spans.windows(2) {
                assert_eq!(pair[0].1, pair[1].0, "{scale}: sem vão nem sobreposição em {spans:?}");
            }
        }
    }
}

#[test]
fn fillets_sit_under_the_rail_on_inner_sides_only() {
    let l = ilhas(1.0, [&[chip("zefiro", 12.0)], &[chip("clock", 60.0)], &[chip("audio", 16.0)]], None);
    let fillets = &l.islands.as_ref().unwrap().fillets;
    let sides: Vec<FilletSide> = fillets.iter().map(|f| f.side).collect();
    assert_eq!(sides, [FilletSide::Right, FilletSide::Left, FilletSide::Right, FilletSide::Left]);
    for f in fillets {
        assert_eq!((f.rect.y, f.rect.w, f.rect.h), (4.0, 8.0, 8.0));
    }
    assert_eq!(fillets[0].rect.x, l.groups[0].right());
    assert_eq!(fillets[1].rect.right(), l.groups[1].x);
    assert_eq!(fillets[2].rect.x, l.groups[1].right());
    assert_eq!(fillets[3].rect.right(), l.groups[2].x);
}

#[test]
fn island_shape_hides_the_top_corners_and_the_outer_corners() {
    let l = ilhas(1.0, [&[chip("zefiro", 12.0)], &[chip("clock", 60.0)], &[chip("audio", 16.0)]], None);
    let [start, center, end] = [l.groups[0], l.groups[1], l.groups[2]];
    assert_eq!(l.group_shape(start, 99.0), Shape { rect: Rect::new(-16.0, -16.0, 72.0, 52.0), radius: 16.0 });
    assert_eq!(l.group_shape(center, 99.0), Shape { rect: Rect::new(center.x, -16.0, center.w, 52.0), radius: 16.0 });
    assert_eq!(l.group_shape(end, 99.0), Shape { rect: Rect::new(end.x, -16.0, end.w + 16.0, 52.0), radius: 16.0 });
    let pills = layout_sections(1920.0, 1.0, BarHeight::Completa, BarFormat::Pilulas, [&[chip("zefiro", 12.0)], &[], &[]], None);
    assert_eq!(pills.group_shape(pills.groups[0], 15.0), Shape { rect: pills.groups[0], radius: 15.0 }, "nas pílulas o raio é o do tema");
}

#[test]
fn scales_to_whole_pixels() {
    let l = ilhas(1.5, [&[chip("zefiro", 12.3)], &[chip("clock", 61.7)], &[chip("audio", 16.0)]], None);
    let islands = l.islands.as_ref().unwrap();
    assert_eq!((islands.rail_h, islands.radius, l.band.h), (6.0, 24.0, 54.0));
    let rects = l.groups.iter().chain(&islands.rails).copied().chain(islands.fillets.iter().map(|f| f.rect)).chain(l.chips.iter().map(|c| c.rect));
    for r in rects {
        for v in [r.x, r.y, r.w, r.h] {
            assert_eq!(v, v.round(), "{r:?}");
        }
    }
    assert_eq!(islands.fillets[0].rect.w, 12.0);
}

#[test]
fn narrow_monitor_keeps_every_chip_inside_an_island() {
    let items = (0..12).map(|n| chip(&format!("item{n}"), 500.0)).collect::<Vec<_>>();
    for width in [80.0, 160.0, 400.0, 1200.0] {
        let l = layout_sections(width, 1.25, BarHeight::Compacta, BarFormat::Ilhas, [&items, &[], &items[..2]], None);
        for c in &l.chips {
            assert!(l.groups.iter().any(|g| g.x <= c.rect.x && c.rect.right() <= g.right()), "{width}: {} fora das ilhas", c.id);
        }
    }
}

#[test]
fn groups_map_to_their_section_slot() {
    let l = ilhas(1.0, [&[chip("zefiro", 12.0)], &[], &[chip("audio", 16.0)]], Some(200.0));
    assert_eq!(l.groups.iter().map(|g| l.group_slot(*g)).collect::<Vec<_>>(), [0, 1, 2]);
    let pills = layout_sections(1920.0, 1.0, BarHeight::Completa, BarFormat::Pilulas, [&[chip("zefiro", 12.0)], &[], &[chip("audio", 16.0)]], Some(200.0));
    assert_eq!(pills.groups.iter().map(|g| pills.group_slot(*g)).collect::<Vec<_>>(), [0, 2], "o fim continua no lugar do fim");
    assert_eq!(l.islands.as_ref().unwrap().fillet, 8.0);
}

#[test]
fn other_formats_have_no_islands() {
    for f in [BarFormat::Faixa, BarFormat::Pilulas] {
        let l = layout_sections(1920.0, 1.0, BarHeight::Completa, f, [&[chip("zefiro", 12.0)], &[], &[]], Some(200.0));
        assert!(l.islands.is_none());
        assert_eq!(l.merges_island(), f == BarFormat::Faixa);
        assert_eq!(l.island_pill_y(), 3.0);
    }
}
