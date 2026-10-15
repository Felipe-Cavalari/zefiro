//! Pontos, tamanhos e retângulos em pixels (f32).

#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Point { pub x: f32, pub y: f32 }

#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Size { pub w: f32, pub h: f32 }

#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Rect { pub x: f32, pub y: f32, pub w: f32, pub h: f32 }

impl Rect {
    pub const fn new(x: f32, y: f32, w: f32, h: f32) -> Rect { Rect { x, y, w, h } }
    pub fn right(&self) -> f32 { self.x + self.w }
    pub fn bottom(&self) -> f32 { self.y + self.h }
    pub fn center_x(&self) -> f32 { self.x + self.w / 2.0 }
    /// Borda esquerda/superior inclusiva, direita/inferior exclusiva.
    pub fn contains(&self, p: Point) -> bool {
        p.x >= self.x && p.x < self.right() && p.y >= self.y && p.y < self.bottom()
    }
    pub fn inset(&self, dx: f32, dy: f32) -> Rect {
        Rect::new(self.x + dx, self.y + dy, (self.w - 2.0 * dx).max(0.0), (self.h - 2.0 * dy).max(0.0))
    }
    pub fn union(&self, o: Rect) -> Rect {
        let x = self.x.min(o.x);
        let y = self.y.min(o.y);
        Rect::new(x, y, self.right().max(o.right()) - x, self.bottom().max(o.bottom()) - y)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rect_helpers() {
        let r = Rect::new(10.0, 20.0, 100.0, 40.0);
        assert_eq!(r.right(), 110.0);
        assert_eq!(r.bottom(), 60.0);
        assert_eq!(r.center_x(), 60.0);
        assert!(r.contains(Point { x: 10.0, y: 20.0 }));
        assert!(!r.contains(Point { x: 110.0, y: 20.0 })); // borda direita é exclusiva
        assert_eq!(r.inset(5.0, 2.0), Rect::new(15.0, 22.0, 90.0, 36.0));
        assert_eq!(r.union(Rect::new(0.0, 0.0, 5.0, 5.0)), Rect::new(0.0, 0.0, 110.0, 60.0));
    }
}
