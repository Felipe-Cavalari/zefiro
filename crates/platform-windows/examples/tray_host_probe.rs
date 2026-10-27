//! Sonda da bandeja (ADR-0023), sem mexer no mouse.
//!
//! `cargo run -p zefiro-platform-windows --example tray_host_probe` sobe o host, espera 10 s e lista
//! os ícones (dica, exe, tamanho da imagem) e quem é a primeira `Shell_TrayWnd`.
//! Com `--check`, não sobe host nenhum: só diz de que processo é a primeira `Shell_TrayWnd`.

use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use windows::Win32::System::Threading::{OpenProcess, PROCESS_NAME_WIN32, PROCESS_QUERY_LIMITED_INFORMATION, QueryFullProcessImageNameW};
use windows::Win32::UI::WindowsAndMessaging::{FindWindowW, GetWindowThreadProcessId};
use windows::core::{PCWSTR, PWSTR, w};
use zefiro_platform_windows::tray_host::TrayHost;

/// Executável dono da primeira `Shell_TrayWnd` (`explorer.exe`, ou este processo com o host no ar).
fn first_owner() -> String {
    // SAFETY: só consultas; o handle do processo é do sistema e não precisa ser fechado aqui.
    unsafe {
        let Ok(h) = FindWindowW(w!("Shell_TrayWnd"), PCWSTR::null()) else { return "nenhuma".into() };
        let mut pid = 0u32;
        GetWindowThreadProcessId(h, Some(&mut pid));
        let Ok(process) = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid) else { return format!("pid {pid}") };
        let mut buf = [0u16; 260];
        let mut len = buf.len() as u32;
        if QueryFullProcessImageNameW(process, PROCESS_NAME_WIN32, PWSTR(buf.as_mut_ptr()), &mut len).is_err() {
            return format!("pid {pid}");
        }
        let path = String::from_utf16_lossy(&buf[..len as usize]);
        path.rsplit('\\').next().unwrap_or_default().to_string()
    }
}

/// Janelas de menu visíveis e o processo de cada uma.
fn menus() -> String {
    use windows::Win32::Foundation::{HWND, LPARAM};
    use windows::Win32::UI::WindowsAndMessaging::{EnumWindows, GetClassNameW, IsWindowVisible};
    use windows::core::BOOL;
    unsafe extern "system" fn each(h: HWND, l: LPARAM) -> BOOL {
        // SAFETY: `l` é o Vec passado por `menus`, vivo durante o EnumWindows.
        let out = unsafe { &mut *(l.0 as *mut Vec<u32>) };
        let mut buf = [0u16; 16];
        // SAFETY: só consultas.
        let n = unsafe { GetClassNameW(h, &mut buf) } as usize;
        if String::from_utf16_lossy(&buf[..n]) == "#32768" && unsafe { IsWindowVisible(h) }.as_bool() {
            let mut pid = 0u32;
            // SAFETY: só consulta.
            unsafe { GetWindowThreadProcessId(h, Some(&mut pid)) };
            out.push(pid);
        }
        BOOL(1)
    }
    let mut pids: Vec<u32> = Vec::new();
    // SAFETY: callback síncrono com ponteiro para um Vec local.
    let _ = unsafe { EnumWindows(Some(each), LPARAM(&mut pids as *mut _ as isize)) };
    format!("{pids:?}")
}

fn main() -> windows::core::Result<()> {
    if std::env::args().any(|a| a == "--check") {
        println!("primeira Shell_TrayWnd: {}", first_owner());
        return Ok(());
    }
    let changes = Arc::new(AtomicUsize::new(0));
    let counter = changes.clone();
    let host = TrayHost::start(Arc::new(move || { counter.fetch_add(1, Ordering::SeqCst); }), 32)?;
    std::thread::sleep(std::time::Duration::from_secs(10));
    println!("primeira Shell_TrayWnd: {} · avisos de mudança: {}", first_owner(), changes.load(Ordering::SeqCst));
    host.prune();
    let items = host.snapshot();
    println!("{} ícones:", items.len());
    for i in &items {
        let size = i.image.as_ref().map_or("sem imagem".into(), |im| format!("{}x{}", im.width(), im.height()));
        println!("  {:<40} {:<32} {size} {:?}", format!("{:?}", i.tip.lines().next().unwrap_or_default()), i.exe, i.key);
    }
    // `--click <trecho do exe> left|right`: manda o clique e diz o que apareceu.
    let args: Vec<String> = std::env::args().collect();
    if let Some(i) = args.iter().position(|a| a == "--click") {
        let target = args.get(i + 1).cloned().unwrap_or_default().to_lowercase();
        let button = if args.get(i + 2).map(String::as_str) == Some("right") { zefiro_integration_systray::click::Button::Right } else { zefiro_integration_systray::click::Button::Left };
        if let Some(item) = items.iter().find(|it| it.exe.to_lowercase().contains(&target)) {
            println!("clique {button:?} em {}", item.exe);
            host.click(&item.key, button, zefiro_platform_windows::tray_host::cursor());
            std::thread::sleep(std::time::Duration::from_millis(1500));
            println!("menus visíveis (#32768): {}", menus());
        }
    }
    drop(host);
    println!("depois de fechar o host: {}", first_owner());
    Ok(())
}
