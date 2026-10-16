//! Ícone do app de uma fonte de mídia, como a ilha C++ (`FindMediaSourceIcon`): apps da Store pelo
//! logo do pacote; os outros pelo ícone da janela do app (pega o ícone de PWAs nos navegadores) ou,
//! sem janela, pelo ícone do executável. Chamado na thread de mídia, nunca na de UI.

use std::collections::HashSet;
use windows::ApplicationModel::AppInfo;
use windows::Foundation::Size;
use windows::Win32::Foundation::{HWND, LPARAM, WPARAM};
use windows::Win32::Graphics::Gdi::{BI_RGB, BITMAPINFO, BITMAPINFOHEADER, CreateCompatibleDC, CreateDIBSection, DIB_RGB_COLORS, DeleteDC, DeleteObject, HGDIOBJ, SelectObject};
use windows::Win32::UI::Shell::ExtractIconExW;
use windows::Win32::UI::WindowsAndMessaging::{
    DI_MASK, DI_NORMAL, DestroyIcon, DrawIconEx, EnumWindows, GCLP_HICON, GW_OWNER, GetClassLongPtrW, GetWindow, GetWindowThreadProcessId, HICON, ICON_BIG, IsWindowVisible,
    SMTO_ABORTIFHUNG, SendMessageTimeoutW, WM_GETICON,
};
use windows::core::{BOOL, HSTRING, Interface};
use zefiro_platform::image::Image;

/// Ícone da fonte em `size` px, ou `None` se o app não tiver janela, executável ou logo legível.
pub fn source_icon(aumid: &str, app_name: &str, size: u32) -> Option<Image> {
    if aumid.contains('!') {
        return packaged_logo(aumid, size);
    }
    let processes = crate::audio_devices::source_processes(aumid, app_name);
    let pids: HashSet<u32> = processes.iter().flat_map(|(_, p)| p.iter().copied()).collect();
    if let Some(icon) = window_icon(&pids).and_then(|h| hicon_to_image(h, size)) {
        return Some(icon);
    }
    processes.iter().find(|(path, _)| !path.starts_with("pid:")).and_then(|(path, _)| exe_icon(path, size))
}

fn packaged_logo(aumid: &str, size: u32) -> Option<Image> {
    let info = AppInfo::GetFromAppUserModelId(&HSTRING::from(aumid)).ok()?;
    let logo = info.DisplayInfo().ok()?.GetLogo(Size { Width: size as f32, Height: size as f32 }).ok()?;
    crate::imaging::decode(&logo.cast().ok()?, size, false).ok().flatten()
}

struct Search<'a> { pids: &'a HashSet<u32>, found: HWND }

extern "system" fn visit(hwnd: HWND, lp: LPARAM) -> BOOL {
    // SAFETY: `lp` aponta para o `Search` vivo durante o EnumWindows abaixo.
    let search = unsafe { &mut *(lp.0 as *mut Search) };
    let mut pid = 0u32;
    // SAFETY: consultas de janela sem acesso à memória dela.
    let candidate = unsafe {
        GetWindowThreadProcessId(hwnd, Some(&mut pid));
        IsWindowVisible(hwnd).as_bool() && GetWindow(hwnd, GW_OWNER).is_err()
    };
    if candidate && search.pids.contains(&pid) {
        search.found = hwnd;
        return BOOL(0);
    }
    BOOL(1)
}

/// Ícone grande da primeira janela visível do app (sem dono: a janela principal).
fn window_icon(pids: &HashSet<u32>) -> Option<HICON> {
    if pids.is_empty() {
        return None;
    }
    let mut search = Search { pids, found: HWND::default() };
    // SAFETY: o callback só usa `search` durante a enumeração.
    let _ = unsafe { EnumWindows(Some(visit), LPARAM(&mut search as *mut Search as isize)) };
    if search.found.0.is_null() {
        return None;
    }
    let mut result = 0usize;
    // SAFETY: mensagem com tempo limite para não travar num app sem resposta; o ícone devolvido pertence à janela.
    unsafe {
        let _ = SendMessageTimeoutW(search.found, WM_GETICON, WPARAM(ICON_BIG as usize), LPARAM(0), SMTO_ABORTIFHUNG, 200, Some(&mut result));
        if result == 0 {
            result = GetClassLongPtrW(search.found, GCLP_HICON);
        }
    }
    (result != 0).then_some(HICON(result as *mut _))
}

fn exe_icon(path: &str, size: u32) -> Option<Image> {
    let mut large = HICON::default();
    // SAFETY: extrai um ícone do executável; é nosso e destruído em seguida.
    unsafe {
        if ExtractIconExW(&HSTRING::from(path), 0, Some(&mut large), None, 1) == 0 || large.is_invalid() {
            return None;
        }
        let image = hicon_to_image(large, size);
        let _ = DestroyIcon(large);
        image
    }
}

/// Desenha o ícone numa DIB de 32 bits e devolve BGRA pré-multiplicado. Ícones antigos sem alfa
/// usam a máscara para saber o que é transparente.
fn hicon_to_image(icon: HICON, size: u32) -> Option<Image> {
    let draw = |flags| -> Option<Vec<u8>> {
        // SAFETY: DIB e DC próprios, liberados antes de sair; os bits são copiados enquanto a DIB vive.
        unsafe {
            let header = BITMAPINFOHEADER { biSize: size_of::<BITMAPINFOHEADER>() as u32, biWidth: size as i32, biHeight: -(size as i32), biPlanes: 1, biBitCount: 32, biCompression: BI_RGB.0, ..Default::default() };
            let info = BITMAPINFO { bmiHeader: header, ..Default::default() };
            let dc = CreateCompatibleDC(None);
            let mut bits = std::ptr::null_mut();
            let Ok(dib) = CreateDIBSection(Some(dc), &info, DIB_RGB_COLORS, &mut bits, None, 0) else {
                let _ = DeleteDC(dc);
                return None;
            };
            let old = SelectObject(dc, HGDIOBJ(dib.0));
            let ok = DrawIconEx(dc, 0, 0, icon, size as i32, size as i32, 0, None, flags).is_ok();
            let pixels = ok.then(|| std::slice::from_raw_parts(bits as *const u8, (size * size * 4) as usize).to_vec());
            SelectObject(dc, old);
            let _ = DeleteObject(HGDIOBJ(dib.0));
            let _ = DeleteDC(dc);
            pixels
        }
    };
    let mut pixels = draw(DI_NORMAL)?;
    if pixels.as_chunks::<4>().0.iter().all(|p| p[3] == 0) {
        let mask = draw(DI_MASK)?;
        for (px, m) in pixels.as_chunks_mut::<4>().0.iter_mut().zip(mask.as_chunks::<4>().0) {
            // Máscara branca = transparente.
            if m[0] > 127 {
                *px = [0, 0, 0, 0];
            } else {
                px[3] = 255;
            }
        }
    }
    Image::new(size, size, pixels)
}
