//! Qual pílula recolhida mostrar: ocioso, mídia ou espiando (troca de faixa por ~3 s, ou o mouse em cima).
//! O expandido é o painel da barra (pílula → painel), aberto pelo clique.

use zefiro_platform::image::Image;

use crate::view::Collapsed;

pub const PEEK_SECS: f64 = 3.0;

/// Uma notificação do Windows, já resumida para a pílula.
#[derive(Debug, Clone, PartialEq)]
pub struct Notice { pub app: String, pub text: String, pub icon: Option<Image> }

/// Avisos que tomam a pílula por alguns segundos, por cima da mídia (§3.1 e §3.2 das notas de porte).
#[derive(Debug, Clone, PartialEq)]
pub enum Activity {
    /// Texto copiado: 2,5 s.
    Clipboard { text: String },
    /// Caps Lock / Num Lock: 2,5 s.
    Lock { caps: bool, on: bool },
    /// Não perturbe ligado ou desligado: 3 s.
    DoNotDisturb { on: bool },
    /// 1,8 s depois da última mudança.
    Volume { level: f32, muted: bool },
    /// 1,8 s depois da última mudança.
    Brightness { level: f32 },
    /// 4 s.
    Notification(Notice),
}

impl Activity {
    pub fn duration_s(&self) -> f64 {
        match self {
            Activity::Volume { .. } | Activity::Brightness { .. } => 1.8,
            Activity::Clipboard { .. } | Activity::Lock { .. } => 2.5,
            Activity::DoNotDisturb { .. } => 3.0,
            Activity::Notification(_) => 4.0,
        }
    }

    /// Prioridade da ilha: menor vence (área de transferência 2 … notificação 9).
    fn priority(&self) -> u8 {
        match self {
            Activity::Clipboard { .. } => 2,
            Activity::Lock { .. } => 3,
            Activity::DoNotDisturb { .. } => 6,
            Activity::Volume { .. } => 7,
            Activity::Brightness { .. } => 8,
            Activity::Notification(_) => 9,
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct IslandState {
    peek_until: Option<f64>,
    hovering: bool,
    track: Option<(String, String, String)>,
    activities: Vec<(Activity, f64)>,
    /// Timer de foco rodando ou pausado: ocupa a pílula acima da mídia.
    pub timer: Option<crate::timer::FocusTimer>,
}

impl IslandState {
    /// Mostra (ou renova) um aviso. O mesmo tipo de aviso substitui o anterior.
    pub fn activity(&mut self, a: Activity, now: f64) {
        self.activities.retain(|(old, until)| std::mem::discriminant(old) != std::mem::discriminant(&a) && *until > now);
        let until = now + a.duration_s();
        self.activities.push((a, until));
    }

    fn current_activity(&self, now: f64) -> Option<Activity> {
        self.activities.iter().filter(|(_, until)| *until > now).map(|(a, _)| a).min_by_key(|a| a.priority()).cloned()
    }

    /// A faixa atual mudou (ou a primeira apareceu). Devolve `true` se começou a espiar.
    pub fn track(&mut self, key: Option<(String, String, String)>, now: f64) -> bool {
        if key == self.track {
            return false;
        }
        let had_track = self.track.is_some();
        self.track = key;
        // A primeira faixa vista ao abrir a barra não espia: só uma troca de verdade.
        if had_track && self.track.is_some() {
            self.peek_until = Some(now + PEEK_SECS);
            return true;
        }
        false
    }

    pub fn hover(&mut self, on: bool) -> bool {
        let changed = self.hovering != on;
        self.hovering = on;
        changed
    }

    /// Quando o próximo `collapsed` pode mudar sozinho (fim da espiada), para agendar um temporizador.
    pub fn next_change(&self, now: f64) -> Option<f64> {
        self.peek_until.into_iter().chain(self.activities.iter().map(|(_, t)| *t)).filter(|t| *t > now).map(|t| t - now).min_by(f64::total_cmp)
    }

    pub fn collapsed(&self, has_media: bool, now: f64) -> Collapsed {
        if let Some(a) = self.current_activity(now) {
            return Collapsed::Activity(a);
        }
        if let Some(t) = self.timer {
            return Collapsed::Timer(t);
        }
        if !has_media {
            return Collapsed::Idle;
        }
        if self.hovering || self.peek_until.is_some_and(|t| now < t) { Collapsed::Peek } else { Collapsed::Media }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn key(t: &str) -> Option<(String, String, String)> {
        Some(("app".into(), t.into(), "artista".into()))
    }

    #[test]
    fn track_change_peeks_for_three_seconds() {
        let mut s = IslandState::default();
        assert!(!s.track(key("a"), 0.0), "primeira faixa não espia");
        assert_eq!(s.collapsed(true, 0.0), Collapsed::Media);
        assert!(s.track(key("b"), 10.0));
        assert_eq!(s.collapsed(true, 12.9), Collapsed::Peek);
        assert_eq!(s.next_change(12.0), Some(1.0));
        assert_eq!(s.collapsed(true, 13.0), Collapsed::Media);
        assert_eq!(s.next_change(13.0), None);
    }

    #[test]
    fn hover_peeks_while_inside_and_no_media_is_idle() {
        let mut s = IslandState::default();
        s.track(key("a"), 0.0);
        assert!(s.hover(true));
        assert_eq!(s.collapsed(true, 100.0), Collapsed::Peek);
        s.hover(false);
        assert_eq!(s.collapsed(true, 100.0), Collapsed::Media);
        assert_eq!(s.collapsed(false, 100.0), Collapsed::Idle);
    }

    #[test]
    fn volume_takes_the_pill_for_1_8_s_and_renews_on_each_change() {
        let mut s = IslandState::default();
        s.track(key("a"), 0.0);
        let v = Activity::Volume { level: 0.5, muted: false };
        s.activity(v.clone(), 10.0);
        assert_eq!(s.collapsed(true, 11.0), Collapsed::Activity(v.clone()));
        let v2 = Activity::Volume { level: 0.6, muted: false };
        s.activity(v2.clone(), 11.5);
        assert_eq!(s.collapsed(true, 13.0), Collapsed::Activity(v2), "renovado");
        assert!((s.next_change(13.0).unwrap() - 0.3).abs() < 1e-9);
        assert_eq!(s.collapsed(true, 13.3), Collapsed::Media);
        assert_eq!(s.collapsed(false, 13.3), Collapsed::Idle);
    }

    #[test]
    fn volume_wins_over_brightness() {
        let mut s = IslandState::default();
        let b = Activity::Brightness { level: 0.3 };
        let v = Activity::Volume { level: 0.5, muted: true };
        s.activity(b, 0.0);
        s.activity(v.clone(), 0.5);
        assert_eq!(s.collapsed(false, 1.0), Collapsed::Activity(v.clone()));
        assert_eq!(s.collapsed(false, 2.0), Collapsed::Activity(v), "volume ainda vale até 2,3 s");
        assert_eq!(s.collapsed(false, 2.4), Collapsed::Idle, "o brilho acabou em 1,8 s");
    }

    #[test]
    fn same_track_again_does_nothing() {
        let mut s = IslandState::default();
        s.track(key("a"), 0.0);
        assert!(!s.track(key("a"), 1.0));
        assert_eq!(s.collapsed(true, 1.0), Collapsed::Media);
    }
}
