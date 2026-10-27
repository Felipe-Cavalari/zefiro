use zefiro_bar::spotify::*;
use zefiro_integration_spotify::api::{self, Kind, PlayTarget, Repeat, Source};
use zefiro_integration_spotify::model::{ApiError, Item, Link, Page, Player};

fn item(kind: Kind, id: &str, name: &str) -> Item {
    let prefix = match kind {
        Kind::Device => String::new(),
        k => format!("spotify:{}:", k.api_name()),
    };
    Item { kind, uri: format!("{prefix}{id}"), name: name.into(), subtitle: String::new(), duration_ms: Some(1000), image: None, album: None, artists: Vec::new(), show: None, editable: false, owner: None, total: None, active: false }
}

fn track(id: &str, name: &str) -> Item {
    let mut t = item(Kind::Track, id, name);
    t.album = Some(Link { uri: "spotify:album:al".into(), name: "Álbum".into() });
    t.artists = vec![Link { uri: "spotify:artist:ar".into(), name: "Artista".into() }];
    t.subtitle = "Artista · Álbum".into();
    t
}

fn page(items: Vec<Item>) -> Page {
    Page { items, next_offset: None, after: None }
}

fn ready() -> SpotifyPanel {
    let mut p = SpotifyPanel::new(Auth::Ready);
    p.opened();
    p
}

fn type_text(p: &mut SpotifyPanel, text: &str) -> Vec<Effect> {
    text.chars().flat_map(|c| p.char(c as u32, None)).collect()
}

fn sends(fx: &[Effect]) -> Vec<api::Request> {
    fx.iter().filter_map(|e| if let Effect::Send(r) = e { Some(r.clone()) } else { None }).collect()
}

fn select(p: &mut SpotifyPanel, row: Row) {
    let i = p.rows().iter().position(|r| *r == row).expect("linha não existe");
    p.select(i);
}

/// Raiz → Sua biblioteca → Playlists, com as playlists carregadas.
fn into_playlists(p: &mut SpotifyPanel, playlists: Vec<Item>) {
    select(p, Row::Entry(Entry::Library));
    p.key(Key::Enter);
    select(p, Row::Entry(Entry::Playlists));
    p.key(Key::Enter);
    let depth = p.stack.len() - 1;
    p.loaded(depth, 0, Ok(Some(page(playlists))));
}

fn into_playlist(p: &mut SpotifyPanel, playlist: Item, tracks: Option<Vec<Item>>) {
    into_playlists(p, vec![playlist]);
    select(p, Row::Item(0));
    let fx = p.key(Key::Enter);
    assert!(fx.iter().any(|e| matches!(e, Effect::Load { source: Source::Playlist(_), .. })), "{fx:?}");
    let depth = p.stack.len() - 1;
    p.loaded(depth, 0, Ok(tracks.map(page)));
}

fn mine(id: &str, name: &str) -> Item {
    let mut pl = item(Kind::Playlist, id, name);
    pl.editable = true;
    pl
}

#[test]
fn opens_at_root_with_empty_field_and_asks_player() {
    let mut p = ready();
    into_playlists(&mut p, vec![]);
    type_text(&mut p, "abc");
    p.query = "busca antiga".into();
    assert_eq!(p.opened(), vec![Effect::Refresh]);
    assert_eq!(p.stack.len(), 1);
    assert_eq!(p.stack[0].screen, Screen::Root);
    assert_eq!(p.query, "");
    assert_eq!(p.rows()[0], Row::Card);
}

