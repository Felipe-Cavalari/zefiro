//! Sonda do Spotify: entra pelo serviço de verdade (`--login`), mostra o estado do player, os
//! aparelhos e 3 resultados de uma busca, e com `--dump <pasta>` grava as respostas cruas da API
//! para virarem fixtures (limpe e-mail, nomes e ids antes de copiar). Nunca imprime o token.
//!
//! `cargo run -p zefiro-platform-windows --example spotify_probe -- [--login] [--dump <pasta>] [busca]`
//! O `client_id` vem de `--client-id <id>` ou do `[spotify]` do `config.toml` (`ZEFIRO_CONFIG_DIR`
//! vale).

use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant};
use windows::Win32::System::WinRT::{RO_INIT_MULTITHREADED, RoInitialize};
use zefiro_integration_spotify::api::{self, Kind, Source};
use zefiro_integration_spotify::auth::Stored;
use zefiro_integration_spotify::model;
use zefiro_integration_spotify::session::Session;
use zefiro_platform_windows::secret;
use zefiro_platform_windows::spotify::{AuthState, Job, Reply, SpotifyService};
use zefiro_platform_windows::spotify_auth::HttpTransport;

fn data_dir() -> PathBuf {
    std::env::var_os("LOCALAPPDATA").map(PathBuf::from).unwrap_or_default().join("zefiro").join("spotify")
}

/// `client_id = "…"` do `[spotify]` (sem puxar o parser de TOML para a sonda).
fn client_id_from_config() -> Option<String> {
    let dir = std::env::var_os("ZEFIRO_CONFIG_DIR").map(PathBuf::from).or_else(|| std::env::var_os("APPDATA").map(|a| PathBuf::from(a).join("zefiro")))?;
    let text = std::fs::read_to_string(dir.join("config.toml")).ok()?;
    let mut in_section = false;
    for line in text.lines().map(str::trim) {
        if line.starts_with('[') {
            in_section = line == "[spotify]";
        } else if in_section && let Some(value) = line.strip_prefix("client_id") {
            return value.trim_start().strip_prefix('=')?.split('#').next()?.trim().trim_matches('"').to_string().into();
        }
    }
    None
}

fn wait(service: &SpotifyService, limit: Duration, mut done: impl FnMut(&Reply) -> bool) -> bool {
    let started = Instant::now();
    while started.elapsed() < limit {
        for r in service.replies() {
            if done(&r) {
                return true;
            }
        }
        std::thread::sleep(Duration::from_millis(100));
    }
    false
}

/// O primeiro estado de login que não seja "esperando o navegador".
fn wait_auth(service: &SpotifyService, limit: Duration) -> Option<AuthState> {
    let mut state = None;
    wait(service, limit, |r| match r {
        Reply::Auth(s) if *s != AuthState::Waiting => {
            state = Some(*s);
            true
        }
        _ => false,
    });
    state
}

fn dump(client_id: &str, out: &PathBuf) -> Result<(), String> {
    let sealed = std::fs::read(data_dir().join("token.bin")).map_err(|e| format!("sem token.bin: {e}"))?;
    let stored = secret::unprotect(&sealed).ok().and_then(|b| Stored::from_bytes(&b)).ok_or("token.bin ilegível")?;
    if stored.client_id != client_id {
        return Err("o token é de outro client_id".into());
    }
    let mut s = Session::new(stored.client_id.clone(), stored.refresh);
    let now = || std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_secs());
    let mut get = |name: &str, req: api::Request| -> Option<String> {
        match s.execute(&HttpTransport, &req, now()) {
            Ok((status, body)) => {
                println!("  {name}: {status}, {} bytes", body.len());
                let _ = std::fs::write(out.join(format!("{name}.json")), &body);
                Some(body)
            }
            Err(e) => {
                println!("  {name}: {e:?}");
                None
            }
        }
    };
    std::fs::create_dir_all(out).map_err(|e| e.to_string())?;
    let me = get("me", api::me()).and_then(|b| model::parse_me(&b)).unwrap_or_default();
    get("search_all", api::search("tame impala", &[Kind::Track, Kind::Artist, Kind::Album, Kind::Playlist, Kind::Show, Kind::Episode], 5, 0));
    get("player_playing", api::player());
    get("queue", api::list(&Source::Queue, 0, None));
    get("devices", api::list(&Source::Devices, 0, None));
    get("liked", api::list(&Source::Liked, 0, None));
    get("following", api::list(&Source::Artists, 0, None));
    get("shows", api::list(&Source::Shows, 0, None));
    get("recently_played", api::recently_played());
    let playlists = get("playlists", api::list(&Source::Playlists, 0, None)).and_then(|b| model::parse_page(&Source::Playlists, &b, &me)).map(|p| p.items).unwrap_or_default();
    if let Some(id) = playlists.iter().find(|p| p.editable).and_then(|p| api::id_of(&p.uri)) {
        get("playlist_items", api::list(&Source::Playlist(id.into()), 0, None));
    }
    if let Some(id) = playlists.iter().find(|p| !p.editable).and_then(|p| api::id_of(&p.uri)) {
        get("playlist_metadata_only", api::list(&Source::Playlist(id.into()), 0, None));
    }
    // O Spotify pode ter trocado o token de renovação: grava o novo, senão o login cai.
    if let Some(refresh) = s.take_new_refresh() {
        let fresh = Stored { client_id: stored.client_id, refresh };
        match secret::protect(&fresh.to_bytes()) {
            Ok(sealed) => {
                let _ = std::fs::write(data_dir().join("token.bin"), sealed);
            }
            Err(e) => eprintln!("não consegui gravar o token novo: {}", e.message()),
        }
    }
    println!("respostas cruas em {}", out.display());
    Ok(())
}

