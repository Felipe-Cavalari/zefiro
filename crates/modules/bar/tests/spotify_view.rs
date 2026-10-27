use zefiro_bar::spotify::*;
use zefiro_bar::spotify_view::*;
use zefiro_integration_spotify::api::{Kind, Repeat};
use zefiro_integration_spotify::model::{Item, Page, Player};
use zefiro_platform::color::Rgba;
use zefiro_platform::geometry::Rect;
use zefiro_platform::image::Image;
use zefiro_platform::text::FixedWidthMeasurer;
use zefiro_resources::theme::{BUILTIN, parse_theme};
use zefiro_resources::tokens::{Tokens, derive_tokens};
use zefiro_ui::paint::{DisplayList, DrawOp};

fn tokens() -> Tokens {
    derive_tokens(&parse_theme("obsidian", "o", BUILTIN[0].1).unwrap(), Rgba::rgb(0, 120, 215), None)
}

fn track(id: &str, name: &str, ms: u64) -> Item {
    Item { kind: Kind::Track, uri: format!("spotify:track:{id}"), name: name.into(), subtitle: "Artista · Álbum".into(), duration_ms: Some(ms), image: Some(format!("img-{id}")), album: None, artists: Vec::new(), show: None, editable: false, owner: None, total: None, active: false }
}

fn ready() -> SpotifyPanel {
    let mut p = SpotifyPanel::new(Auth::Ready);
    p.opened();
    p
}

fn none(_: &str) -> Option<Image> {
    None
}

fn draw(p: &SpotifyPanel) -> Drawn {
    spotify_panel(p, &none, &FixedWidthMeasurer, 1.0, &tokens())
}

fn texts(l: &DisplayList) -> Vec<String> {
    l.ops.iter().filter_map(|o| if let DrawOp::Text { text, .. } = o { Some(text.clone()) } else { None }).collect()
}

fn has(l: &DisplayList, s: &str) -> bool {
    texts(l).iter().any(|t| t == s)
}

fn playing(p: &mut SpotifyPanel, progress_ms: u64, at: u64) {
    p.set_player(Some(Player { item: Some(track("T", "Faixa", 216_000)), playing: true, progress_ms, device: None, shuffle: false, repeat: Repeat::Off, context_uri: None }), at);
}

#[test]
fn root_shows_card_entries_and_recent() {
    let mut p = ready();
    p.up_next = Some("X".into());
    p.recent = vec![Item { kind: Kind::Playlist, ..track("p", "Foco no código", 0) }];
    let d = draw(&p);
    for s in ["Sua biblioteca", "Curtidas, playlists, álbuns, artistas, podcasts", "Fila", "A seguir: X", "Aparelhos", "Tocadas há pouco", "Foco no código", "Buscar no Spotify…"] {
        assert!(has(&d.list, s), "falta {s}: {:?}", texts(&d.list));
    }
    assert_eq!(d.list.size.w, 400.0);
}

#[test]
fn nothing_playing_says_so() {
    let d = draw(&ready());
    assert!(has(&d.list, "Nada tocando"));
}

#[test]
fn field_hint_changes_with_screen() {
    let mut p = ready();
    assert!(has(&draw(&p).list, "Buscar no Spotify…"));
    p.stack.push(Layer::new(Screen::List { source: zefiro_integration_spotify::api::Source::Playlist("P".into()), title: "Foco no código".into(), context: Some("spotify:playlist:P".into()), playlist_id: Some("P".into()), editable: true, parent: None }, Some(Vec::new())));
    let d = draw(&p);
    assert!(has(&d.list, "Filtrar Foco no código…"), "{:?}", texts(&d.list));
    assert!(has(&d.list, "Tocar a playlist"));
}

#[test]
fn search_groups_have_labels_and_durations() {
    let mut p = ready();
    p.char('x' as u32, None);
    let seq = p.seq;
    p.searched(seq, 0, Ok(vec![(Kind::Track, Page { items: vec![track("a", "Primeira", 216_000)], next_offset: None, after: None }), (Kind::Artist, Page { items: vec![Item { kind: Kind::Artist, ..track("r", "Banda", 0) }], next_offset: None, after: None })]));
    let d = draw(&p);
    for s in ["Músicas", "Artistas", "3:36", "Tudo", "Podcasts"] {
        assert!(has(&d.list, s), "falta {s}: {:?}", texts(&d.list));
    }
    assert!(d.hits.iter().any(|(_, h)| *h == Hit::Filter(Filter::Albums)));
}