#[test]
fn typing_at_root_searches_and_old_results_are_dropped() {
    let mut p = ready();
    let fx = type_text(&mut p, "ab");
    let searches: Vec<u64> = fx.iter().filter_map(|e| if let Effect::Search { seq, .. } = e { Some(*seq) } else { None }).collect();
    assert_eq!(searches.len(), 2, "uma busca por tecla (o app junta pelo atraso de 300 ms)");
    assert_eq!(p.stack.last().unwrap().screen, Screen::Search);
    let (old, new) = (searches[0], searches[1]);
    p.searched(new, 0, Ok(vec![(Kind::Track, page(vec![track("t2", "Nova")]))]));
    p.searched(old, 0, Ok(vec![(Kind::Track, page(vec![track("t1", "Velha")]))]));
    assert_eq!(p.stack.last().unwrap().items.as_ref().unwrap()[0].name, "Nova", "resposta velha descartada");
    p.char(8, None);
    p.char(8, None);
    assert_eq!(p.stack.len(), 1, "campo vazio volta à raiz");
}

#[test]
fn typing_inside_a_list_filters_locally_without_accents() {
    let mut p = ready();
    into_playlists(&mut p, vec![mine("p1", "Ação"), mine("p2", "Rock")]);
    let fx = type_text(&mut p, "acao");
    assert!(fx.is_empty(), "{fx:?}");
    assert_eq!(p.rows(), vec![Row::Item(0)]);
    assert_eq!(p.field_text(), "acao");
}

#[test]
fn esc_pops_one_layer_then_closes() {
    let mut p = ready();
    into_playlists(&mut p, vec![mine("p1", "A")]);
    assert_eq!(p.stack.len(), 3);
    p.key(Key::Tab);
    assert!(p.menu.is_some());
    assert!(p.key(Key::Esc).is_empty());
    assert!(p.menu.is_none(), "Esc com o menu aberto fecha só o menu");
    assert_eq!(p.stack.len(), 3);
    p.key(Key::Esc);
    assert_eq!(p.stack.last().unwrap().screen, Screen::Library);
    p.key(Key::Esc);
    assert_eq!(p.stack.len(), 1);
    assert_eq!(p.key(Key::Esc), vec![Effect::Close]);
}

#[test]
fn going_back_keeps_selection_and_scroll() {
    let mut p = ready();
    into_playlists(&mut p, (0..20).map(|i| mine(&format!("p{i}"), &format!("Lista {i}"))).collect());
    p.wheel(-1);
    p.wheel(-1);
    select(&mut p, Row::Item(7));
    p.key(Key::Enter);
    p.key(Key::Esc);
    let top = p.stack.last().unwrap();
    assert_eq!(p.rows()[top.selected], Row::Item(7));
    assert_eq!(top.scroll, 2);
}

#[test]
fn enter_on_track_inside_playlist_plays_in_context() {
    let mut p = ready();
    into_playlist(&mut p, mine("P", "Mix"), Some(vec![track("T", "Faixa")]));
    assert_eq!(p.rows()[0], Row::Play, "1ª linha: Tocar a playlist");
    select(&mut p, Row::Item(0));
    let fx = p.key(Key::Enter);
    assert_eq!(sends(&fx), vec![api::play(&PlayTarget::Context { uri: "spotify:playlist:P".into(), offset_uri: Some("spotify:track:T".into()) }, None)]);
}

#[test]
fn ctrl_enter_queues_and_shift_enter_shuffles() {
    let mut p = ready();
    into_playlist(&mut p, mine("P", "Mix"), Some(vec![track("T", "Faixa")]));
    select(&mut p, Row::Item(0));
    assert_eq!(sends(&p.key(Key::CtrlEnter)), vec![api::queue("spotify:track:T")]);
    select(&mut p, Row::Play);
    assert_eq!(sends(&p.key(Key::ShiftEnter)), vec![api::shuffle(true), api::play(&PlayTarget::Context { uri: "spotify:playlist:P".into(), offset_uri: None }, None)]);
}

#[test]
fn foreign_playlist_shows_only_play_and_note() {
    let mut p = ready();
    into_playlist(&mut p, item(Kind::Playlist, "D", "Daily Mix 1"), None);
    assert_eq!(p.rows(), vec![Row::Play, Row::Note]);
}

