//! Demonstração da forma pílula → painel: clique alterna, Esc fecha.

use std::cell::Cell;
use windows::Win32::Foundation::*;
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::WindowsAndMessaging::*;
use windows::core::w;
use zefiro_platform::color::Rgba;
use zefiro_platform::geometry::Rect;
use zefiro_platform_windows::compositor::Comp;
use zefiro_platform_windows::gfx::Gfx;
use zefiro_platform_windows::morph::{MorphShape, MorphStyle};
use zefiro_ui::panel_layout::Shape;
use zefiro_ui::spring::SpringParams;

thread_local! {
    static CLICKED: Cell<bool> = const { Cell::new(false) };
}

extern "system" fn wndproc(hwnd: HWND, msg: u32, wp: WPARAM, lp: LPARAM) -> LRESULT {
    match msg {
        WM_LBUTTONUP => {
            CLICKED.with(|c| c.set(true));
            LRESULT(0)
        }
        WM_KEYDOWN if wp.0 == 0x1B => {
            // SAFETY: encerra o laço de mensagens desta thread.
            unsafe { PostQuitMessage(0) };
            LRESULT(0)
        }
        // SAFETY: mensagens não tratadas vão para o Windows.
        _ => unsafe { DefWindowProcW(hwnd, msg, wp, lp) },
    }
}

fn main() -> windows::core::Result<()> {
    // SAFETY: registro de classe e criação de janela com estruturas completas.
    let hwnd = unsafe {
        let instance = GetModuleHandleW(None)?;
        let wc = WNDCLASSEXW {
            cbSize: size_of::<WNDCLASSEXW>() as u32,
            lpfnWndProc: Some(wndproc),
            hInstance: instance.into(),
            lpszClassName: w!("ZefiroMorphDemo"),
            ..Default::default()
        };
        RegisterClassExW(&wc);
        let (sw, sh) = (GetSystemMetrics(SM_CXSCREEN), GetSystemMetrics(SM_CYSCREEN));
        CreateWindowExW(
            WS_EX_NOREDIRECTIONBITMAP | WS_EX_TOPMOST | WS_EX_TOOLWINDOW,
            w!("ZefiroMorphDemo"),
            w!("morph"),
            WS_POPUP | WS_VISIBLE,
            (sw - 600) / 2,
            (sh - 400) / 2,
            600,
            400,
            None,
            None,
            Some(instance.into()),
            None,
        )?
    };
    let gfx = Gfx::new()?;
    let comp = Comp::new(&gfx)?;
    let (_target, root) = comp.target(hwnd)?;
    root.SetSize(windows_numerics::Vector2 { X: 600.0, Y: 400.0 })?;
    let style = MorphStyle {
        fill: Rgba::rgb(8, 8, 10).with_alpha(0.96),
        depth: true,
        border: Some(Rgba::rgb(30, 30, 34)),
        border_px: 1.0,
        shadow: Some((Rgba::BLACK.with_alpha(0.55), 14.0)),
        backdrop: false,
    };
    let m = MorphShape::new(&comp, &root, &style)?;
    let pill = Shape { rect: Rect::new(240.0, 20.0, 120.0, 32.0), radius: 16.0 };
    let panel = Shape { rect: Rect::new(124.0, 58.0, 352.0, 186.0), radius: 24.0 };
    m.set(pill)?;
    let mut open = false;
    let mut msg = MSG::default();
    // SAFETY: laço de mensagens padrão.
    unsafe {
        while GetMessageW(&mut msg, None, 0, 0).as_bool() {
            let _ = TranslateMessage(&msg);
            DispatchMessageW(&msg);
            if CLICKED.with(|c| c.replace(false)) {
                open = !open;
                if open {
                    m.animate(&comp, panel, SpringParams::GROW)?;
                    m.nudge(&comp, -6.0, SpringParams::SETTLE)?;
                } else {
                    m.animate(&comp, pill, SpringParams::SHRINK)?;
                }
            }
        }
    }
    Ok(())
}
