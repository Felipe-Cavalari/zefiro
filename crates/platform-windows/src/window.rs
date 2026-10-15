//! Classes de janela, eventos e laço de mensagens.

use std::cell::RefCell;
use std::collections::HashMap;
use std::sync::OnceLock;
use std::sync::atomic::{AtomicIsize, Ordering};
use windows::Win32::Foundation::*;
use windows::Win32::Graphics::Gdi::ScreenToClient;
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::Input::KeyboardAndMouse::{SetFocus, TME_LEAVE, TRACKMOUSEEVENT, TrackMouseEvent};
use windows::Win32::UI::WindowsAndMessaging::*;
use windows::core::{PCWSTR, Result, w};
use zefiro_platform::geometry::{Point, Rect};

pub const WM_USER_BASE: u32 = WM_APP + 0x100;
const WM_APPBAR: u32 = WM_APP + 1;
/// Em `Win32::UI::Controls` na crate windows; definida aqui para não puxar a feature inteira.
const WM_MOUSELEAVE: u32 = 0x02A3;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WindowKind { Bar, Panel, Message }

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum WinEvent {
    MouseMove { hwnd: isize, x: f32, y: f32 },
    MouseLeave { hwnd: isize },
    LeftUp { hwnd: isize, x: f32, y: f32 },
    RightUp { hwnd: isize, x: f32, y: f32 },
    Wheel { hwnd: isize, delta: i32 },
    Key { hwnd: isize, vk: u32 },
    FocusLost { hwnd: isize },
    Timer { id: usize },
    DpiChanged { hwnd: isize },
    DisplayChanged,
    AppBar { hwnd: isize, notification: u32 },
    TaskbarCreated,
    SettingChanged,
    User { code: u32 },
    Quit,
}

const BAR: PCWSTR = w!("ZefiroBar");
const PANEL: PCWSTR = w!("ZefiroPanel");
const MESSAGE: PCWSTR = w!("ZefiroMessage");
static TASKBAR_CREATED: OnceLock<u32> = OnceLock::new();
static MESSAGE_HWND: AtomicIsize = AtomicIsize::new(0);

thread_local! {
    static EVENTS: RefCell<Vec<WinEvent>> = const { RefCell::new(Vec::new()) };
    static INTERACTIVE: RefCell<HashMap<isize, Vec<Rect>>> = RefCell::new(HashMap::new());
    static TRACKING: RefCell<HashMap<isize, bool>> = RefCell::new(HashMap::new());
}

fn push(e: WinEvent) {
    EVENTS.with(|q| q.borrow_mut().push(e));
}

pub fn take_events() -> Vec<WinEvent> {
    EVENTS.with(|q| std::mem::take(&mut *q.borrow_mut()))
}

pub fn set_interactive(hwnd: HWND, rects: Vec<Rect>) {
    INTERACTIVE.with(|m| m.borrow_mut().insert(hwnd.0 as isize, rects));
}

pub(crate) fn is_interactive(hwnd: HWND, p: Point) -> bool {
    INTERACTIVE.with(|m| m.borrow().get(&(hwnd.0 as isize)).is_some_and(|rs| rs.iter().any(|r| r.contains(p))))
}

fn xy(lp: LPARAM) -> (f32, f32) {
    ((lp.0 & 0xFFFF) as i16 as f32, ((lp.0 >> 16) & 0xFFFF) as i16 as f32)
}