#[test]
fn liked_tracks_play_the_loaded_list_from_the_chosen_one() {
    let mut p = ready();
    select(&mut p, Row::Entry(Entry::Library));
    p.key(Key::Enter);
    select(&mut p, Row::Entry(Entry::Liked));
    p.key(Key::Enter);
    p.loaded(2, 0, Ok(Some(page(vec![track("a", "A"), track("b", "B"), track("c", "C")]))));
    select(&mut p, Row::Item(1));
    assert_eq!(sends(&p.key(Key::Enter)), vec![api::play(&PlayTarget::Uris(vec!["spotify:track:b".into(), "spotify:track:c".into()]), None)]);
}

#[test]
fn actions_menu_per_kind() {
    let mut p = ready();
    into_playlist(&mut p, mine("P", "Mix"), Some(vec![track("T", "Faixa")]));
    select(&mut p, Row::Item(0));
    let i = p.stack.last().unwrap().selected;
    let in_mine = p.actions_for(MenuTarget::Row(i));
    assert!(in_mine.contains(&Action::RemoveFromThisPlaylist));
    assert_eq!(in_mine[..2], [Action::Play, Action::Queue]);
    assert!(in_mine.contains(&Action::GoAlbum) && in_mine.contains(&Action::CopyEmbed));

    let mut q = ready();
    type_text(&mut q, "x");
    let seq = q.seq;
    let mut ep = item(Kind::Episode, "E", "Episódio");
    ep.show = Some(Link { uri: "spotify:show:S".into(), name: "Podcast".into() });
    q.searched(seq, 0, Ok(vec![(Kind::Track, page(vec![track("T", "Faixa")])), (Kind::Episode, page(vec![ep]))]));
    let rows = q.rows();
    assert!(!q.actions_for(MenuTarget::Row(0)).contains(&Action::RemoveFromThisPlaylist), "fora de playlist sua não tira");
    let ep_row = rows.iter().position(|r| *r == Row::Item(1)).unwrap();
    assert_eq!(q.actions_for(MenuTarget::Row(ep_row)), vec![Action::Play, Action::Queue, Action::GoShow, Action::CopyLink]);

    let mut d = ready();
    select(&mut d, Row::Entry(Entry::Devices));
    d.key(Key::Enter);
    d.loaded(1, 0, Ok(Some(page(vec![item(Kind::Device, "d1", "Notebook")]))));
    assert_eq!(d.actions_for(MenuTarget::Row(0)), vec![Action::PlayHere, Action::Volume]);

    let mut c = ready();
    c.set_player(Some(Player { item: Some(track("T", "Faixa")), playing: true, progress_ms: 0, device: None, shuffle: false, repeat: Repeat::Off, context_uri: None }), 0);
    let card = c.actions_for(MenuTarget::Card);
    for a in [Action::Shuffle, Action::Repeat, Action::Logout, Action::AddToPlaylist, Action::ToggleLike] {
        assert!(card.contains(&a), "{a:?}");
    }
    assert!(!card.contains(&Action::Back15), "±15 s só em episódio");
    c.set_player(Some(Player { item: Some(item(Kind::Episode, "E", "Ep")), playing: true, progress_ms: 0, device: None, shuffle: false, repeat: Repeat::Off, context_uri: None }), 0);
    assert!(c.actions_for(MenuTarget::Card).contains(&Action::Back15));
}

#[test]
fn no_active_device_opens_devices() {
    let mut p = ready();
    let fx = p.sent(Err(ApiError::NoActiveDevice));
    assert_eq!(p.message.as_deref(), Some("Nenhum aparelho ativo. Abra o Spotify em algum aparelho"));
    assert!(matches!(&p.stack.last().unwrap().screen, Screen::List { source: Source::Devices, .. }));
    assert!(fx.contains(&Effect::Load { depth: 1, source: Source::Devices, offset: 0, after: None }), "{fx:?}");
}

#[test]
fn premium_and_rate_limit_messages() {
    let mut p = ready();
    p.sent(Err(ApiError::RateLimited(7)));
    assert_eq!(p.message.as_deref(), Some("Muitas consultas; tentando de novo em 7 s"));
    p.sent(Err(ApiError::PremiumRequired));
    assert_eq!(p.message.as_deref(), Some("Esta conta não é Premium"));
    p.sent(Err(ApiError::Network));
    assert_eq!(p.message.as_deref(), Some("Sem conexão com o Spotify"));
    p.sent(Err(ApiError::Unauthorized));
    assert_eq!(p.auth, Auth::NeedsLogin);
}

