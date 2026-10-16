//! Formatos de texto do DirectWrite (com cache) e medição.

use std::cell::RefCell;
use std::collections::HashMap;
use windows::Win32::Graphics::DirectWrite::*;
use windows::core::{HSTRING, Result, w};
use zefiro_platform::geometry::Size;
use zefiro_platform::text::{Align, Family, FontWeight, TextMeasurer, TextStyle};

type Key = (String, u32, u8, u8);

pub struct DWriteText {
    dwrite: IDWriteFactory,
    family: RefCell<String>,
    cache: RefCell<HashMap<Key, IDWriteTextFormat>>,
}

impl DWriteText {
    pub fn new(dwrite: IDWriteFactory, family: &str) -> DWriteText {
        DWriteText { dwrite, family: RefCell::new(family.to_string()), cache: RefCell::new(HashMap::new()) }
    }

    /// Troca a família do tema; limpa o cache.
    pub fn set_family(&self, family: &str) {
        *self.family.borrow_mut() = family.to_string();
        self.cache.borrow_mut().clear();
    }

    fn face(&self, family: Family) -> String {
        let base = self.family.borrow().clone();
        match family {
            Family::Icons => "Segoe Fluent Icons".into(),
            Family::Text if base == "Segoe UI Variable" => "Segoe UI Variable Text".into(),
            Family::Display if base == "Segoe UI Variable" => "Segoe UI Variable Display".into(),
            _ => base,
        }
    }

    pub fn format(&self, style: &TextStyle, align: Align) -> Result<IDWriteTextFormat> {
        let face = self.face(style.family);
        let key = (face.clone(), (style.size * 100.0) as u32, style.weight as u8, align as u8);
        if let Some(f) = self.cache.borrow().get(&key) {
            return Ok(f.clone());
        }
        let weight = match style.weight {
            FontWeight::Regular => DWRITE_FONT_WEIGHT_NORMAL,
            FontWeight::SemiBold => DWRITE_FONT_WEIGHT_SEMI_BOLD,
            FontWeight::Bold => DWRITE_FONT_WEIGHT_BOLD,
        };
        // SAFETY: strings válidas durante a chamada; formato criado e configurado antes do uso.
        let f = unsafe {
            let f = self.dwrite.CreateTextFormat(&HSTRING::from(face), None, weight, DWRITE_FONT_STYLE_NORMAL, DWRITE_FONT_STRETCH_NORMAL, style.size, w!("pt-BR"))?;
            f.SetTextAlignment(match align {
                Align::Leading => DWRITE_TEXT_ALIGNMENT_LEADING,
                Align::Center => DWRITE_TEXT_ALIGNMENT_CENTER,
                Align::Trailing => DWRITE_TEXT_ALIGNMENT_TRAILING,
            })?;
            f.SetParagraphAlignment(DWRITE_PARAGRAPH_ALIGNMENT_CENTER)?;
            f.SetWordWrapping(DWRITE_WORD_WRAPPING_NO_WRAP)?;
            // Texto maior que a caixa (título de música) termina em reticências.
            let sign = self.dwrite.CreateEllipsisTrimmingSign(&f)?;
            f.SetTrimming(&DWRITE_TRIMMING { granularity: DWRITE_TRIMMING_GRANULARITY_CHARACTER, delimiter: 0, delimiterCount: 0 }, &sign)?;
            f
        };
        self.cache.borrow_mut().insert(key, f.clone());
        Ok(f)
    }
}

impl TextMeasurer for DWriteText {
    fn measure(&self, text: &str, style: &TextStyle) -> Size {
        let utf16: Vec<u16> = text.encode_utf16().collect();
        let Ok(format) = self.format(style, Align::Leading) else { return Size::default() };
        // SAFETY: buffer UTF-16 local; métricas gravadas numa struct local.
        unsafe {
            let Ok(layout) = self.dwrite.CreateTextLayout(&utf16, &format, 100_000.0, 100_000.0) else { return Size::default() };
            let mut m = DWRITE_TEXT_METRICS::default();
            if layout.GetMetrics(&mut m).is_err() {
                return Size::default();
            }
            Size { w: m.widthIncludingTrailingWhitespace, h: m.height }
        }
    }
}
