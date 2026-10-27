//! Os ícones da bandeja dos outros apps, para o chip `tray` (ADR-0023). Uma thread própria cria
//! uma janela `Shell_TrayWnd` acima da barra de tarefas: o `Shell_NotifyIcon` dos apps acha ela
//! primeiro, o host lê os ícones e repassa **tudo** à barra de tarefas de verdade, que continua
//! funcionando. Não confundir com `tray.rs`, o ícone do próprio Zefiro.
//!
//! A thread nunca espera a da interface: avisa por `on_change` (que só posta) e repassa com
//! `SendMessageTimeout`. Quando o Explorer cai, a janela sai da frente até o `TaskbarCreated` dele.

use std::cell::{Cell, RefCell};
use std::sync::mpsc;
use std::sync::{Arc, Mutex, Once};
use std::thread::JoinHandle;
use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, POINT, RECT, WPARAM};
use windows::Win32::System::DataExchange::COPYDATASTRUCT;
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::System::Threading::GetCurrentThreadId;
use windows::Win32::UI::Accessibility::{HWINEVENTHOOK, SetWinEventHook, UnhookWinEvent};
use windows::Win32::UI::WindowsAndMessaging::{
    AllowSetForegroundWindow, CreateWindowExW, DefWindowProcW, DestroyWindow, DispatchMessageW, EVENT_SYSTEM_FOREGROUND, FindWindowExW, FindWindowW, GetMessageW, GetWindowRect,
    GetWindowThreadProcessId, HICON, HWND_BROADCAST, HWND_TOPMOST, InSendMessage, IsWindow, MSG, PostMessageW, PostThreadMessageW, RegisterClassExW, RegisterWindowMessageW,
    SMTO_ABORTIFHUNG, SWP_NOACTIVATE, SendMessageTimeoutW, SetTimer, SetWindowPos, TranslateMessage, WINEVENT_OUTOFCONTEXT, WINEVENT_SKIPOWNPROCESS, WM_COMMAND, WM_COPYDATA,
    WM_QUIT, WM_TIMER, WM_USER, WNDCLASSEXW, WS_CHILD, WS_EX_TOOLWINDOW, WS_EX_TOPMOST, WS_POPUP,
};
use windows::core::{PCWSTR, Result, w};
use zefiro_integration_systray::click::{self, Button};
use zefiro_integration_systray::model::{Key, TrayModel};
use zefiro_integration_systray::wire::{self, COPYDATA_NOTIFY, NIF_ICON};
use zefiro_platform::image::Image;

/// `lParam` do `TaskbarCreated` que o host manda: o Zefiro reconhece o próprio aviso e não o trata
/// como um reinício do Explorer.
pub const OWN_BROADCAST: isize = 0x5A45_4649;

/// Quanto o repasse espera a barra de tarefas antes de desistir.
const FORWARD_MS: u32 = 300;
/// Conferência da barra de tarefas de verdade (viva? ainda atrás do host?).
const TICK_MS: u32 = 1000;
const TICK: usize = 1;

/// Um ícone como o painel o vê. `exe` é só o nome do arquivo (`Discord.exe`).
#[derive(Debug, Clone, PartialEq)]
pub struct TrayItem {
    pub key: Key,
    pub tip: String,
    pub exe: String,
    pub image: Option<Image>,
}

type Model = Arc<Mutex<TrayModel<Image>>>;
type Notify = Arc<dyn Fn() + Send + Sync>;

pub struct TrayHost {
    model: Model,
    thread_id: u32,
    thread: Option<JoinHandle<()>>,
}

/// O que os procedimentos de janela da thread do host usam. Clonado para fora antes de qualquer
/// chamada que possa reentrar (o `SendMessageTimeout` entrega mensagens enviadas enquanto espera).
#[derive(Clone)]
struct Ctx {
    model: Model,
    on_change: Notify,
    icon_px: u32,
    taskbar_created: u32,
}

thread_local! {
    static CTX: RefCell<Option<Ctx>> = const { RefCell::new(None) };
    static HOST: Cell<isize> = const { Cell::new(0) };
    static NOTIFY_WND: Cell<isize> = const { Cell::new(0) };
}

