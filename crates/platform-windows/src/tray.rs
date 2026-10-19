//! Ícone do Zefiro na bandeja do sistema. Os cliques chegam como `WinEvent::Tray` na janela de
//! mensagens. Quando o Explorer reinicia, a bandeja esquece o ícone: chame `add` de novo.

use std::sync::atomic::{AtomicIsize, Ordering};
use windows::Win32::UI::Shell::{NIF_ICON, NIF_MESSAGE, NIF_TIP, NIM_ADD, NIM_DELETE, NIM_MODIFY, NOTIFYICONDATAW, Shell_NotifyIconW};
use windows::Win32::UI::WindowsAndMessaging::{CreateIconFromResourceEx, DestroyIcon, GetSystemMetrics, HICON, LR_DEFAULTCOLOR, SM_CXSMICON};
use windows::core::Result;
use zefiro_platform::icon::ico_image;

/// Símbolo branco para bandeja escura e escuro para bandeja clara (`assets/icons`).
const LIGHT: &[u8] = include_bytes!("../../../assets/icons/tray-light.ico");
const DARK: &[u8] = include_bytes!("../../../assets/icons/tray-dark.ico");

static ICON: AtomicIsize = AtomicIsize::new(0);

fn data(icon: HICON) -> NOTIFYICONDATAW {
    let mut d = NOTIFYICONDATAW {
        cbSize: size_of::<NOTIFYICONDATAW>() as u32,
        hWnd: crate::window::message_window(),
        uID: 1,
        uFlags: NIF_MESSAGE | NIF_ICON | NIF_TIP,
        uCallbackMessage: crate::window::WM_TRAY,
        hIcon: icon,
        ..Default::default()
    };
    for (slot, c) in d.szTip.iter_mut().zip("Zefiro".encode_utf16()) {
        *slot = c;
    }
    d
}

/// HICON do tamanho que a bandeja usa (`SM_CXSMICON`), na cor que contrasta com ela.
fn load(dark_taskbar: bool) -> Result<HICON> {
    // SAFETY: só lê uma métrica do sistema.
    let size = unsafe { GetSystemMetrics(SM_CXSMICON) }.max(16);
    let image = ico_image(if dark_taskbar { LIGHT } else { DARK }, size as u32).ok_or_else(|| windows::core::Error::from_hresult(windows::Win32::Foundation::E_INVALIDARG))?;
    // SAFETY: `image` é um PNG ou DIB de ícone completo; o HICON criado é nosso e é destruído em `remove`.
    unsafe { CreateIconFromResourceEx(image, true, 0x0003_0000, size, size, LR_DEFAULTCOLOR) }
}

/// Põe o ícone na bandeja (ou troca a cor, se já estiver lá).
pub fn add(dark_taskbar: bool) -> Result<()> {
    let icon = load(dark_taskbar)?;
    let old = ICON.swap(icon.0 as isize, Ordering::SeqCst);
    let d = data(icon);
    // SAFETY: estrutura completa e janela de mensagens viva; NIM_ADD falha se já existe, aí NIM_MODIFY.
    unsafe {
        if !Shell_NotifyIconW(NIM_ADD, &d).as_bool() {
            let _ = Shell_NotifyIconW(NIM_MODIFY, &d);
        }
        if old != 0 {
            let _ = DestroyIcon(HICON(old as *mut _));
        }
    }
    Ok(())
}

pub fn remove() {
    let icon = ICON.swap(0, Ordering::SeqCst);
    // SAFETY: mesmo uID/janela do `add`; o HICON é nosso.
    unsafe {
        let _ = Shell_NotifyIconW(NIM_DELETE, &data(HICON(icon as *mut _)));
        if icon != 0 {
            let _ = DestroyIcon(HICON(icon as *mut _));
        }
    }
}
