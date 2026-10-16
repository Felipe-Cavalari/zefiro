//! Níveis de 0 a 100 %: passo da roda do mouse e conversão de posição no slider.

/// Passo da roda: 2 % por clique (volume) ou 5 % (brilho), arredondado ao múltiplo do passo, para
/// a roda sempre cair em 0, 2, 4… e chegar exatamente a 0 e 100.
pub fn wheel(percent: u32, delta: i32, step: u32) -> u32 {
    let step = step.max(1) as i32;
    // Pelo menos um passo por evento (touchpads mandam frações de 120).
    let clicks = match delta {
        0 => 0,
        d if d > 0 => (d / 120).max(1),
        d => (d / 120).min(-1),
    };
    let base = if clicks > 0 { (percent as i32 / step) * step } else { (percent as i32 + step - 1) / step * step };
    (base + clicks * step).clamp(0, 100) as u32
}

/// Porcentagem de um ponto `x` num slider que começa em `left` e mede `width`.
pub fn slider_percent(x: f32, left: f32, width: f32) -> u32 {
    if width <= 0.0 {
        return 0;
    }
    (((x - left) / width).clamp(0.0, 1.0) * 100.0).round() as u32
}

/// Valor bruto do monitor (0..=max) ↔ porcentagem.
pub fn to_percent(value: u32, max: u32) -> u32 {
    if max == 0 { 0 } else { ((value as f64 * 100.0 / max as f64).round() as u32).min(100) }
}

pub fn from_percent(percent: u32, max: u32) -> u32 {
    ((percent.min(100) as f64 * max as f64 / 100.0).round() as u32).min(max)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wheel_steps_snap_to_the_grid_and_stop_at_the_ends() {
        assert_eq!(wheel(41, 120, 2), 42);
        assert_eq!(wheel(41, -120, 2), 40);
        assert_eq!(wheel(42, 120, 2), 44);
        assert_eq!(wheel(99, 120, 2), 100);
        assert_eq!(wheel(1, -120, 2), 0);
        assert_eq!(wheel(100, 240, 5), 100);
        assert_eq!(wheel(50, -360, 5), 35, "três cliques de uma vez");
        assert_eq!(wheel(50, 0, 5), 50);
        assert_eq!(wheel(50, 60, 5), 55, "meio clique de touchpad ainda anda um passo");
    }

    #[test]
    fn slider_and_raw_values() {
        assert_eq!(slider_percent(50.0, 0.0, 200.0), 25);
        assert_eq!(slider_percent(-10.0, 0.0, 200.0), 0);
        assert_eq!(slider_percent(500.0, 0.0, 200.0), 100);
        assert_eq!(to_percent(50, 100), 50);
        assert_eq!(to_percent(128, 255), 50);
        assert_eq!(from_percent(50, 255), 128);
        assert_eq!(from_percent(150, 100), 100);
        assert_eq!(to_percent(5, 0), 0);
    }
}
