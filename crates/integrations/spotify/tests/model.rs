use zefiro_integration_spotify::api::{Kind, Repeat, Source};
use zefiro_integration_spotify::model::*;

fn fixture(name: &str) -> String {
    std::fs::read_to_string(format!("{}/tests/fixtures/{name}.json", env!("CARGO_MANIFEST_DIR"))).unwrap()
}

#[test]
fn search_groups_in_order_without_empty_groups() {
    let groups = parse_search(&fixture("search_all"), "me");
    let kinds: Vec<Kind> = groups.iter().map(|(k, _)| *k).collect();
    assert_eq!(kinds, vec![Kind::Track, Kind::Artist, Kind::Album, Kind::Playlist, Kind::Episode], "sem Podcasts (vazio)");
    let tracks = &groups[0].1.items;
    assert_eq!(tracks[0].name, "The Less I Know the Better");
    assert_eq!(tracks[0].subtitle, "Tame Impala · Currents");
    assert_eq!(tracks[0].duration_ms, Some(216000));
    assert_eq!(tracks[0].image.as_deref(), Some("https://i.scdn.co/image/cur64"), "a menor com pelo menos 64 px");
    assert_eq!(tracks[0].album.as_ref().unwrap().uri, "spotify:album:cur");
    assert_eq!(tracks[0].artists[0].name, "Tame Impala");
    assert_eq!(groups[3].1.items.len(), 1, "playlist nula na busca é ignorada");
    assert_eq!(groups[3].1.items[0].subtitle, "Playlist · sua");
    assert_eq!(groups[2].1.items[0].subtitle, "Álbum · 2015");
    assert_eq!(groups[1].1.items[0].subtitle, "Artista");
    assert_eq!(groups[4].1.items[0].subtitle, "Café da Manhã · 20/10/2026");
}

#[test]
fn player_playing_paused_and_empty() {
    assert_eq!(parse_player(204, ""), None);
    assert_eq!(parse_player(200, ""), None);
    let p = parse_player(200, &fixture("player_playing")).unwrap();
    assert!(p.playing && p.shuffle);
    assert_eq!(p.repeat, Repeat::Context);
    assert_eq!(p.progress_ms, 90500);
    assert_eq!(p.device.as_ref().unwrap().name, "Notebook");
    assert_eq!(p.device.as_ref().unwrap().volume, Some(60));
    assert_eq!(p.context_uri.as_deref(), Some("spotify:playlist:p1"));
    assert_eq!(p.item.as_ref().unwrap().kind, Kind::Track);
    let e = parse_player(200, &fixture("player_episode")).unwrap();
    assert!(!e.playing);
    let item = e.item.unwrap();
    assert_eq!(item.kind, Kind::Episode);
    assert_eq!(item.show.unwrap().uri, "spotify:show:sh");
    assert_eq!(e.device.unwrap().volume, None);
}

#[test]
fn playlist_items_use_2026_field_names() {
    let page = parse_page(&Source::Playlist("p1".into()), &fixture("playlist_items"), "me").unwrap();
    assert_eq!(page.items.len(), 2, "o item nulo (faixa removida) fica de fora");
    assert_eq!(page.items[0].uri, "spotify:track:t1");
    assert_eq!(page.items[1].kind, Kind::Episode);
    assert_eq!(page.next_offset, None);
}

#[test]
fn playlist_without_items_is_none() {
    assert_eq!(parse_page(&Source::Playlist("p2".into()), &fixture("playlist_metadata_only"), "me"), None);
}

#[test]
fn liked_albums_shows_unwrap_saved_wrappers() {
    let liked = parse_page(&Source::Liked, &fixture("liked"), "me").unwrap();
    assert_eq!(liked.items.len(), 2);
    assert_eq!(liked.items[1].name, "Let It Happen");
    let shows = parse_page(&Source::Shows, &fixture("shows"), "me").unwrap();
    assert_eq!(shows.items[0].kind, Kind::Show);
    assert_eq!(shows.items[0].subtitle, "Podcast · Folha");
    let albums = parse_page(&Source::Artist("ti".into()), &fixture("artist_albums"), "me").unwrap();
    assert_eq!(albums.items.iter().map(|i| i.name.as_str()).collect::<Vec<_>>(), vec!["Currents", "Lonerism"]);
}