fn ctx() -> Option<Ctx> {
    CTX.with(|c| c.borrow().clone())
}

fn host() -> Option<HWND> {
    let h = HOST.with(Cell::get);
    (h != 0).then_some(HWND(h as *mut _))
}

impl TrayHost {
    /// Sobe a thread e a janela. `on_change` roda na thread do host e deve só postar.
    pub fn start(on_change: Notify, icon_px: u32) -> Result<TrayHost> {
        let model: Model = Arc::new(Mutex::new(TrayModel::default()));
        let (tx, rx) = mpsc::channel();
        let ctx = Ctx { model: model.clone(), on_change, icon_px, taskbar_created: 0 };
        let thread = std::thread::Builder::new()
            .name("zefiro-tray-host".into())
            .spawn(move || run(ctx, tx))
            .map_err(|e| windows::core::Error::new(windows::Win32::Foundation::E_FAIL, e.to_string()))?;
        match rx.recv() {
            Ok(Ok(thread_id)) => Ok(TrayHost { model, thread_id, thread: Some(thread) }),
            Ok(Err(e)) => {
                let _ = thread.join();
                Err(e)
            }
            Err(_) => Err(windows::core::Error::new(windows::Win32::Foundation::E_FAIL, "a thread da bandeja terminou sem responder")),
        }
    }

    /// Os ícones visíveis, na ordem de chegada.
    pub fn snapshot(&self) -> Vec<TrayItem> {
        let model = self.model.lock().unwrap_or_else(|e| e.into_inner());
        model
            .visible()
            .into_iter()
            .map(|i| TrayItem { key: i.key, tip: i.tip.clone(), exe: i.exe.rsplit(['\\', '/']).next().unwrap_or_default().to_string(), image: i.image.clone() })
            .collect()
    }

    /// Tira os ícones de apps que morreram sem avisar. Diz se algo saiu.
    pub fn prune(&self) -> bool {
        // SAFETY: `IsWindow` só consulta; um handle inválido dá `false`.
        self.model.lock().unwrap_or_else(|e| e.into_inner()).prune(|h| unsafe { IsWindow(Some(HWND(h as usize as *mut _))) }.as_bool())
    }

    /// Devolve um clique ao app dono do ícone, como a barra de tarefas faz. Chamar a partir de um
    /// clique do usuário: é ele que dá ao Zefiro o direito de liberar o primeiro plano.
    pub fn click(&self, key: &Key, button: Button, cursor: (i32, i32)) {
        let target = self.model.lock().unwrap_or_else(|e| e.into_inner()).get(key).map(|i| (i.hwnd, i.uid, i.callback, i.version));
        let Some((hwnd, uid, callback, version)) = target else { return };
        let hwnd = HWND(hwnd as usize as *mut _);
        // SAFETY: só consulta; um app que morreu dá `false` e nada é mandado.
        if callback == 0 || !unsafe { IsWindow(Some(hwnd)) }.as_bool() {
            return;
        }
        let mut pid = 0u32;
        // SAFETY: consulta do processo dono e permissão de primeiro plano para ele.
        unsafe {
            GetWindowThreadProcessId(hwnd, Some(&mut pid));
            if let Err(e) = AllowSetForegroundWindow(pid) {
                tracing::debug!("AllowSetForegroundWindow falhou para o ícone da bandeja: {e}");
            }
        }
        let clamp = |v: i32| v.clamp(i16::MIN as i32, i16::MAX as i32) as i16;
        for (wp, lp) in click::messages(version, uid, button, (clamp(cursor.0), clamp(cursor.1))) {
            // SAFETY: PostMessage sem ponteiros; um app travado não trava a barra.
            let _ = unsafe { PostMessageW(Some(hwnd), callback, WPARAM(wp), LPARAM(lp)) };
        }
    }
}

impl Drop for TrayHost {
    fn drop(&mut self) {
        // SAFETY: só posta o fim do laço da thread do host.
        let _ = unsafe { PostThreadMessageW(self.thread_id, WM_QUIT, WPARAM(0), LPARAM(0)) };
        if let Some(t) = self.thread.take() {
            let _ = t.join();
        }
    }
}

