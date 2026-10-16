//! Analisador de espectro portado do Dynamic Island for Windows (MIT):
//! janela Hann de 1024, FFT radix-2, 24 bandas logarítmicas, ganho automático,
//! subida 0,65 / queda 0,14, portão de silêncio.

pub const WINDOW: usize = 1024;
pub const HOP: usize = 512;
pub const BANDS: usize = 24;
pub const LOW_HZ: f32 = 40.0;
pub const HIGH_HZ: f32 = 16_000.0;

pub struct Analyzer {
    ring: Vec<f32>,
    write: usize,
    since_hop: usize,
    window: Vec<f32>,
    tw_cos: Vec<f32>,
    tw_sin: Vec<f32>,
    bit_rev: Vec<usize>,
    bin_lo: [usize; BANDS],
    bin_hi: [usize; BANDS],
    smooth: [f32; BANDS],
    agc: f32,
    peak: f32,
}

fn band_edges(sample_rate: f32) -> [f32; BANDS + 1] {
    let high = HIGH_HZ.min(sample_rate * 0.45);
    let mut e = [0.0; BANDS + 1];
    for (b, edge) in e.iter_mut().enumerate() {
        *edge = LOW_HZ * (high / LOW_HZ).powf(b as f32 / BANDS as f32);
    }
    e
}

/// Índice da banda que contém `freq`.
pub fn band_index_for(freq: f32, sample_rate: f32) -> usize {
    let e = band_edges(sample_rate);
    (0..BANDS).find(|&b| freq >= e[b] && freq < e[b + 1]).unwrap_or(BANDS - 1)
}

impl Analyzer {
    pub fn new(sample_rate: f32) -> Self {
        let n = WINDOW;
        let window = (0..n).map(|i| 0.5 - 0.5 * (2.0 * std::f32::consts::PI * i as f32 / (n - 1) as f32).cos()).collect();
        let tw_cos = (0..n / 2).map(|k| (2.0 * std::f32::consts::PI * k as f32 / n as f32).cos()).collect();
        let tw_sin = (0..n / 2).map(|k| -(2.0 * std::f32::consts::PI * k as f32 / n as f32).sin()).collect();
        let bits = n.trailing_zeros();
        let bit_rev = (0..n).map(|i| i.reverse_bits() >> (usize::BITS - bits)).collect();
        let bin_hz = sample_rate / n as f32;
        let nyquist_bin = n / 2;
        let e = band_edges(sample_rate);
        let mut bin_lo = [0; BANDS];
        let mut bin_hi = [0; BANDS];
        for b in 0..BANDS {
            let lo = ((e[b] / bin_hz).floor() as usize).clamp(1, nyquist_bin);
            let hi = ((e[b + 1] / bin_hz).ceil() as usize).max(lo + 1).min(nyquist_bin + 1);
            bin_lo[b] = lo;
            bin_hi[b] = hi;
        }
        Self {
            ring: vec![0.0; n], write: 0, since_hop: 0, window, tw_cos, tw_sin, bit_rev,
            bin_lo, bin_hi, smooth: [0.0; BANDS], agc: 0.04, peak: 0.0,
        }
    }

    pub fn bands(&self) -> [f32; BANDS] {
        self.smooth
    }

    pub fn peak(&self) -> f32 {
        self.peak
    }

    pub fn push(&mut self, mono: &[f32]) -> bool {
        let mut analyzed = false;
        for &s in mono {
            self.ring[self.write] = s;
            self.write = (self.write + 1) % WINDOW;
            self.since_hop += 1;
            if self.since_hop >= HOP {
                self.since_hop = 0;
                self.analyze();
                analyzed = true;
            }
        }
        analyzed
    }

    fn fft(&self, re: &mut [f32], im: &mut [f32]) {
        for i in 0..WINDOW {
            let j = self.bit_rev[i];
            if j > i {
                re.swap(i, j);
                im.swap(i, j);
            }
        }
        let mut len = 2;
        while len <= WINDOW {
            let half = len / 2;
            let step = WINDOW / len;
            for i in (0..WINDOW).step_by(len) {
                for k in 0..half {
                    let (wr, wi) = (self.tw_cos[k * step], self.tw_sin[k * step]);
                    let (a, b) = (i + k, i + k + half);
                    let tr = re[b] * wr - im[b] * wi;
                    let ti = re[b] * wi + im[b] * wr;
                    re[b] = re[a] - tr;
                    im[b] = im[a] - ti;
                    re[a] += tr;
                    im[a] += ti;
                }
            }
            len <<= 1;
        }
    }

