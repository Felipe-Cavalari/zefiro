//! Imagem em memória (capa do álbum, ícone de app), já decodificada pela plataforma.

use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

/// Pixels BGRA 8 bits pré-multiplicados, linha a linha, sem preenchimento.
#[derive(Debug)]
pub struct ImageData {
    pub id: u64,
    pub width: u32,
    pub height: u32,
    pub bgra: Vec<u8>,
}

/// Referência barata de copiar. Duas imagens são iguais quando vêm da mesma decodificação.
#[derive(Debug, Clone)]
pub struct Image(pub Arc<ImageData>);

impl Image {
    /// `None` se o buffer não tiver exatamente `width × height × 4` bytes.
    pub fn new(width: u32, height: u32, bgra: Vec<u8>) -> Option<Image> {
        static NEXT: AtomicU64 = AtomicU64::new(1);
        if width == 0 || height == 0 || bgra.len() != width as usize * height as usize * 4 {
            return None;
        }
        Some(Image(Arc::new(ImageData { id: NEXT.fetch_add(1, Ordering::Relaxed), width, height, bgra })))
    }

    pub fn id(&self) -> u64 {
        self.0.id
    }

    pub fn width(&self) -> u32 {
        self.0.width
    }

    pub fn height(&self) -> u32 {
        self.0.height
    }

    pub fn bgra(&self) -> &[u8] {
        &self.0.bgra
    }
}

impl PartialEq for Image {
    fn eq(&self, other: &Self) -> bool {
        self.0.id == other.0.id
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_wrong_buffer_and_compares_by_decoding() {
        assert!(Image::new(2, 2, vec![0; 15]).is_none());
        assert!(Image::new(0, 2, vec![]).is_none());
        let a = Image::new(1, 1, vec![1, 2, 3, 255]).unwrap();
        let b = Image::new(1, 1, vec![1, 2, 3, 255]).unwrap();
        assert_eq!(a, a.clone());
        assert_ne!(a, b, "decodificações diferentes não são a mesma imagem");
    }
}
