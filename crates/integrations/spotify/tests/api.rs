use zefiro_integration_spotify::api::*;

fn req(r: &Request) -> (&'static str, &str, Option<&str>) {
    (r.method.as_str(), r.path.as_str(), r.body.as_deref())
}

#[test]
fn search_paths() {
    assert_eq!(search("ação & café #1", &[Kind::Track], 10, 20).path, "/search?q=a%C3%A7%C3%A3o%20%26%20caf%C3%A9%20%231&type=track&limit=10&offset=20");
    let all = search("x", &[Kind::Track, Kind::Artist, Kind::Album, Kind::Playlist, Kind::Show, Kind::Episode], 5, 0);
    assert_eq!(all.path, "/search?q=x&type=track,artist,album,playlist,show,episode&limit=5&offset=0");
    assert_eq!(all.method, Method::Get);
}

#[test]
fn list_paths() {
    let cases = [
        (Source::Liked, "/me/tracks?limit=50&offset=0"),
        (Source::Playlists, "/me/playlists?limit=50&offset=0"),
        (Source::Albums, "/me/albums?limit=50&offset=0"),
        (Source::Shows, "/me/shows?limit=50&offset=0"),
        (Source::Artists, "/me/following?type=artist&limit=50"),
        (Source::Album("A".into()), "/albums/A/tracks?limit=50&offset=0"),
        (Source::Artist("R".into()), "/artists/R/albums?include_groups=album,single&limit=50&offset=0"),
        (Source::Show("S".into()), "/shows/S/episodes?limit=50&offset=0"),
        (Source::Playlist("P".into()), "/playlists/P/items?limit=50&offset=0"),
        (Source::Queue, "/me/player/queue"),
        (Source::Devices, "/me/player/devices"),
    ];
    for (source, path) in cases {
        assert_eq!(list(&source, 0, None).path, path, "{source:?}");
    }
    assert_eq!(list(&Source::Liked, 50, None).path, "/me/tracks?limit=50&offset=50");
    assert_eq!(list(&Source::Artists, 0, Some("X")).path, "/me/following?type=artist&limit=50&after=X");
}

#[test]
fn reading_endpoints() {
    assert_eq!(req(&me()), ("GET", "/me", None));
    assert_eq!(req(&player()), ("GET", "/me/player?additional_types=track,episode", None));
    assert_eq!(req(&recently_played()), ("GET", "/me/player/recently-played?limit=50", None));
    assert_eq!(req(&album("A")), ("GET", "/albums/A", None));
    assert_eq!(req(&artist("R")), ("GET", "/artists/R", None));
    assert_eq!(req(&playlist("P")), ("GET", "/playlists/P", None));
}

#[test]
fn player_commands() {
    let target = PlayTarget::Context { uri: "spotify:playlist:P".into(), offset_uri: Some("spotify:track:T".into()) };
    assert_eq!(req(&play(&target, Some("D"))), ("PUT", "/me/player/play?device_id=D", Some(r#"{"context_uri":"spotify:playlist:P","offset":{"uri":"spotify:track:T"}}"#)));
    let target = PlayTarget::Context { uri: "spotify:album:A".into(), offset_uri: None };
    assert_eq!(req(&play(&target, None)), ("PUT", "/me/player/play", Some(r#"{"context_uri":"spotify:album:A"}"#)));
    assert_eq!(req(&play(&PlayTarget::Uris(vec!["spotify:track:T".into()]), None)), ("PUT", "/me/player/play", Some(r#"{"uris":["spotify:track:T"]}"#)));
    assert_eq!(req(&play(&PlayTarget::Resume, None)), ("PUT", "/me/player/play", None));
    assert_eq!(req(&pause()), ("PUT", "/me/player/pause", None));
    assert_eq!(req(&next()), ("POST", "/me/player/next", None));
    assert_eq!(req(&previous()), ("POST", "/me/player/previous", None));
    assert_eq!(req(&seek(15000)), ("PUT", "/me/player/seek?position_ms=15000", None));
    assert_eq!(req(&shuffle(true)), ("PUT", "/me/player/shuffle?state=true", None));
    assert_eq!(req(&repeat(Repeat::Context)), ("PUT", "/me/player/repeat?state=context", None));
    assert_eq!(req(&volume(50, None)), ("PUT", "/me/player/volume?volume_percent=50", None));
    assert_eq!(req(&volume(25, Some("D"))), ("PUT", "/me/player/volume?volume_percent=25&device_id=D", None));
    assert_eq!(req(&queue("spotify:track:T")), ("POST", "/me/player/queue?uri=spotify%3Atrack%3AT", None));
    assert_eq!(req(&transfer("D")), ("PUT", "/me/player", Some(r#"{"device_ids":["D"],"play":true}"#)));
}

#[test]
fn library_and_playlists() {
    let uris = vec!["spotify:track:A".to_string(), "spotify:track:B".to_string()];
    assert_eq!(req(&library_save(&uris)), ("PUT", "/me/library?uris=spotify%3Atrack%3AA,spotify%3Atrack%3AB", None));
    assert_eq!(req(&library_remove(&uris)), ("DELETE", "/me/library?uris=spotify%3Atrack%3AA,spotify%3Atrack%3AB", None));
    assert_eq!(req(&library_contains(&uris)), ("GET", "/me/library/contains?uris=spotify%3Atrack%3AA,spotify%3Atrack%3AB", None));
    assert_eq!(req(&playlist_add("P", "spotify:track:T")), ("POST", "/playlists/P/items", Some(r#"{"uris":["spotify:track:T"]}"#)));
    assert_eq!(req(&playlist_remove("P", "spotify:track:T")), ("DELETE", "/playlists/P/items", Some(r#"{"items":[{"uri":"spotify:track:T"}]}"#)));
}

#[test]
fn repeat_cycles_off_context_track() {
    assert_eq!(Repeat::Off.next(), Repeat::Context);
    assert_eq!(Repeat::Context.next(), Repeat::Track);
    assert_eq!(Repeat::Track.next(), Repeat::Off);
    assert_eq!(Repeat::Track.api_name(), "track");
}

#[test]
fn links_and_embed() {
    assert_eq!(link("spotify:episode:E").as_deref(), Some("https://open.spotify.com/episode/E"));
    assert_eq!(link("spotify:track:T").as_deref(), Some("https://open.spotify.com/track/T"));
    assert_eq!(link("lixo"), None);
    assert_eq!(id_of("spotify:album:A"), Some("A"));
    assert_eq!(id_of("lixo"), None);
    let e = embed("spotify:track:T").unwrap();
    assert!(e.starts_with("<iframe") && e.contains("src=\"https://open.spotify.com/embed/track/T\""), "{e}");
}
