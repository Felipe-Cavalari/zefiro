//! Spike descartável da bandeja (plano 2026-10-26-bandeja, Task 1). Cria uma `Shell_TrayWnd` acima
//! da barra de tarefas, grava o `lpData` de cada `WM_COPYDATA` de ícone em
//! `%TEMP%\zefiro-tray-spike\`, repassa tudo à barra de tarefas de verdade e manda o
//! `TaskbarCreated` para os apps reenviarem os ícones.
//!
//! `cargo run -p zefiro-platform-windows --example tray_spike -- [--secs 60] [--raise] [--target Discord]`
//!
//! Com `--target <trecho da dica>`, aparece uma janelinha: clique esquerdo ou direito nela manda o
//! mesmo clique ao ícone cuja dica contém o trecho (o clique real dá o direito de primeiro plano).

use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, AtomicIsize, AtomicUsize, Ordering};
use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, POINT, RECT, WPARAM};
use windows::Win32::System::DataExchange::COPYDATASTRUCT;
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::WindowsAndMessaging::{
    AllowSetForegroundWindow, CreateWindowExW, DefWindowProcW, DestroyWindow, DispatchMessageW, FindWindowExW, FindWindowW, GetCursorPos, GetMessageW, GetWindowRect,
    GetWindowThreadProcessId, HWND_BROADCAST, HWND_TOPMOST, InSendMessage, KillTimer, MSG, PostMessageW, PostQuitMessage, RegisterClassExW, RegisterWindowMessageW,
    SMTO_ABORTIFHUNG, SWP_NOACTIVATE, SWP_SHOWWINDOW, SendMessageTimeoutW, SetTimer, SetWindowPos, TranslateMessage, WM_COPYDATA, WM_DESTROY, WM_LBUTTONUP, WM_RBUTTONUP,
    WM_TIMER, WM_USER, WNDCLASSEXW, WS_CHILD, WS_EX_TOOLWINDOW, WS_EX_TOPMOST, WS_OVERLAPPED, WS_POPUP, WS_VISIBLE,
};
use windows::core::{PCWSTR, w};

const MARK: isize = 0x5A45_4649;
const TICK: usize = 1;

static HOST: AtomicIsize = AtomicIsize::new(0);
static NOTIFY: AtomicIsize = AtomicIsize::new(0);
static SEQ: AtomicUsize = AtomicUsize::new(0);
static TASKBAR_CREATED: AtomicUsize = AtomicUsize::new(0);
static RAISE: AtomicBool = AtomicBool::new(false);
static SEEN: Mutex<Vec<Seen>> = Mutex::new(Vec::new());
static TARGET: Mutex<String> = Mutex::new(String::new());
static ELAPSED: AtomicUsize = AtomicUsize::new(0);
static SECS: AtomicUsize = AtomicUsize::new(60);

#[derive(Clone)]
struct Seen { hwnd: u32, uid: u32, callback: u32, version: u32, tip: String }

fn u32_at(b: &[u8], at: usize) -> u32 {
    b.get(at..at + 4).map_or(0, |s| u32::from_le_bytes([s[0], s[1], s[2], s[3]]))
}

fn tip_at(b: &[u8], at: usize) -> String {
    let units: Vec<u16> = b.get(at..at + 256).unwrap_or_default().as_chunks::<2>().0.iter().map(|c| u16::from_le_bytes(*c)).take_while(|&u| u != 0).collect();
    String::from_utf16_lossy(&units)
}

/// A primeira `Shell_TrayWnd` que não é a nossa.
fn real_taskbar() -> Option<HWND> {
    let ours = HWND(HOST.load(Ordering::SeqCst) as *mut _);
    let mut after: Option<HWND> = None;
    loop {
        // SAFETY: só procura janelas de topo pela classe.
        let h = unsafe { FindWindowExW(None, after, w!("Shell_TrayWnd"), PCWSTR::null()) }.ok()?;
        if h != ours {
            return Some(h);
        }
        after = Some(h);
    }
}

