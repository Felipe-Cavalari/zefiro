//! Distros do WSL para o uso do Claude e do Codex. Ler `\\wsl.localhost\<distro>` de uma distro
//! parada a liga, então só as que já rodam viram origem.

use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::{Duration, Instant};
use windows::Win32::Foundation::ERROR_SUCCESS;
use windows::Win32::System::Registry::*;
use windows::core::{HSTRING, PWSTR};
use zefiro_integration_usage::origins::{Discovery, Origin, OriginKind, is_ignored_distro, proper_names, wsl_origins};

const LXSS: &str = r"Software\Microsoft\Windows\CurrentVersion\Lxss";
/// O Claude e o Codex consultam a cada 120 s: com este cache, os dois dividem um `wsl.exe`.
const RUNNING_TTL: Duration = Duration::from_secs(100);

static RUNNING: Mutex<Option<(Instant, Vec<String>)>> = Mutex::new(None);

fn reg_string(key: &str, value: &str) -> Option<String> {
    let mut buf = [0u16; 512];
    let mut size = (buf.len() * 2) as u32;
    // SAFETY: `buf` tem `size` bytes; o Windows grava uma string terminada em zero.
    let r = unsafe { RegGetValueW(HKEY_CURRENT_USER, &HSTRING::from(key), &HSTRING::from(value), RRF_RT_REG_SZ, None, Some(buf.as_mut_ptr().cast()), Some(&mut size)) };
    if r != ERROR_SUCCESS {
        return None;
    }
    let len = buf.iter().position(|&c| c == 0).unwrap_or(buf.len());
    Some(String::from_utf16_lossy(&buf[..len]))
}

/// Distros instaladas (`DistributionName` de cada chave em `Lxss`), sem ligar o WSL.
pub fn installed_distros() -> Vec<String> {
    let mut key = HKEY::default();
    // SAFETY: abre uma chave do usuário só para leitura; fechada abaixo.
    if unsafe { RegOpenKeyExW(HKEY_CURRENT_USER, &HSTRING::from(LXSS), None, KEY_READ, &mut key) } != ERROR_SUCCESS {
        return Vec::new();
    }
    let mut names = Vec::new();
    for index in 0.. {
        let mut buf = [0u16; 256];
        let mut len = buf.len() as u32;
        // SAFETY: `buf` tem `len` caracteres; a chave está aberta.
        let r = unsafe { RegEnumKeyExW(key, index, Some(PWSTR(buf.as_mut_ptr())), &mut len, None, None, None, None) };
        if r != ERROR_SUCCESS {
            break;
        }
        let sub = String::from_utf16_lossy(&buf[..len as usize]);
        if let Some(name) = reg_string(&format!(r"{LXSS}\{sub}"), "DistributionName") {
            names.push(name);
        }
    }
    // SAFETY: par do RegOpenKeyExW.
    let _ = unsafe { RegCloseKey(key) };
    names
}

/// Distros rodando, com o nome do registro e sem as do Docker. Um `wsl.exe` a cada 100 s, no máximo.
pub fn running_distros() -> Vec<String> {
    if let Ok(cache) = RUNNING.lock()
        && let Some((at, names)) = cache.as_ref()
        && at.elapsed() < RUNNING_TTL
    {
        return names.clone();
    }
    let names = proper_names(&crate::system::wsl_running_distros(), &installed_distros());
    if let Ok(mut cache) = RUNNING.lock() {
        *cache = Some((Instant::now(), names.clone()));
    }
    names
}

/// `\\wsl.localhost\<distro>`.
pub fn root(distro: &str) -> PathBuf {
    PathBuf::from(format!(r"\\wsl.localhost\{distro}"))
}

/// O Windows e cada pasta pessoal das distros rodando; `wsl_stopped` se há distro instalada parada.
pub fn discover(windows_home: &Path) -> Discovery {
    let running = running_distros();
    let wsl_stopped = installed_distros().iter().any(|d| !is_ignored_distro(d) && !running.iter().any(|r| r.eq_ignore_ascii_case(d)));
    let mut origins = vec![Origin { kind: OriginKind::Windows, home: windows_home.to_path_buf() }];
    origins.extend(wsl_origins(&running, &root));
    Discovery { origins, running, wsl_stopped }
}
