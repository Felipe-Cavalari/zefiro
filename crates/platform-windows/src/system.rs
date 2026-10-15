//! Preferências do Windows que a barra respeita.

use std::path::{Path, PathBuf};
use windows::Win32::Foundation::{ERROR_ALREADY_EXISTS, GetLastError};
use windows::Win32::System::Registry::*;
use windows::Win32::System::Threading::CreateMutexW;
use windows::Win32::UI::Shell::ShellExecuteW;
use windows::Win32::UI::WindowsAndMessaging::*;
use windows::core::{BOOL, HSTRING, w};
use zefiro_platform::color::Rgba;

fn reg_dword(key: &str, value: &str) -> Option<u32> {
    let mut data = 0u32;
    let mut size = 4u32;
    // SAFETY: buffer de 4 bytes para um REG_DWORD.
    let r = unsafe {
        RegGetValueW(HKEY_CURRENT_USER, &HSTRING::from(key), &HSTRING::from(value), RRF_RT_REG_DWORD, None, Some(&mut data as *mut u32 as *mut _), Some(&mut size))
    };
    r.is_ok().then_some(data)
}

/// "Efeitos de animação" do Windows.
pub fn animations_enabled() -> bool {
    let mut on = BOOL(1);
    // SAFETY: SPI_GETCLIENTAREAANIMATION grava um BOOL.
    let _ = unsafe { SystemParametersInfoW(SPI_GETCLIENTAREAANIMATION, 0, Some(&mut on as *mut BOOL as *mut _), SYSTEM_PARAMETERS_INFO_UPDATE_FLAGS(0)) };
    on.as_bool()
}

/// "Efeitos de transparência" do Windows.
pub fn transparency_enabled() -> bool {
    reg_dword(r"Software\Microsoft\Windows\CurrentVersion\Themes\Personalize", "EnableTransparency").unwrap_or(1) != 0
}

/// Cor de destaque do Windows (DWM\AccentColor, formato ABGR).
pub fn system_accent() -> Rgba {
    let abgr = reg_dword(r"Software\Microsoft\Windows\DWM", "AccentColor").unwrap_or(0xFFD7_7800);
    Rgba::rgb((abgr & 0xFF) as u8, ((abgr >> 8) & 0xFF) as u8, ((abgr >> 16) & 0xFF) as u8)
}

pub fn open_folder(path: &Path) {
    // SAFETY: strings válidas durante a chamada.
    unsafe { ShellExecuteW(None, w!("open"), &HSTRING::from(path.as_os_str()), None, None, SW_SHOWNORMAL) };
}

pub fn exe_dir() -> PathBuf {
    std::env::current_exe().ok().and_then(|p| p.parent().map(Path::to_path_buf)).unwrap_or_else(|| PathBuf::from("."))
}

/// `true` se esta é a única instância (o mutex vive até o processo terminar).
pub fn single_instance(name: &str) -> bool {
    // SAFETY: cria um mutex nomeado; o handle é mantido aberto de propósito.
    unsafe {
        let _ = CreateMutexW(None, true, &HSTRING::from(name));
        GetLastError() != ERROR_ALREADY_EXISTS
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_system_preferences_without_failing() {
        let _ = animations_enabled();
        let _ = transparency_enabled();
        let a = system_accent();
        assert_eq!(a.a, 1.0);
        assert!(exe_dir().exists());
    }
}
