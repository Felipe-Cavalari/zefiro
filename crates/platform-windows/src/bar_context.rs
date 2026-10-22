//! Recuperação pelo botão direito sem capturar o botão esquerdo na barra vazia.

use std::cell::RefCell;
use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, WPARAM};
use windows::Win32::UI::WindowsAndMessaging::*;
use zefiro_platform::geometry::{Point, Rect};

thread_local! {
    static ZONES: RefCell<Vec<(isize, Rect)>> = const { RefCell::new(Vec::new()) };
    static PRESSED: RefCell<Option<isize>> = const { RefCell::new(None) };
    static WATCHES: RefCell<Vec<Watch>> = const { RefCell::new(Vec::new()) };
}

/// Ocultar automaticamente: onde fica o topo de verdade do monitor de uma barra e a faixa dela, e
/// onde o cursor está em relação a eles. O hook só observa; o clique segue para quem está embaixo.
struct Watch { hwnd: isize, edge_y: f32, edges: Vec<(f32, f32)>, band: Rect, at_edge: bool, over: bool }

/// Passa a observar o cursor para a barra `hwnd`: `edges` são os trechos (x inicial, x final) da
/// linha `edge_y` que são borda de verdade; `band` é a faixa da barra, na tela. Atualizar mantém o
/// estado atual do cursor.
pub fn watch_autohide(hwnd: HWND, edge_y: f32, edges: Vec<(f32, f32)>, band: Rect) {
    WATCHES.with(|w| {
        let mut w = w.borrow_mut();
        match w.iter_mut().find(|x| x.hwnd == hwnd.0 as isize) {
            Some(x) => (x.edge_y, x.edges, x.band) = (edge_y, edges, band),
            None => w.push(Watch { hwnd: hwnd.0 as isize, edge_y, edges, band, at_edge: false, over: false }),
        }
    });
}

pub fn unwatch_autohide(hwnd: HWND) { WATCHES.with(|w| w.borrow_mut().retain(|x| x.hwnd != hwnd.0 as isize)); }

/// (cursor no topo, cursor na faixa) da barra `hwnd`, pelo último movimento visto.
pub fn autohide_cursor(hwnd: HWND) -> (bool, bool) {
    WATCHES.with(|w| w.borrow().iter().find(|x| x.hwnd == hwnd.0 as isize).map_or((false, false), |x| (x.at_edge, x.over)))
}

fn cursor_moved(p: Point) {
    let changed = WATCHES.with(|w| {
        let mut changed = false;
        for x in w.borrow_mut().iter_mut() {
            let at_edge = p.y <= x.edge_y && x.edges.iter().any(|&(a, b)| p.x >= a && p.x < b);
            let over = x.band.contains(p);
            changed |= (at_edge, over) != (x.at_edge, x.over);
            (x.at_edge, x.over) = (at_edge, over);
        }
        changed
    });
    if changed {
        crate::window::post_autohide();
    }
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
        WATCHES.with(|w| w.borrow_mut().clear());
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
    if code >= 0 && wp.0 as u32 == WM_MOUSEMOVE && WATCHES.with(|w| !w.borrow().is_empty()) {
        // SAFETY: para HC_ACTION, o Windows fornece MSLLHOOKSTRUCT válido durante o callback.
        let event = unsafe { &*(lp.0 as *const MSLLHOOKSTRUCT) };
        cursor_moved(Point { x: event.pt.x as f32, y: event.pt.y as f32 });
    }
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
