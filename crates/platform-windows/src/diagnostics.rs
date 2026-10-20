//! Medidas para o HUD de debug e o exportar diagnóstico: memória do processo, tempos do DWM, o
//! atalho global, a pasta Downloads e a versão do Windows.

use std::path::{Path, PathBuf};
use windows::Win32::Foundation::HWND;
use windows::Win32::Graphics::Dwm::{DWM_TIMING_INFO, DwmGetCompositionTimingInfo};
use windows::Win32::System::ProcessStatus::{GetProcessMemoryInfo, PROCESS_MEMORY_COUNTERS, PROCESS_MEMORY_COUNTERS_EX};
use windows::Win32::System::Registry::{HKEY_LOCAL_MACHINE, RRF_RT_REG_DWORD, RRF_RT_REG_SZ, RegGetValueW};
use windows::Win32::System::Threading::GetCurrentProcess;
use windows::Win32::UI::Input::KeyboardAndMouse::{MOD_ALT, MOD_CONTROL, MOD_NOREPEAT, MOD_SHIFT, RegisterHotKey, UnregisterHotKey};
use windows::Win32::UI::Shell::{FOLDERID_Downloads, KF_FLAG_DEFAULT, SHGetKnownFolderPath, ShellExecuteW};
use windows::Win32::UI::WindowsAndMessaging::SW_SHOWNORMAL;
use windows::core::{HSTRING, w};

/// Memória privada e working set do próprio processo, em bytes.
pub fn process_memory() -> Option<(u64, u64)> {
    let mut c = PROCESS_MEMORY_COUNTERS_EX { cb: size_of::<PROCESS_MEMORY_COUNTERS_EX>() as u32, ..Default::default() };
    // SAFETY: a estrutura EX começa com a PROCESS_MEMORY_COUNTERS e `cb` diz o tamanho real; o
    // pseudo-handle do processo atual não precisa ser fechado.
    let ok = unsafe { GetProcessMemoryInfo(GetCurrentProcess(), &mut c as *mut PROCESS_MEMORY_COUNTERS_EX as *mut PROCESS_MEMORY_COUNTERS, c.cb) };
    ok.is_ok().then_some((c.PrivateUsage as u64, c.WorkingSetSize as u64))
}

/// Contadores do compositor da área de trabalho: quadros compostos, taxa do monitor e perdidos.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DwmTiming { pub frames: u64, pub refresh_hz: f32, pub dropped: u64 }

pub fn dwm_timing() -> Option<DwmTiming> {
    let mut info = DWM_TIMING_INFO { cbSize: size_of::<DWM_TIMING_INFO>() as u32, ..Default::default() };
    // SAFETY: `cbSize` preenchido; sem janela, o DWM devolve os tempos da composição inteira.
    unsafe { DwmGetCompositionTimingInfo(HWND::default(), &mut info) }.ok()?;
    let ratio = info.rateRefresh;
    let refresh_hz = if ratio.uiDenominator == 0 { 0.0 } else { ratio.uiNumerator as f32 / ratio.uiDenominator as f32 };
    Some(DwmTiming { frames: info.cFrame, refresh_hz, dropped: info.cFramesDropped + info.cFramesMissed })
}

/// Id do atalho do HUD (`Ctrl+Alt+Shift+D`).
pub const HOTKEY_HUD: i32 = 1;

/// Registra `Ctrl+Alt+Shift+D` na janela de mensagens. `false` se outro app já usa o atalho.
pub fn register_hud_hotkey() -> bool {
    // SAFETY: a janela de mensagens vive até o fim do app; o WM_HOTKEY chega no wndproc dela.
    unsafe { RegisterHotKey(Some(crate::window::message_window()), HOTKEY_HUD, MOD_CONTROL | MOD_ALT | MOD_SHIFT | MOD_NOREPEAT, u32::from(b'D')) }.is_ok()
}