fn record(cds: &COPYDATASTRUCT) {
    // SAFETY: o sistema garante `cbData` bytes em `lpData` enquanto a mensagem é tratada.
    let bytes = unsafe { std::slice::from_raw_parts(cds.lpData as *const u8, cds.cbData as usize) };
    let n = SEQ.fetch_add(1, Ordering::SeqCst);
    let (sig, op, hwnd, uid, flags, callback, tip, version) =
        (u32_at(bytes, 0), u32_at(bytes, 4), u32_at(bytes, 12), u32_at(bytes, 16), u32_at(bytes, 20), u32_at(bytes, 24), tip_at(bytes, 32), u32_at(bytes, 808));
    println!("#{n} op={op} sig={sig:#x} len={} hwnd={hwnd:#x} uid={uid} flags={flags:#x} cb={callback:#x} version={version} tip={tip:?}", bytes.len());
    let dir = std::env::temp_dir().join("zefiro-tray-spike");
    let _ = std::fs::create_dir_all(&dir);
    let _ = std::fs::write(dir.join(format!("{n:03}-op{op}.bin")), bytes);
    let mut seen = SEEN.lock().unwrap_or_else(|e| e.into_inner());
    match op {
        0 | 1 => {
            if let Some(s) = seen.iter_mut().find(|s| s.hwnd == hwnd && s.uid == uid) {
                if flags & 0x1 != 0 { s.callback = callback; }
                if flags & 0x4 != 0 { s.tip = tip; }
            } else if op == 0 {
                seen.push(Seen { hwnd, uid, callback, version: 0, tip });
            }
        }
        2 => seen.retain(|s| !(s.hwnd == hwnd && s.uid == uid)),
        4 => {
            if let Some(s) = seen.iter_mut().find(|s| s.hwnd == hwnd && s.uid == uid) {
                s.version = version;
            }
        }
        _ => {}
    }
}

fn mirror_rect(host: HWND) {
    let Some(real) = real_taskbar() else { return };
    let mut r = RECT::default();
    // SAFETY: janelas válidas; só lê e move a nossa.
    unsafe {
        let _ = GetWindowRect(real, &mut r);
        let _ = SetWindowPos(host, Some(HWND_TOPMOST), r.left, r.top, r.right - r.left, r.bottom - r.top, SWP_NOACTIVATE);
        let notify = HWND(NOTIFY.load(Ordering::SeqCst) as *mut _);
        if let Ok(rn) = FindWindowExW(Some(real), None, w!("TrayNotifyWnd"), PCWSTR::null()) {
            let mut n = RECT::default();
            let _ = GetWindowRect(rn, &mut n);
            let _ = SetWindowPos(notify, None, n.left - r.left, n.top - r.top, n.right - n.left, n.bottom - n.top, SWP_NOACTIVATE);
        }
    }
}

unsafe extern "system" fn host_proc(h: HWND, msg: u32, wp: WPARAM, lp: LPARAM) -> LRESULT {
    if msg == WM_COPYDATA {
        // SAFETY: `lp` aponta para um COPYDATASTRUCT válido durante o WM_COPYDATA.
        let cds = unsafe { &*(lp.0 as *const COPYDATASTRUCT) };
        if cds.dwData == 1 {
            record(cds);
        } else {
            println!("WM_COPYDATA dwData={} len={}", cds.dwData, cds.cbData);
        }
        let Some(real) = real_taskbar() else { return LRESULT(0) };
        let mut result = 0usize;
        // SAFETY: repassa a mesma mensagem; o sistema copia o COPYDATASTRUCT para o outro processo.
        let ok = unsafe { SendMessageTimeoutW(real, WM_COPYDATA, wp, lp, SMTO_ABORTIFHUNG, 500, Some(&mut result)) };
        if ok.0 == 0 {
            println!("  repasse falhou ou expirou");
        }
        return LRESULT(result as isize);
    }
    if msg >= WM_USER && msg as usize != TASKBAR_CREATED.load(Ordering::SeqCst) {
        // SAFETY: só repassa inteiros; o que a barra de tarefas receberia direto.
        let sent = unsafe { InSendMessage() }.as_bool();
        println!("mensagem {msg:#x} wp={:#x} lp={:#x} ({})", wp.0, lp.0, if sent { "enviada" } else { "postada" });
        if let Some(real) = real_taskbar() {
            let mut result = 0usize;
            // SAFETY: mesmos parâmetros, sem ponteiro nosso.
            unsafe {
                if sent {
                    let _ = SendMessageTimeoutW(real, msg, wp, lp, SMTO_ABORTIFHUNG, 500, Some(&mut result));
                } else {
                    let _ = PostMessageW(Some(real), msg, wp, lp);
                }
            }
            return LRESULT(result as isize);
        }
    }
    // SAFETY: padrão.
    unsafe { DefWindowProcW(h, msg, wp, lp) }
}

