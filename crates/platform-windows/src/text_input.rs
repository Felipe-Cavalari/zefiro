//! Campo EDIT nativo: seleção, clipboard, IME e Unicode ficam com o Windows.

use std::{cell::RefCell, collections::HashMap, sync::OnceLock};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::{core::{HSTRING, Result, w}, Win32::{Foundation::*, Graphics::Gdi::*, UI::{Input::KeyboardAndMouse::{GetFocus, GetKeyState, SetFocus}, WindowsAndMessaging::*}}};
use zefiro_platform::{color::Rgba, geometry::Rect};

const SELECT: u32 = 0x00B1; // EM_SETSEL
const LIMIT: u32 = 0x00C5; // EM_LIMITTEXT
struct State { previous: isize, parent: HWND, owner: isize, host: HWND, background: COLORREF, text: COLORREF, brush: HBRUSH }
thread_local! { static FIELDS: RefCell<HashMap<isize, State>> = RefCell::new(HashMap::new()); }

pub struct TextInput { hwnd: HWND, host: HWND }

fn color(value: Rgba) -> COLORREF { COLORREF((value.r * 255.0).round() as u32 | ((value.g * 255.0).round() as u32) << 8 | ((value.b * 255.0).round() as u32) << 16) }

impl TextInput {
    pub fn new(owner: HWND, rect: Rect, text: &str, background: Rgba, foreground: Rgba) -> Result<Self> {
        let parent = crate::window::input_window(owner).unwrap_or(owner);
        static REGISTERED: OnceLock<std::result::Result<(), windows::core::HRESULT>> = OnceLock::new();
        REGISTERED.get_or_init(|| {
            // SAFETY: classe exclusiva com procedimento estático; registro uma vez por processo.
            unsafe {
                let instance = GetModuleHandleW(None).map_err(|error| error.code())?;
                let class = WNDCLASSW { lpfnWndProc: Some(host_procedure), hInstance: instance.into(), lpszClassName: w!("ZefiroTextFieldHost"), ..Default::default() };
                if RegisterClassW(&class) == 0 { return Err(windows::core::Error::from_win32().code()); }
            }
            Ok(())
        }).map_err(windows::core::Error::from)?;
        let mut origin = POINT { x: rect.x as i32, y: rect.y as i32 };
        // SAFETY: converte a posição do painel para uma janela GDI própria, com redirecionamento.
        let host = unsafe {
            let _ = ClientToScreen(parent, &mut origin);
            CreateWindowExW(WS_EX_TOOLWINDOW | WS_EX_TOPMOST, w!("ZefiroTextFieldHost"), w!("Formato do relógio"), WS_POPUP, origin.x, origin.y, rect.w as i32, rect.h as i32, Some(parent), None, None, None)?
        };
        // SAFETY: EDIT filho da janela redirecionada; se falhar, libera o host recém-criado.
        let hwnd = match unsafe { CreateWindowExW(WS_EX_CLIENTEDGE, w!("EDIT"), w!(""), WS_CHILD | WS_VISIBLE | WS_TABSTOP | WINDOW_STYLE(ES_AUTOHSCROLL as u32), 0, 0, rect.w as i32, rect.h as i32, Some(host), None, None, None) } {
            Ok(hwnd) => hwnd,
            Err(error) => {
                // SAFETY: janela ainda sem filho e sem publicação externa.
                let _ = unsafe { DestroyWindow(host) };
                return Err(error);
            }
        };
        // SAFETY: subclasse na mesma thread; procedimento anterior guardado até restaurar no Drop.
        let previous = unsafe { SetWindowLongPtrW(hwnd, GWLP_WNDPROC, procedure as *const () as isize) };
        // SAFETY: cria pincel GDI com cor válida; será liberado pelo dono.
        let brush = unsafe { CreateSolidBrush(color(background)) };
        FIELDS.with(|fields| fields.borrow_mut().insert(hwnd.0 as isize, State { previous, parent, owner: owner.0 as isize, host, background: color(background), text: color(foreground), brush }));
        let field = Self { hwnd, host };
        // SAFETY: controle EDIT criado acima, mensagens padrão sem ponteiros externos.
        unsafe {
            SendMessageW(hwnd, LIMIT, Some(WPARAM(1024)), None);
            SendMessageW(hwnd, WM_SETFONT, Some(WPARAM(GetStockObject(DEFAULT_GUI_FONT).0 as usize)), Some(LPARAM(1)));
        }
        field.set_text(text);
        // SAFETY: mostra o host sem ativar; o chamador decide quando focar.
        let _ = unsafe { ShowWindow(host, SW_SHOWNOACTIVATE) };
        Ok(field)
    }

