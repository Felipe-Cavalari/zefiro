//! Brilho por DDC/CI: o monitor responde devagar (dezenas de ms por escrita), então a interface
//! atualiza na hora (otimista) e o worker grava só o último valor pedido, uma escrita por vez e com
//! um intervalo mínimo entre elas (PLANNING §3.4, "Escritas lentas"; spike S3).

use std::time::{Duration, Instant};

/// Brilho de um monitor como a barra mostra.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MonitorBrightness {
    /// Nome GDI (`\\.\DISPLAY1`): liga ao monitor da barra.
    pub device: String,
    /// Nome amigável (descrição do monitor físico).
    pub name: String,
    /// `None` = o monitor não respondeu ao DDC/CI.
    pub percent: Option<u32>,
}

/// Guarda só o último valor pedido e libera uma escrita por vez, respeitando um intervalo mínimo
/// entre o fim de uma e o início da próxima.
#[derive(Debug, Clone)]
pub struct Coalescer {
    pending: Option<u32>,
    in_flight: bool,
    last_done: Option<Instant>,
    min_interval: Duration,
}

impl Coalescer {
    pub fn new(min_interval: Duration) -> Coalescer {
        Coalescer { pending: None, in_flight: false, last_done: None, min_interval }
    }

    pub fn request(&mut self, value: u32) {
        self.pending = Some(value);
    }

    /// Valor a escrever agora, se puder.
    pub fn next_write(&mut self, now: Instant) -> Option<u32> {
        if self.in_flight || self.last_done.is_some_and(|t| now.duration_since(t) < self.min_interval) {
            return None;
        }
        let v = self.pending.take()?;
        self.in_flight = true;
        Some(v)
    }

    pub fn complete(&mut self, now: Instant) {
        self.in_flight = false;
        self.last_done = Some(now);
    }

    /// Quanto esperar até a próxima escrita poder sair (`None` = nada pendente).
    pub fn wait(&self, now: Instant) -> Option<Duration> {
        self.pending?;
        if self.in_flight {
            return Some(self.min_interval);
        }
        Some(self.last_done.map_or(Duration::ZERO, |t| self.min_interval.saturating_sub(now.duration_since(t))))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_the_last_requested_value_is_written() {
        let t0 = Instant::now();
        let mut c = Coalescer::new(Duration::ZERO);
        c.request(10);
        assert_eq!(c.next_write(t0), Some(10));
        for v in 11..=60 {
            c.request(v);
        }
        assert_eq!(c.next_write(t0), None, "escrita em andamento bloqueia a próxima");
        c.complete(t0);
        assert_eq!(c.next_write(t0), Some(60));
        c.complete(t0);
        assert_eq!(c.next_write(t0), None, "nada pendente");
        assert_eq!(c.wait(t0), None);
    }

    #[test]
    fn respects_the_minimum_interval() {
        let t0 = Instant::now();
        let mut c = Coalescer::new(Duration::from_millis(50));
        c.request(1);
        assert_eq!(c.next_write(t0), Some(1));
        c.complete(t0);
        c.request(2);
        assert_eq!(c.wait(t0 + Duration::from_millis(20)), Some(Duration::from_millis(30)));
        assert_eq!(c.next_write(t0 + Duration::from_millis(49)), None);
        assert_eq!(c.next_write(t0 + Duration::from_millis(50)), Some(2));
    }
}
