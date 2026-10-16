//! Desenha uma `DisplayList` num `ID2D1DeviceContext`.

use windows::Win32::Graphics::Direct2D::Common::*;
use windows::Win32::Graphics::Direct2D::*;
use windows::Win32::Graphics::DirectWrite::DWRITE_MEASURING_MODE_NATURAL;
use windows::Win32::Graphics::Dxgi::Common::DXGI_FORMAT_B8G8R8A8_UNORM;
use windows::core::Result;
use windows_numerics::{Matrix3x2, Vector2};
use zefiro_platform::color::Rgba;
use zefiro_platform::geometry::Rect;
use zefiro_ui::paint::{DisplayList, DrawOp};

use crate::text::DWriteText;

pub fn d2d_color(c: Rgba) -> D2D1_COLOR_F {
    D2D1_COLOR_F { r: c.r, g: c.g, b: c.b, a: c.a }
}

fn d2d_rect(r: Rect) -> D2D_RECT_F {
    D2D_RECT_F { left: r.x, top: r.y, right: r.right(), bottom: r.bottom() }
}

pub fn paint(dc: &ID2D1DeviceContext, list: &DisplayList, text: &DWriteText) -> Result<()> {
    // SAFETY: `dc` está entre BeginDraw/EndDraw (garantido por quem chama); objetos locais.
    unsafe {
        dc.SetTextAntialiasMode(D2D1_TEXT_ANTIALIAS_MODE_GRAYSCALE);
        for op in &list.ops {
            match op {
                DrawOp::RoundRect { rect, radius, color } => {
                    let b = dc.CreateSolidColorBrush(&d2d_color(*color), None)?;
                    dc.FillRoundedRectangle(&D2D1_ROUNDED_RECT { rect: d2d_rect(*rect), radiusX: *radius, radiusY: *radius }, &b);
                }
                DrawOp::RoundRectStroke { rect, radius, color, width } => {
                    let b = dc.CreateSolidColorBrush(&d2d_color(*color), None)?;
                    let r = rect.inset(width / 2.0, width / 2.0);
                    dc.DrawRoundedRectangle(&D2D1_ROUNDED_RECT { rect: d2d_rect(r), radiusX: *radius, radiusY: *radius }, &b, *width, None);
                }
                DrawOp::Circle { center, radius, color } => {
                    let b = dc.CreateSolidColorBrush(&d2d_color(*color), None)?;
                    dc.FillEllipse(&D2D1_ELLIPSE { point: Vector2 { X: center.x, Y: center.y }, radiusX: *radius, radiusY: *radius }, &b);
                }
                DrawOp::Text { rect, text: s, style, color, align } => {
                    let b = dc.CreateSolidColorBrush(&d2d_color(*color), None)?;
                    let format = text.format(style, *align)?;
                    let utf16: Vec<u16> = s.encode_utf16().collect();
                    // Fonte colorida: os emojis do clima saem coloridos (Segoe UI Emoji por fallback).
                    dc.DrawText(&utf16, &format, &d2d_rect(*rect), &b, D2D1_DRAW_TEXT_OPTIONS_CLIP | D2D1_DRAW_TEXT_OPTIONS_ENABLE_COLOR_FONT, DWRITE_MEASURING_MODE_NATURAL);
                }
                DrawOp::PushClip(rect) => dc.PushAxisAlignedClip(&d2d_rect(*rect), D2D1_ANTIALIAS_MODE_PER_PRIMITIVE),
                DrawOp::PopClip => dc.PopAxisAlignedClip(),
                DrawOp::Image { rect, image, radius, opacity } => {
                    let props = D2D1_BITMAP_PROPERTIES1 {
                        pixelFormat: D2D1_PIXEL_FORMAT { format: DXGI_FORMAT_B8G8R8A8_UNORM, alphaMode: D2D1_ALPHA_MODE_PREMULTIPLIED },
                        dpiX: 96.0,
                        dpiY: 96.0,
                        ..Default::default()
                    };
                    let size = D2D_SIZE_U { width: image.width(), height: image.height() };
                    let bitmap = dc.CreateBitmap(size, Some(image.bgra().as_ptr() as *const _), image.width() * 4, &props)?;
                    let sx = rect.w / image.width() as f32;
                    let sy = rect.h / image.height() as f32;
                    let brush_props = D2D1_BRUSH_PROPERTIES { opacity: *opacity, transform: Matrix3x2 { M11: sx, M12: 0.0, M21: 0.0, M22: sy, M31: rect.x, M32: rect.y } };
                    let bitmap_props = D2D1_BITMAP_BRUSH_PROPERTIES1 { extendModeX: D2D1_EXTEND_MODE_CLAMP, extendModeY: D2D1_EXTEND_MODE_CLAMP, interpolationMode: D2D1_INTERPOLATION_MODE_HIGH_QUALITY_CUBIC };
                    let brush = dc.CreateBitmapBrush(&bitmap, Some(&bitmap_props), Some(&brush_props))?;
                    dc.FillRoundedRectangle(&D2D1_ROUNDED_RECT { rect: d2d_rect(*rect), radiusX: *radius, radiusY: *radius }, &brush);
                }
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::gfx::Gfx;
    use crate::text::DWriteText;
    use windows::Win32::Graphics::Dxgi::Common::DXGI_FORMAT_B8G8R8A8_UNORM;
    use zefiro_platform::color::Rgba;
    use zefiro_platform::geometry::{Rect, Size};
    use zefiro_platform::text::{Align, Family, FontWeight, TextMeasurer, TextStyle};
    use zefiro_ui::paint::{DisplayList, DrawOp};

    fn props(options: D2D1_BITMAP_OPTIONS) -> D2D1_BITMAP_PROPERTIES1 {
        D2D1_BITMAP_PROPERTIES1 {
            pixelFormat: D2D1_PIXEL_FORMAT { format: DXGI_FORMAT_B8G8R8A8_UNORM, alphaMode: D2D1_ALPHA_MODE_PREMULTIPLIED },
            dpiX: 96.0,
            dpiY: 96.0,
            bitmapOptions: options,
            ..Default::default()
        }
    }

    #[test]
    fn paints_a_round_rect_and_measures_text_on_warp() {
        let gfx = Gfx::new().unwrap();
        let text = DWriteText::new(gfx.dwrite.clone(), "Segoe UI Variable");
        let style = TextStyle { size: 13.0, weight: FontWeight::SemiBold, family: Family::Text };
        assert!(text.measure("qui 15 15:47", &style).w > 40.0);

        let mut list = DisplayList::new(Size { w: 64.0, h: 32.0 });
        list.push(DrawOp::RoundRect { rect: Rect::new(0.0, 0.0, 64.0, 32.0), radius: 16.0, color: Rgba::rgb(255, 0, 0) });
        list.push(DrawOp::Text { rect: Rect::new(0.0, 0.0, 64.0, 32.0), text: "Z".into(), style, color: Rgba::WHITE, align: Align::Center });

        // SAFETY: chamadas padrão de Direct2D em objetos recém-criados; ponteiros locais válidos.
        unsafe {
            let dc = gfx.d2d_device.CreateDeviceContext(D2D1_DEVICE_CONTEXT_OPTIONS_NONE).unwrap();
            let size = D2D_SIZE_U { width: 64, height: 32 };
            let target = dc.CreateBitmap(size, None, 0, &props(D2D1_BITMAP_OPTIONS_TARGET | D2D1_BITMAP_OPTIONS_CANNOT_DRAW)).unwrap();
            dc.SetTarget(&target);
            dc.BeginDraw();
            dc.Clear(Some(&D2D1_COLOR_F::default()));
            paint(&dc, &list, &text).unwrap();
            dc.EndDraw(None, None).unwrap();

            let read = dc.CreateBitmap(size, None, 0, &props(D2D1_BITMAP_OPTIONS_CPU_READ | D2D1_BITMAP_OPTIONS_CANNOT_DRAW)).unwrap();
            read.CopyFromBitmap(None, &target, None).unwrap();
            let map = read.Map(D2D1_MAP_OPTIONS_READ).unwrap();
            let px = |x: usize, y: usize| std::slice::from_raw_parts(map.bits.add(y * map.pitch as usize + x * 4), 4).to_vec();
            assert_eq!(px(8, 16), vec![0, 0, 255, 255]); // BGRA: vermelho opaco dentro da pílula
            assert_eq!(px(0, 0)[3], 0); // canto fora do raio: transparente
            read.Unmap().unwrap();
        }
    }
}
