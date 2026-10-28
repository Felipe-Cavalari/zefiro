//! Chat com o Claude (`claude -p` em stream-json), com o Codex (`codex app-server`) e com o Grok
//! (`grok agent stdio`, ACP), portável: os protocolos, os argumentos (Windows e WSL), a escolha da
//! origem, a conversa e as conversas antigas (spec 2026-10-21-chat-claude, ADR-0027).

pub mod agent;
pub mod codex;
pub mod command;
pub mod conversation;
pub mod grok;
pub mod history;
pub mod image;
pub mod origin;
pub mod protocol;
