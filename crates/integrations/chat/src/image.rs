//! Imagens coladas no chat, portável: base64 (para mandar ao Claude e ao Codex) e o DIB da área de
//! transferência como arquivo BMP (que o decodificador do Windows abre).

const ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

/// Mais imagens que isto numa pergunta são ignoradas.
pub const MAX_IMAGES: usize = 4;
/// Lado maior das imagens mandadas (px): o Claude reduz acima disso de qualquer forma.
pub const MAX_SIDE: u32 = 1568;

pub fn base64(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for chunk in bytes.chunks(3) {
        let b = [chunk[0], chunk.get(1).copied().unwrap_or(0), chunk.get(2).copied().unwrap_or(0)];
        let n = (u32::from(b[0]) << 16) | (u32::from(b[1]) << 8) | u32::from(b[2]);
        for (i, shift) in [18, 12, 6, 0].into_iter().enumerate() {
            if i <= chunk.len() {
                out.push(ALPHABET[((n >> shift) & 63) as usize] as char);
            } else {
                out.push('=');
            }
        }
    }
    out
}

/// Base64 de volta para bytes (a imagem que o Codex gera). Quebras de linha e o `=` do fim são
/// ignorados; qualquer outro caractere fora do alfabeto dá `None`.
pub fn decode_base64(text: &str) -> Option<Vec<u8>> {
    let mut table = [u8::MAX; 256];
    for (i, &a) in ALPHABET.iter().enumerate() {
        table[a as usize] = i as u8;
    }
    let mut out = Vec::with_capacity(text.len() / 4 * 3);
    let (mut acc, mut bits) = (0u32, 0u32);
    for c in text.bytes().filter(|c| !matches!(c, b'\r' | b'\n' | b'=')) {
        let v = table[c as usize];
        if v == u8::MAX {
            return None;
        }
        let v = u32::from(v);
        acc = (acc << 6) | v;
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            out.push((acc >> bits) as u8);
            acc &= (1 << bits) - 1;
        }
    }
    // Sobra de 6 bits: um caractere a mais que não forma byte.
    (bits < 6).then_some(out)
}

/// Os 8 bytes da assinatura do PNG.
pub fn is_png(bytes: &[u8]) -> bool {
    bytes.starts_with(&[0x89, b'P', b'N', b'G', 13, 10, 26, 10])
}

/// Um `CF_DIB` (cabeçalho `BITMAPINFO` + pixels) vira um arquivo `.bmp`: falta só o cabeçalho de
/// 14 bytes, com o deslocamento até os pixels (depois das máscaras e da paleta, se houver).
pub fn dib_to_bmp(dib: &[u8]) -> Option<Vec<u8>> {
    let u32_at = |i: usize| dib.get(i..i + 4).map(|b| u32::from_le_bytes([b[0], b[1], b[2], b[3]]));
    let header = u32_at(0)? as usize;
    if header < 40 || dib.len() < header {
        return None;
    }
    let bits = u16::from_le_bytes([dib[14], dib[15]]);
    let compression = u32_at(16)?;
    let used = u32_at(32)? as usize;
    let colors = if used > 0 { used } else if bits <= 8 { 1 << bits } else { 0 };
    // BI_BITFIELDS com o cabeçalho de 40 bytes: as três máscaras vêm logo depois dele.
    let masks = if compression == 3 && header == 40 { 12 } else { 0 };
    let offset = 14 + header + masks + colors * 4;
    let size = 14 + dib.len();
    if offset > size {
        return None;
    }
    let mut out = Vec::with_capacity(size);
    out.extend_from_slice(b"BM");
    out.extend_from_slice(&(size as u32).to_le_bytes());
    out.extend_from_slice(&[0, 0, 0, 0]);
    out.extend_from_slice(&(offset as u32).to_le_bytes());
    out.extend_from_slice(dib);
    Some(out)
}

