//! Ações de energia do Windows que a barra oferece (painel "Energia").

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PowerAction { Lock, Sleep, SignOut, Restart, Shutdown }
