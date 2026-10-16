use zefiro_core::config::{BarFormat, BarHeight};
use zefiro_ui::bar_layout::{ChipSpec, layout_sections};

fn chip(id: &str, width: f32) -> ChipSpec { ChipSpec { id: id.into(), content_w: width } }

#[test]
fn center_is_geometrically_centered_and_pills_do_not_overlap() {
    let items = [vec![chip("zefiro", 50.0)], vec![chip("center", 80.0)], vec![chip("clock", 90.0)]];
    for scale in [1.0, 1.25, 1.5] {
        let layout = layout_sections(1920.0, scale, BarHeight::Completa, BarFormat::Pilulas, [&items[0], &items[1], &items[2]], None);
        let center = layout.chips.iter().find(|c| c.id == "center").unwrap();
        assert!((center.rect.center_x() - 960.0).abs() <= 1.0);
        assert_eq!(layout.groups.len(), 3);
        for pair in layout.groups.windows(2) { assert!(pair[0].right() + 7.0 * scale <= pair[1].x); }
    }
}

#[test]
fn island_reserves_center_and_center_items_are_not_clickable() {
    let start = [chip("zefiro", 80.0)];
    let center = [chip("clock", 80.0)];
    let layout = layout_sections(600.0, 1.0, BarHeight::Completa, BarFormat::Faixa, [&start, &center, &[]], Some(300.0));
    assert!(!layout.chips.iter().any(|c| c.id == "clock"));
    assert!(layout.chips.iter().all(|c| c.rect.right() <= 142.0));
    assert!(layout.overflow.iter().all(|group| !group.ids.iter().any(|id| id == "clock")));
}

#[test]
fn narrow_layout_keeps_items_accessible_without_negative_or_overlapping_targets() {
    let items = (0..12).map(|n| chip(&format!("item{n}"), 500.0)).collect::<Vec<_>>();
    for width in [0.0, 10.0, 80.0, 160.0, 400.0] {
        let layout = layout_sections(width, 1.25, BarHeight::Compacta, BarFormat::Pilulas, [&items, &[], &[]], None);
        for c in &layout.chips {
            assert!(c.rect.x >= 0.0 && c.rect.w > 0.0 && c.rect.right() <= width);
        }
        let visible = layout.chips.iter().filter(|c| !c.id.starts_with("__more")).count();
        let hidden: usize = layout.overflow.iter().map(|o| o.ids.len()).sum();
        assert_eq!(visible + hidden, items.len());
    }
}