unsafe extern "system" fn watch_proc(h: HWND, msg: u32, wp: WPARAM, lp: LPARAM) -> LRESULT {
    if msg as usize == TASKBAR_CREATED.load(Ordering::SeqCst) && msg != 0 {
        println!("TaskbarCreated recebido (lParam={:#x}, {})", lp.0, if lp.0 == MARK { "o nosso" } else { "do Explorer" });
        if lp.0 != MARK && HOST.load(Ordering::SeqCst) == 0 {
            match create_host() {
                Ok(_) => println!("host recriado depois do Explorer"),
                Err(e) => println!("não consegui recriar o host: {e}"),
            }
        }
        return LRESULT(0);
    }
    if msg == WM_TIMER {
        let ms = ELAPSED.fetch_add(500, Ordering::SeqCst) + 500;
        let host = HWND(HOST.load(Ordering::SeqCst) as *mut _);
        if !host.is_invalid() && real_taskbar().is_none() {
            println!("[{:>5.1}s] barra de tarefas sumiu: destruindo o host", ms as f32 / 1000.0);
            // SAFETY: nossa janela, nossa thread.
            unsafe {
                let _ = DestroyWindow(host);
            }
            HOST.store(0, Ordering::SeqCst);
        } else if ms.is_multiple_of(2000) {
            // SAFETY: só consulta.
            let first = unsafe { FindWindowW(w!("Shell_TrayWnd"), PCWSTR::null()) }.unwrap_or_default();
            let owner = if host.is_invalid() { "sem host" } else if first == host { "spike" } else { "Explorer" };
            println!("[{:>5.1}s] primeira Shell_TrayWnd: {owner} · ícones: {}", ms as f32 / 1000.0, SEEN.lock().map_or(0, |s| s.len()));
            if RAISE.load(Ordering::SeqCst) && !host.is_invalid() && first != host {
                mirror_rect(host);
            }
        }
        if ms >= SECS.load(Ordering::SeqCst) * 1000 {
            // SAFETY: nossas janelas, nossa thread.
            unsafe {
                let _ = KillTimer(Some(h), TICK);
                if !host.is_invalid() {
                    let _ = DestroyWindow(host);
                }
                let _ = DestroyWindow(h);
            }
        }
        return LRESULT(0);
    }
    if msg == WM_DESTROY {
        // SAFETY: encerra o laço desta thread.
        unsafe { PostQuitMessage(0) };
        return LRESULT(0);
    }
    // SAFETY: padrão.
    unsafe { DefWindowProcW(h, msg, wp, lp) }
}

/// Cria a `Shell_TrayWnd` e a filha, copia o retângulo da barra de tarefas e pede os ícones.
fn create_host() -> windows::core::Result<HWND> {
    // SAFETY: janelas desta thread.
    unsafe {
        let host = CreateWindowExW(WS_EX_TOOLWINDOW | WS_EX_TOPMOST, w!("Shell_TrayWnd"), PCWSTR::null(), WS_POPUP, 0, 0, 0, 0, None, None, None, None)?;
        HOST.store(host.0 as isize, Ordering::SeqCst);
        let notify = CreateWindowExW(Default::default(), w!("TrayNotifyWnd"), PCWSTR::null(), WS_CHILD, 0, 0, 0, 0, Some(host), None, None, None)?;
        NOTIFY.store(notify.0 as isize, Ordering::SeqCst);
        mirror_rect(host);
        let first = FindWindowW(w!("Shell_TrayWnd"), PCWSTR::null()).unwrap_or_default();
        println!("host {:?}; primeira Shell_TrayWnd é a do spike: {}", host.0, first == host);
        let _ = PostMessageW(Some(HWND_BROADCAST), TASKBAR_CREATED.load(Ordering::SeqCst) as u32, WPARAM(0), LPARAM(MARK));
        Ok(host)
    }
}

unsafe extern "system" fn click_proc(h: HWND, msg: u32, wp: WPARAM, lp: LPARAM) -> LRESULT {
    if msg == WM_LBUTTONUP || msg == WM_RBUTTONUP {
        let right = msg == WM_RBUTTONUP;
        let target = TARGET.lock().map(|t| t.to_lowercase()).unwrap_or_default();
        let found = SEEN.lock().ok().and_then(|s| s.iter().find(|s| s.tip.to_lowercase().contains(&target)).cloned());
        match found {
            Some(s) => send_click(&s, right),
            None => println!("nenhum ícone com {target:?} na dica"),
        }
        return LRESULT(0);
    }
    // SAFETY: padrão.
    unsafe { DefWindowProcW(h, msg, wp, lp) }
}

