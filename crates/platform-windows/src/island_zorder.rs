//! Mantém a janela da ilha acima da barra (spec §7.5).

use windows::Win32::UI::WindowsAndMessaging::*;
use windows::core::w;

pub fn raise_island() -> bool {
    // SAFETY: FindWindow/SetWindowPos com parâmetros simples; a janela pode ter sumido, o erro é ignorado.
    unsafe {
        let Ok(hwnd) = FindWindowW(w!("Windhawk.DynamicIslandForWindows"), None) else { return false };
        if hwnd.is_invalid() {
            return false;
        }
        SetWindowPos(hwnd, Some(HWND_TOPMOST), 0, 0, 0, 0, SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE).is_ok()
    }
}