/// Extensões de arquivo que o chat aceita colar (arquivo copiado no Explorer).
pub fn is_image_file(name: &str) -> bool {
    let lower = name.to_lowercase();
    [".png", ".jpg", ".jpeg", ".gif", ".bmp", ".webp"].iter().any(|e| lower.ends_with(e))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn base64_matches_the_rfc_examples() {
        for (input, expected) in [("", ""), ("f", "Zg=="), ("fo", "Zm8="), ("foo", "Zm9v"), ("foob", "Zm9vYg=="), ("fooba", "Zm9vYmE="), ("foobar", "Zm9vYmFy")] {
            assert_eq!(base64(input.as_bytes()), expected, "{input}");
        }
        assert_eq!(base64(&[0xff, 0xfe, 0xfd]), "//79");
    }

    fn dib(header: u32, bits: u16, compression: u32, used: u32, extra: usize) -> Vec<u8> {
        let mut d = vec![0u8; header as usize + extra];
        d[0..4].copy_from_slice(&header.to_le_bytes());
        d[14..16].copy_from_slice(&bits.to_le_bytes());
        d[16..20].copy_from_slice(&compression.to_le_bytes());
        d[32..36].copy_from_slice(&used.to_le_bytes());
        d
    }

    fn offset(bmp: &[u8]) -> u32 {
        u32::from_le_bytes([bmp[10], bmp[11], bmp[12], bmp[13]])
    }

    #[test]
    fn dib_gets_the_bmp_header() {
        let d = dib(40, 32, 0, 0, 16);
        let bmp = dib_to_bmp(&d).unwrap();
        assert_eq!(&bmp[..2], b"BM");
        assert_eq!(u32::from_le_bytes([bmp[2], bmp[3], bmp[4], bmp[5]]) as usize, bmp.len());
        assert_eq!(offset(&bmp), 54, "32 bits: pixels logo depois do cabeçalho");
        assert_eq!(&bmp[14..], &d[..]);
        assert_eq!(offset(&dib_to_bmp(&dib(40, 32, 3, 0, 16)).unwrap()), 66, "máscaras do BI_BITFIELDS");
        assert_eq!(offset(&dib_to_bmp(&dib(40, 8, 0, 0, 1100)).unwrap()), 54 + 1024, "paleta de 256 cores");
        assert_eq!(offset(&dib_to_bmp(&dib(124, 32, 3, 0, 16)).unwrap()), 138, "V5: máscaras dentro do cabeçalho");
        assert!(dib_to_bmp(&[1, 2, 3]).is_none());
        assert!(dib_to_bmp(&dib(40, 8, 0, 0, 0)).is_none(), "paleta maior que os dados");
    }

    #[test]
    fn base64_decodes_back() {
        for input in ["", "f", "fo", "foo", "foob", "fooba", "foobar"] {
            assert_eq!(decode_base64(&base64(input.as_bytes())).as_deref(), Some(input.as_bytes()), "{input}");
        }
        let bytes: Vec<u8> = (0..=255).collect();
        assert_eq!(decode_base64(&base64(&bytes)), Some(bytes));
        assert_eq!(decode_base64("Zm9v
YmFy").as_deref(), Some(&b"foobar"[..]), "quebras de linha são ignoradas");
        assert_eq!(decode_base64("Zm9vYg"), Some(b"foob".to_vec()), "sem o preenchimento");
        assert!(decode_base64("Zm9v!").is_none() && decode_base64("Z").is_none());
    }

    #[test]
    fn png_signature() {
        assert!(is_png(&[0x89, b'P', b'N', b'G', 13, 10, 26, 10, 0]));
        assert!(!is_png(b"GIF89a") && !is_png(&[]));
    }

    #[test]
    fn image_files() {
        assert!(is_image_file(r"C:\a\Print.PNG") && is_image_file("foto.jpeg") && !is_image_file("nota.txt"));
    }
}
