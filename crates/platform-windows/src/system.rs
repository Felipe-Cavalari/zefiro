//! Preferências do Windows que a barra respeita.

use std::path::{Path, PathBuf};
use windows::Win32::Foundation::{ERROR_ALREADY_EXISTS, GetLastError};
use windows::Win32::System::Registry::*;
use windows::Win32::System::Threading::CreateMutexW;
use windows::Win32::UI::Shell::ShellExecuteW;
use windows::Win32::UI::WindowsAndMessaging::*;
use windows::core::{BOOL, HSTRING, w};
use zefiro_platform::color::Rgba;
use zefiro_platform::geometry::Rect;

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

/// Abre uma URI do Windows (`ms-settings:…`) com o app associado.
pub fn open_uri(uri: &str) {
    // SAFETY: strings válidas durante a chamada.
    unsafe { ShellExecuteW(None, w!("open"), &HSTRING::from(uri), None, None, SW_SHOWNORMAL) };
}

pub fn exe_dir() -> PathBuf {
    std::env::current_exe().ok().and_then(|p| p.parent().map(Path::to_path_buf)).unwrap_or_else(|| PathBuf::from("."))
}

/// Liga DPI por monitor (Per-Monitor v2) no processo. Precisa vir antes de criar qualquer janela;
/// sem isso o Windows estica a barra em bitmap nos monitores fora de 100% e não manda
/// `WM_DPICHANGED`. Devolve `true` se o processo está (ou já estava) em Per-Monitor v2.
pub fn enable_per_monitor_dpi() -> bool {
    use windows::Win32::UI::HiDpi::{AreDpiAwarenessContextsEqual, DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2, GetThreadDpiAwarenessContext, SetProcessDpiAwarenessContext};
    // SAFETY: chamadas sem ponteiros; falhar (já definido) é tratado pela conferência abaixo.
    unsafe {
        let _ = SetProcessDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2);
        AreDpiAwarenessContextsEqual(GetThreadDpiAwarenessContext(), DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2).as_bool()
    }
}

/// Uma janela comum (não a área de trabalho, a barra de tarefas nem o Zefiro) cobrindo o monitor inteiro.
pub fn is_fullscreen(class: &str, window: Rect, monitor: Rect) -> bool {
    // A área de trabalho e a barra de tarefas cobrem o monitor sem ser app; as janelas do próprio
    // Zefiro também (o painel aberto tem uma janela de entrada do tamanho do monitor, com o foco).
    if matches!(class, "Progman" | "WorkerW" | "Shell_TrayWnd" | "Shell_SecondaryTrayWnd" | "ZefiroInput" | "ZefiroPanel" | "ZefiroBar") {
        return false;
    }
    window.x <= monitor.x && window.y <= monitor.y && window.right() >= monitor.right() && window.bottom() >= monitor.bottom()
}

/// Classe e programa da janela em primeiro plano ("Chrome_WidgetWin_1 · brave.exe"), para o log.
pub fn foreground_description() -> String {
    use windows::Win32::System::Threading::{OpenProcess, PROCESS_NAME_WIN32, PROCESS_QUERY_LIMITED_INFORMATION, QueryFullProcessImageNameW};
    // SAFETY: buffers locais; o handle do processo é fechado logo depois da leitura.
    unsafe {
        let hwnd = GetForegroundWindow();
        let mut buf = [0u16; 128];
        let n = GetClassNameW(hwnd, &mut buf);
        let class = String::from_utf16_lossy(&buf[..n.max(0) as usize]);
        let mut pid = 0u32;
        GetWindowThreadProcessId(hwnd, Some(&mut pid));
        let mut exe = String::new();
        if let Ok(process) = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid) {
            let mut path = [0u16; 260];
            let mut len = path.len() as u32;
            if QueryFullProcessImageNameW(process, PROCESS_NAME_WIN32, windows::core::PWSTR(path.as_mut_ptr()), &mut len).is_ok() {
                let full = String::from_utf16_lossy(&path[..len as usize]);
                exe = full.rsplit('\\').next().unwrap_or_default().to_string();
            }
            let _ = windows::Win32::Foundation::CloseHandle(process);
        }
        format!("{class} · {exe}")
    }
}

/// A janela em primeiro plano está em tela cheia em `monitor`?
pub fn foreground_fullscreen_on(monitor: Rect) -> bool {
    use windows::Win32::Foundation::RECT;
    // SAFETY: buffers locais; a janela pode sumir entre as chamadas, e aí as funções falham.
    unsafe {
        let hwnd = GetForegroundWindow();
        if hwnd.is_invalid() {
            return false;
        }
        let mut buf = [0u16; 128];
        let n = GetClassNameW(hwnd, &mut buf);
        let class = String::from_utf16_lossy(&buf[..n.max(0) as usize]);
        let mut r = RECT::default();
        if GetWindowRect(hwnd, &mut r).is_err() {
            return false;
        }
        let rect = Rect::new(r.left as f32, r.top as f32, (r.right - r.left) as f32, (r.bottom - r.top) as f32);
        is_fullscreen(&class, rect, monitor)
    }
}

