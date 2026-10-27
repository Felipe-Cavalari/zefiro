//! Os pedidos da Web API (fevereiro de 2026): caminho relativo à `BASE`, com a query, e o corpo em
//! JSON. Quem manda é a sessão (`session.rs`).

use crate::auth::url_encode;
use serde_json::json;

pub const BASE: &str = "https://api.spotify.com/v1";
/// Tamanho de página fora da busca (a API aceita até 50).
const PAGE: u32 = 50;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Method { Get, Put, Post, Delete }

impl Method {
    pub fn as_str(&self) -> &'static str {
        match self {
            Method::Get => "GET",
            Method::Put => "PUT",
            Method::Post => "POST",
            Method::Delete => "DELETE",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Request { pub method: Method, pub path: String, pub body: Option<String> }

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Kind { Track, Episode, Album, Artist, Playlist, Show }

impl Kind {
    pub fn api_name(&self) -> &'static str {
        match self {
            Kind::Track => "track",
            Kind::Episode => "episode",
            Kind::Album => "album",
            Kind::Artist => "artist",
            Kind::Playlist => "playlist",
            Kind::Show => "show",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Repeat { Off, Context, Track }

impl Repeat {
    /// Desligado → contexto → faixa → desligado.
    pub fn next(self) -> Repeat {
        match self {
            Repeat::Off => Repeat::Context,
            Repeat::Context => Repeat::Track,
            Repeat::Track => Repeat::Off,
        }
    }

    pub fn api_name(&self) -> &'static str {
        match self {
            Repeat::Off => "off",
            Repeat::Context => "context",
            Repeat::Track => "track",
        }
    }
}

/// De onde vem uma lista do painel. Os `String` são ids.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Source { Liked, Playlists, Albums, Artists, Shows, Album(String), Artist(String), Show(String), Playlist(String), Queue, Devices }

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PlayTarget { Uris(Vec<String>), Context { uri: String, offset_uri: Option<String> }, Resume }

fn get(path: String) -> Request {
    Request { method: Method::Get, path, body: None }
}

fn with(method: Method, path: impl Into<String>, body: Option<String>) -> Request {
    Request { method, path: path.into(), body }
}

fn uris_param(uris: &[String]) -> String {
    uris.iter().map(|u| url_encode(u)).collect::<Vec<_>>().join(",")
}

pub fn search(q: &str, kinds: &[Kind], limit: u32, offset: u32) -> Request {
    let types = kinds.iter().map(Kind::api_name).collect::<Vec<_>>().join(",");
    get(format!("/search?q={}&type={types}&limit={limit}&offset={offset}", url_encode(q)))
}

/// Uma página de `source`; `after` é o cursor de Artistas (seguidos não têm `offset`).
pub fn list(source: &Source, offset: u32, after: Option<&str>) -> Request {
    let page = format!("limit={PAGE}&offset={offset}");
    get(match source {
        Source::Liked => format!("/me/tracks?{page}"),
        Source::Playlists => format!("/me/playlists?{page}"),
        Source::Albums => format!("/me/albums?{page}"),
        Source::Shows => format!("/me/shows?{page}"),
        Source::Artists => match after {
            Some(cursor) => format!("/me/following?type=artist&limit={PAGE}&after={}", url_encode(cursor)),
            None => format!("/me/following?type=artist&limit={PAGE}"),
        },
        Source::Album(id) => format!("/albums/{id}/tracks?{page}"),
        Source::Artist(id) => format!("/artists/{id}/albums?include_groups=album,single&{page}"),
        Source::Show(id) => format!("/shows/{id}/episodes?{page}"),
        Source::Playlist(id) => format!("/playlists/{id}/items?{page}"),
        Source::Queue => "/me/player/queue".into(),
        Source::Devices => "/me/player/devices".into(),
    })
}

pub fn me() -> Request {
    get("/me".into())
}

pub fn player() -> Request {
    get("/me/player?additional_types=track,episode".into())
}

pub fn recently_played() -> Request {
    get("/me/player/recently-played?limit=50".into())
}

pub fn album(id: &str) -> Request {
    get(format!("/albums/{id}"))
}

pub fn artist(id: &str) -> Request {
    get(format!("/artists/{id}"))
}

pub fn playlist(id: &str) -> Request {
    get(format!("/playlists/{id}"))
}

pub fn play(target: &PlayTarget, device: Option<&str>) -> Request {
    let path = match device {
        Some(d) => format!("/me/player/play?device_id={}", url_encode(d)),
        None => "/me/player/play".into(),
    };
    let body = match target {
        PlayTarget::Uris(uris) => Some(json!({ "uris": uris })),
        PlayTarget::Context { uri, offset_uri: Some(o) } => Some(json!({ "context_uri": uri, "offset": { "uri": o } })),
        PlayTarget::Context { uri, offset_uri: None } => Some(json!({ "context_uri": uri })),
        PlayTarget::Resume => None,
    };
    with(Method::Put, path, body.map(|b| b.to_string()))
}

pub fn pause() -> Request {
    with(Method::Put, "/me/player/pause", None)
}

pub fn next() -> Request {
    with(Method::Post, "/me/player/next", None)
}

pub fn previous() -> Request {
    with(Method::Post, "/me/player/previous", None)
}

pub fn seek(ms: u64) -> Request {
    with(Method::Put, format!("/me/player/seek?position_ms={ms}"), None)
}

pub fn shuffle(on: bool) -> Request {
    with(Method::Put, format!("/me/player/shuffle?state={on}"), None)
}

pub fn repeat(r: Repeat) -> Request {
    with(Method::Put, format!("/me/player/repeat?state={}", r.api_name()), None)
}

pub fn volume(percent: u8, device: Option<&str>) -> Request {
    let path = match device {
        Some(d) => format!("/me/player/volume?volume_percent={}&device_id={}", percent.min(100), url_encode(d)),
        None => format!("/me/player/volume?volume_percent={}", percent.min(100)),
    };
    with(Method::Put, path, None)
}

pub fn queue(uri: &str) -> Request {
    with(Method::Post, format!("/me/player/queue?uri={}", url_encode(uri)), None)
}

/// Passa a tocar no aparelho `device`.
pub fn transfer(device: &str) -> Request {
    with(Method::Put, "/me/player", Some(json!({ "device_ids": [device], "play": true }).to_string()))
}

pub fn library_save(uris: &[String]) -> Request {
    with(Method::Put, format!("/me/library?uris={}", uris_param(uris)), None)
}

pub fn library_remove(uris: &[String]) -> Request {
    with(Method::Delete, format!("/me/library?uris={}", uris_param(uris)), None)
}

pub fn library_contains(uris: &[String]) -> Request {
    get(format!("/me/library/contains?uris={}", uris_param(uris)))
}

pub fn playlist_add(id: &str, uri: &str) -> Request {
    with(Method::Post, format!("/playlists/{id}/items"), Some(json!({ "uris": [uri] }).to_string()))
}

pub fn playlist_remove(id: &str, uri: &str) -> Request {
    with(Method::Delete, format!("/playlists/{id}/items"), Some(json!({ "items": [{ "uri": uri }] }).to_string()))
}

/// `spotify:album:A` → `("album", "A")`.
fn parts(uri: &str) -> Option<(&str, &str)> {
    let mut it = uri.split(':');
    match (it.next(), it.next(), it.next(), it.next()) {
        (Some("spotify"), Some(kind), Some(id), None) if !kind.is_empty() && !id.is_empty() => Some((kind, id)),
        _ => None,
    }
}

pub fn id_of(uri: &str) -> Option<&str> {
    parts(uri).map(|(_, id)| id)
}

pub fn link(uri: &str) -> Option<String> {
    parts(uri).map(|(kind, id)| format!("https://open.spotify.com/{kind}/{id}"))
}

/// O código de incorporação nas medidas padrão do Spotify.
pub fn embed(uri: &str) -> Option<String> {
    parts(uri).map(|(kind, id)| {
        format!(
            r#"<iframe style="border-radius:12px" src="https://open.spotify.com/embed/{kind}/{id}" width="100%" height="352" frameBorder="0" allowfullscreen="" allow="autoplay; clipboard-write; encrypted-media; fullscreen; picture-in-picture" loading="lazy"></iframe>"#
        )
    })
}
