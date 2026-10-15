//! Monitores, primário primeiro.

use windows::Win32::Foundation::{LPARAM, RECT, TRUE};
use windows::Win32::Graphics::Gdi::*;
use windows::Win32::UI::HiDpi::{GetDpiForMonitor, MDT_EFFECTIVE_DPI};
use windows::Win32::UI::WindowsAndMessaging::MONITORINFOF_PRIMARY;
use windows::core::{BOOL, PCWSTR};
use zefiro_platform::geometry::Rect;

#[derive(Debug, Clone, PartialEq)]
pub struct Monitor { pub device: String, pub rect: Rect, pub dpi: u32, pub primary: bool, pub refresh_hz: u32 }

pub fn list() -> Vec<Monitor> {
    let mut out: Vec<Monitor> = Vec::new();

    unsafe extern "system" fn cb(h: HMONITOR, _: HDC, _: *mut RECT, data: LPARAM) -> BOOL {
        // SAFETY: `data` aponta para o Vec de `list`, vivo durante a enumeração.
        let out = unsafe { &mut *(data.0 as *mut Vec<Monitor>) };
        let mut mi = MONITORINFOEXW::default();
        mi.monitorInfo.cbSize = size_of::<MONITORINFOEXW>() as u32;
        // SAFETY: buffer do tamanho correto.
        if unsafe { GetMonitorInfoW(h, &mut mi as *mut _ as *mut MONITORINFO) }.as_bool() {
            let (mut dx, mut dy) = (96u32, 96u32);
            // SAFETY: ponteiros para u32 locais.
            let _ = unsafe { GetDpiForMonitor(h, MDT_EFFECTIVE_DPI, &mut dx, &mut dy) };
            let mut dm = DEVMODEW { dmSize: size_of::<DEVMODEW>() as u16, ..Default::default() };
            // SAFETY: `szDevice` é uma string terminada em zero dentro de `mi`.
            let hz = if unsafe { EnumDisplaySettingsW(PCWSTR(mi.szDevice.as_ptr()), ENUM_CURRENT_SETTINGS, &mut dm) }.as_bool() { dm.dmDisplayFrequency } else { 60 };
            let r = mi.monitorInfo.rcMonitor;
            out.push(Monitor {
                device: String::from_utf16_lossy(&mi.szDevice).trim_end_matches('\0').to_string(),
                rect: Rect::new(r.left as f32, r.top as f32, (r.right - r.left) as f32, (r.bottom - r.top) as f32),
                dpi: dx,
                primary: mi.monitorInfo.dwFlags & MONITORINFOF_PRIMARY != 0,
                refresh_hz: hz,
            });
        }
        TRUE
    }

    // SAFETY: o callback respeita o contrato de EnumDisplayMonitors.
    unsafe {
        let _ = EnumDisplayMonitors(None, None, Some(cb), LPARAM(&mut out as *mut _ as isize));
    }
    out.sort_by_key(|m| !m.primary);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lists_at_least_the_primary_monitor_first() {
        let ms = list();
        assert!(!ms.is_empty());
        assert!(ms[0].primary);
        assert!(ms[0].rect.w > 0.0 && ms[0].dpi >= 96);
    }
}
