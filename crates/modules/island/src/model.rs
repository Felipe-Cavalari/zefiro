//! Tudo o que a barra precisa saber da ilha num instante: sessões, fonte escolhida e estado recolhido.

use crate::media::{MediaSession, Playback, SourceChooser};
use crate::state::IslandState;
use crate::view::Collapsed;

#[derive(Debug, Clone, Default)]
pub struct IslandModel {
    sessions: Vec<MediaSession>,
    current: Option<String>,
    chooser: SourceChooser,
    pub state: IslandState,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Changes {
    /// A faixa mostrada mudou (a pílula começou a espiar).
    pub track: bool,
    /// A capa mostrada mudou (o destaque pode mudar).
    pub cover: bool,
}

impl IslandModel {
    /// Novo retrato das sessões. `now` em segundos de um relógio monotônico qualquer.
    pub fn update(&mut self, sessions: Vec<MediaSession>, now: f64) -> Changes {
        let old_cover = self.current().and_then(|s| s.cover.as_ref().map(|c| c.id()));
        self.current = self.chooser.update(&sessions);
        self.sessions = sessions;
        let key = self.current().filter(|s| s.has_track()).map(MediaSession::track_key);
        let track = self.state.track(key, now);
        let new_cover = self.current().and_then(|s| s.cover.as_ref().map(|c| c.id()));
        Changes { track, cover: old_cover != new_cover }
    }

    /// Escolha na barra de fontes do painel expandido.
    pub fn pick(&mut self, id: &str, now: f64) -> Changes {
        self.chooser.pick(id);
        let sessions = std::mem::take(&mut self.sessions);
        self.update(sessions, now)
    }

    pub fn current(&self) -> Option<&MediaSession> {
        let id = self.current.as_ref()?;
        self.sessions.iter().find(|s| &s.id == id)
    }

    /// Há mídia para mostrar (uma sessão com faixa).
    pub fn media(&self) -> Option<&MediaSession> {
        self.current().filter(|s| s.has_track())
    }

    pub fn playing(&self) -> bool {
        self.media().is_some_and(|s| s.playback == Playback::Playing)
    }

    /// (id, nome) de todas as sessões, na ordem do Windows.
    pub fn sources(&self) -> Vec<(String, String)> {
        self.sessions.iter().map(|s| (s.id.clone(), s.app_name.clone())).collect()
    }

    pub fn collapsed(&self, now: f64) -> Collapsed {
        self.state.collapsed(self.media().is_some(), now)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::media::session;
    use zefiro_platform::image::Image;

    #[test]
    fn track_change_peeks_and_cover_change_is_reported() {
        let mut m = IslandModel::default();
        let mut a = session("spotify", Playback::Playing);
        a.cover = Image::new(1, 1, vec![0, 0, 0, 255]);
        let first = m.update(vec![a.clone()], 0.0);
        assert!(first.cover && !first.track);
        assert_eq!(m.collapsed(0.0), Collapsed::Media);
        assert!(m.playing());
        let mut b = a.clone();
        b.title = "outra".into();
        b.cover = Image::new(1, 1, vec![0, 0, 0, 255]);
        let next = m.update(vec![b.clone()], 5.0);
        assert!(next.track && next.cover);
        assert_eq!(m.collapsed(6.0), Collapsed::Peek);
        assert!(!m.update(vec![b], 7.0).cover, "mesma capa");
    }

    #[test]
    fn no_sessions_is_idle_and_pick_switches_source() {
        let mut m = IslandModel::default();
        m.update(vec![], 0.0);
        assert_eq!(m.collapsed(0.0), Collapsed::Idle);
        m.update(vec![session("spotify", Playback::Playing), session("brave", Playback::Paused)], 1.0);
        assert_eq!(m.current().unwrap().id, "spotify");
        m.pick("brave", 2.0);
        assert_eq!(m.current().unwrap().id, "brave");
        assert_eq!(m.sources().len(), 2);
        assert!(!m.playing());
    }
}