fn register_classes() {
    static ONCE: Once = Once::new();
    ONCE.call_once(|| {
        // SAFETY: classes deste processo, com procedimentos `extern "system"` deste módulo.
        unsafe {
            let instance = GetModuleHandleW(None).unwrap_or_default().into();
            for (name, proc) in [(w!("Shell_TrayWnd"), host_proc as WndProc), (w!("TrayNotifyWnd"), default_proc), (w!("ZefiroTrayWatch"), watch_proc)] {
                let wc = WNDCLASSEXW { cbSize: size_of::<WNDCLASSEXW>() as u32, lpfnWndProc: Some(proc), hInstance: instance, lpszClassName: name, ..Default::default() };
                RegisterClassExW(&wc);
            }
        }
    });
}

type WndProc = unsafe extern "system" fn(HWND, u32, WPARAM, LPARAM) -> LRESULT;

fn run(mut ctx: Ctx, ready: mpsc::Sender<Result<u32>>) {
    register_classes();
    // SAFETY: registra (ou acha) a mensagem global do Explorer.
    ctx.taskbar_created = unsafe { RegisterWindowMessageW(w!("TaskbarCreated")) };
    CTX.with(|c| *c.borrow_mut() = Some(ctx));
    // SAFETY: janelas e gancho desta thread, desfeitos antes de ela terminar.
    let watch = match unsafe { CreateWindowExW(WS_EX_TOOLWINDOW, w!("ZefiroTrayWatch"), PCWSTR::null(), WS_POPUP, 0, 0, 0, 0, None, None, None, None) } {
        Ok(h) => h,
        Err(e) => {
            let _ = ready.send(Err(e));
            return;
        }
    };
    if let Err(e) = create_host() {
        // SAFETY: janela desta thread.
        let _ = unsafe { DestroyWindow(watch) };
        let _ = ready.send(Err(e));
        return;
    }
    // SAFETY: temporizador e gancho desta thread; o callback do gancho roda dentro do GetMessage.
    let hook = unsafe {
        SetTimer(Some(watch), TICK, TICK_MS, None);
        SetWinEventHook(EVENT_SYSTEM_FOREGROUND, EVENT_SYSTEM_FOREGROUND, None, Some(on_foreground), 0, 0, WINEVENT_OUTOFCONTEXT | WINEVENT_SKIPOWNPROCESS)
    };
    // SAFETY: id desta thread, para o `Drop` postar o WM_QUIT.
    let _ = ready.send(Ok(unsafe { GetCurrentThreadId() }));
    tracing::info!("bandeja: host no ar");
    let mut m = MSG::default();
    // SAFETY: laço de mensagens desta thread.
    unsafe {
        while GetMessageW(&mut m, None, 0, 0).as_bool() {
            let _ = TranslateMessage(&m);
            DispatchMessageW(&m);
        }
        if !hook.is_invalid() {
            let _ = UnhookWinEvent(hook);
        }
    }
    destroy_host();
    // SAFETY: janela desta thread.
    let _ = unsafe { DestroyWindow(watch) };
    CTX.with(|c| c.borrow_mut().take());
    tracing::info!("bandeja: host fora do ar");
}

/// A primeira `Shell_TrayWnd` que não é a do host: a barra de tarefas de verdade.
fn real_taskbar() -> Option<HWND> {
    let ours = host();
    let mut after: Option<HWND> = None;
    loop {
        // SAFETY: só procura janelas de topo pela classe.
        let h = unsafe { FindWindowExW(None, after, w!("Shell_TrayWnd"), PCWSTR::null()) }.ok()?;
        if Some(h) != ours {
            return Some(h);
        }
        after = Some(h);
    }
}

