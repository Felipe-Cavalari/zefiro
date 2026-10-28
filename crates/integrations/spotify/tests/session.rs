use std::cell::RefCell;
use std::collections::VecDeque;
use zefiro_integration_spotify::api;
use zefiro_integration_spotify::cache::CoverCache;
use zefiro_integration_spotify::model::ApiError;
use zefiro_integration_spotify::session::*;


/// Método, URL, cabeçalhos e corpo de um pedido feito.
type Sent = (String, String, Vec<(String, String)>, Option<String>);

#[derive(Default)]
struct Fake { replies: RefCell<VecDeque<Reply>>, sent: RefCell<Vec<Sent>> }

impl Fake {
    fn new(replies: Vec<Reply>) -> Fake {
        Fake { replies: RefCell::new(replies.into()), ..Default::default() }
    }
    fn urls(&self) -> Vec<String> {
        self.sent.borrow().iter().map(|(_, u, _, _)| u.clone()).collect()
    }
    fn header(&self, i: usize, name: &str) -> Option<String> {
        self.sent.borrow()[i].2.iter().find(|(n, _)| n == name).map(|(_, v)| v.clone())
    }
}

impl Transport for Fake {
    fn send(&self, method: &str, url: &str, headers: &[(&str, &str)], body: Option<(&str, &str)>) -> Reply {
        self.sent.borrow_mut().push((method.into(), url.into(), headers.iter().map(|(a, b)| (a.to_string(), b.to_string())).collect(), body.map(|(_, b)| b.to_string())));
        self.replies.borrow_mut().pop_front().unwrap_or(Err(NetworkError))
    }
}