    fn analyze(&mut self) {
        let mut re = vec![0.0f32; WINDOW];
        let mut im = vec![0.0f32; WINDOW];
        for (i, (slot, w)) in re.iter_mut().zip(&self.window).enumerate() {
            *slot = self.ring[(self.write + i) % WINDOW] * w;
        }
        self.fft(&mut re, &mut im);
        let mut raw = [0.0f32; BANDS];
        let mut peak = 0.0f32;
        for (b, out) in raw.iter_mut().enumerate() {
            let (mut sum, mut max) = (0.0f32, 0.0f32);
            for k in self.bin_lo[b]..self.bin_hi[b] {
                let p = re[k] * re[k] + im[k] * im[k];
                sum += p;
                max = max.max(p);
            }
            // Metade média, metade bin mais forte.
            let power = 0.5 * (sum / (self.bin_hi[b] - self.bin_lo[b]) as f32) + 0.5 * max;
            // 4/N normaliza uma senoide com janela Hann para ~sua amplitude.
            let mut amp = power.sqrt() * (4.0 / WINDOW as f32);
            // Música tem inclinação espectral forte: levanta os agudos.
            amp *= 1.0 + 2.2 * b as f32 / (BANDS - 1) as f32;
            *out = amp;
            peak = peak.max(amp);
        }
        self.peak = peak;
        self.agc = peak.max(self.agc * 0.9985);
        let norm = 1.0 / self.agc.max(0.02);
        let gate = (peak * 40.0).clamp(0.0, 1.0);
        for (smooth, raw) in self.smooth.iter_mut().zip(raw) {
            let v = (raw * norm).clamp(0.0, 1.0).sqrt() * gate;
            let rate = if v > *smooth { 0.65 } else { 0.14 };
            *smooth += rate * (v - *smooth);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SR: f32 = 48_000.0;

    fn sine(freq: f32, amp: f32, secs: f32) -> Vec<f32> {
        (0..(SR * secs) as usize).map(|i| amp * (2.0 * std::f32::consts::PI * freq * i as f32 / SR).sin()).collect()
    }

    fn feed(a: &mut Analyzer, samples: &[f32]) {
        for chunk in samples.chunks(480) {
            a.push(chunk);
        }
    }

    fn argmax(b: &[f32; BANDS]) -> usize {
        b.iter().enumerate().max_by(|x, y| x.1.total_cmp(y.1)).map(|(i, _)| i).unwrap()
    }

    #[test]
    fn silence_stays_silent() {
        let mut a = Analyzer::new(SR);
        feed(&mut a, &vec![0.0; WINDOW * 10]);
        assert!(a.bands().iter().all(|v| *v < 0.01), "{:?}", a.bands());
    }

    #[test]
    fn a_tone_in_the_middle_of_a_band_lights_that_band() {
        let mut a = Analyzer::new(SR);
        // 900 Hz fica no meio da banda 12 (800–1026 Hz); perto da borda os bins da FFT são compartilhados.
        feed(&mut a, &sine(900.0, 0.5, 2.0));
        assert_eq!(argmax(&a.bands()), band_index_for(900.0, SR));
    }

    #[test]
    fn a_low_tone_lights_a_lower_band_than_a_high_tone() {
        let mut low = Analyzer::new(SR);
        feed(&mut low, &sine(120.0, 0.5, 2.0));
        let mut high = Analyzer::new(SR);
        feed(&mut high, &sine(6000.0, 0.5, 2.0));
        assert!(argmax(&low.bands()) < argmax(&high.bands()));
    }

    #[test]
    fn loud_tone_is_normalized_near_full_scale() {
        let mut a = Analyzer::new(SR);
        feed(&mut a, &sine(1000.0, 0.9, 3.0));
        let max = a.bands().iter().copied().fold(0.0, f32::max);
        assert!(max > 0.7, "{max}");
        assert!(max <= 1.0);
    }

    #[test]
    fn push_reports_a_new_analysis_every_hop() {
        let mut a = Analyzer::new(SR);
        assert!(!a.push(&vec![0.0; HOP - 1]));
        assert!(a.push(&[0.0]));
    }
}
