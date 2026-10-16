use zefiro_platform::geometry::{Point, Rect};
use zefiro_ui::edit_layout::{DropTarget, insert_index, starts_drag};

#[test]
fn movement_threshold_is_in_dip_and_midpoint_ties_are_stable() {
    assert!(!starts_drag(Point { x: 10.0, y: 10.0 }, Point { x: 14.0, y: 10.0 }, 1.25));
    assert!(starts_drag(Point { x: 10.0, y: 10.0 }, Point { x: 15.0, y: 10.0 }, 1.25));
    assert_eq!(insert_index(&[], 10.0), 0);
    let boxes = [Rect::new(0.0, 0.0, 40.0, 24.0), Rect::new(44.0, 0.0, 40.0, 24.0)];
    assert_eq!(insert_index(&boxes, 20.0), 0);
    assert_eq!(insert_index(&boxes, 20.1), 1);
    assert_eq!(insert_index(&boxes, 80.0), 2);
    assert_eq!(DropTarget { section: 1, index: 0 }, DropTarget { section: 1, index: insert_index(&[], 0.0) });
}
