//! Monitores, primário primeiro.

use windows::Win32::Foundation::{LPARAM, RECT, TRUE};
use windows::Win32::Graphics::Gdi::*;
use windows::Win32::UI::HiDpi::{GetDpiForMonitor, MDT_EFFECTIVE_DPI};
use windows::Win32::UI::WindowsAndMessaging::MONITORINFOF_PRIMARY;
use windows::core::{BOOL, PCWSTR};
use zefiro_platform::geometry::Rect;
use zefiro_platform::monitor::{MonitorIdentity, reject_ambiguous};

#[derive(Debug, Clone, PartialEq)]
pub struct Monitor { pub device: String, pub identity: MonitorIdentity, pub rect: Rect, pub dpi: u32, pub primary: bool, pub refresh_hz: u32 }

fn decode(text: &[u16]) -> String {
    String::from_utf16_lossy(&text[..text.iter().position(|c| *c == 0).unwrap_or(text.len())])
}

fn identity(device: PCWSTR) -> MonitorIdentity {
    let mut found = Vec::new();
    for index in 0..128 {
        let mut display = DISPLAY_DEVICEW { cb: size_of::<DISPLAY_DEVICEW>() as u32, ..Default::default() };
        // SAFETY: `device` está vivo durante a enumeração; buffer inicializado e com tamanho correto.
        if !unsafe { EnumDisplayDevicesW(device, index, &mut display, 1) }.as_bool() { break; }
        if display.StateFlags.contains(DISPLAY_DEVICE_ACTIVE) {
            let id = decode(&display.DeviceID).to_lowercase();
            found.push(MonitorIdentity { id: (!id.is_empty()).then_some(id), name: decode(&display.DeviceString) });
        }
    }
    if found.len() == 1 { found.remove(0) } else { MonitorIdentity { id: None, name: "Monitor sem identificação única".into() } }
}

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
                identity: identity(PCWSTR(mi.szDevice.as_ptr())),
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
    let mut identities: Vec<_> = out.iter().map(|m| m.identity.clone()).collect();
    reject_ambiguous(&mut identities);
    for (monitor, identity) in out.iter_mut().zip(identities) { monitor.identity = identity; }
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
