//! Timer de foco da ilha (menu: 25 ou 50 min de foco, 5 min de pausa; pausar, retomar, parar).
//! Enquanto roda, ocupa a pílula (prioridade 12, acima da mídia).

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TimerKind { Focus, Break }

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FocusTimer {
    pub kind: TimerKind,
    pub total_s: f64,
    /// Rodando: quando termina. Pausado: `None`.
    end_at: Option<f64>,
    paused_remaining: f64,
}

impl FocusTimer {
    pub fn start(kind: TimerKind, minutes: u32, now: f64) -> FocusTimer {
        let total_s = minutes as f64 * 60.0;
        FocusTimer { kind, total_s, end_at: Some(now + total_s), paused_remaining: total_s }
    }

    pub fn running(&self) -> bool {
        self.end_at.is_some()
    }

    pub fn remaining(&self, now: f64) -> f64 {
        match self.end_at {
            Some(end) => (end - now).max(0.0),
            None => self.paused_remaining,
        }
    }

    pub fn finished(&self, now: f64) -> bool {
        self.running() && self.remaining(now) <= 0.0
    }

    pub fn pause(&mut self, now: f64) {
        if let Some(end) = self.end_at.take() {
            self.paused_remaining = (end - now).max(0.0);
        }
    }

    pub fn resume(&mut self, now: f64) {
        if self.end_at.is_none() {
            self.end_at = Some(now + self.paused_remaining);
        }
    }

    /// Fração já passada (0 → 1), para a barra de progresso.
    pub fn progress(&self, now: f64) -> f32 {
        if self.total_s <= 0.0 { 1.0 } else { (1.0 - self.remaining(now) / self.total_s).clamp(0.0, 1.0) as f32 }
    }
}

/// "mm:ss" (ou "h:mm:ss").
pub fn clock(remaining_s: f64) -> String {
    let s = remaining_s.max(0.0).ceil() as u64;
    if s >= 3600 { format!("{}:{:02}:{:02}", s / 3600, s / 60 % 60, s % 60) } else { format!("{:02}:{:02}", s / 60, s % 60) }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn runs_pauses_resumes_and_finishes() {
        let mut t = FocusTimer::start(TimerKind::Focus, 25, 100.0);
        assert_eq!(t.remaining(160.0), 1440.0);
        assert!((t.progress(850.0) - 0.5).abs() < 1e-6);
        t.pause(200.0);
        assert!(!t.running());
        assert_eq!(t.remaining(10_000.0), 1400.0, "pausado não anda");
        t.resume(1000.0);
        assert_eq!(t.remaining(1000.0), 1400.0);
        assert!(!t.finished(2399.0));
        assert!(t.finished(2400.0));
        assert_eq!(clock(1400.0), "23:20");
        assert_eq!(clock(59.2), "01:00");
        assert_eq!(clock(3725.0), "1:02:05");
    }
}
