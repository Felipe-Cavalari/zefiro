//! Classes de janela, eventos e laço de mensagens.

use std::cell::RefCell;
use std::collections::HashMap;
use std::sync::OnceLock;
use std::sync::atomic::{AtomicIsize, Ordering};
use windows::Win32::Foundation::*;
use windows::Win32::Graphics::Gdi::{CombineRgn, CreateRectRgn, DeleteObject, RGN_OR, ScreenToClient, SetWindowRgn};
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
const INPUT: PCWSTR = w!("ZefiroInput");
static TASKBAR_CREATED: OnceLock<u32> = OnceLock::new();
static MESSAGE_HWND: AtomicIsize = AtomicIsize::new(0);

thread_local! {
    static EVENTS: RefCell<Vec<WinEvent>> = const { RefCell::new(Vec::new()) };
    static INTERACTIVE: RefCell<HashMap<isize, Vec<Rect>>> = RefCell::new(HashMap::new());
    static TRACKING: RefCell<HashMap<isize, bool>> = RefCell::new(HashMap::new());
    /// Janela de desenho → janela de entrada que a acompanha.
    static INPUTS: RefCell<HashMap<isize, isize>> = RefCell::new(HashMap::new());
    /// Janela de entrada → janela de desenho dona dela.
    static OWNERS: RefCell<HashMap<isize, isize>> = RefCell::new(HashMap::new());
}

/// A janela de entrada de `hwnd` (barra e painel têm uma; a de mensagens não).
pub fn input_window(hwnd: HWND) -> Option<HWND> {
    INPUTS.with(|m| m.borrow().get(&(hwnd.0 as isize)).map(|&h| HWND(h as *mut _)))
}

fn owner_of(hwnd: HWND) -> Option<isize> {
    OWNERS.with(|m| m.borrow().get(&(hwnd.0 as isize)).copied())
}

fn push(e: WinEvent) {
    EVENTS.with(|q| q.borrow_mut().push(e));
}

pub fn take_events() -> Vec<WinEvent> {
    EVENTS.with(|q| std::mem::take(&mut *q.borrow_mut()))
}

/// Áreas clicáveis de `hwnd`, em coordenadas da janela. A janela de entrada passa a ter
/// exatamente essa forma (região); fora dela o clique vai para o que estiver embaixo,
/// inclusive janelas de outros processos.
pub fn set_interactive(hwnd: HWND, rects: Vec<Rect>) {
    if let Some(input) = input_window(hwnd) {
        // SAFETY: regiões GDI locais; depois de SetWindowRgn a região pertence ao sistema.
        unsafe {
            let region = CreateRectRgn(0, 0, 0, 0);
            for r in &rects {
                let part = CreateRectRgn(r.x.round() as i32, r.y.round() as i32, (r.x + r.w).round() as i32, (r.y + r.h).round() as i32);
                CombineRgn(Some(region), Some(region), Some(part), RGN_OR);
                let _ = DeleteObject(part.into());
            }
            SetWindowRgn(input, Some(region), true);
        }
    }
    INTERACTIVE.with(|m| m.borrow_mut().insert(hwnd.0 as isize, rects));
}

pub(crate) fn is_interactive(hwnd: HWND, p: Point) -> bool {
    INTERACTIVE.with(|m| m.borrow().get(&(hwnd.0 as isize)).is_some_and(|rs| rs.iter().any(|r| r.contains(p))))
}

fn xy(lp: LPARAM) -> (f32, f32) {
    ((lp.0 & 0xFFFF) as i16 as f32, ((lp.0 >> 16) & 0xFFFF) as i16 as f32)
}