fn token(access: &str, refresh: Option<&str>) -> Reply {
    let refresh = refresh.map_or(String::new(), |r| format!(r#","refresh_token":"{r}""#));
    Ok((200, format!(r#"{{"access_token":"{access}","expires_in":3600{refresh}}}"#), None))
}

fn ok(body: &str) -> Reply {
    Ok((200, body.into(), None))
}

#[test]
fn first_call_refreshes_then_sends_bearer() {
    let t = Fake::new(vec![token("A1", None), ok("{}")]);
    let mut s = Session::new("abc".into(), "R1".into());
    assert_eq!(s.execute(&t, &api::me(), 1000), Ok((200, "{}".into())));
    let urls = t.urls();
    assert_eq!(urls[0], "https://accounts.spotify.com/api/token");
    assert!(t.sent.borrow()[0].3.as_deref().unwrap().contains("refresh_token=R1"));
    assert_eq!(urls[1], "https://api.spotify.com/v1/me");
    assert_eq!(t.header(1, "Authorization").as_deref(), Some("Bearer A1"));
}

#[test]
fn expired_access_token_refreshes_once_and_retries() {
    let t = Fake::new(vec![token("A1", None), ok("{}"), token("A2", None), ok("{}")]);
    let mut s = Session::new("abc".into(), "R1".into());
    s.execute(&t, &api::me(), 1000).unwrap();
    s.execute(&t, &api::me(), 1000 + 3600 - 30).unwrap();
    assert_eq!(t.urls().len(), 4, "renovou antes de vencer (menos de 60 s)");
    assert_eq!(t.header(3, "Authorization").as_deref(), Some("Bearer A2"));
    s.execute(&t, &api::me(), 1000 + 3600 - 30 + 10).ok();
    assert_eq!(t.header(4, "Authorization").as_deref(), Some("Bearer A2"), "não renova de novo com o token novo");
}

#[test]
fn unauthorized_retries_once_then_gives_up() {
    let t = Fake::new(vec![token("A1", None), Ok((401, String::new(), None)), token("A2", None), Ok((401, String::new(), None))]);
    let mut s = Session::new("abc".into(), "R1".into());
    assert_eq!(s.execute(&t, &api::me(), 1000), Err(ApiError::Unauthorized));
    assert_eq!(t.urls().len(), 4, "renova uma vez só, sem laço");
}

#[test]
fn invalid_grant_is_unauthorized() {
    let t = Fake::new(vec![Ok((400, r#"{"error":"invalid_grant"}"#.into(), None))]);
    let mut s = Session::new("abc".into(), "R1".into());
    assert_eq!(s.execute(&t, &api::me(), 1000), Err(ApiError::Unauthorized));
}

#[test]
fn new_refresh_token_is_reported_once() {
    let t = Fake::new(vec![token("A1", Some("R2")), ok("{}"), Ok((401, String::new(), None)), token("A2", None), ok("{}")]);
    let mut s = Session::new("abc".into(), "R1".into());
    s.execute(&t, &api::me(), 1000).unwrap();
    assert_eq!(s.take_new_refresh().as_deref(), Some("R2"));
    assert_eq!(s.take_new_refresh(), None);
    assert_eq!(s.stored().refresh, "R2");
    s.execute(&t, &api::me(), 1001).unwrap();
    assert!(t.sent.borrow()[3].3.as_deref().unwrap().contains("refresh_token=R2"), "a renovação seguinte usa o novo");
}

#[test]
fn rate_limit_passes_retry_after() {
    let t = Fake::new(vec![token("A1", None), Ok((429, String::new(), Some(7)))]);
    let mut s = Session::new("abc".into(), "R1".into());
    assert_eq!(s.execute(&t, &api::me(), 1000), Err(ApiError::RateLimited(7)));
}

#[test]
fn network_failure_is_network() {
    let t = Fake::new(vec![token("A1", None), Err(NetworkError)]);
    let mut s = Session::new("abc".into(), "R1".into());
    assert_eq!(s.execute(&t, &api::me(), 1000), Err(ApiError::Network));
}

#[test]
fn bodies_go_as_json_and_status_204_is_ok() {
    let t = Fake::new(vec![token("A1", None), Ok((204, String::new(), None))]);
    let mut s = Session::new("abc".into(), "R1".into());
    assert_eq!(s.execute(&t, &api::playlist_add("P", "spotify:track:T"), 1000), Ok((204, String::new())));
    assert_eq!(t.sent.borrow()[1].0, "POST");
    assert_eq!(t.sent.borrow()[1].3.as_deref(), Some(r#"{"uris":["spotify:track:T"]}"#));
}

#[test]
fn exchange_code_reads_tokens() {
    let t = Fake::new(vec![token("A1", Some("R1"))]);
    let tokens = exchange_code(&t, "abc", "c", "v", 51234).unwrap();
    assert_eq!(tokens.refresh.as_deref(), Some("R1"));
    let body = t.sent.borrow()[0].3.clone().unwrap();
    assert!(body.contains("grant_type=authorization_code"));
    assert!(body.contains("redirect_uri=http%3A%2F%2F127.0.0.1%3A51234%2Fcallback"), "a mesma porta do pedido de login: {body}");
    let t = Fake::new(vec![Ok((400, r#"{"error":"invalid_grant"}"#.into(), None))]);
    assert_eq!(exchange_code(&t, "abc", "c", "v", 51234), Err(ApiError::Unauthorized));
}

#[test]
fn cover_cache_evicts_least_recent() {
    let mut c = CoverCache::new(2);
    c.insert("a".into(), 1);
    c.insert("b".into(), 2);
    assert_eq!(c.get("a"), Some(&1));
    c.insert("c".into(), 3);
    assert_eq!(c.get("b"), None);
    assert_eq!(c.get("a"), Some(&1));
    assert_eq!(c.len(), 2);
    c.clear();
    assert_eq!(c.len(), 0);
}

#[test]
fn rate_limit_holds_further_calls_without_touching_the_network() {
    let t = Fake::new(vec![token("A1", None), Ok((429, String::new(), Some(7))), ok("{}")]);
    let mut s = Session::new("abc".into(), "R1".into());
    assert_eq!(s.execute(&t, &api::me(), 1000), Err(ApiError::RateLimited(7)));
    assert_eq!(s.execute(&t, &api::me(), 1003), Err(ApiError::RateLimited(4)), "ainda esperando");
    assert_eq!(t.urls().len(), 2, "a espera não manda nada");
    assert_eq!(s.execute(&t, &api::me(), 1007), Ok((200, "{}".into())));
}

#[test]
fn rate_limit_wait_is_capped_at_a_minute() {
    let t = Fake::new(vec![token("A1", None), Ok((429, String::new(), Some(300))), ok("{}")]);
    let mut s = Session::new("abc".into(), "R1".into());
    assert_eq!(s.execute(&t, &api::me(), 1000), Err(ApiError::RateLimited(60)));
    assert_eq!(s.execute(&t, &api::me(), 1060), Ok((200, "{}".into())));
}
