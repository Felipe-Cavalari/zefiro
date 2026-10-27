//! Ocultar automaticamente (`[bar] autohide`): quando a barra de um monitor desce e quando sobe.
//! Só a regra; observar o mouse, animar e esconder a janela ficam com a `platform-windows`.

use zefiro_platform::geometry::Rect;

/// Tempo com o cursor encostado no topo até a barra descer: sem isso, ir clicar numa aba do
/// navegador ou no X de uma janela maximizada faria a barra pular por cima delas.
pub const SHOW_DELAY_MS: u64 = 300;
/// Tempo com o cursor fora da barra até ela subir.
pub const HIDE_DELAY_MS: u64 = 500;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Phase {
    Hidden,
    /// Cursor no topo desde `since` (ms).
    Arming { since: u64 },
    Shown,
    /// Cursor fora da barra desde `since` (ms).
    Leaving { since: u64 },
}

/// O que a barra sabe agora para decidir.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Inputs {
    /// Cursor num gatilho do topo do monitor (`top_triggers`).
    pub at_edge: bool,
    /// Cursor sobre a faixa da barra.
    pub over_bar: bool,
    /// Painel, menu ou Editar barra aberto neste monitor: a barra não sobe.
    pub held: bool,
    /// App em tela cheia neste monitor: a borda não faz nada e a barra fica fora.
    pub fullscreen: bool,
}

/// Resultado de um passo: a barra deve estar visível e, se for o caso, daqui a quanto acordar.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Step { pub visible: bool, pub wake_in_ms: Option<u64> }

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AutoHide { phase: Phase }

impl Default for AutoHide {
    fn default() -> Self {
        AutoHide { phase: Phase::Hidden }
    }
}

impl AutoHide {
    pub fn visible(&self) -> bool {
        matches!(self.phase, Phase::Shown | Phase::Leaving { .. })
    }

    /// Descer agora, como se o cursor tivesse ficado no topo ("Mostrar a barra" na bandeja). Ela sobe
    /// pela regra normal quando o cursor não estiver nela.
    pub fn reveal(&mut self) {
        self.phase = Phase::Shown;
    }

    /// Recalcula com o estado de agora (`now` em ms, de qualquer relógio que só cresce).
    pub fn step(&mut self, i: Inputs, now: u64) -> Step {
        let wait = |since: u64, delay: u64| delay.saturating_sub(now.saturating_sub(since));
        self.phase = if i.fullscreen {
            Phase::Hidden
        } else if i.held {
            Phase::Shown
        } else {
            match self.phase {
                Phase::Hidden if i.at_edge => Phase::Arming { since: now },
                Phase::Hidden => Phase::Hidden,
                Phase::Arming { .. } if !i.at_edge => Phase::Hidden,
                Phase::Arming { since } if wait(since, SHOW_DELAY_MS) == 0 => Phase::Shown,
                Phase::Shown | Phase::Leaving { .. } if i.over_bar || i.at_edge => Phase::Shown,
                Phase::Shown => Phase::Leaving { since: now },
                Phase::Leaving { since } if wait(since, HIDE_DELAY_MS) == 0 => Phase::Hidden,
                other => other,
            }
        };
        let wake_in_ms = match self.phase {
            Phase::Arming { since } => Some(wait(since, SHOW_DELAY_MS).max(1)),
            Phase::Leaving { since } => Some(wait(since, HIDE_DELAY_MS).max(1)),
            _ => None,
        };
        Step { visible: self.visible(), wake_in_ms }
    }
}

/// Trechos (x inicial, x final) da borda de cima de `monitor` que são borda de verdade da área de
/// trabalho: onde há outro monitor logo acima, o cursor passa direto para ele e não "encosta".
pub fn top_edges(monitor: Rect, others: &[Rect]) -> Vec<(f32, f32)> {
    let mut edges = vec![(monitor.x, monitor.right())];
    for o in others.iter().filter(|o| (o.bottom() - monitor.y).abs() < 0.5) {
        edges = edges
            .into_iter()
            .flat_map(|(a, b)| {
                let (cut_a, cut_b) = (o.x.max(a), o.right().min(b));
                if cut_a >= cut_b { vec![(a, b)] } else { [(a, cut_a), (cut_b, b)].into_iter().filter(|(x, y)| y > x).collect() }
            })
            .collect();
    }
    edges
}

