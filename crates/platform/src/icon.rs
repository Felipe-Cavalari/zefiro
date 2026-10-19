//! Arquivos `.ico`: escolher a imagem do tamanho certo (a bandeja pede 16, 20 ou 24 px conforme o DPI).

/// Bytes da imagem (PNG ou DIB) com o menor lado ≥ `size`; se nenhuma for tão grande, a maior.
/// `None` se o arquivo não for um `.ico` válido.
pub fn ico_image(ico: &[u8], size: u32) -> Option<&[u8]> {
    let u16_at = |i: usize| ico.get(i..i + 2).map(|b| u16::from_le_bytes([b[0], b[1]]));
    let u32_at = |i: usize| ico.get(i..i + 4).map(|b| u32::from_le_bytes([b[0], b[1], b[2], b[3]]));
    if u16_at(0)? != 0 || u16_at(2)? != 1 {
        return None;
    }
    let count = u16_at(4)? as usize;
    let mut entries = Vec::with_capacity(count);
    for i in 0..count {
        let at = 6 + 16 * i;
        // 0 no byte de largura quer dizer 256.
        let side = match *ico.get(at)? { 0 => 256, w => w as u32 };
        let (len, offset) = (u32_at(at + 8)? as usize, u32_at(at + 12)? as usize);
        entries.push((side, ico.get(offset..offset.checked_add(len)?)?));
    }
    let fits = entries.iter().filter(|(side, _)| *side >= size).min_by_key(|(side, _)| *side);
    fits.or_else(|| entries.iter().max_by_key(|(side, _)| *side)).map(|(_, bytes)| *bytes)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `.ico` mínimo com imagens falsas de 16 e 32 px (o conteúdo marca qual é qual).
    fn ico() -> Vec<u8> {
        let mut out = vec![0, 0, 1, 0, 2, 0];
        let images: [(u8, &[u8]); 2] = [(16, b"dezesseis"), (32, b"trinta-e-dois")];
        let mut offset = 6 + 16 * images.len();
        for (side, data) in images {
            out.extend([side, side, 0, 0, 1, 0, 32, 0]);
            out.extend((data.len() as u32).to_le_bytes());
            out.extend((offset as u32).to_le_bytes());
            offset += data.len();
        }
        for (_, data) in images {
            out.extend(data);
        }
        out
    }

    #[test]
    fn picks_the_smallest_image_that_is_big_enough() {
        let ico = ico();
        assert_eq!(ico_image(&ico, 16), Some(&b"dezesseis"[..]));
        assert_eq!(ico_image(&ico, 20), Some(&b"trinta-e-dois"[..]), "20 px: usa a de 32 e reduz");
        assert_eq!(ico_image(&ico, 48), Some(&b"trinta-e-dois"[..]), "nenhuma grande o bastante: a maior");
        assert_eq!(ico_image(b"PNG", 16), None);
    }
}
