//! Chat com o Claude pelo `claude -p` em stream-json, portável: as linhas do protocolo, os
//! argumentos (Windows e WSL), a escolha da origem e a conversa (spec 2026-10-21-chat-claude).

pub mod command;
pub mod conversation;
pub mod origin;
pub mod protocol;
