//! Sonda de entrada real em dois processos. Restaura ponteiro e foco ao terminar.

use std::{process::Command, sync::atomic::{AtomicU32, Ordering}, time::{Duration, Instant}};
use windows::{core::w, Win32::{Foundation::*, UI::{Input::KeyboardAndMouse::*, WindowsAndMessaging::*}, System::LibraryLoader::GetModuleHandleW}};
use zefiro_platform::geometry::Rect;
use zefiro_platform_windows::{bar_context, window};

static LEFT: AtomicU32 = AtomicU32::new(0);
static RIGHT: AtomicU32 = AtomicU32::new(0);
const QUERY_LEFT: u32 = WM_APP + 40;
const QUERY_RIGHT: u32 = WM_APP + 41;

struct ReceiverGuard(std::process::Child);
impl Drop for ReceiverGuard {
    fn drop(&mut self) {
        // SAFETY: encerra somente a janela de classe exclusiva do processo auxiliar.
        unsafe {
            if let Ok(hwnd) = FindWindowW(w!("ZefiroProbeReceiver"), None) { let _ = PostMessageW(Some(hwnd), WM_CLOSE, WPARAM(0), LPARAM(0)); }
        }
        let deadline = Instant::now() + Duration::from_secs(2);
        while matches!(self.0.try_wait(), Ok(None)) && Instant::now() < deadline { std::thread::sleep(Duration::from_millis(10)); }
        if matches!(self.0.try_wait(), Ok(None)) { let _ = self.0.kill(); }
        let _ = self.0.wait();
    }
}

struct DesktopRestore { cursor: POINT, foreground: isize }
impl Drop for DesktopRestore {
    fn drop(&mut self) {
        // SAFETY: restaura o estado observado antes da sonda; valida a janela antes de ativá-la.
        unsafe {
            let _ = SetCursorPos(self.cursor.x, self.cursor.y);
            if IsWindow(Some(HWND(self.foreground as *mut _))).as_bool() { let _ = SetForegroundWindow(HWND(self.foreground as *mut _)); }
        }
    }
}

extern "system" fn receiver(hwnd: HWND, message: u32, wp: WPARAM, lp: LPARAM) -> LRESULT {
    match message {
        WM_LBUTTONUP => { LEFT.fetch_add(1, Ordering::Relaxed); LRESULT(0) }
        WM_RBUTTONUP => { RIGHT.fetch_add(1, Ordering::Relaxed); LRESULT(0) }
        QUERY_LEFT => LRESULT(LEFT.load(Ordering::Relaxed) as isize),
        QUERY_RIGHT => LRESULT(RIGHT.load(Ordering::Relaxed) as isize),
        WM_DESTROY => {
            // SAFETY: encerra somente o laço do processo auxiliar.
            unsafe { PostQuitMessage(0) };
            LRESULT(0)
        }
        // SAFETY: mensagens não tratadas são do Windows.
        _ => unsafe { DefWindowProcW(hwnd, message, wp, lp) },
    }
}

fn pump(ms: u64) {
    let until = Instant::now() + Duration::from_millis(ms);
    while Instant::now() < until { window::pump_pending(); std::thread::sleep(Duration::from_millis(5)); }
}