/// `true` se esta é a única instância (o mutex vive até o processo terminar).
pub fn single_instance(name: &str) -> bool {
    // SAFETY: cria um mutex nomeado; o handle é mantido aberto de propósito.
    unsafe {
        let _ = CreateMutexW(None, true, &HSTRING::from(name));
        GetLastError() != ERROR_ALREADY_EXISTS
    }
}

/// Abre um programa de janela sem console e sem prender o Zefiro a ele.
pub fn spawn_detached(exe: &Path, args: &[String]) -> std::io::Result<()> {
    use std::os::windows::process::CommandExt;
    const DETACHED_PROCESS: u32 = 0x0000_0008;
    std::process::Command::new(exe).args(args).creation_flags(DETACHED_PROCESS).spawn().map(|_| ())
}

/// Barra de tarefas escura (o padrão do Windows 11)? Define a cor do ícone da bandeja.
pub fn taskbar_dark() -> bool {
    reg_dword(r"Software\Microsoft\Windows\CurrentVersion\Themes\Personalize", "SystemUsesLightTheme").unwrap_or(0) == 0
}

/// Distros do WSL que já estão rodando (minúsculas). Consultar `\\wsl.localhost` de uma distro
/// parada a liga, então só se confere caminho nas desta lista. Sem WSL, lista vazia.
pub fn wsl_running_distros() -> Vec<String> {
    use std::os::windows::process::CommandExt;
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;
    let Ok(out) = std::process::Command::new("wsl.exe").args(["--list", "--running", "--quiet"]).creation_flags(CREATE_NO_WINDOW).output() else { return Vec::new() };
    if !out.status.success() {
        return Vec::new();
    }
    // O wsl.exe escreve em UTF-16LE.
    let wide: Vec<u16> = out.stdout.as_chunks::<2>().0.iter().map(|b| u16::from_le_bytes(*b)).collect();
    String::from_utf16_lossy(&wide).lines().map(|l| l.trim().trim_matches('\0').to_lowercase()).filter(|l| !l.is_empty()).collect()
}

/// O `Code.exe` do VS Code: ao lado do `bin\code.cmd` que estiver no PATH, senão nas pastas padrão
/// de instalação (do usuário e do sistema).
pub fn find_code_exe() -> Option<PathBuf> {
    let from_path = std::env::var_os("PATH").into_iter().flat_map(|p| std::env::split_paths(&p).collect::<Vec<_>>()).find_map(|dir| {
        dir.join("code.cmd").is_file().then(|| dir.parent().map(|p| p.join("Code.exe"))).flatten()
    });
    let defaults = [("LOCALAPPDATA", r"Programs\Microsoft VS Code\Code.exe"), ("ProgramFiles", r"Microsoft VS Code\Code.exe")]
        .into_iter()
        .filter_map(|(var, rel)| std::env::var_os(var).map(|base| PathBuf::from(base).join(rel)));
    from_path.into_iter().chain(defaults).find(|p| p.is_file())
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

    #[test]
    fn fullscreen_means_a_normal_window_covering_the_whole_monitor() {
        let mon = Rect::new(0.0, 0.0, 1920.0, 1080.0);
        assert!(is_fullscreen("Chrome_WidgetWin_1", Rect::new(0.0, 0.0, 1920.0, 1080.0), mon));
        assert!(is_fullscreen("UnityWndClass", Rect::new(-8.0, -8.0, 1936.0, 1096.0), mon), "maior que o monitor também cobre");
        assert!(!is_fullscreen("Chrome_WidgetWin_1", Rect::new(0.0, 32.0, 1920.0, 1048.0), mon), "maximizada abaixo da barra não é tela cheia");
        assert!(!is_fullscreen("Chrome_WidgetWin_1", Rect::new(1920.0, 0.0, 1920.0, 1080.0), mon), "outro monitor");
        for shell in ["Progman", "WorkerW", "Shell_TrayWnd"] {
            assert!(!is_fullscreen(shell, mon, mon), "{shell} é a área de trabalho, não um app");
        }
        // O painel aberto tem uma janela de entrada do tamanho do monitor com o foco: não é tela cheia.
        for ours in ["ZefiroInput", "ZefiroPanel", "ZefiroBar"] {
            assert!(!is_fullscreen(ours, mon, mon), "{ours} é do próprio Zefiro");
        }
    }

    #[test]
    fn process_becomes_per_monitor_dpi_aware_v2() {
        assert!(enable_per_monitor_dpi(), "a barra precisa de DPI por monitor (Per-Monitor v2)");
        // Chamar de novo (já ligado) continua dizendo que está ligado.
        assert!(enable_per_monitor_dpi());
    }
}