fn create_host() -> Result<()> {
    // SAFETY: janelas desta thread, destruídas em `destroy_host`.
    unsafe {
        let h = CreateWindowExW(WS_EX_TOOLWINDOW | WS_EX_TOPMOST, w!("Shell_TrayWnd"), PCWSTR::null(), WS_POPUP, 0, 0, 0, 0, None, None, None, None)?;
        HOST.with(|c| c.set(h.0 as isize));
        let n = CreateWindowExW(Default::default(), w!("TrayNotifyWnd"), PCWSTR::null(), WS_CHILD, 0, 0, 0, 0, Some(h), None, None, None)?;
        NOTIFY_WND.with(|c| c.set(n.0 as isize));
        raise(h);
    }
    // Os apps respondem reenviando os ícones (NIM_DELETE + NIM_ADD).
    if let Some(c) = ctx() {
        // SAFETY: PostMessage sem ponteiros para todas as janelas de topo.
        let _ = unsafe { PostMessageW(Some(HWND_BROADCAST), c.taskbar_created, WPARAM(0), LPARAM(OWN_BROADCAST)) };
    }
    Ok(())
}

fn destroy_host() {
    if let Some(h) = host() {
        HOST.with(|c| c.set(0));
        NOTIFY_WND.with(|c| c.set(0));
        // SAFETY: janela desta thread (a filha vai junto).
        let _ = unsafe { DestroyWindow(h) };
    }
}

/// Põe o host no topo e com o retângulo da barra de tarefas (e a filha com o da área de
/// notificação): alguns apps posicionam janelas por ele.
fn raise(h: HWND) {
    let Some(real) = real_taskbar() else { return };
    let mut r = RECT::default();
    // SAFETY: janelas válidas; só lê a barra de tarefas e move as nossas.
    unsafe {
        let _ = GetWindowRect(real, &mut r);
        let _ = SetWindowPos(h, Some(HWND_TOPMOST), r.left, r.top, r.right - r.left, r.bottom - r.top, SWP_NOACTIVATE);
        let notify = HWND(NOTIFY_WND.with(Cell::get) as *mut _);
        if let Ok(rn) = FindWindowExW(Some(real), None, w!("TrayNotifyWnd"), PCWSTR::null()) {
            let mut n = RECT::default();
            let _ = GetWindowRect(rn, &mut n);
            let _ = SetWindowPos(notify, None, n.left - r.left, n.top - r.top, n.right - n.left, n.bottom - n.top, SWP_NOACTIVATE);
        }
    }
}

/// O host continua sendo o primeiro? Se não, volta para cima.
fn keep_first() {
    let Some(h) = host() else { return };
    // SAFETY: só consulta.
    let first = unsafe { FindWindowW(w!("Shell_TrayWnd"), PCWSTR::null()) }.ok();
    if first != Some(h) {
        raise(h);
    }
}

/// Lê um ícone e atualiza o modelo.
fn record(cds: &COPYDATASTRUCT, c: &Ctx) {
    // SAFETY: o sistema garante `cbData` bytes em `lpData` enquanto o WM_COPYDATA é tratado.
    let bytes = unsafe { std::slice::from_raw_parts(cds.lpData as *const u8, cds.cbData as usize) };
    let Some(n) = wire::parse(bytes) else { return };
    // O HICON é de outro processo e pode sumir: vira imagem agora e o handle não é guardado.
    let image = (n.flags & NIF_ICON != 0 && n.hicon != 0).then(|| crate::app_icon::hicon_to_image(HICON(n.hicon as usize as *mut _), c.icon_px)).flatten();
    let changed = c.model.lock().unwrap_or_else(|e| e.into_inner()).apply(&n, image);
    if changed {
        (c.on_change)();
    }
}

/// Mensagens que apps mandam à barra de tarefas e que a barra de verdade precisa receber.
fn forwarded(msg: u32, c: &Ctx) -> bool {
    msg == WM_COMMAND || ((WM_USER..0x8000).contains(&msg) || msg >= 0xC000) && msg != c.taskbar_created
}

