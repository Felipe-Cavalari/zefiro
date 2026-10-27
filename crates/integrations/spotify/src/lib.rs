//! Spotify pela Web API, portável: login por PKCE, os pedidos, a leitura das respostas, a sessão
//! (renovação do token e 429) e o cache de capas (ADR-0024). A rede e o DPAPI ficam na
//! `platform-windows`.

pub mod api;
pub mod auth;
pub mod cache;
pub mod model;
pub mod session;
