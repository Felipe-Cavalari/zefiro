use zefiro_integration_spotify::auth::*;

fn random() -> [u8; 96] {
    let mut r = [0u8; 96];
    for (i, b) in r.iter_mut().enumerate() {
        *b = (i * 37 + 11) as u8;
    }
    r
}

#[test]
fn challenge_matches_rfc7636() {
    assert_eq!(challenge("dBjftJeZ4CVP-mB92K27uhbUJU1p1r_wW1gFWFOEjXk"), "E9Melhoa2OwvFrEMTJguCHaoeK1t8URWbuGJSstw-cM");
}

#[test]
fn pkce_verifier_is_base64url_within_limits() {
    let p = pkce(&random());
    assert!((43..=128).contains(&p.verifier.len()), "{}", p.verifier.len());
    assert!(p.verifier.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_'));
    assert_eq!(challenge(&p.verifier), p.challenge);
    assert!(!p.state.is_empty());
    assert_ne!(p.state, p.verifier);
}

#[test]
fn authorize_url_has_every_parameter() {
    let p = pkce(&random());
    let url = authorize_url("abc", &p, 51234);
    assert!(url.starts_with("https://accounts.spotify.com/authorize?"), "{url}");
    for part in ["response_type=code", "client_id=abc", "code_challenge_method=S256", &format!("code_challenge={}", p.challenge), &format!("state={}", p.state), "redirect_uri=http%3A%2F%2F127.0.0.1%3A51234%2Fcallback", "scope=user-read-playback-state%20user-modify-playback-state"] {
        assert!(url.contains(part), "falta {part} em {url}");
    }
    // No app do Spotify fica o loopback sem porta; a porta livre do momento vai no pedido.
    assert_eq!(REDIRECT_URI, "http://127.0.0.1/callback");
    assert_eq!(redirect_for(51234), "http://127.0.0.1:51234/callback");
}

#[test]
fn callback_reads_code_state_and_denial() {
    assert_eq!(parse_callback("GET /callback?code=xyz&state=s HTTP/1.1", "s"), Callback::Code("xyz".into()));
    assert_eq!(parse_callback("GET /callback?state=s&code=xyz HTTP/1.1", "s"), Callback::Code("xyz".into()));
    assert_eq!(parse_callback("GET /callback?code=xyz&state=outro HTTP/1.1", "s"), Callback::WrongState);
    assert_eq!(parse_callback("GET /callback?error=access_denied&state=s HTTP/1.1", "s"), Callback::Denied("access_denied".into()));
    assert_eq!(parse_callback("GET /favicon.ico HTTP/1.1", "s"), Callback::NotCallback);
    assert_eq!(parse_callback("", "s"), Callback::NotCallback);
}

#[test]
fn bodies_are_form_encoded() {
    let body = code_body("abc", "c d", "v", 51234);
    for part in ["grant_type=authorization_code", "code=c%20d", "code_verifier=v", "client_id=abc", "redirect_uri=http%3A%2F%2F127.0.0.1%3A51234%2Fcallback"] {
        assert!(body.contains(part), "falta {part} em {body}");
    }
    let body = refresh_body("abc", "r1");
    for part in ["grant_type=refresh_token", "refresh_token=r1", "client_id=abc"] {
        assert!(body.contains(part), "falta {part} em {body}");
    }
}

#[test]
fn tokens_keep_the_new_refresh_when_present() {
    let t = parse_tokens(r#"{"access_token":"A1","token_type":"Bearer","expires_in":3600,"refresh_token":"R2","scope":"x"}"#).unwrap();
    assert_eq!(t, Tokens { access: "A1".into(), expires_in: 3600, refresh: Some("R2".into()) });
    let t = parse_tokens(r#"{"access_token":"A1","expires_in":3600}"#).unwrap();
    assert_eq!(t.refresh, None);
    assert!(parse_tokens(r#"{"error":"invalid_grant"}"#).is_none());
    assert!(parse_tokens("lixo").is_none());
}

#[test]
fn stored_round_trips() {
    let s = Stored { client_id: "abc".into(), refresh: "R1".into() };
    assert_eq!(Stored::from_bytes(&s.to_bytes()), Some(s));
    assert_eq!(Stored::from_bytes(b"lixo"), None);
}

#[test]
fn url_encode_handles_accents_and_reserved() {
    assert_eq!(url_encode("ação & café #1"), "a%C3%A7%C3%A3o%20%26%20caf%C3%A9%20%231");
    assert_eq!(url_encode("A-z_0.9~"), "A-z_0.9~");
}

#[test]
fn done_page_is_a_full_http_response() {
    assert!(DONE_PAGE.starts_with("HTTP/1.1 200 OK\r\n"));
    assert!(DONE_PAGE.contains("Pronto, pode voltar ao Zefiro"));
}
