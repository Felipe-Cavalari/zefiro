//! Login no Spotify por Authorization Code com PKCE: não há segredo, só o Client ID do app que o
//! usuário criou. O verificador e o `state` vêm de bytes aleatórios que a plataforma sorteia.

use serde_json::Value;
use zefiro_integration_chat::image::base64;
use zefiro_integration_update::sha256;

/// Porta fixa: o Spotify compara o Redirect URI exato, porta incluída.
pub const PORT: u16 = 47823;
pub const REDIRECT_URI: &str = "http://127.0.0.1:47823/callback";
pub const AUTHORIZE: &str = "https://accounts.spotify.com/authorize";
pub const TOKEN: &str = "https://accounts.spotify.com/api/token";
pub const SCOPES: &str = "user-read-playback-state user-modify-playback-state user-read-currently-playing user-read-recently-played user-library-read user-library-modify playlist-read-private playlist-read-collaborative playlist-modify-private playlist-modify-public user-follow-read";

/// Resposta que o listener manda ao navegador depois do login.
pub const DONE_PAGE: &str = "HTTP/1.1 200 OK\r\nContent-Type: text/html; charset=utf-8\r\nConnection: close\r\n\r\n<!doctype html><html lang=\"pt-BR\"><meta charset=\"utf-8\"><title>Zefiro</title><body style=\"font-family:Segoe UI,sans-serif;background:#08080A;color:#fff;display:flex;align-items:center;justify-content:center;height:90vh\"><p>Pronto, pode voltar ao Zefiro.</p></body></html>";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Pkce { pub verifier: String, pub challenge: String, pub state: String }

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Callback { Code(String), Denied(String), WrongState, NotCallback }

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Tokens { pub access: String, pub expires_in: u64, pub refresh: Option<String> }

/// O que vai cifrado para o `token.bin`: o token de renovação e de qual app ele é.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Stored { pub client_id: String, pub refresh: String }

impl Stored {
    pub fn to_bytes(&self) -> Vec<u8> {
        serde_json::json!({ "client_id": self.client_id, "refresh": self.refresh }).to_string().into_bytes()
    }

    pub fn from_bytes(bytes: &[u8]) -> Option<Stored> {
        let v: Value = serde_json::from_slice(bytes).ok()?;
        Some(Stored { client_id: v["client_id"].as_str()?.to_string(), refresh: v["refresh"].as_str()?.to_string() })
    }
}

/// Base64 sem `=`, com `-` e `_` no lugar de `+` e `/` (RFC 4648 §5).
fn base64url(bytes: &[u8]) -> String {
    base64(bytes).trim_end_matches('=').replace('+', "-").replace('/', "_")
}

/// Escape de URL do UTF-8; só `A-Za-z0-9-._~` passam como estão.
pub fn url_encode(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for b in s.bytes() {
        if b.is_ascii_alphanumeric() || matches!(b, b'-' | b'.' | b'_' | b'~') {
            out.push(b as char);
        } else {
            out.push_str(&format!("%{b:02X}"));
        }
    }
    out
}

/// `%C3%A7` → "ç"; `+` fica como está (o Spotify manda `%20`).
fn url_decode(s: &str) -> String {
    let bytes = s.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%'
            && let Some(v) = s.get(i + 1..i + 3).and_then(|h| u8::from_str_radix(h, 16).ok())
        {
            out.push(v);
            i += 3;
            continue;
        }
        out.push(bytes[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

pub fn challenge(verifier: &str) -> String {
    base64url(&sha256(verifier.as_bytes()))
}

/// Os 64 primeiros bytes viram o verificador (86 caracteres); os 32 últimos, o `state`.
pub fn pkce(random: &[u8; 96]) -> Pkce {
    let verifier = base64url(&random[..64]);
    Pkce { challenge: challenge(&verifier), state: base64url(&random[64..]), verifier }
}

pub fn authorize_url(client_id: &str, p: &Pkce) -> String {
    format!(
        "{AUTHORIZE}?response_type=code&client_id={}&scope={}&redirect_uri={}&code_challenge_method=S256&code_challenge={}&state={}",
        url_encode(client_id),
        url_encode(SCOPES),
        url_encode(REDIRECT_URI),
        p.challenge,
        p.state
    )
}

/// 1ª linha do pedido que chegou ao listener (`GET /callback?… HTTP/1.1`).
pub fn parse_callback(request_head: &str, state: &str) -> Callback {
    let Some(target) = request_head.strip_prefix("GET ").and_then(|r| r.split(' ').next()) else { return Callback::NotCallback };
    let Some(query) = target.strip_prefix("/callback?") else { return Callback::NotCallback };
    let param = |name: &str| query.split('&').find_map(|kv| kv.strip_prefix(name).and_then(|v| v.strip_prefix('='))).map(url_decode);
    if param("state").as_deref() != Some(state) {
        return Callback::WrongState;
    }
    match (param("code"), param("error")) {
        (_, Some(error)) => Callback::Denied(error),
        (Some(code), None) => Callback::Code(code),
        (None, None) => Callback::NotCallback,
    }
}

pub fn code_body(client_id: &str, code: &str, verifier: &str) -> String {
    format!("grant_type=authorization_code&code={}&redirect_uri={}&client_id={}&code_verifier={}", url_encode(code), url_encode(REDIRECT_URI), url_encode(client_id), url_encode(verifier))
}

pub fn refresh_body(client_id: &str, refresh: &str) -> String {
    format!("grant_type=refresh_token&refresh_token={}&client_id={}", url_encode(refresh), url_encode(client_id))
}

pub fn parse_tokens(body: &str) -> Option<Tokens> {
    let v: Value = serde_json::from_str(body).ok()?;
    Some(Tokens { access: v["access_token"].as_str()?.to_string(), expires_in: v["expires_in"].as_u64().unwrap_or(3600), refresh: v["refresh_token"].as_str().map(str::to_string) })
}
