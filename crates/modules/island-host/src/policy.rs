//! Até `max` reinícios dentro de `window`; depois desiste.

use std::collections::VecDeque;
use std::time::{Duration, Instant};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Decision { Restart, GiveUp }

#[derive(Debug)]
pub struct RestartPolicy { max: usize, window: Duration, restarts: VecDeque<Instant>, gave_up: bool }

impl RestartPolicy {
    pub fn new(max: usize, window: Duration) -> Self {
        Self { max, window, restarts: VecDeque::new(), gave_up: false }
    }

    /// A ilha saiu sem ser pedida. Reinicia se houve menos de `max` reinícios dentro de `window`.
    pub fn on_exit(&mut self, now: Instant) -> Decision {
        if self.gave_up {
            return Decision::GiveUp;
        }
        while self.restarts.front().is_some_and(|t| now.duration_since(*t) > self.window) {
            self.restarts.pop_front();
        }
        if self.restarts.len() >= self.max {
            self.gave_up = true;
            return Decision::GiveUp;
        }
        self.restarts.push_back(now);
        Decision::Restart
    }

    /// "Reiniciar a ilha" no menu: começa a contagem de novo.
    pub fn reset(&mut self) {
        self.restarts.clear();
        self.gave_up = false;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{Duration, Instant};

    #[test]
    fn restart_policy_allows_three_in_a_minute() {
        let mut p = RestartPolicy::new(3, Duration::from_secs(60));
        let t0 = Instant::now();
        for i in 0..3 {
            assert_eq!(p.on_exit(t0 + Duration::from_secs(i)), Decision::Restart);
        }
    }

    #[test]
    fn restart_policy_gives_up_on_fourth() {
        let mut p = RestartPolicy::new(3, Duration::from_secs(60));
        let t0 = Instant::now();
        for i in 0..3 {
            p.on_exit(t0 + Duration::from_secs(i));
        }
        assert_eq!(p.on_exit(t0 + Duration::from_secs(10)), Decision::GiveUp);
        assert_eq!(p.on_exit(t0 + Duration::from_secs(500)), Decision::GiveUp); // desistiu até reset()
    }

    #[test]
    fn restart_policy_window_slides() {
        let mut p = RestartPolicy::new(3, Duration::from_secs(60));
        let t0 = Instant::now();
        for i in 0..3 {
            p.on_exit(t0 + Duration::from_secs(i * 25)); // 0, 25, 50
        }
        assert_eq!(p.on_exit(t0 + Duration::from_secs(61)), Decision::Restart); // o de 0 s saiu da janela
    }

    #[test]
    fn restart_policy_reset_after_manual_restart() {
        let mut p = RestartPolicy::new(3, Duration::from_secs(60));
        let t0 = Instant::now();
        for i in 0..4 {
            p.on_exit(t0 + Duration::from_secs(i));
        }
        p.reset();
        assert_eq!(p.on_exit(t0 + Duration::from_secs(5)), Decision::Restart);
    }
}
