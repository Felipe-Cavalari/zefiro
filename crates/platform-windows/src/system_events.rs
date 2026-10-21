//! Eventos do sistema para a ilha, entregues na janela de mensagens: área de transferência
//! (`AddClipboardFormatListener`) e Caps/Num Lock (entrada bruta com `RIDEV_INPUTSINK`). Os dois são
//! avisos assíncronos: nada entra na cadeia de entrada do teclado como um hook faria.

use windows::Win32::Foundation::{GlobalFree, HANDLE, HGLOBAL, HWND, LPARAM};
use windows::Win32::System::DataExchange::{AddClipboardFormatListener, CloseClipboard, EmptyClipboard, GetClipboardData, OpenClipboard, RemoveClipboardFormatListener, SetClipboardData};
use windows::Win32::System::Memory::{GMEM_MOVEABLE, GlobalAlloc, GlobalLock, GlobalUnlock};
use windows::Win32::System::Ole::CF_UNICODETEXT;
use windows::Win32::UI::Input::KeyboardAndMouse::{GetKeyState, VK_CAPITAL, VK_CONTROL, VK_NUMLOCK, VK_SHIFT};
use windows::Win32::UI::Input::{GetRawInputData, HRAWINPUT, RAWINPUT, RAWINPUTDEVICE, RAWINPUTHEADER, RID_INPUT, RIDEV_INPUTSINK, RIDEV_REMOVE, RIM_TYPEKEYBOARD, RegisterRawInputDevices};
use windows::Win32::UI::WindowsAndMessaging::RI_KEY_BREAK;
use windows::core::Result;

/// Liga os avisos na janela de mensagens (chamar uma vez, na thread de UI).
pub fn listen(message: HWND) -> Result<()> {
    // SAFETY: janela de mensagens viva; estruturas locais de tamanho correto.
    unsafe {
        AddClipboardFormatListener(message)?;
        let keyboard = RAWINPUTDEVICE { usUsagePage: 0x01, usUsage: 0x06, dwFlags: RIDEV_INPUTSINK, hwndTarget: message };
        RegisterRawInputDevices(&[keyboard], size_of::<RAWINPUTDEVICE>() as u32)?;
    }
    Ok(())
}

pub fn stop(message: HWND) {
    // SAFETY: desfaz o que `listen` registrou para esta janela.
    unsafe {
        let _ = RemoveClipboardFormatListener(message);
        let keyboard = RAWINPUTDEVICE { usUsagePage: 0x01, usUsage: 0x06, dwFlags: RIDEV_REMOVE, hwndTarget: HWND::default() };
        let _ = RegisterRawInputDevices(&[keyboard], size_of::<RAWINPUTDEVICE>() as u32);
    }
}

/// `WM_INPUT`: `Some(caps)` quando Caps Lock (`true`) ou Num Lock (`false`) foi solto.
pub(crate) fn lock_key(lp: LPARAM) -> Option<bool> {
    let mut raw = RAWINPUT::default();
    let mut size = size_of::<RAWINPUT>() as u32;
    // SAFETY: buffer do tamanho de RAWINPUT; teclado cabe nele.
    let n = unsafe { GetRawInputData(HRAWINPUT(lp.0 as *mut _), RID_INPUT, Some(&mut raw as *mut _ as *mut _), &mut size, size_of::<RAWINPUTHEADER>() as u32) };
    if n == u32::MAX || raw.header.dwType != RIM_TYPEKEYBOARD.0 {
        return None;
    }
    // SAFETY: o cabeçalho diz que é teclado.
    let k = unsafe { raw.data.keyboard };
    if k.Flags as u32 & RI_KEY_BREAK == 0 {
        return None;
    }
    match k.VKey {
        v if v == VK_CAPITAL.0 => Some(true),
        v if v == VK_NUMLOCK.0 => Some(false),
        _ => None,
    }
}

/// Estado ligado/desligado de Caps Lock ou Num Lock.
pub fn lock_on(caps: bool) -> bool {
    // SAFETY: consulta de estado de tecla.
    unsafe { GetKeyState(if caps { VK_CAPITAL.0 } else { VK_NUMLOCK.0 } as i32) & 1 != 0 }
}

