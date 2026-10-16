//! Implementação Windows: Direct2D (WARP), Windows.UI.Composition, janelas e AppBar.
#![cfg(windows)]

pub mod appbar;
pub mod audio_devices;
pub mod bluetooth;
pub mod audio_capture;
pub mod bar_context;
pub mod compositor;
pub mod dnd;
pub mod gfx;
pub mod hardware;
pub mod http;
pub mod imaging;
pub mod island_zorder;
pub mod media;
pub mod monitors;
pub mod morph;
pub mod notifications;
pub mod painter;
pub mod popup_menu;
pub mod privacy;
pub mod system;
pub mod system_events;
pub mod text;
pub mod text_input;
pub mod volume;
pub mod weather;
pub mod window;