extern "system" fn wndproc(hwnd: HWND, msg: u32, wp: WPARAM, lp: LPARAM) -> LRESULT {
    // Eventos da janela de entrada são entregues em nome da janela de desenho.
    let input = owner_of(hwnd);
    let h = input.unwrap_or(hwnd.0 as isize);
    match msg {
        // A região da janela de entrada já recorta o clique.
        WM_NCHITTEST if input.is_some() => return LRESULT(HTCLIENT as isize),
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
    // get_or_init bloqueia as outras threads enquanto a primeira registra (testes rodam em paralelo).
    static DONE: OnceLock<std::result::Result<(), windows::core::HRESULT>> = OnceLock::new();
    let done = DONE.get_or_init(|| {
        // SAFETY: estruturas completas; nomes estáticos.
        unsafe {
            let instance = GetModuleHandleW(None).map_err(|e| e.code())?;
            for name in [BAR, PANEL, MESSAGE, INPUT] {
                let wc = WNDCLASSEXW {
                    cbSize: size_of::<WNDCLASSEXW>() as u32,
                    lpfnWndProc: Some(wndproc),
                    hInstance: instance.into(),
                    lpszClassName: name,
                    hCursor: LoadCursorW(None, IDC_ARROW).map_err(|e| e.code())?,
                    ..Default::default()
                };
                if RegisterClassExW(&wc) == 0 {
                    return Err(windows::core::Error::from_win32().code());
                }
            }
            let _ = TASKBAR_CREATED.set(RegisterWindowMessageW(w!("TaskbarCreated")));
        }
        Ok(())
    });
    done.map_err(windows::core::Error::from)
}

/// Barra e painel são duas janelas: a de desenho (composição, sombra) deixa todo clique
/// passar (`WS_EX_LAYERED | WS_EX_TRANSPARENT`), e uma janela de entrada invisível, dona-
/// dependente dela e com a forma dos retângulos clicáveis, recebe mouse e teclado. Um
/// hit-test `HTTRANSPARENT` só repassa o clique para janelas da mesma thread: a margem da
/// sombra engolia cliques nas janelas dos outros programas.
pub fn create(kind: WindowKind, r: Rect) -> Result<HWND> {
    let layered = WS_EX_LAYERED | WS_EX_TRANSPARENT | WS_EX_NOACTIVATE;
    let (class, ex, parent) = match kind {
        WindowKind::Bar => (BAR, WS_EX_TOOLWINDOW | WS_EX_NOREDIRECTIONBITMAP | WS_EX_TOPMOST | layered, None),
        WindowKind::Panel => (PANEL, WS_EX_TOOLWINDOW | WS_EX_NOREDIRECTIONBITMAP | WS_EX_TOPMOST | layered, None),
        WindowKind::Message => (MESSAGE, WINDOW_EX_STYLE(0), Some(HWND_MESSAGE)),
    };
    // SAFETY: classe registrada; parâmetros simples.
    let hwnd = unsafe {
        let instance = GetModuleHandleW(None)?;
        CreateWindowExW(ex, class, w!("Zefiro"), WS_POPUP, r.x as i32, r.y as i32, r.w as i32, r.h as i32, parent, None, Some(instance.into()), None)?
    };
    if kind == WindowKind::Message {
        MESSAGE_HWND.store(hwnd.0 as isize, Ordering::SeqCst);
        return Ok(hwnd);
    }
    let input_ex = WS_EX_TOOLWINDOW | WS_EX_NOREDIRECTIONBITMAP | WS_EX_TOPMOST | if kind == WindowKind::Bar { WS_EX_NOACTIVATE } else { WINDOW_EX_STYLE(0) };
    // SAFETY: janela de desenho recém-criada; a de entrada pertence a ela (fica sempre acima).
    let input = unsafe {
        SetLayeredWindowAttributes(hwnd, COLORREF(0), 255, LWA_ALPHA)?;
        let instance = GetModuleHandleW(None)?;
        let input = CreateWindowExW(input_ex, INPUT, w!("Zefiro"), WS_POPUP, r.x as i32, r.y as i32, r.w as i32, r.h as i32, Some(hwnd), None, Some(instance.into()), None)?;
        SetWindowRgn(input, Some(CreateRectRgn(0, 0, 0, 0)), false);
        input
    };
    INPUTS.with(|m| m.borrow_mut().insert(hwnd.0 as isize, input.0 as isize));
    OWNERS.with(|m| m.borrow_mut().insert(input.0 as isize, hwnd.0 as isize));
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
    // Desenho primeiro, entrada depois: a de entrada fica por cima, com os mesmos limites.
    for h in std::iter::once(hwnd).chain(input_window(hwnd)) {
        // SAFETY: hwnd válido.
        let _ = unsafe { SetWindowPos(h, Some(HWND_TOPMOST), r.x as i32, r.y as i32, r.w as i32, r.h as i32, flags) };
    }
}

pub fn hide(hwnd: HWND) {
    for h in input_window(hwnd).into_iter().chain(std::iter::once(hwnd)) {
        // SAFETY: hwnd válido.
        let _ = unsafe { ShowWindow(h, SW_HIDE) };
    }
}

/// Destrói a janela e a de entrada dela (barras e painéis recriados quando os monitores mudam).
pub fn destroy(hwnd: HWND) {
    let h = hwnd.0 as isize;
    if let Some(input) = INPUTS.with(|m| m.borrow_mut().remove(&h)) {
        OWNERS.with(|m| m.borrow_mut().remove(&input));
        TRACKING.with(|m| m.borrow_mut().remove(&input));
        // SAFETY: janela de entrada criada por nós nesta thread.
        let _ = unsafe { DestroyWindow(HWND(input as *mut _)) };
    }
    INTERACTIVE.with(|m| m.borrow_mut().remove(&h));
    TRACKING.with(|m| m.borrow_mut().remove(&h));
    // SAFETY: janela criada por nós nesta thread.
    let _ = unsafe { DestroyWindow(hwnd) };
}

/// Dá o foco do teclado (o clique que abriu o painel dá ao processo o direito de fazer isso).
pub fn focus(hwnd: HWND) {
    let hwnd = input_window(hwnd).unwrap_or(hwnd);
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
    let ours = input_window(ours).unwrap_or(ours);
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
    use windows::Win32::Graphics::Gdi::GetWindowRgnBox;

    /// A janela de mensagens é global: os testes que criam janelas rodam um de cada vez.
    static SERIAL: std::sync::Mutex<()> = std::sync::Mutex::new(());

    #[test]
    fn hit_test_is_transparent_outside_interactive_rects() {
        let _serial = SERIAL.lock().unwrap_or_else(|e| e.into_inner());
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

    #[test]
    fn input_goes_through_a_companion_shaped_to_the_interactive_rects() {
        let _serial = SERIAL.lock().unwrap_or_else(|e| e.into_inner());
        register_classes().unwrap();
        create(WindowKind::Message, Rect::default()).unwrap();
        let hwnd = create(WindowKind::Bar, Rect::new(-4000.0, -4000.0, 200.0, 50.0)).unwrap();
        // SAFETY: lê o estilo da janela recém-criada.
        let ex = unsafe { GetWindowLongPtrW(hwnd, GWL_EXSTYLE) } as u32;
        assert_ne!(ex & WS_EX_TRANSPARENT.0, 0, "a janela de desenho deixa o clique passar");
        assert_ne!(ex & WS_EX_LAYERED.0, 0);
        let input = input_window(hwnd).expect("janela de entrada criada junto");
        set_interactive(hwnd, vec![Rect::new(10.0, 10.0, 50.0, 20.0)]);
        let mut r = RECT::default();
        // SAFETY: lê a região da janela de entrada.
        unsafe { GetWindowRgnBox(input, &mut r) };
        assert_eq!((r.left, r.top, r.right, r.bottom), (10, 10, 60, 30));
        // SAFETY: mensagem síncrona para a nossa janela.
        unsafe { SendMessageW(input, WM_LBUTTONUP, Some(WPARAM(0)), Some(LPARAM((15 << 16) | 20))) };
        let events = take_events();
        assert!(events.contains(&WinEvent::LeftUp { hwnd: hwnd.0 as isize, x: 20.0, y: 15.0 }), "{events:?}");

        destroy(hwnd);
        assert!(input_window(hwnd).is_none());
        // SAFETY: só consulta se os handles ainda existem.
        unsafe {
            assert!(!IsWindow(Some(hwnd)).as_bool());
            assert!(!IsWindow(Some(input)).as_bool());
        }
    }
}
