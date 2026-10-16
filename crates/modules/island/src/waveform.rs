//! Forma de onda do estilo "barras" da ilha (Dynamic Island for Windows, MIT, §7.1 das notas de porte):
//! nível RMS × 4 com envelope (sobe 0,3/0,7, desce 0,85/0,15) num anel de 48 amostras.

pub const RING: usize = 48;
/// Cada barra lê o anel com passo de 4 amostras (~40 ms), para as barras não ficarem iguais.
pub const STEP: usize = 4;
/// Uma amostra do anel a cada 10 ms de áudio.
const BLOCK_S: f32 = 0.010;

#[derive(Debug, Clone)]
pub struct Waveform {
    ring: [f32; RING],
    head: usize,
    level: f32,
    acc: f32,
    acc_n: usize,
}

impl Default for Waveform {
    fn default() -> Self {
        Self::new()
    }
}

impl Waveform {
    pub fn new() -> Self {
        Self { ring: [0.0; RING], head: 0, level: 0.0, acc: 0.0, acc_n: 0 }
    }

    /// Recebe áudio mono. Devolve `true` se entrou pelo menos uma amostra nova no anel.
    pub fn push(&mut self, mono: &[f32], sample_rate: f32) -> bool {
        let block = ((sample_rate * BLOCK_S) as usize).max(1);
        let mut pushed = false;
        for &s in mono {
            self.acc += s * s;
            self.acc_n += 1;
            if self.acc_n >= block {
                let rms = (self.acc / self.acc_n as f32).sqrt();
                let target = (rms * 4.0).clamp(0.0, 1.0);
                self.level = if target > self.level { 0.3 * self.level + 0.7 * target } else { 0.85 * self.level + 0.15 * target };
                self.ring[self.head] = self.level;
                self.head = (self.head + 1) % RING;
                self.acc = 0.0;
                self.acc_n = 0;
                pushed = true;
            }
        }
        pushed
    }

    /// Altura de cada uma das `count` barras, de 0 a 1 (a mais recente à direita).
    pub fn bars(&self, count: usize) -> Vec<f32> {
        (0..count)
            .map(|i| {
                let back = (count - 1 - i) * STEP % RING;
                self.ring[(self.head + RING - 1 - back) % RING]
            })
            .collect()
    }

    pub fn is_silent(&self) -> bool {
        self.ring.iter().all(|v| *v < 0.01)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SR: f32 = 48_000.0;

    #[test]
    fn silence_stays_flat_and_a_tone_lifts_the_bars() {
        let mut w = Waveform::new();
        w.push(&vec![0.0; 48_00], SR);
        assert!(w.is_silent());
        let tone: Vec<f32> = (0..30_000).map(|i| 0.3 * (i as f32 * 0.05).sin()).collect();
        assert!(w.push(&tone, SR));
        let bars = w.bars(12);
        assert_eq!(bars.len(), 12);
        assert!(bars.iter().all(|b| *b > 0.5 && *b <= 1.0), "{bars:?}");
    }

    #[test]
    fn rising_is_faster_than_falling() {
        let mut w = Waveform::new();
        let loud = vec![0.25f32; 480];
        w.push(&loud, SR);
        let up = w.bars(1)[0];
        assert!(up > 0.6, "subiu 70 % de uma vez: {up}");
        w.push(&[0.0; 480], SR);
        let down = w.bars(1)[0];
        assert!(down > 0.8 * up, "desce devagar: {up} → {down}");
    }
}