    pub fn handle(&self) -> isize { self.hwnd.0 as isize }
    pub fn has_focus(&self) -> bool {
        // SAFETY: consultas de estado de janelas desta thread.
        unsafe { GetForegroundWindow() == self.host || GetFocus() == self.hwnd }
    }
    pub fn text(&self) -> String {
        // SAFETY: buffer local do tamanho do texto, limitado pelo próprio campo.
        unsafe {
            let mut text = vec![0u16; (GetWindowTextLengthW(self.hwnd).clamp(0, 1024) + 1) as usize];
            let length = GetWindowTextW(self.hwnd, &mut text);
            String::from_utf16_lossy(&text[..length.max(0) as usize])
        }
    }
    pub fn set_text(&self, text: &str) {
        // SAFETY: HSTRING permanece viva até a mensagem síncrona retornar.
        let _ = unsafe { SetWindowTextW(self.hwnd, &HSTRING::from(text)) };
    }
    pub fn focus(&self) {
        // SAFETY: controle filho vivo, selecionado em resposta a ação do usuário.
        unsafe { let _ = SetForegroundWindow(self.host); let _ = SetFocus(Some(self.hwnd)); SendMessageW(self.hwnd, SELECT, Some(WPARAM(0)), Some(LPARAM(-1))); }
    }
}

impl Drop for TextInput {
    fn drop(&mut self) {
        if let Some(state) = FIELDS.with(|fields| fields.borrow_mut().remove(&(self.hwnd.0 as isize))) {
            // SAFETY: restaura procedimento antes de destruir e libera pincel apenas depois da janela.
            unsafe {
                if IsWindow(Some(self.hwnd)).as_bool() {
                    if GetForegroundWindow() == self.host { let _ = SetForegroundWindow(state.parent); let _ = SetFocus(Some(state.parent)); }
                    SetWindowLongPtrW(self.hwnd, GWLP_WNDPROC, state.previous);
                    let _ = DestroyWindow(self.hwnd);
                }
                if IsWindow(Some(self.host)).as_bool() { let _ = DestroyWindow(self.host); }
                let _ = DeleteObject(state.brush.into());
            }
        }
    }
}

extern "system" fn host_procedure(hwnd: HWND, message: u32, wp: WPARAM, lp: LPARAM) -> LRESULT {
    if message == WM_CTLCOLOREDIT && let Some(color) = control_color(wp, lp) { return color; }
    if message == WM_COMMAND && ((wp.0 >> 16) & 0xffff) as u32 == EN_CHANGE {
        let owner = FIELDS.with(|fields| fields.borrow().get(&lp.0).map(|field| field.owner));
        if let Some(owner) = owner { crate::window::push(crate::window::WinEvent::ControlChanged { hwnd: owner, control: lp.0 }); }
        return LRESULT(0);
    }
    if message == WM_ACTIVATE && wp.0 as u32 & 0xffff == WA_INACTIVE {
        let owner = FIELDS.with(|fields| fields.borrow().values().find(|field| field.host == hwnd).map(|field| field.owner));
        if let Some(owner) = owner { crate::window::push(crate::window::WinEvent::FocusLost { hwnd: owner }); }
    }
    // SAFETY: mensagens alheias seguem o procedimento padrão do host GDI.
    unsafe { DefWindowProcW(hwnd, message, wp, lp) }
}

pub(crate) fn control_color(wp: WPARAM, lp: LPARAM) -> Option<LRESULT> {
    FIELDS.with(|fields| fields.borrow().get(&lp.0).map(|state| {
        // SAFETY: WM_CTLCOLOREDIT fornece HDC válido enquanto o pai trata a mensagem.
        unsafe { SetBkColor(HDC(wp.0 as *mut _), state.background); SetTextColor(HDC(wp.0 as *mut _), state.text); }
        LRESULT(state.brush.0 as isize)
    }))
}

extern "system" fn procedure(hwnd: HWND, message: u32, wp: WPARAM, lp: LPARAM) -> LRESULT {
    let state = FIELDS.with(|fields| fields.borrow().get(&(hwnd.0 as isize)).map(|state| (state.previous, state.parent)));
    if let Some((previous, parent)) = state {
        if message == WM_KEYDOWN && matches!(wp.0, 0x09 | 0x0D | 0x1B) {
            // SAFETY: pai e controle pertencem à mesma thread; o evento é processado após esta chamada.
            let _ = unsafe { PostMessageW(Some(parent), WM_KEYDOWN, wp, lp) };
            return LRESULT(0);
        }
        // SAFETY: consulta tecla de controle válida.
        if message == WM_KEYDOWN && wp.0 == 0x41 && (unsafe { GetKeyState(0x11) }) < 0 {
            // SAFETY: seleção padrão do EDIT, sem ponteiros.
            unsafe { SendMessageW(hwnd, SELECT, Some(WPARAM(0)), Some(LPARAM(-1))) };
            return LRESULT(0);
        }
        if message == WM_CHAR && matches!(wp.0, 0x09 | 0x0D | 0x1B) { return LRESULT(0); }
        // SAFETY: `previous` veio de GWLP_WNDPROC desta mesma janela e ainda é válido.
        let procedure: WNDPROC = unsafe { std::mem::transmute(previous) };
        // SAFETY: encaminha à implementação original do controle com os argumentos recebidos.
        return unsafe { CallWindowProcW(procedure, hwnd, message, wp, lp) };
    }
    // SAFETY: fallback durante criação/destruição quando ainda não há metadados.
    unsafe { DefWindowProcW(hwnd, message, wp, lp) }
}
