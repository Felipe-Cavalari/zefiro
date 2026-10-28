//! A sessão com o Spotify: guarda o token de acesso (só em memória), renova antes de vencer e uma
//! vez depois de um 401, e avisa quando o Spotify troca o token de renovação. A rede vem de fora
//! (`Transport`), para os testes.

use crate::api::{BASE, Request};
use crate::auth::{Stored, TOKEN, Tokens, code_body, parse_tokens, refresh_body};
use crate::model::{ApiError, api_error};

/// Renova quando faltar menos que isto para o token de acesso vencer.
const MARGIN_S: u64 = 60;
/// Esperas de um 429 maiores que isto ficam neste teto.
const MAX_BACKOFF_S: u64 = 60;
const FORM: &str = "application/x-www-form-urlencoded";
const JSON: &str = "application/json";

/// A requisição não chegou ao Spotify (sem rede, tempo esgotado).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NetworkError;

/// Status, corpo e o `Retry-After` em segundos.
pub type Reply = Result<(u16, String, Option<u64>), NetworkError>;

pub trait Transport {
    /// `body` é (content-type, texto).
    fn send(&self, method: &str, url: &str, headers: &[(&str, &str)], body: Option<(&str, &str)>) -> Reply;
}

pub struct Session {
    client_id: String,
    refresh: String,
    access: Option<(String, u64)>,
    new_refresh: Option<String>,
    /// Depois de um 429, nada vai à rede até esta hora (s).
    blocked_until: Option<u64>,
}

fn token_request(t: &dyn Transport, body: &str) -> Result<Tokens, ApiError> {
    match t.send("POST", TOKEN, &[], Some((FORM, body))) {
        Err(NetworkError) => Err(ApiError::Network),
        Ok((200, text, _)) => parse_tokens(&text).ok_or(ApiError::Unauthorized),
        Ok((400 | 401, _, _)) => Err(ApiError::Unauthorized),
        Ok((status, text, retry)) => Err(api_error(status, &text, retry)),
    }
}

/// Troca o código do callback pelos tokens; `port` é a do pedido de login.
pub fn exchange_code(t: &dyn Transport, client_id: &str, code: &str, verifier: &str, port: u16) -> Result<Tokens, ApiError> {
    token_request(t, &code_body(client_id, code, verifier, port))
}

impl Session {
    pub fn new(client_id: String, refresh: String) -> Session {
        Session { client_id, refresh, access: None, new_refresh: None, blocked_until: None }
    }

    fn renew(&mut self, t: &dyn Transport, now_s: u64) -> Result<String, ApiError> {
        let tokens = token_request(t, &refresh_body(&self.client_id, &self.refresh))?;
        if let Some(r) = tokens.refresh.filter(|r| *r != self.refresh) {
            self.refresh = r.clone();
            self.new_refresh = Some(r);
        }
        self.access = Some((tokens.access.clone(), now_s + tokens.expires_in));
        Ok(tokens.access)
    }

    fn send(&self, t: &dyn Transport, req: &Request, access: &str) -> Result<(u16, String, Option<u64>), ApiError> {
        let auth = format!("Bearer {access}");
        let url = format!("{BASE}{}", req.path);
        t.send(req.method.as_str(), &url, &[("Authorization", &auth)], req.body.as_deref().map(|b| (JSON, b))).map_err(|NetworkError| ApiError::Network)
    }

    /// Manda `req`; 2xx volta como (status, corpo), o resto como erro. Depois de um 429, os pedidos
    /// voltam `RateLimited` com o que falta da espera, sem ir à rede.
    pub fn execute(&mut self, t: &dyn Transport, req: &Request, now_s: u64) -> Result<(u16, String), ApiError> {
        if let Some(until) = self.blocked_until {
            if now_s < until {
                return Err(ApiError::RateLimited(until - now_s));
            }
            self.blocked_until = None;
        }
        let access = match &self.access {
            Some((token, until)) if now_s + MARGIN_S < *until => token.clone(),
            _ => self.renew(t, now_s)?,
        };
        let mut reply = self.send(t, req, &access)?;
        if reply.0 == 401 {
            let access = self.renew(t, now_s)?;
            reply = self.send(t, req, &access)?;
        }
        match reply {
            (status @ 200..=299, body, _) => Ok((status, body)),
            (status, body, retry) => {
                if status == 401 {
                    self.access = None;
                }
                match api_error(status, &body, retry) {
                    ApiError::RateLimited(n) => {
                        let wait = n.min(MAX_BACKOFF_S);
                        self.blocked_until = Some(now_s + wait);
                        Err(ApiError::RateLimited(wait))
                    }
                    e => Err(e),
                }
            }
        }
    }

    /// O token de renovação novo que chegou, uma vez, para gravar.
    pub fn take_new_refresh(&mut self) -> Option<String> {
        self.new_refresh.take()
    }

    pub fn stored(&self) -> Stored {
        Stored { client_id: self.client_id.clone(), refresh: self.refresh.clone() }
    }
}
