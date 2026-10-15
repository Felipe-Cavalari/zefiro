//! Estilo de texto e medição (implementada pela plataforma com DirectWrite).

use crate::geometry::Size;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FontWeight { Regular, SemiBold, Bold }

/// `Text` = corpo (Segoe UI Variable Text); `Display` = números grandes (Segoe UI Variable Display).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Family { Text, Display }

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Align { Leading, Center, Trailing }

/// Tamanho em pixels (já multiplicado pela escala do monitor e pela escala de texto do tema).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TextStyle { pub size: f32, pub weight: FontWeight, pub family: Family }

pub trait TextMeasurer {
    /// Caixa do texto numa linha, sem quebra.
    fn measure(&self, text: &str, style: &TextStyle) -> Size;
}

/// Medidor aproximado para testes: 0,55 × tamanho por caractere, altura 1,3 × tamanho.
pub struct FixedWidthMeasurer;

impl TextMeasurer for FixedWidthMeasurer {
    fn measure(&self, text: &str, style: &TextStyle) -> Size {
        Size { w: text.chars().count() as f32 * style.size * 0.55, h: style.size * 1.3 }
    }
}