unsafe extern "system" fn host_proc(h: HWND, msg: u32, wp: WPARAM, lp: LPARAM) -> LRESULT {
    let Some(c) = ctx() else {
        // SAFETY: padrão.
        return unsafe { DefWindowProcW(h, msg, wp, lp) };
    };
    if msg == WM_COPYDATA {
        // SAFETY: `lp` aponta para um COPYDATASTRUCT válido durante o WM_COPYDATA.
        let cds = unsafe { &*(lp.0 as *const COPYDATASTRUCT) };
        if cds.dwData == COPYDATA_NOTIFY {
            record(cds, &c);
        }
        let Some(real) = real_taskbar() else { return LRESULT(0) };
        let mut result = 0usize;
        // SAFETY: repassa a mesma mensagem; o sistema copia o COPYDATASTRUCT para o Explorer.
        if unsafe { SendMessageTimeoutW(real, WM_COPYDATA, wp, lp, SMTO_ABORTIFHUNG, FORWARD_MS, Some(&mut result)) }.0 == 0 {
            tracing::debug!(data = cds.dwData, "bandeja: repasse à barra de tarefas expirou");
        }
        return LRESULT(result as isize);
    }
    if forwarded(msg, &c) {
        let Some(real) = real_taskbar() else { return LRESULT(0) };
        let mut result = 0usize;
        // SAFETY: mesmos parâmetros, que a barra de tarefas receberia direto do app.
        unsafe {
            if InSendMessage().as_bool() {
                let _ = SendMessageTimeoutW(real, msg, wp, lp, SMTO_ABORTIFHUNG, FORWARD_MS, Some(&mut result));
            } else {
                let _ = PostMessageW(Some(real), msg, wp, lp);
            }
        }
        return LRESULT(result as isize);
    }
    // SAFETY: padrão.
    unsafe { DefWindowProcW(h, msg, wp, lp) }
}

unsafe extern "system" fn watch_proc(h: HWND, msg: u32, wp: WPARAM, lp: LPARAM) -> LRESULT {
    let Some(c) = ctx() else {
        // SAFETY: padrão.
        return unsafe { DefWindowProcW(h, msg, wp, lp) };
    };
    if msg == c.taskbar_created && msg != 0 {
        if lp.0 != OWN_BROADCAST {
            // O Explorer (re)subiu: o host volta para a frente da barra nova e pede os ícones.
            tracing::info!("bandeja: Explorer de volta, recriando o host");
            destroy_host();
            if let Err(e) = create_host() {
                tracing::warn!("bandeja: não consegui recriar o host: {e}");
            }
        }
        return LRESULT(0);
    }
    if msg == WM_TIMER {
        match (host(), real_taskbar()) {
            // A barra de tarefas sumiu: sai da frente para o Explorer subir sem achar a nossa.
            (Some(_), None) => {
                tracing::info!("bandeja: barra de tarefas sumiu, host fora até o Explorer voltar");
                destroy_host();
            }
            (Some(_), Some(_)) => keep_first(),
            _ => {}
        }
        return LRESULT(0);
    }
    // SAFETY: padrão.
    unsafe { DefWindowProcW(h, msg, wp, lp) }
}

unsafe extern "system" fn default_proc(h: HWND, msg: u32, wp: WPARAM, lp: LPARAM) -> LRESULT {
    // SAFETY: padrão.
    unsafe { DefWindowProcW(h, msg, wp, lp) }
}

unsafe extern "system" fn on_foreground(_: HWINEVENTHOOK, _: u32, _: HWND, _: i32, _: i32, _: u32, _: u32) {
    keep_first();
}

/// Posição do cursor na tela, para o clique devolvido ao app.
pub fn cursor() -> (i32, i32) {
    let mut p = POINT::default();
    // SAFETY: só consulta.
    let _ = unsafe { windows::Win32::UI::WindowsAndMessaging::GetCursorPos(&mut p) };
    (p.x, p.y)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_foreign_messages_are_forwarded() {
        let c = Ctx { model: Arc::default(), on_change: Arc::new(|| {}), icon_px: 32, taskbar_created: 0xC0B3 };
        assert!(forwarded(WM_COMMAND, &c));
        assert!(forwarded(0x4EF, &c));
        assert!(forwarded(0xC123, &c));
        assert!(!forwarded(0xC0B3, &c), "o TaskbarCreated não volta ao Explorer");
        assert!(!forwarded(0x0001, &c), "WM_CREATE é da própria janela");
        assert!(!forwarded(0x0014, &c));
    }
}
