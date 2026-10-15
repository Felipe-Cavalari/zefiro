//! Molas do Dynamic Island for Windows (MIT): constantes, estilos, velocidade e conversão para o compositor.

use zefiro_resources::theme::SpringStyle;

/// "Cutucada" vertical ao abrir: começa em −6 DIP e volta a 0 com a mola SETTLE.
pub const NUDGE_DIP: f32 = -6.0;
const FIXED_DT: f32 = 0.0005;
const SETTLE_EPS: f32 = 0.01;
const MAX_DT: f32 = 0.050;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SpringParams { pub stiffness: f32, pub damping: f32 }

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CompositorSpring { pub period_s: f32, pub damping_ratio: f32 }

impl SpringParams {
    pub const GROW: Self = Self { stiffness: 380.0, damping: 26.0 };
    pub const SHRINK: Self = Self { stiffness: 200.0, damping: 28.0 };
    pub const SETTLE: Self = Self { stiffness: 280.0, damping: 24.0 };

    pub fn toward(from: f32, to: f32) -> Self {
        if to > from {
            Self::GROW
        } else if to < from {
            Self::SHRINK
        } else {
            Self::SETTLE
        }
    }

    pub fn styled(self, style: SpringStyle) -> Self {
        let (s, d) = match style {
            SpringStyle::Smooth => (1.0, 1.35),
            SpringStyle::Default => (1.0, 1.0),
            SpringStyle::Bouncy => (1.1, 0.70),
            SpringStyle::Snappy => (1.5, 1.25),
        };
        Self { stiffness: self.stiffness * s, damping: self.damping * d }
    }

    /// Velocidade v: equivale a multiplicar o tempo por v (a ilha multiplica o `dt`).
    pub fn with_speed(self, v: f32) -> Self {
        Self { stiffness: self.stiffness * v * v, damping: self.damping * v }
    }

    /// Modelo do compositor: período e razão de amortecimento (c / 2ω), com ω = √k. O
    /// `Period` do Windows.UI.Composition age como 1/ω, não 2π/ω: medido na tela, 2π/ω deixava
    /// as molas 4–5× mais lentas que as da ilha.
    pub fn compositor(self) -> CompositorSpring {
        let omega = self.stiffness.sqrt();
        CompositorSpring { period_s: 1.0 / omega, damping_ratio: self.damping / (2.0 * omega) }
    }
}

/// Integrador da ilha: Euler semi-implícito em passos fixos de 0,5 ms.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Spring { pub value: f32, pub velocity: f32, pub target: f32 }

impl Spring {
    pub fn new(value: f32) -> Self {
        Self { value, velocity: 0.0, target: value }
    }

    pub fn is_settled(&self) -> bool {
        (self.value - self.target).abs() < SETTLE_EPS && self.velocity.abs() < SETTLE_EPS
    }

    pub fn step(&mut self, dt: f32, p: SpringParams) {
        let mut remaining = dt.clamp(0.0, MAX_DT);
        while remaining > 0.0 {
            let h = remaining.min(FIXED_DT);
            let accel = -p.stiffness * (self.value - self.target) - p.damping * self.velocity;
            self.velocity += accel * h;
            self.value += self.velocity * h;
            remaining -= h;
        }
        if self.is_settled() {
            self.value = self.target;
            self.velocity = 0.0;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use zefiro_resources::theme::SpringStyle;

    fn settle_time(p: SpringParams) -> f32 {
        let mut s = Spring::new(0.0);
        s.target = 100.0;
        let mut t = 0.0;
        while !s.is_settled() && t < 10.0 {
            s.step(1.0 / 240.0, p);
            t += 1.0 / 240.0;
        }
        t
    }

    #[test]
    fn constants_match_the_island() {
        assert_eq!(SpringParams::GROW, SpringParams { stiffness: 380.0, damping: 26.0 });
        assert_eq!(SpringParams::SHRINK, SpringParams { stiffness: 200.0, damping: 28.0 });
        assert_eq!(SpringParams::SETTLE, SpringParams { stiffness: 280.0, damping: 24.0 });
        assert_eq!(NUDGE_DIP, -6.0);
    }

    #[test]
    fn styles_multiply_like_the_island() {
        let p = SpringParams::SETTLE.styled(SpringStyle::Snappy);
        assert!((p.stiffness - 420.0).abs() < 1e-3 && (p.damping - 30.0).abs() < 1e-3);
        let p = SpringParams::SETTLE.styled(SpringStyle::Bouncy);
        assert!((p.stiffness - 308.0).abs() < 1e-3 && (p.damping - 16.8).abs() < 1e-3);
        assert_eq!(SpringParams::SETTLE.styled(SpringStyle::Default), SpringParams::SETTLE);
    }

    #[test]
    fn double_speed_halves_the_settle_time() {
        let normal = settle_time(SpringParams::GROW);
        let fast = settle_time(SpringParams::GROW.with_speed(2.0));
        assert!((fast / normal - 0.5).abs() < 0.05, "normal={normal} fast={fast}");
    }

    #[test]
    fn compositor_conversion() {
        // O Period do compositor se comporta como 1/ω (medido na tela: com 2π/ω as molas
        // ficavam 4–5× mais lentas que as da ilha).
        let c = SpringParams::SETTLE.compositor();
        assert!((c.period_s - 0.059761).abs() < 1e-4, "{c:?}");
        assert!((c.damping_ratio - 0.71714).abs() < 1e-3, "{c:?}");
    }

    #[test]
    fn toward_picks_grow_or_shrink() {
        assert_eq!(SpringParams::toward(10.0, 20.0), SpringParams::GROW);
        assert_eq!(SpringParams::toward(20.0, 10.0), SpringParams::SHRINK);
        assert_eq!(SpringParams::toward(10.0, 10.0), SpringParams::SETTLE);
    }
}