/// Espessura da faixa de ativação onde outro monitor está logo acima (DIP).
pub const COVERED_BAND_DIP: f32 = 6.0;

/// `COVERED_BAND_DIP` em pixels num monitor de `dpi`.
pub fn covered_band(dpi: u32) -> f32 {
    COVERED_BAND_DIP * dpi as f32 / 96.0
}

/// Onde o cursor faz a barra de `monitor` descer, em pixels de tela. Na borda de verdade, a linha de
/// cima, onde o cursor encosta e para. Onde outro monitor está logo acima, o cursor passaria direto
/// para ele, então o gatilho é uma faixa de `band` px no alto do próprio monitor (`covered_band`).
/// Juntos, cobrem toda a largura: a barra nunca fica só pela bandeja.
pub fn top_triggers(monitor: Rect, others: &[Rect], band: f32) -> Vec<Rect> {
    let mut triggers = Vec::new();
    let mut x = monitor.x;
    for (a, b) in top_edges(monitor, others) {
        if a > x {
            triggers.push(Rect::new(x, monitor.y, a - x, band));
        }
        triggers.push(Rect::new(a, monitor.y, b - a, 1.0));
        x = b;
    }
    if monitor.right() > x {
        triggers.push(Rect::new(x, monitor.y, monitor.right() - x, band));
    }
    triggers
}

#[cfg(test)]
mod tests {
    use super::*;

    const EDGE: Inputs = Inputs { at_edge: true, over_bar: false, held: false, fullscreen: false };
    const AWAY: Inputs = Inputs { at_edge: false, over_bar: false, held: false, fullscreen: false };
    const OVER: Inputs = Inputs { at_edge: false, over_bar: true, held: false, fullscreen: false };

    #[test]
    fn the_bar_comes_down_only_after_resting_on_the_edge() {
        let mut a = AutoHide::default();
        assert_eq!(a.step(EDGE, 1000), Step { visible: false, wake_in_ms: Some(SHOW_DELAY_MS) });
        assert_eq!(a.step(EDGE, 1200), Step { visible: false, wake_in_ms: Some(100) });
        assert!(a.step(EDGE, 1300).visible, "300 ms no topo: desce");
        // Passar pelo topo rápido (aba do navegador) não traz a barra.
        let mut b = AutoHide::default();
        b.step(EDGE, 0);
        assert_eq!(b.step(AWAY, 150), Step { visible: false, wake_in_ms: None });
        assert!(!b.step(EDGE, 200).visible, "a contagem recomeça");
        assert_eq!(b.step(EDGE, 200).wake_in_ms, Some(SHOW_DELAY_MS));
    }

    #[test]
    fn it_goes_back_up_after_the_cursor_leaves_unless_it_returns() {
        let mut a = AutoHide::default();
        a.reveal();
        assert!(a.step(OVER, 0).visible);
        assert_eq!(a.step(AWAY, 100), Step { visible: true, wake_in_ms: Some(HIDE_DELAY_MS) });
        assert!(a.step(OVER, 400).visible, "voltou antes dos 500 ms");
        assert_eq!(a.step(AWAY, 450).wake_in_ms, Some(HIDE_DELAY_MS));
        assert!(a.step(AWAY, 900).visible);
        assert_eq!(a.step(AWAY, 950), Step { visible: false, wake_in_ms: None });
    }

    #[test]
    fn panels_hold_it_down_and_fullscreen_keeps_it_away() {
        let mut a = AutoHide::default();
        let held = Inputs { held: true, ..AWAY };
        assert!(a.step(held, 0).visible, "menu da bandeja ou painel aberto: desce e fica");
        assert!(a.step(held, 5000).visible);
        // Fechou o painel com o cursor fora: sobe pela regra normal.
        assert!(a.step(AWAY, 5000).visible);
        assert!(!a.step(AWAY, 5500).visible);
        // Jogo em tela cheia: o topo não faz nada, e a barra que estava aparecendo sai.
        let game = Inputs { fullscreen: true, ..EDGE };
        assert_eq!(a.step(game, 6000), Step { visible: false, wake_in_ms: None });
        a.reveal();
        assert!(!a.step(Inputs { fullscreen: true, ..OVER }, 6100).visible);
    }

