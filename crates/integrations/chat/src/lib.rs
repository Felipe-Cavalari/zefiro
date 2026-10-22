//! Chat com o Claude (`claude -p` em stream-json) e com o Codex (`codex app-server`), portável: os
//! protocolos, os argumentos (Windows e WSL), a escolha da origem, a conversa e as conversas
//! antigas (spec 2026-10-21-chat-claude).

pub mod agent;
pub mod codex;
pub mod command;
pub mod conversation;
pub mod history;
pub mod origin;
pub mod protocol;
