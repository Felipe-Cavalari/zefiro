//! Windows Terminal: a pasta do pacote (ícones `ProfileIcons`) e o `wt.exe` que abre um perfil.
//! A leitura dos perfis é portável (`zefiro-integration-terminal`).

use std::path::PathBuf;
use windows::Win32::Foundation::{ERROR_INSUFFICIENT_BUFFER, ERROR_SUCCESS};
use windows::Win32::Storage::Packaging::Appx::{GetPackagePathByFullName, GetPackagesByPackageFamily};
use windows::core::{HSTRING, PWSTR};

const FAMILIES: [&str; 2] = ["Microsoft.WindowsTerminal_8wekyb3d8bbwe", "Microsoft.WindowsTerminalPreview_8wekyb3d8bbwe"];

/// Pasta de instalação do Terminal (Stable, depois Preview), ou `None` sem pacote.
pub fn package_dir() -> Option<PathBuf> {
    FAMILIES.iter().find_map(|family| full_name(family).and_then(|name| package_path(&name)))
}

/// Primeiro pacote instalado da família.
fn full_name(family: &str) -> Option<String> {
    let family = HSTRING::from(family);
    let (mut count, mut length) = (0u32, 0u32);
    // SAFETY: primeira chamada só pede os tamanhos (ponteiros nulos para as listas).
    let first = unsafe { GetPackagesByPackageFamily(&family, &mut count, None, &mut length, None) };
    if first != ERROR_INSUFFICIENT_BUFFER || count == 0 {
        return None;
    }
    let mut names = vec![PWSTR::null(); count as usize];
    let mut buffer = vec![0u16; length as usize];
    // SAFETY: `names` tem `count` posições e `buffer` tem `length` u16, como a primeira chamada pediu.
    let second = unsafe { GetPackagesByPackageFamily(&family, &mut count, Some(names.as_mut_ptr()), &mut length, Some(PWSTR(buffer.as_mut_ptr()))) };
    if second != ERROR_SUCCESS {
        return None;
    }
    // SAFETY: cada nome aponta para dentro de `buffer`, terminado em zero, e `buffer` ainda vive.
    names.first().and_then(|n| unsafe { n.to_string() }.ok())
}

fn package_path(full_name: &str) -> Option<PathBuf> {
    let name = HSTRING::from(full_name);
    let mut length = 0u32;
    // SAFETY: primeira chamada só pede o tamanho.
    let first = unsafe { GetPackagePathByFullName(&name, &mut length, None) };
    if first != ERROR_INSUFFICIENT_BUFFER {
        return None;
    }
    let mut buffer = vec![0u16; length as usize];
    // SAFETY: `buffer` tem os `length` u16 pedidos.
    let second = unsafe { GetPackagePathByFullName(&name, &mut length, Some(PWSTR(buffer.as_mut_ptr()))) };
    if second != ERROR_SUCCESS {
        return None;
    }
    let end = buffer.iter().position(|&c| c == 0).unwrap_or(buffer.len());
    Some(PathBuf::from(String::from_utf16_lossy(&buffer[..end])))
}

/// O alias `%LOCALAPPDATA%\Microsoft\WindowsApps\wt.exe`. É um ponto de nova análise que
/// `Path::exists` não segue (daria `false`), por isso `symlink_metadata`.
pub fn wt_exe() -> Option<PathBuf> {
    let path = PathBuf::from(std::env::var_os("LOCALAPPDATA")?).join(r"Microsoft\WindowsApps\wt.exe");
    std::fs::symlink_metadata(&path).is_ok().then_some(path)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Sonda na máquina: `cargo test -p zefiro-platform-windows windows_terminal -- --ignored --nocapture`.
    #[test]
    #[ignore]
    fn probe_package_wt_and_icon() {
        let dir = package_dir().expect("pacote do Terminal");
        println!("pacote: {}", dir.display());
        println!("wt: {:?}", wt_exe().expect("alias do wt.exe"));
        let icon = crate::imaging::decode_file(&dir.join(r"ProfileIcons\{0caa0dad-35be-5f56-a8ff-afceeeaa6101}.scale-200.png"), 36).expect("ícone do cmd");
        println!("ícone: {}x{}", icon.0.width, icon.0.height);
        assert_eq!((icon.0.width, icon.0.height), (36, 36));
        // `.ico` do WSL, se houver uma distro.
        let wsl = PathBuf::from(std::env::var_os("LOCALAPPDATA").unwrap()).join("wsl");
        let ico = std::fs::read_dir(wsl).into_iter().flatten().flatten().map(|e| e.path().join("shortcut.ico")).find(|p| p.is_file());
        if let Some(ico) = ico {
            let image = crate::imaging::decode_file(&ico, 36).expect("ícone .ico");
            println!("ico: {}x{}", image.0.width, image.0.height);
        }
    }
}