fn main() {
    // SAFETY: WinRT na thread principal da sonda (HttpClient); o processo acaba logo.
    let _ = unsafe { RoInitialize(RO_INIT_MULTITHREADED) };
    let mut args = std::env::args().skip(1);
    let (mut login, mut dump_dir, mut client_id, mut query) = (false, None, None, None);
    while let Some(a) = args.next() {
        match a.as_str() {
            "--login" => login = true,
            "--dump" => dump_dir = args.next().map(PathBuf::from),
            "--client-id" => client_id = args.next(),
            other => query = Some(other.to_string()),
        }
    }
    let Some(client_id) = client_id.or_else(client_id_from_config).filter(|c| !c.is_empty()) else {
        eprintln!("sem client_id: use --client-id <id> ou grave [spotify] client_id no config.toml");
        std::process::exit(1);
    };
    let service = SpotifyService::start(data_dir(), client_id.clone(), Arc::new(|| {})).expect("serviço");
    // O serviço diz o estado ao subir (com ou sem token gravado).
    let mut state = wait_auth(&service, Duration::from_secs(5)).unwrap_or(AuthState::NeedsLogin);
    if login && state != AuthState::Ready {
        service.send(Job::Login);
        println!("abrindo o login no navegador (5 min)…");
        let got = wait_auth(&service, Duration::from_secs(300));
        let ok = got.is_some();
        state = got.unwrap_or(state);
        if !ok || state != AuthState::Ready {
            eprintln!("login não concluído ({state:?})");
            std::process::exit(1);
        }
    }
    println!("estado: {state:?}");
    if state != AuthState::Ready {
        std::process::exit(1);
    }
    service.send(Job::Player);
    wait(&service, Duration::from_secs(15), |r| match r {
        Reply::Player(Ok(Some(p))) => {
            let item = p.item.as_ref().map_or("—".to_string(), |i| format!("{} ({})", i.name, i.subtitle));
            println!("player: {} {item}, aparelho {:?}, aleatório {}, repetir {:?}", if p.playing { "tocando" } else { "pausado" }, p.device.as_ref().map(|d| &d.name), p.shuffle, p.repeat);
            true
        }
        Reply::Player(Ok(None)) => {
            println!("player: nada tocando");
            true
        }
        Reply::Player(Err(e)) => {
            println!("player: {e:?}");
            true
        }
        _ => false,
    });
    service.send(Job::Load { depth: 0, source: Source::Devices, offset: 0, after: None });
    wait(&service, Duration::from_secs(15), |r| match r {
        Reply::Loaded { result, .. } => {
            let names: Vec<String> = result.as_ref().ok().and_then(|p| p.as_ref()).map(|p| p.items.iter().map(|d| format!("{} [{}]", d.name, d.subtitle)).collect()).unwrap_or_default();
            println!("aparelhos: {names:?}");
            true
        }
        _ => false,
    });
    if let Some(q) = query {
        service.send(Job::Search { seq: 1, query: q, kinds: vec![Kind::Track], limit: 3, offset: 0 });
        wait(&service, Duration::from_secs(15), |r| match r {
            Reply::Searched { result, .. } => {
                for (_, page) in result.iter().flatten() {
                    for item in &page.items {
                        println!("  {} — {}", item.name, item.subtitle);
                    }
                }
                true
            }
            _ => false,
        });
    }
    if let Some(out) = dump_dir {
        drop(service);
        if let Err(e) = dump(&client_id, &out) {
            eprintln!("dump: {e}");
            std::process::exit(1);
        }
    }
}