pub fn unregister_hud_hotkey() {
    // SAFETY: desfazer um registro que pode não existir só falha.
    let _ = unsafe { UnregisterHotKey(Some(crate::window::message_window()), HOTKEY_HUD) };
}

/// Pasta Downloads do usuário.
pub fn downloads_dir() -> Option<PathBuf> {
    // SAFETY: o Windows aloca a string; `to_string` copia e o `CoTaskMemFree` a libera.
    unsafe {
        let p = SHGetKnownFolderPath(&FOLDERID_Downloads, KF_FLAG_DEFAULT, None).ok()?;
        let s = p.to_string().ok();
        windows::Win32::System::Com::CoTaskMemFree(Some(p.0 as *const _));
        s.map(PathBuf::from)
    }
}

/// Abre o Explorer com `file` selecionado.
pub fn reveal_file(file: &Path) {
    let args = HSTRING::from(format!("/select,\"{}\"", file.display()));
    // SAFETY: strings válidas durante a chamada.
    unsafe { ShellExecuteW(None, w!("open"), w!("explorer.exe"), &args, None, SW_SHOWNORMAL) };
}

fn reg_string(value: &str) -> Option<String> {
    let key = HSTRING::from(r"SOFTWARE\Microsoft\Windows NT\CurrentVersion");
    let mut buf = [0u16; 128];
    let mut size = (buf.len() * 2) as u32;
    // SAFETY: buffer e tamanho em bytes coerentes; a string volta terminada em zero.
    unsafe { RegGetValueW(HKEY_LOCAL_MACHINE, &key, &HSTRING::from(value), RRF_RT_REG_SZ, None, Some(buf.as_mut_ptr() as *mut _), Some(&mut size)) }.ok().ok()?;
    let len = buf.iter().position(|c| *c == 0).unwrap_or(buf.len());
    Some(String::from_utf16_lossy(&buf[..len]))
}

/// "Windows 11 Pro 24H2 (build 26200.1234)", do registro.
pub fn windows_version() -> String {
    let key = HSTRING::from(r"SOFTWARE\Microsoft\Windows NT\CurrentVersion");
    let mut ubr = 0u32;
    let mut size = 4u32;
    // SAFETY: DWORD de 4 bytes.
    let ubr = unsafe { RegGetValueW(HKEY_LOCAL_MACHINE, &key, w!("UBR"), RRF_RT_REG_DWORD, None, Some(&mut ubr as *mut u32 as *mut _), Some(&mut size)) }.is_ok().then_some(ubr);
    let build = reg_string("CurrentBuild").unwrap_or_default();
    // O registro ainda diz "Windows 10" no 11; o build ≥ 22000 é o 11.
    let product = reg_string("ProductName").unwrap_or_else(|| "Windows".into());
    let product = if build.parse::<u32>().is_ok_and(|b| b >= 22000) { product.replace("Windows 10", "Windows 11") } else { product };
    let display = reg_string("DisplayVersion").unwrap_or_default();
    let build = match ubr { Some(u) => format!("{build}.{u}"), None => build };
    format!("{product} {display} (build {build})").replace("  ", " ")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn own_memory_is_readable_and_plausible() {
        let (private, ws) = process_memory().expect("memória do processo");
        assert!(private > 1 << 20 && ws > 1 << 20, "{private} {ws}");
    }

    #[test]
    fn windows_version_and_downloads_are_known() {
        let v = windows_version();
        assert!(v.starts_with("Windows") && v.contains("build"), "{v}");
        assert!(downloads_dir().is_some_and(|d| d.is_absolute()));
    }

    #[test]
    fn dwm_reports_a_refresh_rate() {
        // Numa sessão sem DWM (serviço), a chamada falha; aqui só não pode dar valores absurdos.
        if let Some(t) = dwm_timing() {
            assert!((0.0..=1000.0).contains(&t.refresh_hz), "{t:?}");
        }
    }
}
