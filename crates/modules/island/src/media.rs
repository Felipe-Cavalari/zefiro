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
    /// Ícone do app (janela, executável ou logo do pacote), para a barra de fontes e o selo da capa.
    pub app_icon: Option<Image>,
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

    /// (escolhida na barra de fontes, última que começou a tocar), para o log.
    pub fn reasons(&self) -> (Option<&str>, Option<&str>) {
        (self.picked.as_deref(), self.last_started.as_deref())
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

/// Identidade de uma faixa (`MediaSession::track_key`).
pub type TrackKey = (String, String, String);

/// Capas decodificadas por faixa, lembrando de que miniatura cada uma veio.
#[derive(Debug, Default)]
pub struct CoverCache {
    covers: std::collections::HashMap<TrackKey, (u64, Image)>,
}

impl CoverCache {
    /// Capa da faixa `key`. `thumbnail` é a impressão digital dos bytes da miniatura publicada
    /// agora (`None` sem miniatura); `decode` só roda quando ela não é a da capa guardada.
    pub fn resolve(&mut self, key: &TrackKey, thumbnail: Option<u64>, decode: impl FnOnce() -> Option<Image>) -> Option<Image> {
        let cached = self.covers.get(key);
        // Sem miniatura agora (o navegador a troca em dois passos): a capa da mesma faixa fica.
        let Some(print) = thumbnail else { return cached.map(|(_, image)| image.clone()) };
        if let Some((p, image)) = cached
            && *p == print
        {
            return Some(image.clone());
        }
        match decode() {
            Some(image) => {
                self.covers.insert(key.clone(), (print, image.clone()));
                Some(image)
            }
            None => cached.map(|(_, image)| image.clone()),
        }
    }

    /// Esquece as faixas que já não estão em nenhuma sessão.
    pub fn retain(&mut self, keep: impl Fn(&TrackKey) -> bool) {
        self.covers.retain(|k, _| keep(k));
    }
}

/// Releituras da capa depois de uma faixa nova, em ms: a miniatura definitiva do navegador chega
/// depois do título, às vezes sem aviso (o fork C++ relê por 3 s pelo mesmo motivo).
pub const COVER_SETTLE_MS: [i64; 4] = [150, 500, 1_200, 3_000];

/// Alguma sessão de `now` traz uma faixa que `last` não tinha.
pub fn new_tracks(last: &[MediaSession], now: &[MediaSession]) -> bool {
    now.iter().filter(|s| s.has_track()).any(|s| !last.iter().any(|l| l.track_key() == s.track_key()))
}

/// Sessões que sumiram do Windows tocando. O YouTube no Brave tira a sessão do SMTC por uns
/// 5 s a cada troca de vídeo (próxima, anterior ou o fim de um vídeo do Mix): sem isto a ilha ficava
/// ociosa, o painel fechava e a capa piscava. A cópia segurada mantém faixa e capa, mas sem
/// controles, porque não há sessão para receber o comando até ela voltar.
#[derive(Debug, Default)]
pub struct VanishHold {
    previous: Vec<MediaSession>,
    held: Vec<(MediaSession, i64)>,
}

impl VanishHold {
    /// Quanto uma sessão sumida fica na ilha.
    pub const HOLD_MS: i64 = 10_000;

    /// `published`: o que o Windows mostra agora. Devolve as sessões com as seguradas no fim.
    pub fn apply(&mut self, published: Vec<MediaSession>, now_ms: i64) -> Vec<MediaSession> {
        let present = |id: &str| published.iter().any(|s| s.id == id);
        self.held.retain(|(s, until)| *until > now_ms && !present(&s.id));
        for s in &self.previous {
            if s.playback == Playback::Playing && s.has_track() && !present(&s.id) {
                let held = MediaSession { can_previous: false, can_next: false, can_play_pause: false, can_seek: false, ..s.clone() };
                self.held.push((held, now_ms + Self::HOLD_MS));
            }
        }
        self.previous = published.clone();
        let mut out = published;
        out.extend(self.held.iter().map(|(s, _)| s.clone()));
        out
    }

    /// Quando a próxima sessão segurada expira (ms desde a época Unix).
    pub fn deadline(&self) -> Option<i64> {
        self.held.iter().map(|(_, until)| *until).min()
    }
}

/// Impressão digital (FNV-1a de 64 bits) dos bytes de uma miniatura.
pub fn fingerprint(bytes: &[u8]) -> u64 {
    bytes.iter().fold(0xcbf2_9ce4_8422_2325, |h, b| (h ^ u64::from(*b)).wrapping_mul(0x0000_0100_0000_01b3))
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
        app_icon: None,
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

    fn pixel(v: u8) -> Option<Image> {
        Image::new(1, 1, vec![v, v, v, 255])
    }

    #[test]
    fn cover_follows_the_thumbnail_bytes_of_the_same_track() {
        // Navegadores (Brave, Chrome) publicam o título novo um instante antes da miniatura nova:
        // a primeira leitura da faixa ainda traz a capa da anterior.
        let mut cache = CoverCache::default();
        let key = session("brave", Playing).track_key();
        let stale = cache.resolve(&key, Some(fingerprint(b"capa antiga")), || pixel(1)).expect("capa");
        let fresh = cache.resolve(&key, Some(fingerprint(b"capa nova")), || pixel(2)).expect("capa");
        assert_ne!(fresh.id(), stale.id(), "a miniatura nova troca a capa da mesma faixa");
        let again = cache.resolve(&key, Some(fingerprint(b"capa nova")), || panic!("mesmos bytes não decodificam de novo"));
        assert_eq!(again.map(|c| c.id()), Some(fresh.id()));
        let gone = cache.resolve(&key, None, || panic!("sem miniatura não há o que decodificar"));
        assert_eq!(gone.map(|c| c.id()), Some(fresh.id()), "a miniatura sumiu por um instante: a capa fica");
        cache.retain(|_| false);
        assert!(cache.resolve(&key, None, || None).is_none());
    }

    #[test]
    fn unreadable_new_thumbnail_keeps_the_cover_of_the_same_track() {
        let mut cache = CoverCache::default();
        let key = session("brave", Playing).track_key();
        let first = cache.resolve(&key, Some(1), || pixel(1)).expect("capa");
        assert_eq!(cache.resolve(&key, Some(2), || None).map(|c| c.id()), Some(first.id()));
        assert_ne!(fingerprint(b"a"), fingerprint(b"b"));
    }

    #[test]
    fn a_session_that_vanishes_while_playing_is_held_without_controls() {
        let mut hold = VanishHold::default();
        let brave = session("brave", Playing);
        assert_eq!(hold.apply(vec![brave.clone(), session("spotify", Paused)], 0).len(), 2);
        // Próxima no YouTube: o Brave some do SMTC enquanto o vídeo novo carrega.
        let gap = hold.apply(vec![session("spotify", Paused)], 1_000);
        let held = gap.iter().find(|s| s.id == "brave").expect("o Brave fica na ilha");
        assert_eq!((held.title.as_str(), held.playback), (brave.title.as_str(), Playing));
        assert!(!held.can_next && !held.can_previous && !held.can_play_pause && !held.can_seek, "sem sessão, sem controles");
        assert_eq!(hold.deadline(), Some(1_000 + VanishHold::HOLD_MS));
        // Ele volta com a faixa nova: a cópia sai.
        let mut next = session("brave", Playing);
        next.title = "outra".into();
        let back = hold.apply(vec![session("spotify", Paused), next.clone()], 5_000);
        assert_eq!(back, vec![session("spotify", Paused), next]);
        assert_eq!(hold.deadline(), None);
    }

    #[test]
    fn held_sessions_expire_and_paused_ones_leave_at_once() {
        let mut hold = VanishHold::default();
        hold.apply(vec![session("brave", Playing), session("spotify", Paused)], 0);
        assert_eq!(hold.apply(vec![], 1_000).iter().map(|s| s.id.as_str()).collect::<Vec<_>>(), ["brave"], "pausado sai na hora");
        assert_eq!(hold.apply(vec![], 1_000 + VanishHold::HOLD_MS - 1).len(), 1);
        assert!(hold.apply(vec![], 1_000 + VanishHold::HOLD_MS).is_empty(), "o app fechou de vez");
        assert!(hold.apply(vec![], 30_000).is_empty(), "uma vez expirada, não volta a ser segurada");
    }

    #[test]
    fn new_tracks_only_when_a_track_key_appears() {
        let a = session("brave", Playing);
        let mut moved = a.clone();
        moved.position_s = 99.0;
        assert!(!new_tracks(std::slice::from_ref(&a), &[moved]), "só a posição mudou");
        let mut b = a.clone();
        b.title = "outra".into();
        assert!(new_tracks(std::slice::from_ref(&a), &[b]));
        assert!(new_tracks(&[], &[a]), "sessão nova");
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
