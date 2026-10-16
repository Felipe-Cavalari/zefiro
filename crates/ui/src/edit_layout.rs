//! Cálculos puros do arraste: distância em DIP e destinos estáveis.

use zefiro_platform::geometry::{Point, Rect};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DropTarget { pub section: usize, pub index: usize }

pub fn starts_drag(origin: Point, current: Point, scale: f32) -> bool {
    (current.x - origin.x).hypot(current.y - origin.y) >= 4.0 * scale
}

pub fn insert_index(items: &[Rect], x: f32) -> usize {
    items.iter().position(|r| x <= r.center_x()).unwrap_or(items.len())
}

pub fn drop_target(regions: &[Rect; 3], items: [&[Rect]; 3], point: Point, center_blocked: bool) -> Option<DropTarget> {
    regions.iter().enumerate().find(|(i, r)| !(center_blocked && *i == 1) && r.contains(point)).map(|(section, _)| DropTarget { section, index: insert_index(items[section], point.x) })
}