/// Ctrl apertado agora (para Ctrl+setas no painel).
pub fn ctrl_down() -> bool {
    // SAFETY: consulta de estado de tecla.
    let state = unsafe { GetKeyState(VK_CONTROL.0 as i32) };
    state < 0
}

fn open_clipboard(owner: Option<HWND>) -> bool {
    for _ in 0..5 {
        // SAFETY: abre a área de transferência para esta thread; o chamador fecha.
        if unsafe { OpenClipboard(owner) }.is_ok() {
            return true;
        }
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
    false
}

/// Texto da área de transferência, cru, até `max_units` unidades UTF-16.
fn read_clipboard(max_units: usize) -> Option<String> {
    if !open_clipboard(None) {
        return None;
    }
    // SAFETY: a área está aberta; o bloco é lido com GlobalLock/GlobalUnlock e o ponteiro só é
    // lido até o NUL ou o limite.
    let text = unsafe {
        GetClipboardData(CF_UNICODETEXT.0 as u32).ok().and_then(|h: HANDLE| {
            let global = HGLOBAL(h.0);
            let ptr = GlobalLock(global) as *const u16;
            if ptr.is_null() {
                return None;
            }
            let mut len = 0usize;
            while len < max_units && *ptr.add(len) != 0 {
                len += 1;
            }
            let s = String::from_utf16_lossy(std::slice::from_raw_parts(ptr, len));
            let _ = GlobalUnlock(global);
            Some(s)
        })
    };
    // SAFETY: par do OpenClipboard acima.
    let _ = unsafe { CloseClipboard() };
    text
}

/// Texto da área de transferência, se houver (limitado a 200 caracteres).
pub fn clipboard_text() -> Option<String> {
    read_clipboard(4096).map(|t| t.trim().chars().take(200).collect::<String>()).filter(|t| !t.is_empty())
}

/// Texto inteiro da área de transferência (até 64 mil unidades), sem aparar: o colar do chat.
pub fn clipboard_text_full() -> Option<String> {
    read_clipboard(64 * 1024).filter(|t| !t.is_empty())
}

/// Põe `text` na área de transferência. `owner` precisa ser uma janela: sem dono, o
/// `SetClipboardData` falha depois do `EmptyClipboard`.
pub fn set_clipboard_text(owner: HWND, text: &str) -> bool {
    let wide: Vec<u16> = text.encode_utf16().chain(std::iter::once(0)).collect();
    if !open_clipboard(Some(owner)) {
        return false;
    }
    // SAFETY: a área está aberta por esta thread; o bloco móvel tem o tamanho de `wide`, é copiado
    // com GlobalLock/GlobalUnlock e passa a ser do sistema quando o SetClipboardData dá certo (só é
    // liberado aqui se falhar).
    let ok = unsafe {
        let _ = EmptyClipboard();
        match GlobalAlloc(GMEM_MOVEABLE, wide.len() * 2) {
            Ok(global) => {
                let ptr = GlobalLock(global) as *mut u16;
                if ptr.is_null() {
                    let _ = GlobalFree(Some(global));
                    false
                } else {
                    std::ptr::copy_nonoverlapping(wide.as_ptr(), ptr, wide.len());
                    let _ = GlobalUnlock(global);
                    match SetClipboardData(CF_UNICODETEXT.0 as u32, Some(HANDLE(global.0))) {
                        Ok(_) => true,
                        Err(_) => {
                            let _ = GlobalFree(Some(global));
                            false
                        }
                    }
                }
            }
            Err(_) => false,
        }
    };
    // SAFETY: par do OpenClipboard acima.
    let _ = unsafe { CloseClipboard() };
    ok
}

/// Shift apertado agora (Shift+Enter quebra a linha no chat).
pub fn shift_down() -> bool {
    // SAFETY: consulta de estado de tecla.
    let state = unsafe { GetKeyState(VK_SHIFT.0 as i32) };
    state < 0
}