#[test]
fn add_to_playlist_lists_only_editable_and_toasts() {
    let mut p = ready();
    type_text(&mut p, "x");
    let seq = p.seq;
    p.searched(seq, 0, Ok(vec![(Kind::Track, page(vec![track("T", "Faixa")]))]));
    p.select(0);
    assert_eq!(p.key(Key::CtrlP), vec![Effect::LoadPlaylists]);
    p.set_playlists(vec![mine("P", "Mix"), item(Kind::Playlist, "D", "Daily Mix 1")]);
    let Some(Menu { sub: Some(SubMenu::Playlists(Some(list))), .. }) = &p.menu else { panic!("{:?}", p.menu) };
    assert_eq!(list.len(), 1, "só as que dá para editar");
    let fx = p.key(Key::Enter);
    assert_eq!(sends(&fx), vec![api::playlist_add("P", "spotify:track:T")]);
    p.sent(Ok(()));
    assert_eq!(p.toast.as_deref(), Some("Adicionada a Mix"));
}

#[test]
fn remove_from_this_playlist_drops_the_row_and_toasts() {
    let mut p = ready();
    into_playlist(&mut p, mine("P", "Mix"), Some(vec![track("T", "Faixa"), track("U", "Outra")]));
    select(&mut p, Row::Item(0));
    p.key(Key::Tab);
    let at = p.menu.as_ref().unwrap().actions.iter().position(|a| *a == Action::RemoveFromThisPlaylist).unwrap();
    p.menu.as_mut().unwrap().selected = at;
    assert_eq!(sends(&p.key(Key::Enter)), vec![api::playlist_remove("P", "spotify:track:T")]);
    assert_eq!(p.stack.last().unwrap().items.as_ref().unwrap().len(), 1);
    p.sent(Ok(()));
    assert_eq!(p.toast.as_deref(), Some("Tirada de Mix"));
}

#[test]
fn space_with_empty_field_toggles_play() {
    let mut p = ready();
    let player = |playing| Some(Player { item: Some(track("T", "Faixa")), playing, progress_ms: 0, device: None, shuffle: false, repeat: Repeat::Off, context_uri: None });
    p.set_player(player(true), 0);
    assert_eq!(sends(&p.char(32, None)), vec![api::pause()]);
    p.set_player(player(false), 0);
    assert_eq!(sends(&p.char(32, None)), vec![api::play(&PlayTarget::Resume, None)]);
    assert_eq!(p.query, "", "o espaço não entra no campo vazio");
}

#[test]
fn ctrl_tab_cycles_filters_and_researches() {
    let mut p = ready();
    type_text(&mut p, "x");
    let fx = p.key(Key::CtrlTab);
    assert_eq!(p.filter, Filter::Tracks);
    assert!(fx.iter().any(|e| matches!(e, Effect::Search { filter: Filter::Tracks, offset: 0, .. })), "{fx:?}");
    p.key(Key::CtrlShiftTab);
    p.key(Key::CtrlShiftTab);
    assert_eq!(p.filter, Filter::Podcasts);
    assert_eq!(Filter::All.kinds(), vec![Kind::Track, Kind::Artist, Kind::Album, Kind::Playlist, Kind::Show, Kind::Episode]);
    assert_eq!(Filter::Podcasts.kinds(), vec![Kind::Show, Kind::Episode]);
}

