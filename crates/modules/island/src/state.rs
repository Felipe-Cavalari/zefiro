//! Qual pílula recolhida mostrar: ocioso, mídia ou espiando (troca de faixa por ~3 s, ou o mouse em cima).
//! O expandido é o painel da barra (pílula → painel), aberto pelo clique.

use crate::view::Collapsed;

pub const PEEK_SECS: f64 = 3.0;

#[derive(Debug, Clone, Default)]
pub struct IslandState {
    peek_until: Option<f64>,
    hovering: bool,
    track: Option<(String, String, String)>,
}

impl IslandState {
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
        self.peek_until.filter(|t| *t > now).map(|t| t - now)
    }

    pub fn collapsed(&self, has_media: bool, now: f64) -> Collapsed {
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
    fn same_track_again_does_nothing() {
        let mut s = IslandState::default();
        s.track(key("a"), 0.0);
        assert!(!s.track(key("a"), 1.0));
        assert_eq!(s.collapsed(true, 1.0), Collapsed::Media);
    }
}
