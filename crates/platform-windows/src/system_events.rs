//! Eventos do sistema para a ilha, entregues na janela de mensagens: área de transferência
//! (`AddClipboardFormatListener`) e Caps/Num Lock (entrada bruta com `RIDEV_INPUTSINK`). Os dois são
//! avisos assíncronos: nada entra na cadeia de entrada do teclado como um hook faria.

use windows::Win32::Foundation::{HANDLE, HGLOBAL, HWND, LPARAM};
use windows::Win32::System::DataExchange::{AddClipboardFormatListener, CloseClipboard, GetClipboardData, OpenClipboard, RemoveClipboardFormatListener};
use windows::Win32::System::Memory::{GlobalLock, GlobalUnlock};
use windows::Win32::System::Ole::CF_UNICODETEXT;
use windows::Win32::UI::Input::KeyboardAndMouse::{GetKeyState, VK_CAPITAL, VK_CONTROL, VK_NUMLOCK};
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

/// Texto da área de transferência, se houver (limitado a 200 caracteres).
pub fn clipboard_text() -> Option<String> {
    // SAFETY: a área é aberta, lida com GlobalLock/GlobalUnlock e fechada; o ponteiro só é lido até o NUL.
    unsafe {
        let mut opened = false;
        for _ in 0..5 {
            if OpenClipboard(None).is_ok() {
                opened = true;
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
        if !opened {
            return None;
        }
        let text = GetClipboardData(CF_UNICODETEXT.0 as u32).ok().and_then(|h: HANDLE| {
            let global = HGLOBAL(h.0);
            let ptr = GlobalLock(global) as *const u16;
            if ptr.is_null() {
                return None;
            }
            let mut len = 0usize;
            while len < 4096 && *ptr.add(len) != 0 {
                len += 1;
            }
            let s = String::from_utf16_lossy(std::slice::from_raw_parts(ptr, len));
            let _ = GlobalUnlock(global);
            Some(s)
        });
        let _ = CloseClipboard();
        text.map(|t| t.trim().chars().take(200).collect::<String>()).filter(|t| !t.is_empty())
    }
}
