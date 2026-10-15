//! AppBar no topo do monitor (SHAppBarMessage).

use windows::Win32::Foundation::{HWND, RECT};
use windows::Win32::UI::Shell::*;
use windows::Win32::UI::WindowsAndMessaging::WM_APP;
use zefiro_platform::geometry::Rect;

const WM_APPBAR: u32 = WM_APP + 1; // a mesma do window.rs

fn data(hwnd: HWND) -> APPBARDATA {
    APPBARDATA { cbSize: size_of::<APPBARDATA>() as u32, hWnd: hwnd, uCallbackMessage: WM_APPBAR, uEdge: ABE_TOP, ..Default::default() }
}

fn to_rect(r: Rect) -> RECT {
    RECT { left: r.x as i32, top: r.y as i32, right: r.right() as i32, bottom: r.bottom() as i32 }
}

fn from_rect(r: RECT) -> Rect {
    Rect::new(r.left as f32, r.top as f32, (r.right - r.left) as f32, (r.bottom - r.top) as f32)
}

/// Registra a janela como AppBar no topo de `monitor` e reserva `height` px.
pub fn register(hwnd: HWND, monitor: Rect, height: i32) -> Rect {
    let mut abd = data(hwnd);
    // SAFETY: `abd` com cbSize correto.
    unsafe { SHAppBarMessage(ABM_NEW, &mut abd) };
    set_pos(hwnd, monitor, height)
}

pub fn set_pos(hwnd: HWND, monitor: Rect, height: i32) -> Rect {
    let mut abd = data(hwnd);
    let mut r = to_rect(monitor);
    r.bottom = r.top + height;
    abd.rc = r;
    // SAFETY: `abd` válido.
    unsafe { SHAppBarMessage(ABM_QUERYPOS, &mut abd) };
    abd.rc.bottom = abd.rc.top + height;
    // SAFETY: `abd` válido.
    unsafe { SHAppBarMessage(ABM_SETPOS, &mut abd) };
    from_rect(abd.rc)
}

pub fn unregister(hwnd: HWND) {
    let mut abd = data(hwnd);
    // SAFETY: `abd` válido.
    unsafe { SHAppBarMessage(ABM_REMOVE, &mut abd) };
}
