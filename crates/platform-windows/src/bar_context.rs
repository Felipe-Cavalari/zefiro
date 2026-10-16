//! Recuperação pelo botão direito sem capturar o botão esquerdo na barra vazia.

use std::cell::RefCell;
use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, WPARAM};
use windows::Win32::UI::WindowsAndMessaging::*;
use zefiro_platform::geometry::{Point, Rect};

thread_local! {
    static ZONES: RefCell<Vec<(isize, Rect)>> = const { RefCell::new(Vec::new()) };
    static PRESSED: RefCell<Option<isize>> = const { RefCell::new(None) };
}

pub struct BarContextHook(HHOOK);

impl BarContextHook {
    pub fn install() -> windows::core::Result<Self> {
        // SAFETY: hook de baixo nível executa nesta thread, cujo laço bombeia mensagens.
        unsafe { SetWindowsHookExW(WH_MOUSE_LL, Some(mouse_hook), None, 0).map(Self) }
    }
}

impl Drop for BarContextHook {
    fn drop(&mut self) {
        // SAFETY: hook pertence a este guard; removido antes de destruir o laço.
        let _ = unsafe { UnhookWindowsHookEx(self.0) };
        ZONES.with(|zones| zones.borrow_mut().clear());
        PRESSED.with(|pressed| *pressed.borrow_mut() = None);
    }
}

pub fn set_zone(hwnd: HWND, screen: Rect) {
    ZONES.with(|zones| {
        let mut zones = zones.borrow_mut();
        zones.retain(|(h, _)| *h != hwnd.0 as isize);
        zones.push((hwnd.0 as isize, screen));
    });
}

pub fn remove_zone(hwnd: HWND) { ZONES.with(|zones| zones.borrow_mut().retain(|(h, _)| *h != hwnd.0 as isize)); }

unsafe extern "system" fn mouse_hook(code: i32, wp: WPARAM, lp: LPARAM) -> LRESULT {
    if code >= 0 && (wp.0 as u32 == WM_RBUTTONDOWN || wp.0 as u32 == WM_RBUTTONUP) {
        // SAFETY: para HC_ACTION, o Windows fornece MSLLHOOKSTRUCT válido durante o callback.
        let event = unsafe { &*(lp.0 as *const MSLLHOOKSTRUCT) };
        let p = Point { x: event.pt.x as f32, y: event.pt.y as f32 };
        if wp.0 as u32 == WM_RBUTTONDOWN {
            let target = ZONES.with(|zones| zones.borrow().iter().find_map(|(raw, r)| {
                let hwnd = HWND(*raw as *mut _);
                // SAFETY: só consulta a visibilidade de uma janela cadastrada, sem acessar memória dela.
                let visible = unsafe { IsWindowVisible(hwnd) }.as_bool();
                if visible && r.contains(p) && !crate::window::is_interactive(hwnd, Point { x: p.x - r.x, y: p.y - r.y }) { Some(*raw) } else { None }
            }));
            if let Some(target) = target {
                PRESSED.with(|pressed| *pressed.borrow_mut() = Some(target));
                return LRESULT(1);
            }
        } else if let Some(raw) = PRESSED.with(|pressed| pressed.borrow_mut().take()) {
            let target = ZONES.with(|zones| zones.borrow().iter().find(|(h, r)| *h == raw && r.contains(p)).copied());
            if let Some((_, zone)) = target {
                crate::window::push(crate::window::WinEvent::RightUp { hwnd: raw, x: p.x - zone.x, y: p.y - zone.y });
            }
            return LRESULT(1);
        }
    }
    // SAFETY: repassa todos os eventos alheios, sem alterar o movimento ou o clique esquerdo.
    unsafe { CallNextHookEx(None, code, wp, lp) }
}
