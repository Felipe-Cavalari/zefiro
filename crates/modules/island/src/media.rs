//! Estado da mídia (sessões do SMTC) e a escolha da fonte mostrada pela ilha.

use zefiro_platform::image::Image;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Playback { Playing, Paused, Stopped }

#[derive(Debug, Clone, PartialEq)]
pub struct MediaSession {
    /// AUMID da sessão (ex.: `Spotify.exe`, `SpotifyAB.SpotifyMusic_…!Spotify`).
    pub id: String,
    /// Nome para mostrar (Spotify, Brave…).
    pub app_name: String,
    pub title: String,
    pub artist: String,
    pub album: String,
    pub playback: Playback,
    /// Posição no instante `sampled_ms` (ms desde a época Unix).
    pub position_s: f64,
    pub sampled_ms: i64,
    pub duration_s: f64,
    pub can_previous: bool,
    pub can_next: bool,
    pub can_play_pause: bool,
    pub can_seek: bool,
    pub cover: Option<Image>,
}

impl MediaSession {
    /// Posição agora: tocando, avança com o relógio desde a última amostra do SMTC.
    pub fn position_at(&self, now_ms: i64) -> f64 {
        let elapsed = if self.playback == Playback::Playing { (now_ms - self.sampled_ms).max(0) as f64 / 1000.0 } else { 0.0 };
        let p = self.position_s + elapsed;
        if self.duration_s > 0.0 { p.clamp(0.0, self.duration_s) } else { p.max(0.0) }
    }

    pub fn has_track(&self) -> bool {
        !self.title.is_empty()
    }

    /// "Título — Artista" (ou só o título).
    pub fn headline(&self) -> String {
        if self.artist.is_empty() { self.title.clone() } else { format!("{} — {}", self.title, self.artist) }
    }

    /// Identidade da faixa: muda quando a música troca.
    pub fn track_key(&self) -> (String, String, String) {
        (self.id.clone(), self.title.clone(), self.artist.clone())
    }
}

/// Fonte mostrada (§11 das notas de porte): (1) a escolhida na barra de fontes, até algo **novo**
/// começar a tocar ou o app sumir; (2) automático: o app que começou a tocar por último; sem nada
/// tocando, o último que tocou.
#[derive(Debug, Clone, Default)]
pub struct SourceChooser {
    picked: Option<String>,
    last_started: Option<String>,
    playing: Vec<String>,
}

impl SourceChooser {
    pub fn pick(&mut self, id: &str) {
        self.picked = Some(id.to_string());
    }

    pub fn update(&mut self, sessions: &[MediaSession]) -> Option<String> {
        let now_playing: Vec<String> = sessions.iter().filter(|s| s.playback == Playback::Playing).map(|s| s.id.clone()).collect();
        let started: Vec<&String> = now_playing.iter().filter(|id| !self.playing.contains(id)).collect();
        if let Some(id) = started.last() {
            self.last_started = Some((*id).clone());
            if self.picked.as_ref().is_some_and(|p| p != *id) {
                self.picked = None;
            }
        }
        self.playing = now_playing;
        let exists = |id: &String| sessions.iter().any(|s| &s.id == id);
        if self.picked.as_ref().is_some_and(|p| !exists(p)) {
            self.picked = None;
        }
        if let Some(p) = &self.picked {
            return Some(p.clone());
        }
        // O último que começou continua valendo enquanto existir (mesmo pausado).
        if let Some(id) = self.last_started.as_ref().filter(|id| exists(id))
            && (self.playing.contains(id) || self.playing.is_empty())
        {
            return Some(id.clone());
        }
        if let Some(id) = self.playing.last() {
            return Some(id.clone());
        }
        sessions.iter().find(|s| s.has_track()).or(sessions.first()).map(|s| s.id.clone())
    }
}