fn click(flags: [MOUSE_EVENT_FLAGS; 2]) {
    let inputs = flags.map(|dw_flags| INPUT { r#type: INPUT_MOUSE, Anonymous: INPUT_0 { mi: MOUSEINPUT { dwFlags: dw_flags, ..Default::default() } } });
    // SAFETY: dois eventos para a superfície auxiliar previamente conferida.
    assert_eq!(unsafe { SendInput(&inputs, size_of::<INPUT>() as i32) }, 2);
}

fn main() -> windows::core::Result<()> {
    if std::env::args().any(|a| a == "--receiver") {
        // SAFETY: classe e janela pertencem exclusivamente ao processo de sonda.
        unsafe {
            let instance = GetModuleHandleW(None)?;
            let wc = WNDCLASSW { lpfnWndProc: Some(receiver), hInstance: instance.into(), lpszClassName: w!("ZefiroProbeReceiver"), ..Default::default() };
            RegisterClassW(&wc);
            let hwnd = CreateWindowExW(WS_EX_TOOLWINDOW | WS_EX_TOPMOST, w!("ZefiroProbeReceiver"), w!("Sonda de entrada do Zefiro"), WS_POPUP | WS_VISIBLE, 80, 100, 300, 90, None, None, Some(instance.into()), None)?;
            let _ = SetForegroundWindow(hwnd);
            let mut message = MSG::default();
            while GetMessageW(&mut message, None, 0, 0).0 > 0 { DispatchMessageW(&message); }
        }
        return Ok(());
    }
    let previous = window::foreground();
    let mut cursor = POINT::default();
    // SAFETY: armazena posição para restauração ao fim da sonda.
    unsafe { GetCursorPos(&mut cursor)? };
    let _restore = DesktopRestore { cursor, foreground: previous };
    let _child = ReceiverGuard(Command::new(std::env::current_exe().unwrap()).arg("--receiver").spawn().unwrap());
    let deadline = Instant::now() + Duration::from_secs(5);
    let target = loop {
        // SAFETY: apenas procura a classe exclusiva do processo auxiliar.
        if let Ok(hwnd) = unsafe { FindWindowW(w!("ZefiroProbeReceiver"), None) } { break hwnd; }
        assert!(Instant::now() < deadline, "janela auxiliar não abriu");
        std::thread::sleep(Duration::from_millis(10));
    };
    window::register_classes()?;
    let bar = window::create(window::WindowKind::Bar, Rect::new(80.0, 100.0, 300.0, 44.0))?;
    window::set_bounds(bar, Rect::new(80.0, 100.0, 300.0, 44.0), true);
    window::set_interactive(bar, vec![]);
    bar_context::set_zone(bar, Rect::new(80.0, 100.0, 300.0, 44.0));
    let hook = if std::env::args().any(|a| a == "--without-hook") { None } else { Some(bar_context::BarContextHook::install()?) };
    let result = (|| -> windows::core::Result<()> {
        // SAFETY: só move para a área da sonda; confere quem recebe antes de enviar cliques.
        unsafe {
            SetCursorPos(100, 120)?;
            assert_eq!(WindowFromPoint(POINT { x: 100, y: 120 }), target, "clique esquerdo deve atingir outro processo");
        }
        pump(80);
        click([MOUSEEVENTF_LEFTDOWN, MOUSEEVENTF_LEFTUP]);
        pump(100);
        click([MOUSEEVENTF_RIGHTDOWN, MOUSEEVENTF_RIGHTUP]);
        pump(100);
        let events = window::take_events();
        // SAFETY: consulta contadores no processo auxiliar; sem ponteiros entre processos.
        let left = unsafe { SendMessageW(target, QUERY_LEFT, None, None) }.0;
        let right = unsafe { SendMessageW(target, QUERY_RIGHT, None, None) }.0;
        let recovered = events.iter().any(|e| matches!(e, window::WinEvent::RightUp { hwnd, .. } if *hwnd == bar.0 as isize));
        println!("Esquerdo no outro processo: {left}; direito no outro processo: {right}; menu recuperado: {recovered}");
        if left != 1 || right != 0 || !recovered { return Err(windows::core::Error::new(windows::core::HRESULT(0x80004005u32 as i32), "Falha no contrato de entrada")); }
        Ok(())
    })();
    drop(hook);
    window::destroy(bar);
    // SAFETY: restaura cursor/foco e encerra somente a janela de sonda identificada.
    unsafe {
        let _ = SetCursorPos(cursor.x, cursor.y);
        let _ = PostMessageW(Some(target), WM_CLOSE, WPARAM(0), LPARAM(0));
        if IsWindow(Some(HWND(previous as *mut _))).as_bool() { let _ = SetForegroundWindow(HWND(previous as *mut _)); }
    }
    result
}
