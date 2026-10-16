use zefiro_platform::geometry::Rect;
use zefiro_platform_windows::window;
use windows::Win32::Foundation::{LPARAM, WPARAM};
use windows::Win32::UI::WindowsAndMessaging::{SendMessageW, WM_LBUTTONDOWN};

#[test]
fn press_is_delivered_as_owner_and_capture_is_released_before_destruction() {
    window::register_classes().unwrap();
    let hwnd = window::create(window::WindowKind::Bar, Rect::new(-4000.0, -4000.0, 100.0, 50.0)).unwrap();
    let input = window::input_window(hwnd).unwrap();
    // SAFETY: mensagem síncrona para a janela de teste criada nesta thread.
    unsafe { SendMessageW(input, WM_LBUTTONDOWN, Some(WPARAM(0)), Some(LPARAM(10 | (15 << 16)))) };
    assert!(window::take_events().contains(&window::WinEvent::LeftDown { hwnd: hwnd.0 as isize, x: 10.0, y: 15.0 }));
    window::capture(hwnd);
    assert!(window::has_capture(hwnd));
    window::release_capture();
    assert!(!window::has_capture(hwnd));
    window::destroy(hwnd);
}

#[test]
fn native_format_field_preserves_unicode_and_notifies_its_owner() {
    window::register_classes().unwrap();
    let hwnd = window::create(window::WindowKind::Panel, Rect::new(-4000.0, -4000.0, 300.0, 100.0)).unwrap();
    let field = zefiro_platform_windows::text_input::TextInput::new(hwnd, Rect::new(10.0, 10.0, 220.0, 32.0), "ddd '🌬️' MMM", zefiro_platform::color::Rgba::BLACK, zefiro_platform::color::Rgba::WHITE).unwrap();
    assert_eq!(field.text(), "ddd '🌬️' MMM");
    // O controle GDI precisa de uma janela com redirecionamento; caso contrário existe e recebe
    // texto, mas fica invisível atrás da superfície do compositor (regressão encontrada na tela).
    // SAFETY: consulta estilos da raiz do controle de teste.
    unsafe {
        use windows::Win32::UI::WindowsAndMessaging::*;
        let root = GetAncestor(windows::Win32::Foundation::HWND(field.handle() as *mut _), GA_ROOT);
        assert_eq!(GetWindowLongPtrW(root, GWL_EXSTYLE) as u32 & WS_EX_NOREDIRECTIONBITMAP.0, 0);
    }
    field.set_text("dddd 'às' d");
    assert_eq!(field.text(), "dddd 'às' d");
    assert!(window::take_events().iter().any(|event| matches!(event, window::WinEvent::ControlChanged { hwnd: raw, .. } if *raw == hwnd.0 as isize)));
    drop(field);
    window::destroy(hwnd);
}
