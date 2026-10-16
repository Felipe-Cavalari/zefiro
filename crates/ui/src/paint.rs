//! Lista de desenho portável. A plataforma a desenha com Direct2D.

use zefiro_platform::color::Rgba;
use zefiro_platform::geometry::{Point, Rect, Size};
use zefiro_platform::image::Image;
use zefiro_platform::text::{Align, TextStyle};

#[derive(Debug, Clone, PartialEq)]
pub enum DrawOp {
    RoundRect { rect: Rect, radius: f32, color: Rgba },
    RoundRectStroke { rect: Rect, radius: f32, color: Rgba, width: f32 },
    Circle { center: Point, radius: f32, color: Rgba },
    Text { rect: Rect, text: String, style: TextStyle, color: Rgba, align: Align },
    /// Imagem esticada para `rect`, recortada num retângulo arredondado.
    Image { rect: Rect, image: Image, radius: f32, opacity: f32 },
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct DisplayList { pub size: Size, pub ops: Vec<DrawOp> }

impl DisplayList {
    pub fn new(size: Size) -> Self {
        Self { size, ops: Vec::new() }
    }

    pub fn push(&mut self, op: DrawOp) {
        self.ops.push(op)
    }

    /// Cópia com todas as operações deslocadas.
    pub fn translated(&self, dx: f32, dy: f32) -> DisplayList {
        let mv = |r: Rect| Rect::new(r.x + dx, r.y + dy, r.w, r.h);
        let ops = self
            .ops
            .iter()
            .map(|op| match op.clone() {
                DrawOp::RoundRect { rect, radius, color } => DrawOp::RoundRect { rect: mv(rect), radius, color },
                DrawOp::RoundRectStroke { rect, radius, color, width } => DrawOp::RoundRectStroke { rect: mv(rect), radius, color, width },
                DrawOp::Circle { center, radius, color } => DrawOp::Circle { center: Point { x: center.x + dx, y: center.y + dy }, radius, color },
                DrawOp::Text { rect, text, style, color, align } => DrawOp::Text { rect: mv(rect), text, style, color, align },
                DrawOp::Image { rect, image, radius, opacity } => DrawOp::Image { rect: mv(rect), image, radius, opacity },
            })
            .collect();
        DisplayList { size: self.size, ops }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use zefiro_platform::color::Rgba;
    use zefiro_platform::geometry::{Point, Rect, Size};

    #[test]
    fn translated_moves_every_op() {
        let mut l = DisplayList::new(Size { w: 10.0, h: 10.0 });
        l.push(DrawOp::Circle { center: Point { x: 1.0, y: 2.0 }, radius: 1.0, color: Rgba::WHITE });
        l.push(DrawOp::RoundRect { rect: Rect::new(0.0, 0.0, 5.0, 5.0), radius: 2.0, color: Rgba::WHITE });
        let t = l.translated(10.0, 20.0);
        assert_eq!(t.ops[0], DrawOp::Circle { center: Point { x: 11.0, y: 22.0 }, radius: 1.0, color: Rgba::WHITE });
        assert_eq!(t.ops[1], DrawOp::RoundRect { rect: Rect::new(10.0, 20.0, 5.0, 5.0), radius: 2.0, color: Rgba::WHITE });
    }
}
