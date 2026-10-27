//! Respostas da Web API → dados do painel. Formato de fevereiro de 2026: os itens de playlist vêm
//! em `items[].item` (antes `tracks.items[].track`) e playlists de outro dono não trazem `items`.

use crate::api::{Kind, Repeat, Source};
use serde_json::Value;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Link { pub uri: String, pub name: String }

/// Uma linha do painel: faixa, episódio, álbum, artista, playlist, podcast ou aparelho. De um
/// aparelho, `uri` é o id dele.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Item {
    pub kind: Kind,
    pub uri: String,
    pub name: String,
    pub subtitle: String,
    pub duration_ms: Option<u64>,
    pub image: Option<String>,
    pub album: Option<Link>,
    pub artists: Vec<Link>,
    pub show: Option<Link>,
    /// Playlist do usuário ou colaborativa.
    pub editable: bool,
    pub owner: Option<String>,
    pub total: Option<u32>,
    /// Aparelho tocando agora.
    pub active: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Page { pub items: Vec<Item>, pub next_offset: Option<u32>, pub after: Option<String> }

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Device { pub id: String, pub name: String, pub kind: String, pub active: bool, pub volume: Option<u8> }

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Player {
    pub item: Option<Item>,
    pub playing: bool,
    pub progress_ms: u64,
    pub device: Option<Device>,
    pub shuffle: bool,
    pub repeat: Repeat,
    pub context_uri: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ApiError { Network, Unauthorized, NoActiveDevice, PremiumRequired, RateLimited(u64), Http(u16) }

pub fn api_error(status: u16, body: &str, retry_after: Option<u64>) -> ApiError {
    let reason = serde_json::from_str::<Value>(body).ok().and_then(|v| v["error"]["reason"].as_str().map(str::to_string));
    match (status, reason.as_deref()) {
        (_, Some("NO_ACTIVE_DEVICE")) => ApiError::NoActiveDevice,
        (_, Some("PREMIUM_REQUIRED")) => ApiError::PremiumRequired,
        (401, _) => ApiError::Unauthorized,
        (429, _) => ApiError::RateLimited(retry_after.unwrap_or(5)),
        (other, _) => ApiError::Http(other),
    }
}

fn s(v: &Value) -> String {
    v.as_str().unwrap_or_default().to_string()
}

fn link(v: &Value) -> Option<Link> {
    Some(Link { uri: v["uri"].as_str()?.to_string(), name: s(&v["name"]) })
}

/// A menor imagem com pelo menos 64 px; sem medidas (playlists), a primeira.
fn image(images: &Value) -> Option<String> {
    let list = images.as_array()?;
    let sized = list.iter().filter(|i| i["width"].as_u64().is_some_and(|w| w >= 64)).min_by_key(|i| i["width"].as_u64());
    sized.or_else(|| list.first()).and_then(|i| i["url"].as_str()).map(str::to_string)
}

/// `2026-10-20` → `20/10/2026`; só o ano ou o mês passam como estão.
fn date_br(date: &str) -> String {
    match date.split('-').collect::<Vec<_>>()[..] {
        [y, m, d] => format!("{d}/{m}/{y}"),
        _ => date.to_string(),
    }
}

fn joined(parts: &[&str]) -> String {
    parts.iter().filter(|p| !p.is_empty()).copied().collect::<Vec<_>>().join(" · ")
}

fn base(kind: Kind, v: &Value) -> Item {
    Item { kind, uri: s(&v["uri"]), name: s(&v["name"]), subtitle: String::new(), duration_ms: v["duration_ms"].as_u64(), image: image(&v["images"]), album: None, artists: Vec::new(), show: None, editable: false, owner: None, total: None, active: false }
}

fn track_subtitle(item: &Item) -> String {
    let artists = item.artists.iter().map(|a| a.name.as_str()).collect::<Vec<_>>().join(", ");
    joined(&[&artists, item.album.as_ref().map_or("", |a| a.name.as_str())])
}

fn episode_subtitle(item: &Item, release: &str) -> String {
    joined(&[item.show.as_ref().map_or("", |s| s.name.as_str()), &date_br(release)])
}

/// Um objeto da API como linha (`None` para nulo ou tipo desconhecido).
fn item(v: &Value, me: &str) -> Option<Item> {
    v.as_object()?;
    let kind = match v["type"].as_str()? {
        "track" => Kind::Track,
        "episode" => Kind::Episode,
        "album" => Kind::Album,
        "artist" => Kind::Artist,
        "playlist" => Kind::Playlist,
        "show" => Kind::Show,
        _ => return None,
    };
    let mut it = base(kind, v);
    match kind {
        Kind::Track => {
            it.album = link(&v["album"]);
            it.artists = v["artists"].as_array().map(|a| a.iter().filter_map(link).collect()).unwrap_or_default();
            it.image = image(&v["album"]["images"]);
            it.subtitle = track_subtitle(&it);
        }
        Kind::Episode => {
            it.show = link(&v["show"]);
            it.image = it.image.or_else(|| image(&v["show"]["images"]));
            it.subtitle = episode_subtitle(&it, v["release_date"].as_str().unwrap_or_default());
        }
        Kind::Album => {
            it.artists = v["artists"].as_array().map(|a| a.iter().filter_map(link).collect()).unwrap_or_default();
            it.total = v["total_tracks"].as_u64().map(|t| t as u32);
            it.subtitle = joined(&["Álbum", v["release_date"].as_str().unwrap_or_default().get(..4).unwrap_or_default()]);
        }
        Kind::Artist => it.subtitle = "Artista".into(),
        Kind::Playlist => {
            let owner = s(&v["owner"]["id"]);
            it.editable = owner == me || v["collaborative"].as_bool().unwrap_or(false);
            let total = if v["items"]["total"].is_u64() { &v["items"]["total"] } else { &v["tracks"]["total"] };
            it.total = total.as_u64().map(|t| t as u32);
            let who = if owner == me { "sua".to_string() } else { v["owner"]["display_name"].as_str().map_or(owner.clone(), str::to_string) };
            it.subtitle = joined(&["Playlist", &who]);
            it.owner = Some(owner);
        }
        Kind::Show => it.subtitle = joined(&["Podcast", v["publisher"].as_str().unwrap_or_default()]),
        Kind::Device => {}
    }
    Some(it)
}

fn device_kind_br(kind: &str) -> &str {
    match kind {
        "Computer" => "Computador",
        "Smartphone" => "Celular",
        "Speaker" => "Alto-falante",
        "TV" => "TV",
        "Tablet" => "Tablet",
        "CastAudio" | "CastVideo" => "Chromecast",
        "Automobile" => "Carro",
        "GameConsole" => "Videogame",
        other => other,
    }
}

fn device(v: &Value) -> Option<Device> {
    Some(Device { id: v["id"].as_str()?.to_string(), name: s(&v["name"]), kind: s(&v["type"]), active: v["is_active"].as_bool().unwrap_or(false), volume: v["volume_percent"].as_u64().map(|p| p.min(100) as u8) })
}

/// `offset=50` do campo `next`.
fn next_offset(v: &Value) -> Option<u32> {
    let next = v["next"].as_str()?;
    next.split(['?', '&']).find_map(|kv| kv.strip_prefix("offset=")).and_then(|o| o.parse().ok())
}

fn page_of(paging: &Value, unwrap: Option<&str>, me: &str) -> Page {
    let items = paging["items"].as_array().map(|list| list.iter().filter_map(|e| item(unwrap.map_or(e, |key| &e[key]), me)).collect()).unwrap_or_default();
    Page { items, next_offset: next_offset(paging), after: None }
}

pub fn parse_me(body: &str) -> Option<String> {
    serde_json::from_str::<Value>(body).ok()?["id"].as_str().map(str::to_string)
}

/// `None` quando nada toca (204 ou corpo vazio).
pub fn parse_player(status: u16, body: &str) -> Option<Player> {
    if status == 204 {
        return None;
    }
    let v: Value = serde_json::from_str(body).ok()?;
    let repeat = match v["repeat_state"].as_str() {
        Some("context") => Repeat::Context,
        Some("track") => Repeat::Track,
        _ => Repeat::Off,
    };
    Some(Player { item: item(&v["item"], ""), playing: v["is_playing"].as_bool().unwrap_or(false), progress_ms: v["progress_ms"].as_u64().unwrap_or(0), device: device(&v["device"]), shuffle: v["shuffle_state"].as_bool().unwrap_or(false), repeat, context_uri: v["context"]["uri"].as_str().map(str::to_string) })
}

/// Grupos da busca na ordem do painel, só os que têm resultado. `me` é o id do usuário (para
/// "Playlist · sua").
pub fn parse_search(body: &str, me: &str) -> Vec<(Kind, Page)> {
    let Ok(v) = serde_json::from_str::<Value>(body) else { return Vec::new() };
    [(Kind::Track, "tracks"), (Kind::Artist, "artists"), (Kind::Album, "albums"), (Kind::Playlist, "playlists"), (Kind::Show, "shows"), (Kind::Episode, "episodes")]
        .into_iter()
        .map(|(kind, key)| (kind, page_of(&v[key], None, me)))
        .filter(|(_, page)| !page.items.is_empty())
        .collect()
}

/// Uma página de `source`. `None` só numa playlist que a API não abre (de outro dono).
pub fn parse_page(source: &Source, body: &str, me: &str) -> Option<Page> {
    let v: Value = serde_json::from_str(body).ok()?;
    Some(match source {
        Source::Liked | Source::Albums => page_of(&v, Some(if *source == Source::Liked { "track" } else { "album" }), me),
        Source::Shows => page_of(&v, Some("show"), me),
        Source::Playlist(_) => {
            v["items"].as_array()?;
            let items = v["items"].as_array().map(|list| list.iter().filter_map(|e| item(if e["item"].is_object() { &e["item"] } else { &e["track"] }, me)).collect()).unwrap_or_default();
            Page { items, next_offset: next_offset(&v), after: None }
        }
        Source::Artists => {
            let mut page = page_of(&v["artists"], None, me);
            page.after = v["artists"]["next"].as_str().and(v["artists"]["cursors"]["after"].as_str()).map(str::to_string);
            page
        }
        Source::Album(_) | Source::Show(_) => {
            let fallback = if matches!(source, Source::Album(_)) { "track" } else { "episode" };
            let items = v["items"].as_array().map(|list| list.iter().filter_map(|e| if e["type"].is_string() { item(e, me) } else { item(&with_type(e, fallback), me) }).collect()).unwrap_or_default();
            Page { items, next_offset: next_offset(&v), after: None }
        }
        Source::Playlists | Source::Artist(_) => page_of(&v, None, me),
        Source::Queue => {
            let (current, next) = parse_queue(body);
            Page { items: current.into_iter().chain(next).collect(), next_offset: None, after: None }
        }
        Source::Devices => {
            let items = parse_devices(body)
                .into_iter()
                .map(|d| {
                    let kind = device_kind_br(&d.kind).to_string();
                    let subtitle = if d.active { joined(&[&kind, "tocando agora"]) } else { kind };
                    Item { kind: Kind::Device, uri: d.id, name: d.name, subtitle, duration_ms: None, image: None, album: None, artists: Vec::new(), show: None, editable: false, owner: None, total: None, active: d.active }
                })
                .collect();
            Page { items, next_offset: None, after: None }
        }
    })
}

fn with_type(v: &Value, kind: &str) -> Value {
    let mut v = v.clone();
    if let Some(o) = v.as_object_mut() {
        o.insert("type".into(), Value::String(kind.into()));
    }
    v
}

/// Faixas de um álbum e episódios de um podcast não trazem o álbum nem o podcast: vêm de quem abriu
/// a lista.
pub fn inherit(page: &mut Page, parent: &Item) {
    for it in &mut page.items {
        match (it.kind, parent.kind) {
            (Kind::Track, Kind::Album) if it.album.is_none() => {
                it.album = Some(Link { uri: parent.uri.clone(), name: parent.name.clone() });
                it.image = it.image.take().or_else(|| parent.image.clone());
                it.subtitle = track_subtitle(it);
            }
            (Kind::Episode, Kind::Show) if it.show.is_none() => {
                it.show = Some(Link { uri: parent.uri.clone(), name: parent.name.clone() });
                it.image = it.image.take().or_else(|| parent.image.clone());
                it.subtitle = joined(&[&parent.name, &it.subtitle]);
            }
            _ => {}
        }
    }
}

pub fn parse_queue(body: &str) -> (Option<Item>, Vec<Item>) {
    let Ok(v) = serde_json::from_str::<Value>(body) else { return (None, Vec::new()) };
    (item(&v["currently_playing"], ""), v["queue"].as_array().map(|q| q.iter().filter_map(|e| item(e, "")).collect()).unwrap_or_default())
}

pub fn parse_devices(body: &str) -> Vec<Device> {
    serde_json::from_str::<Value>(body).ok().and_then(|v| v["devices"].as_array().map(|d| d.iter().filter_map(device).collect())).unwrap_or_default()
}

/// Contextos (playlist, álbum, artista) distintos das tocadas há pouco, até 5, na ordem.
pub fn recent_contexts(body: &str) -> Vec<String> {
    let Ok(v) = serde_json::from_str::<Value>(body) else { return Vec::new() };
    let mut out: Vec<String> = Vec::new();
    for uri in v["items"].as_array().into_iter().flatten().filter_map(|e| e["context"]["uri"].as_str()) {
        if !out.iter().any(|u| u == uri) {
            out.push(uri.to_string());
            if out.len() == 5 {
                break;
            }
        }
    }
    out
}

/// Um objeto avulso (`GET /albums/{id}`…), para nome e capa de "Tocadas há pouco".
pub fn parse_one(kind: Kind, body: &str, me: &str) -> Option<Item> {
    let v: Value = serde_json::from_str(body).ok()?;
    item(&if v["type"].is_string() { v } else { with_type(&v, kind.api_name()) }, me)
}