extern "system" fn wndproc(hwnd: HWND, msg: u32, wp: WPARAM, lp: LPARAM) -> LRESULT {
    let h = hwnd.0 as isize;
    match msg {
        WM_NCHITTEST => {
            let (sx, sy) = xy(lp);
            let mut pt = POINT { x: sx as i32, y: sy as i32 };
            // SAFETY: converte coordenadas de tela para cliente desta janela.
            let _ = unsafe { ScreenToClient(hwnd, &mut pt) };
            let inside = is_interactive(hwnd, Point { x: pt.x as f32, y: pt.y as f32 });
            return LRESULT(if inside { HTCLIENT as isize } else { HTTRANSPARENT as isize });
        }
        WM_MOUSEMOVE => {
            let tracking = TRACKING.with(|t| t.borrow_mut().insert(h, true).unwrap_or(false));
            if !tracking {
                let mut tme = TRACKMOUSEEVENT { cbSize: size_of::<TRACKMOUSEEVENT>() as u32, dwFlags: TME_LEAVE, hwndTrack: hwnd, dwHoverTime: 0 };
                // SAFETY: struct local completa.
                let _ = unsafe { TrackMouseEvent(&mut tme) };
            }
            let (x, y) = xy(lp);
            push(WinEvent::MouseMove { hwnd: h, x, y });
        }
        WM_MOUSELEAVE => {
            TRACKING.with(|t| t.borrow_mut().insert(h, false));
            push(WinEvent::MouseLeave { hwnd: h });
        }
        WM_LBUTTONUP => {
            let (x, y) = xy(lp);
            push(WinEvent::LeftUp { hwnd: h, x, y });
        }
        WM_RBUTTONUP => {
            let (x, y) = xy(lp);
            push(WinEvent::RightUp { hwnd: h, x, y });
        }
        WM_MOUSEWHEEL => push(WinEvent::Wheel { hwnd: h, delta: ((wp.0 >> 16) & 0xFFFF) as i16 as i32 }),
        WM_KEYDOWN => push(WinEvent::Key { hwnd: h, vk: wp.0 as u32 }),
        WM_KILLFOCUS => push(WinEvent::FocusLost { hwnd: h }),
        WM_ACTIVATE if (wp.0 & 0xFFFF) as u32 == WA_INACTIVE => push(WinEvent::FocusLost { hwnd: h }),
        WM_TIMER => push(WinEvent::Timer { id: wp.0 }),
        WM_DPICHANGED => push(WinEvent::DpiChanged { hwnd: h }),
        WM_DISPLAYCHANGE => push(WinEvent::DisplayChanged),
        WM_SETTINGCHANGE => push(WinEvent::SettingChanged),
        WM_MOUSEACTIVATE => {
            // SAFETY: lê o estilo estendido desta janela.
            let ex = unsafe { GetWindowLongPtrW(hwnd, GWL_EXSTYLE) } as u32;
            if ex & WS_EX_NOACTIVATE.0 != 0 {
                return LRESULT(MA_NOACTIVATE as isize);
            }
            // SAFETY: repassa ao Windows.
            return unsafe { DefWindowProcW(hwnd, msg, wp, lp) };
        }
        WM_CLOSE => push(WinEvent::Quit),
        m if m == WM_APPBAR => push(WinEvent::AppBar { hwnd: h, notification: wp.0 as u32 }),
        m if (WM_USER_BASE..WM_USER_BASE + 0x100).contains(&m) => push(WinEvent::User { code: m - WM_USER_BASE }),
        m if Some(&m) == TASKBAR_CREATED.get() => push(WinEvent::TaskbarCreated),
        // SAFETY: mensagens não tratadas vão para o Windows.
        _ => return unsafe { DefWindowProcW(hwnd, msg, wp, lp) },
    }
    LRESULT(0)
}

pub fn register_classes() -> Result<()> {
    static DONE: OnceLock<()> = OnceLock::new();
    if DONE.get().is_some() {
        return Ok(());
    }
    // SAFETY: estruturas completas; nomes estáticos.
    unsafe {
        let instance = GetModuleHandleW(None)?;
        for name in [BAR, PANEL, MESSAGE] {
            let wc = WNDCLASSEXW {
                cbSize: size_of::<WNDCLASSEXW>() as u32,
                lpfnWndProc: Some(wndproc),
                hInstance: instance.into(),
                lpszClassName: name,
                hCursor: LoadCursorW(None, IDC_ARROW)?,
                ..Default::default()
            };
            if RegisterClassExW(&wc) == 0 {
                return Err(windows::core::Error::from_win32());
            }
        }
        let _ = TASKBAR_CREATED.set(RegisterWindowMessageW(w!("TaskbarCreated")));
    }
    let _ = DONE.set(());
    Ok(())
}

pub fn create(kind: WindowKind, r: Rect) -> Result<HWND> {
    let (class, ex, parent) = match kind {
        WindowKind::Bar => (BAR, WS_EX_TOOLWINDOW | WS_EX_NOREDIRECTIONBITMAP | WS_EX_TOPMOST | WS_EX_NOACTIVATE, None),
        WindowKind::Panel => (PANEL, WS_EX_TOOLWINDOW | WS_EX_NOREDIRECTIONBITMAP | WS_EX_TOPMOST, None),
        WindowKind::Message => (MESSAGE, WINDOW_EX_STYLE(0), Some(HWND_MESSAGE)),
    };
    // SAFETY: classe registrada; parâmetros simples.
    let hwnd = unsafe {
        let instance = GetModuleHandleW(None)?;
        CreateWindowExW(ex, class, w!("Zefiro"), WS_POPUP, r.x as i32, r.y as i32, r.w as i32, r.h as i32, parent, None, Some(instance.into()), None)?
    };
    if kind == WindowKind::Message {
        MESSAGE_HWND.store(hwnd.0 as isize, Ordering::SeqCst);
    }
    Ok(hwnd)
}