#[test]
fn scroll_near_end_loads_next_page_once() {
    let mut p = ready();
    select(&mut p, Row::Entry(Entry::Library));
    p.key(Key::Enter);
    select(&mut p, Row::Entry(Entry::Liked));
    p.key(Key::Enter);
    let tracks: Vec<Item> = (0..12).map(|i| track(&format!("t{i}"), "x")).collect();
    p.loaded(2, 0, Ok(Some(Page { items: tracks, next_offset: Some(50), after: None })));
    let mut loads = 0;
    for _ in 0..4 {
        loads += p.wheel(-1).iter().filter(|e| matches!(e, Effect::Load { offset: 50, .. })).count();
    }
    assert_eq!(loads, 1);
    p.loaded(2, 50, Ok(Some(page(vec![track("t99", "y")]))));
    assert_eq!(p.stack[2].items.as_ref().unwrap().len(), 13, "a página nova vai para o fim");
}

#[test]
fn reopening_keeps_loaded_lists_in_memory() {
    let mut p = ready();
    into_playlists(&mut p, vec![mine("p1", "A")]);
    p.opened();
    select(&mut p, Row::Entry(Entry::Library));
    p.key(Key::Enter);
    select(&mut p, Row::Entry(Entry::Playlists));
    let fx = p.key(Key::Enter);
    assert!(!fx.iter().any(|e| matches!(e, Effect::Load { .. })), "veio da memória: {fx:?}");
    assert_eq!(p.stack.last().unwrap().items.as_ref().unwrap().len(), 1);
    p.idle();
    p.opened();
    select(&mut p, Row::Entry(Entry::Library));
    p.key(Key::Enter);
    select(&mut p, Row::Entry(Entry::Playlists));
    assert!(p.key(Key::Enter).iter().any(|e| matches!(e, Effect::Load { .. })), "depois de 10 min fechado, lê de novo");
}

#[test]
fn album_from_a_track_inherits_its_cover() {
    let mut p = ready();
    type_text(&mut p, "x");
    let seq = p.seq;
    let mut t = track("T", "Faixa");
    t.image = Some("capa".into());
    p.searched(seq, 0, Ok(vec![(Kind::Track, page(vec![t]))]));
    p.select(0);
    p.key(Key::Tab);
    let at = p.menu.as_ref().unwrap().actions.iter().position(|a| *a == Action::GoAlbum).unwrap();
    p.menu.as_mut().unwrap().selected = at;
    let fx = p.key(Key::Enter);
    assert!(fx.contains(&Effect::Load { depth: 2, source: Source::Album("al".into()), offset: 0, after: None }), "{fx:?}");
    let mut bare = track("T", "Faixa");
    bare.album = None;
    bare.image = None;
    p.loaded(2, 0, Ok(Some(page(vec![bare]))));
    assert_eq!(p.stack[2].items.as_ref().unwrap()[0].image.as_deref(), Some("capa"));
}

#[test]
fn copy_actions_and_toast() {
    let mut p = ready();
    type_text(&mut p, "x");
    let seq = p.seq;
    p.searched(seq, 0, Ok(vec![(Kind::Track, page(vec![track("T", "Faixa")]))]));
    p.select(0);
    assert_eq!(p.key(Key::CtrlC), vec![Effect::Copy("https://open.spotify.com/track/T".into())]);
    assert_eq!(p.toast.as_deref(), Some("Copiado"));
}

#[test]
fn setup_screens_take_the_client_id_and_login() {
    let mut p = SpotifyPanel::new(Auth::NeedsClientId);
    p.opened();
    assert!(p.char(22, Some("  abc123 \n")).is_empty());
    assert_eq!(p.client_field, "abc123");
    assert_eq!(p.key(Key::Enter), vec![Effect::SaveClientId("abc123".into())]);
    let mut p = SpotifyPanel::new(Auth::NeedsLogin);
    p.opened();
    assert_eq!(p.key(Key::Enter), vec![Effect::Login]);
}

#[test]
fn ctrl_l_toggles_like_of_the_selected_track() {
    let mut p = ready();
    type_text(&mut p, "x");
    let seq = p.seq;
    p.searched(seq, 0, Ok(vec![(Kind::Track, page(vec![track("T", "Faixa")]))]));
    p.select(0);
    assert_eq!(p.key(Key::CtrlL), vec![Effect::ToggleLike("spotify:track:T".into())]);
}