#[test]
fn cover_placeholder_until_loaded() {
    let mut p = ready();
    p.char('x' as u32, None);
    let seq = p.seq;
    p.searched(seq, 0, Ok(vec![(Kind::Track, Page { items: vec![track("a", "A", 1000)], next_offset: None, after: None })]));
    let d = draw(&p);
    assert_eq!(d.missing, vec!["img-a".to_string()], "capa que falta é pedida");
    assert!(!d.list.ops.iter().any(|o| matches!(o, DrawOp::Image { .. })));
    let img = Image::new(1, 1, vec![0, 0, 0, 255]).unwrap();
    let with = spotify_panel(&p, &|_| Some(img.clone()), &FixedWidthMeasurer, 1.0, &tokens());
    assert!(with.list.ops.iter().any(|o| matches!(o, DrawOp::Image { .. })));
    assert!(with.missing.is_empty());
}

#[test]
fn menu_overlays_and_hits_its_items() {
    let mut p = ready();
    p.char('x' as u32, None);
    let seq = p.seq;
    p.searched(seq, 0, Ok(vec![(Kind::Track, Page { items: vec![track("a", "A", 1000)], next_offset: None, after: None })]));
    p.select(0);
    p.key(Key::Tab);
    let d = draw(&p);
    for s in ["Tocar", "Pôr na fila", "Ctrl+↵", "Adicionar a playlist…", "Copiar link"] {
        assert!(has(&d.list, s), "falta {s}");
    }
    let items: Vec<&Rect> = d.hits.iter().filter(|(_, h)| matches!(h, Hit::MenuItem(_))).map(|(r, _)| r).collect();
    assert_eq!(items.len(), p.menu.as_ref().unwrap().actions.len());
    assert!(items.iter().all(|r| r.bottom() <= d.list.size.h), "o menu cabe no painel");
    let menu_hits = d.hits.iter().position(|(_, h)| matches!(h, Hit::MenuItem(_))).unwrap();
    let row_hits = d.hits.iter().position(|(_, h)| matches!(h, Hit::Row(_))).unwrap();
    assert!(menu_hits < row_hits, "o menu vem primeiro nas áreas de clique (fica por cima)");
}

#[test]
fn setup_screens() {
    let p = SpotifyPanel::new(Auth::NeedsClientId);
    let d = draw(&p);
    for s in ["http://127.0.0.1:47823/callback", "Copiar", "Salvar"] {
        assert!(has(&d.list, s), "falta {s}: {:?}", texts(&d.list));
    }
    assert!(d.hits.iter().any(|(_, h)| *h == Hit::SaveClientId) && d.hits.iter().any(|(_, h)| *h == Hit::CopyRedirect));
    let d = draw(&SpotifyPanel::new(Auth::NeedsLogin));
    assert!(has(&d.list, "Entrar com o Spotify") && d.hits.iter().any(|(_, h)| *h == Hit::Login));
    assert!(has(&draw(&SpotifyPanel::new(Auth::PortBusy)).list, "A porta 47823 está em uso"));
}

#[test]
fn empty_list_says_no_results() {
    let mut p = ready();
    p.char('x' as u32, None);
    assert!(has(&draw(&p).list, "Carregando…"));
    let seq = p.seq;
    p.searched(seq, 0, Ok(vec![]));
    assert!(has(&draw(&p).list, "Nenhum resultado"));
}

#[test]
fn progress_advances_with_time_while_playing() {
    let mut p = ready();
    playing(&mut p, 60_000, 1_000);
    p.now_ms = 1_000;
    assert!(has(&draw(&p).list, "1:00"));
    p.now_ms = 6_000;
    assert!(has(&draw(&p).list, "1:05"));
}

#[test]
fn footer_shows_shortcuts_or_toast() {
    let mut p = ready();
    assert!(texts(&draw(&p).list).iter().any(|t| t.contains("Tab ações")));
    p.toast = Some("Copiado".into());
    assert!(has(&draw(&p).list, "Copiado"));
    p.message = Some("Sem conexão com o Spotify".into());
    assert!(has(&draw(&p).list, "Sem conexão com o Spotify"));
}

#[test]
fn card_buttons_and_rows_are_clickable() {
    let mut p = ready();
    playing(&mut p, 0, 0);
    let d = draw(&p);
    for b in [CardButton::Previous, CardButton::PlayPause, CardButton::Next, CardButton::Like] {
        assert!(d.hits.iter().any(|(_, h)| *h == Hit::Card(b)), "{b:?}");
    }
    assert!(d.hits.iter().any(|(_, h)| *h == Hit::Row(1)), "entrada Sua biblioteca");
}

#[test]
fn list_shows_at_most_ten_rows_and_scrolls() {
    let mut p = ready();
    p.char('x' as u32, None);
    let seq = p.seq;
    let items: Vec<Item> = (0..25).map(|i| track(&format!("{i}"), &format!("Faixa {i}"), 1000)).collect();
    p.searched(seq, 0, Ok(vec![(Kind::Track, Page { items, next_offset: None, after: None })]));
    let d = draw(&p);
    assert_eq!(d.hits.iter().filter(|(_, h)| matches!(h, Hit::Row(_))).count(), VISIBLE);
    p.wheel(-1);
    let d = draw(&p);
    assert!(!has(&d.list, "Faixa 0") && has(&d.list, "Faixa 1"));
}