    #[test]
    fn only_real_desktop_edges_count() {
        let main = Rect::new(0.0, 0.0, 1920.0, 1080.0);
        assert_eq!(top_edges(main, &[]), [(0.0, 1920.0)]);
        // Monitor ao lado não muda nada; um em cima, no meio, corta a borda.
        assert_eq!(top_edges(main, &[Rect::new(1920.0, 0.0, 1920.0, 1080.0)]), [(0.0, 1920.0)]);
        assert_eq!(top_edges(main, &[Rect::new(500.0, -1080.0, 1000.0, 1080.0)]), [(0.0, 500.0), (1500.0, 1920.0)]);
        assert!(top_edges(main, &[Rect::new(-100.0, -1440.0, 2560.0, 1440.0)]).is_empty(), "coberto inteiro: nenhuma borda de verdade (fica a faixa, `top_triggers`)");
    }

    fn triggers_at(triggers: &[Rect], x: f32, y: f32) -> bool {
        triggers.iter().any(|r| r.contains(zefiro_platform::geometry::Point { x, y }))
    }

    /// Arranjo de um tester: o 1 (principal) embaixo, o 2 logo acima alinhado à esquerda e mais estreito,
    /// o 3 em retrato à esquerda dos dois. Só a ponta direita do topo do 1 era borda de verdade, e a barra
    /// não descia pelo meio.
    #[test]
    fn a_monitor_with_another_above_has_a_band_under_it() {
        let main = Rect::new(0.0, 0.0, 2560.0, 1440.0);
        let above = Rect::new(0.0, -1080.0, 1920.0, 1080.0);
        let portrait = Rect::new(-1440.0, -1080.0, 1440.0, 2560.0);
        let t = top_triggers(main, &[above, portrait], covered_band(96));
        assert!(triggers_at(&t, 1280.0, 0.0) && triggers_at(&t, 1280.0, 5.0), "o meio do topo dispara: {t:?}");
        assert!(!triggers_at(&t, 1280.0, 6.0), "a faixa tem 6 px a 100 %");
        assert!(!triggers_at(&t, 1280.0, -1.0), "no monitor de cima não conta");
        // A ponta direita é borda de verdade: só a linha onde o cursor para.
        assert!(triggers_at(&t, 2200.0, 0.0) && !triggers_at(&t, 2200.0, 1.0));
        assert_eq!(covered_band(144), 9.0);
    }

    /// Em nenhum arranjo a barra fica só pela bandeja: todo ponto da linha de cima do monitor dispara.
    #[test]
    fn every_point_of_the_top_reaches_the_bar() {
        let main = Rect::new(0.0, 0.0, 2560.0, 1440.0);
        let arrangements: [&[Rect]; 7] = [
            &[],
            &[Rect::new(2560.0, 0.0, 1920.0, 1080.0)],
            &[Rect::new(-100.0, -1440.0, 2760.0, 1440.0)],
            &[Rect::new(500.0, -1080.0, 1000.0, 1080.0)],
            &[Rect::new(-500.0, -1080.0, 1500.0, 1080.0)],
            &[Rect::new(0.0, -1080.0, 1280.0, 1080.0), Rect::new(1280.0, -1080.0, 1280.0, 1080.0)],
            &[Rect::new(0.0, -1080.0, 1920.0, 1080.0), Rect::new(-1440.0, -1080.0, 1440.0, 2560.0)],
        ];
        for others in arrangements {
            let t = top_triggers(main, others, covered_band(96));
            for x in 0..2560 {
                assert!(triggers_at(&t, x as f32, 0.0), "x = {x} não dispara com {others:?}");
            }
        }
    }
}
