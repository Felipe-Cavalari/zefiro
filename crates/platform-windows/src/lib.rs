//! Implementação Windows: Direct2D (WARP), Windows.UI.Composition, janelas e AppBar.
#![cfg(windows)]

pub mod appbar;
pub mod bar_context;
pub mod compositor;
pub mod gfx;
pub mod island_zorder;
pub mod monitors;
pub mod morph;
pub mod painter;
pub mod system;
pub mod text;
pub mod text_input;
pub mod window;