/// Nome para mostrar a partir do AUMID: `Spotify.exe` → Spotify, `SpotifyAB.SpotifyMusic_…!Spotify`
/// → Spotify, `Brave.Perfil` → Brave, `Microsoft.ZuneMusic_…!Microsoft.ZuneMusic` → ZuneMusic.
pub fn app_name_from_aumid(aumid: &str) -> String {
    let name = match aumid.rsplit_once('!') {
        Some((_, app)) => app.rsplit('.').next().unwrap_or(app),
        None => {
            let base = aumid.rsplit(['\\', '/']).next().unwrap_or(aumid);
            let base = if base.to_ascii_lowercase().ends_with(".exe") { &base[..base.len() - 4] } else { base };
            base.split('.').next().unwrap_or(base)
        }
    };
    let mut chars = name.chars();
    match chars.next() {
        Some(first) => first.to_uppercase().chain(chars).collect(),
        None => aumid.to_string(),
    }
}

#[cfg(test)]
pub(crate) fn session(id: &str, playback: Playback) -> MediaSession {
    MediaSession {
        id: id.into(),
        app_name: id.into(),
        title: format!("{id} música"),
        artist: "Artista".into(),
        album: String::new(),
        playback,
        position_s: 10.0,
        sampled_ms: 1_000,
        duration_s: 200.0,
        can_previous: true,
        can_next: true,
        can_play_pause: true,
        can_seek: true,
        cover: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use Playback::*;

    #[test]
    fn position_advances_only_while_playing_and_stops_at_the_end() {
        let mut s = session("a", Playing);
        assert_eq!(s.position_at(3_500), 12.5);
        assert_eq!(s.position_at(10_000_000), 200.0);
        s.playback = Paused;
        assert_eq!(s.position_at(3_500), 10.0);
    }

    #[test]
    fn latest_started_wins_and_paused_one_stays_when_nothing_plays() {
        let mut c = SourceChooser::default();
        assert_eq!(c.update(&[session("spotify", Playing), session("brave", Paused)]).as_deref(), Some("spotify"));
        assert_eq!(c.update(&[session("spotify", Playing), session("brave", Playing)]).as_deref(), Some("brave"));
        assert_eq!(c.update(&[session("spotify", Paused), session("brave", Paused)]).as_deref(), Some("brave"));
    }

    #[test]
    fn manual_pick_lasts_until_something_new_starts_or_the_app_closes() {
        let mut c = SourceChooser::default();
        c.update(&[session("spotify", Playing), session("brave", Paused)]);
        c.pick("brave");
        assert_eq!(c.update(&[session("spotify", Playing), session("brave", Paused)]).as_deref(), Some("brave"));
        // Outro app começa a tocar: a escolha manual termina.
        assert_eq!(c.update(&[session("spotify", Playing), session("brave", Paused), session("edge", Playing)]).as_deref(), Some("edge"));
        c.pick("brave");
        assert_eq!(c.update(&[session("spotify", Playing)]).as_deref(), Some("spotify"), "app sumiu");
    }

    #[test]
    fn app_names_from_aumids() {
        assert_eq!(app_name_from_aumid("Spotify.exe"), "Spotify");
        assert_eq!(app_name_from_aumid("SpotifyAB.SpotifyMusic_zpdnekdrzrea0!Spotify"), "Spotify");
        assert_eq!(app_name_from_aumid("Brave.ABCDEF"), "Brave");
        assert_eq!(app_name_from_aumid("MSEdge"), "MSEdge");
        assert_eq!(app_name_from_aumid("Microsoft.ZuneMusic_8wekyb3d8bbwe!Microsoft.ZuneMusic"), "ZuneMusic");
        assert_eq!(app_name_from_aumid("chrome"), "Chrome");
        assert_eq!(app_name_from_aumid(r"C:\Apps\foobar2000.exe"), "Foobar2000");
    }

    #[test]
    fn nothing_playing_falls_back_to_a_session_with_a_track() {
        let mut c = SourceChooser::default();
        let mut empty = session("x", Stopped);
        empty.title.clear();
        assert_eq!(c.update(&[empty, session("y", Paused)]).as_deref(), Some("y"));
        assert_eq!(c.update(&[]), None);
    }
}
