//! Ilha de mídia embutida na barra (porte do Dynamic Island for Windows, MIT; ver
//! `docs/island-port-notes.md`). Esta crate é portável: estado, escolhas e desenho em `DisplayList`.
//! A plataforma fornece as sessões de mídia e o áudio; a barra desenha e anima.

pub mod accent;
pub mod analyzer;
pub mod media;
pub mod model;
pub mod state;
pub mod view;
pub mod waveform;
