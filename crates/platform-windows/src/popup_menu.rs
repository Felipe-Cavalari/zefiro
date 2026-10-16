//! Menu nativo do Windows (`TrackPopupMenu`) a partir das entradas portáveis da ilha.

use windows::Win32::Foundation::{HWND, POINT};
use windows::Win32::UI::WindowsAndMessaging::{
    AppendMenuW, CreatePopupMenu, DestroyMenu, GetCursorPos, HMENU, MF_CHECKED, MF_GRAYED, MF_POPUP, MF_SEPARATOR, MF_STRING, MENU_ITEM_FLAGS, PostMessageW, SetForegroundWindow,
    TPM_NONOTIFY, TPM_RETURNCMD, TPM_RIGHTBUTTON, TrackPopupMenu, WM_NULL,
};
use windows::core::{HSTRING, PCWSTR, Result};
use zefiro_island::menu::MenuEntry;

fn fill(menu: HMENU, entries: &[MenuEntry]) -> Result<()> {
    for e in entries {
        // SAFETY: menu criado por nós; o texto vive até o fim de cada chamada (AppendMenu copia).
        unsafe {
            if e.separator {
                AppendMenuW(menu, MF_SEPARATOR, 0, PCWSTR::null())?;
                continue;
            }
            let text = HSTRING::from(e.label.replace('&', "&&"));
            let mut flags = MF_STRING;
            if e.checked {
                flags |= MF_CHECKED;
            }
            if !e.enabled {
                flags |= MF_GRAYED;
            }
            if e.children.is_empty() {
                AppendMenuW(menu, flags, e.id as usize, PCWSTR(text.as_ptr()))?;
            } else {
                let sub = CreatePopupMenu()?;
                fill(sub, &e.children)?;
                // O submenu passa a pertencer ao menu pai (DestroyMenu do pai o destrói).
                AppendMenuW(menu, flags | MENU_ITEM_FLAGS(MF_POPUP.0), sub.0 as usize, PCWSTR(text.as_ptr()))?;
            }
        }
    }
    Ok(())
}

/// Mostra o menu no cursor e espera a escolha. `None` = fechado sem escolher.
pub fn show(owner: HWND, entries: &[MenuEntry]) -> Result<Option<u32>> {
    let owner = crate::window::input_window(owner).unwrap_or(owner);
    // SAFETY: menu criado, mostrado e destruído nesta função, na thread de UI dona de `owner`.
    unsafe {
        let menu = CreatePopupMenu()?;
        let result = fill(menu, entries).map(|_| {
            let mut p = POINT::default();
            let _ = GetCursorPos(&mut p);
            // Sem trazer a janela para frente, o menu não fecha ao clicar fora (documentação do TrackPopupMenu).
            let _ = SetForegroundWindow(owner);
            let id = TrackPopupMenu(menu, TPM_RETURNCMD | TPM_RIGHTBUTTON | TPM_NONOTIFY, p.x, p.y, Some(0), owner, None).0 as u32;
            let _ = PostMessageW(Some(owner), WM_NULL, Default::default(), Default::default());
            (id != 0).then_some(id)
        });
        let _ = DestroyMenu(menu);
        result
    }
}
