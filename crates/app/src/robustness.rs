//! Eventos do sistema: monitores, DPI, Explorer, preferências (preenchido na Task 24).

pub fn handle(_: &mut crate::event_loop::Loop, _: zefiro_platform_windows::window::WinEvent) -> windows::core::Result<bool> {
    Ok(true)
}