fn send_click(s: &Seen, right: bool) {
    let hwnd = HWND(s.hwnd as usize as *mut _);
    let mut p = POINT::default();
    let mut pid = 0u32;
    // SAFETY: só consulta.
    unsafe {
        let _ = GetCursorPos(&mut p);
        GetWindowThreadProcessId(hwnd, Some(&mut pid));
        if let Err(e) = AllowSetForegroundWindow(pid) {
            println!("AllowSetForegroundWindow falhou: {e}");
        }
    }
    let seq: &[u32] = match (s.version >= 4, right) {
        (true, false) => &[0x201, 0x202, 0x400],
        (true, true) => &[0x204, 0x205, 0x7B],
        (false, false) => &[0x201, 0x202],
        (false, true) => &[0x204, 0x205],
    };
    for &m in seq {
        let (wp, lp) = if s.version >= 4 {
            ((p.x as u16 as usize) | ((p.y as u16 as usize) << 16), (m | (s.uid << 16)) as isize)
        } else {
            (s.uid as usize, m as isize)
        };
        // SAFETY: PostMessage sem ponteiros.
        let _ = unsafe { PostMessageW(Some(hwnd), s.callback, WPARAM(wp), LPARAM(lp)) };
    }
    println!("clique {} mandado a {:?} (versão {})", if right { "direito" } else { "esquerdo" }, s.tip, s.version);
}

unsafe extern "system" fn default_proc(h: HWND, msg: u32, wp: WPARAM, lp: LPARAM) -> LRESULT {
    // SAFETY: padrão.
    unsafe { DefWindowProcW(h, msg, wp, lp) }
}

fn class(name: PCWSTR, proc: unsafe extern "system" fn(HWND, u32, WPARAM, LPARAM) -> LRESULT) {
    // SAFETY: registra uma classe deste processo.
    unsafe {
        let wc = WNDCLASSEXW { cbSize: size_of::<WNDCLASSEXW>() as u32, lpfnWndProc: Some(proc), hInstance: GetModuleHandleW(None).unwrap_or_default().into(), lpszClassName: name, ..Default::default() };
        RegisterClassExW(&wc);
    }
}

fn main() -> windows::core::Result<()> {
    let args: Vec<String> = std::env::args().collect();
    let arg = |name: &str| args.iter().position(|a| a == name).and_then(|i| args.get(i + 1)).cloned();
    if let Some(s) = arg("--secs").and_then(|s| s.parse().ok()) {
        SECS.store(s, Ordering::SeqCst);
    }
    RAISE.store(args.iter().any(|a| a == "--raise"), Ordering::SeqCst);
    let target = arg("--target");
    // SAFETY: Win32 na thread principal, que bombeia mensagens até o fim.
    unsafe {
        TASKBAR_CREATED.store(RegisterWindowMessageW(w!("TaskbarCreated")) as usize, Ordering::SeqCst);
        class(w!("Shell_TrayWnd"), host_proc);
        class(w!("TrayNotifyWnd"), default_proc);
        class(w!("ZefiroTraySpikeWatch"), watch_proc);
        class(w!("ZefiroTraySpikeClick"), click_proc);
        let watch = CreateWindowExW(WS_EX_TOOLWINDOW, w!("ZefiroTraySpikeWatch"), PCWSTR::null(), WS_POPUP, 0, 0, 0, 0, None, None, None, None)?;
        create_host()?;
        if let Some(t) = target {
            *TARGET.lock().unwrap_or_else(|e| e.into_inner()) = t.clone();
            let c = CreateWindowExW(WS_EX_TOPMOST, w!("ZefiroTraySpikeClick"), w!("Spike: clique aqui"), WS_OVERLAPPED | WS_VISIBLE, 200, 200, 320, 120, None, None, None, None)?;
            let _ = SetWindowPos(c, Some(HWND_TOPMOST), 200, 200, 320, 120, SWP_SHOWWINDOW);
            println!("janela de clique aberta para {t:?}");
        }
        SetTimer(Some(watch), TICK, 500, None);
        let mut m = MSG::default();
        while GetMessageW(&mut m, None, 0, 0).as_bool() {
            let _ = TranslateMessage(&m);
            DispatchMessageW(&m);
        }
    }
    let seen = SEEN.lock().map(|s| s.clone()).unwrap_or_default();
    println!("\n{} ícones no fim:", seen.len());
    for s in seen {
        println!("  uid={} versão={} dica={:?}", s.uid, s.version, s.tip);
    }
    Ok(())
}