#[test]
fn album_tracks_inherit_album_image_and_link() {
    let parent = parse_search(&fixture("search_all"), "me").into_iter().find(|(k, _)| *k == Kind::Album).unwrap().1.items.remove(0);
    let mut page = parse_page(&Source::Album("cur".into()), &fixture("album_tracks"), "me").unwrap();
    assert_eq!(page.items[0].image, None);
    inherit(&mut page, &parent);
    assert_eq!(page.items[0].image.as_deref(), Some("https://i.scdn.co/image/cur64"));
    assert_eq!(page.items[0].album.as_ref().unwrap().name, "Currents");
    assert_eq!(page.items[0].subtitle, "Tame Impala · Currents");
    let show = parse_page(&Source::Shows, &fixture("shows"), "me").unwrap().items.remove(0);
    let mut episodes = parse_page(&Source::Show("sh".into()), &fixture("show_episodes"), "me").unwrap();
    inherit(&mut episodes, &show);
    assert_eq!(episodes.items[0].show.as_ref().unwrap().uri, "spotify:show:sh");
    assert_eq!(episodes.items[0].subtitle, "Café da Manhã · 20/10/2026");
}

#[test]
fn following_reads_cursor_after() {
    let page = parse_page(&Source::Artists, &fixture("following"), "me").unwrap();
    assert_eq!(page.items[0].kind, Kind::Artist);
    assert_eq!(page.after.as_deref(), Some("ti"));
    assert_eq!(page.next_offset, None);
}

#[test]
fn playlist_editable_when_owner_is_me_or_collaborative() {
    let page = parse_page(&Source::Playlists, &fixture("playlists"), "me").unwrap();
    let editable: Vec<bool> = page.items.iter().map(|i| i.editable).collect();
    assert_eq!(editable, vec![true, false, true]);
    assert_eq!(page.items[1].subtitle, "Playlist · Spotify");
    assert_eq!(page.items[0].total, Some(42));
    assert_eq!(page.items[0].owner.as_deref(), Some("me"));
}

#[test]
fn empty_items_give_empty_page() {
    let page = parse_page(&Source::Liked, r#"{"items":[],"next":null}"#, "me").unwrap();
    assert!(page.items.is_empty());
    assert_eq!(page.next_offset, None);
}

#[test]
fn errors_map_reason_and_retry() {
    assert_eq!(api_error(404, &fixture("error_no_device"), None), ApiError::NoActiveDevice);
    assert_eq!(api_error(403, &fixture("error_premium"), None), ApiError::PremiumRequired);
    assert_eq!(api_error(401, "", None), ApiError::Unauthorized);
    assert_eq!(api_error(429, "", Some(7)), ApiError::RateLimited(7));
    assert_eq!(api_error(429, "", None), ApiError::RateLimited(5), "sem Retry-After, 5 s");
    assert_eq!(api_error(500, "", None), ApiError::Http(500));
}

#[test]
fn recent_contexts_are_distinct_and_capped_at_five() {
    let uris = recent_contexts(&fixture("recently_played"));
    assert_eq!(uris, vec!["spotify:playlist:p1", "spotify:album:cur", "spotify:artist:a0", "spotify:artist:a1", "spotify:artist:a2"]);
}

#[test]
fn next_offset_comes_from_next_url() {
    assert_eq!(parse_page(&Source::Liked, &fixture("liked"), "me").unwrap().next_offset, Some(50));
}

#[test]
fn queue_and_devices() {
    let (current, next) = parse_queue(&fixture("queue"));
    assert_eq!(current.unwrap().name, "The Less I Know the Better");
    assert_eq!(next.iter().map(|i| i.name.as_str()).collect::<Vec<_>>(), vec!["Let It Happen", "Episódio 1"]);
    let devices = parse_devices(&fixture("devices"));
    assert_eq!(devices.len(), 2);
    assert!(devices[0].active && !devices[1].active);
    assert_eq!(devices[1].kind, "Smartphone");
    let page = parse_page(&Source::Devices, &fixture("devices"), "me").unwrap();
    assert_eq!(page.items[0].kind, Kind::Device);
    assert_eq!(page.items[0].uri, "d1", "o uri de um aparelho é o id dele");
    assert!(page.items[0].active);
    assert_eq!(page.items[0].subtitle, "Computador · tocando agora");
    assert_eq!(page.items[1].subtitle, "Celular");
    let queue = parse_page(&Source::Queue, &fixture("queue"), "me").unwrap();
    assert_eq!(queue.items.len(), 3, "a atual e as próximas");
}

#[test]
fn me_and_single_objects() {
    assert_eq!(parse_me(&fixture("me")).as_deref(), Some("me"));
    let p = parse_one(Kind::Playlist, &fixture("playlist_metadata_only"), "me").unwrap();
    assert_eq!(p.name, "Daily Mix 1");
    assert!(!p.editable);
    assert_eq!(parse_one(Kind::Album, "lixo", "me"), None);
}
