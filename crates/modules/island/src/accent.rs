//! Cor de destaque tirada da capa, portada do Dynamic Island for Windows (MIT): histograma
//! 16 × 16 × 16, vence o mais saturado entre os 3 baldes mais votados, e a cor final é a média
//! desse balde com os 26 vizinhos (estável quando a cor cai na fronteira entre baldes).
//! O contraste com o fundo fica com `tokens::ensure_contrast`, como no resto do tema.

use zefiro_platform::color::Rgba;
use zefiro_platform::image::Image;

#[derive(Debug, Clone, Copy, Default)]
struct Bucket { count: u32, r: u32, g: u32, b: u32, sat: u32 }

/// `None` quando a capa não tem nenhuma cor viva (preto e branco, quase cinza).
pub fn cover_accent(image: &Image) -> Option<Rgba> {
    let mut buckets = vec![Bucket::default(); 16 * 16 * 16];
    for px in image.bgra().as_chunks::<4>().0 {
        let (blue, green, red, alpha) = (px[0] as i32, px[1] as i32, px[2] as i32, px[3]);
        if alpha < 32 {
            continue;
        }
        let maxc = red.max(green).max(blue);
        let minc = red.min(green).min(blue);
        let luminance = (54 * red + 183 * green + 19 * blue) / 256;
        let saturation = maxc - minc;
        // Descarta quase preto, quase branco e quase cinza.
        if !(36..=220).contains(&luminance) || saturation < 28 {
            continue;
        }
        let index = (((red >> 4) << 8) | ((green >> 4) << 4) | (blue >> 4)) as usize;
        let weight = 1 + (saturation / 32) as u32;
        let bucket = &mut buckets[index];
        bucket.count += weight;
        bucket.r += red as u32 * weight;
        bucket.g += green as u32 * weight;
        bucket.b += blue as u32 * weight;
        bucket.sat += saturation as u32 * weight;
    }
    let mut top: Vec<usize> = (0..buckets.len()).filter(|&i| buckets[i].count > 0).collect();
    // Ordem estável: mais votos primeiro, índice menor desempata (igual ao laço do C++).
    top.sort_by(|&a, &b| buckets[b].count.cmp(&buckets[a].count).then(a.cmp(&b)));
    top.truncate(3);
    let vibrancy = |i: usize| buckets[i].sat as f32 / buckets[i].count as f32;
    let best = top.iter().copied().fold(None::<usize>, |best, i| match best {
        Some(b) if vibrancy(b) >= vibrancy(i) => Some(b),
        _ => Some(i),
    })?;
    let (ri, gi, bi) = ((best >> 8) as i32 & 15, (best >> 4) as i32 & 15, best as i32 & 15);
    let (mut count, mut r, mut g, mut b) = (0u64, 0f64, 0f64, 0f64);
    for dr in -1..=1 {
        for dg in -1..=1 {
            for db in -1..=1 {
                let (ni, nj, nk) = (ri + dr, gi + dg, bi + db);
                if !(0..16).contains(&ni) || !(0..16).contains(&nj) || !(0..16).contains(&nk) {
                    continue;
                }
                let nb = buckets[((ni << 8) | (nj << 4) | nk) as usize];
                count += nb.count as u64;
                r += nb.r as f64;
                g += nb.g as f64;
                b += nb.b as f64;
            }
        }
    }
    (count > 0).then(|| {
        let c = count as f64 * 255.0;
        Rgba { r: (r / c) as f32, g: (g / c) as f32, b: (b / c) as f32, a: 1.0 }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn image(pixels: &[(u8, u8, u8)], repeat: usize) -> Image {
        let mut bgra = Vec::new();
        for _ in 0..repeat {
            for &(r, g, b) in pixels {
                bgra.extend_from_slice(&[b, g, r, 255]);
            }
        }
        Image::new((pixels.len() * repeat) as u32, 1, bgra).unwrap()
    }

    #[test]
    fn grayscale_cover_has_no_accent() {
        assert_eq!(cover_accent(&image(&[(10, 10, 10), (128, 128, 130), (250, 250, 250)], 50)), None);
    }

    #[test]
    fn vivid_color_beats_a_larger_muted_area() {
        // Bege (pouco saturado, muitos pixels) contra vermelho vivo (menos pixels).
        let mut px = vec![(200, 180, 150); 6];
        px.extend([(220, 30, 40); 3]);
        let c = cover_accent(&image(&px, 40)).unwrap();
        assert!(c.r > 0.8 && c.g < 0.2 && c.b < 0.25, "{c:?}");
    }

    #[test]
    fn neighbours_pool_a_color_split_across_buckets() {
        // 0x7F e 0x80 caem em baldes vizinhos; a média fica no meio.
        let c = cover_accent(&image(&[(0x7F, 20, 200), (0x80, 20, 200)], 30)).unwrap();
        assert!((c.r * 255.0 - 127.5).abs() < 1.0, "{c:?}");
    }
}