/// Pode ser chamado de qualquer thread: acorda o laço com `WinEvent::User { code }`.
pub fn post_user(code: u32) {
    let h = MESSAGE_HWND.load(Ordering::SeqCst);
    if h != 0 {
        // SAFETY: PostMessage é seguro entre threads; a janela de mensagens vive até o fim do app.
        let _ = unsafe { PostMessageW(Some(HWND(h as *mut _)), WM_USER_BASE + code, WPARAM(0), LPARAM(0)) };
    }
}

pub fn set_bounds(hwnd: HWND, r: Rect, show: bool) {
    let flags = SWP_NOACTIVATE | if show { SWP_SHOWWINDOW } else { SET_WINDOW_POS_FLAGS(0) };
    // SAFETY: hwnd válido.
    let _ = unsafe { SetWindowPos(hwnd, Some(HWND_TOPMOST), r.x as i32, r.y as i32, r.w as i32, r.h as i32, flags) };
}

pub fn hide(hwnd: HWND) {
    // SAFETY: hwnd válido.
    let _ = unsafe { ShowWindow(hwnd, SW_HIDE) };
}

/// Dá o foco do teclado (o clique que abriu o painel dá ao processo o direito de fazer isso).
pub fn focus(hwnd: HWND) {
    // SAFETY: hwnd válido.
    unsafe {
        let _ = SetForegroundWindow(hwnd);
        let _ = SetFocus(Some(hwnd));
    }
}

/// Janela em primeiro plano agora (para devolver o foco depois).
pub fn foreground() -> isize {
    // SAFETY: sem parâmetros.
    unsafe { GetForegroundWindow() }.0 as isize
}

/// Devolve o foco a `previous` se `ours` ainda estiver em primeiro plano (spec §6.1, item 5).
pub fn restore_foreground(previous: isize, ours: HWND) {
    // SAFETY: handles podem ter morrido; IsWindow confere antes de usar.
    unsafe {
        let prev = HWND(previous as *mut _);
        if previous != 0 && GetForegroundWindow() == ours && IsWindow(Some(prev)).as_bool() {
            let _ = SetForegroundWindow(prev);
        }
    }
}

pub fn set_timer(id: usize, ms: u32) {
    let h = MESSAGE_HWND.load(Ordering::SeqCst);
    // SAFETY: janela de mensagens válida; WM_TIMER chega no wndproc.
    unsafe { SetTimer(Some(HWND(h as *mut _)), id, ms, None) };
}

pub fn kill_timer(id: usize) {
    let h = MESSAGE_HWND.load(Ordering::SeqCst);
    // SAFETY: janela de mensagens válida.
    let _ = unsafe { KillTimer(Some(HWND(h as *mut _)), id) };
}

/// Espera e despacha uma mensagem. `false` = WM_QUIT.
pub fn pump() -> bool {
    let mut msg = MSG::default();
    // SAFETY: buffer local.
    unsafe {
        if !GetMessageW(&mut msg, None, 0, 0).as_bool() {
            return false;
        }
        let _ = TranslateMessage(&msg);
        DispatchMessageW(&msg);
    }
    true
}

/// Despacha o que já está na fila, sem esperar.
pub fn pump_pending() {
    let mut msg = MSG::default();
    // SAFETY: buffer local.
    unsafe {
        while PeekMessageW(&mut msg, None, 0, 0, PM_REMOVE).as_bool() {
            let _ = TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hit_test_is_transparent_outside_interactive_rects() {
        register_classes().unwrap();
        create(WindowKind::Message, Rect::default()).unwrap();
        let hwnd = create(WindowKind::Bar, Rect::new(-4000.0, -4000.0, 200.0, 50.0)).unwrap();
        set_interactive(hwnd, vec![Rect::new(10.0, 10.0, 50.0, 20.0)]);
        assert!(is_interactive(hwnd, Point { x: 20.0, y: 15.0 }));
        assert!(!is_interactive(hwnd, Point { x: 100.0, y: 15.0 }));
        post_user(7);
        let mut seen = false;
        for _ in 0..20 {
            pump_pending();
            if take_events().iter().any(|e| matches!(e, WinEvent::User { code: 7 })) {
                seen = true;
                break;
            }
        }
        assert!(seen);
    }
}
