//! Energia: bloquear, suspender, sair, reiniciar e desligar; tempo ligado; nome e foto da conta.

pub use zefiro_platform::power::PowerAction;
use std::time::Duration;
use windows::Foundation::IReference;
use windows::System::{KnownUserProperties, User, UserPictureSize};
use windows::Win32::Foundation::{CloseHandle, HANDLE, LUID};
use windows::Win32::Security::Authentication::Identity::{GetUserNameExW, NameDisplay};
use windows::Win32::Security::{AdjustTokenPrivileges, LUID_AND_ATTRIBUTES, LookupPrivilegeValueW, SE_PRIVILEGE_ENABLED, TOKEN_ADJUST_PRIVILEGES, TOKEN_PRIVILEGES, TOKEN_QUERY};
use windows::Win32::System::Power::SetSuspendState;
use windows::Win32::System::Shutdown::{EWX_LOGOFF, EWX_POWEROFF, EWX_REBOOT, EXIT_WINDOWS_FLAGS, ExitWindowsEx, LockWorkStation, SHTDN_REASON_FLAG_PLANNED, SHTDN_REASON_MAJOR_OTHER};
use windows::Win32::System::SystemInformation::GetTickCount64;
use windows::Win32::System::Threading::{GetCurrentProcess, OpenProcessToken};
use windows::core::{HSTRING, Interface, PWSTR, Result, w};
use zefiro_platform::image::Image;

/// Liga o privilégio de desligamento no token do processo (exigido por `ExitWindowsEx` para
/// reiniciar e desligar).
fn enable_shutdown_privilege() -> Result<()> {
    // SAFETY: token aberto e fechado aqui; estruturas locais.
    unsafe {
        let mut token = HANDLE::default();
        OpenProcessToken(GetCurrentProcess(), TOKEN_ADJUST_PRIVILEGES | TOKEN_QUERY, &mut token)?;
        let mut luid = LUID::default();
        let result = LookupPrivilegeValueW(None, w!("SeShutdownPrivilege"), &mut luid).and_then(|_| {
            let privileges = TOKEN_PRIVILEGES { PrivilegeCount: 1, Privileges: [LUID_AND_ATTRIBUTES { Luid: luid, Attributes: SE_PRIVILEGE_ENABLED }] };
            AdjustTokenPrivileges(token, false, Some(&privileges), 0, None, None)
        });
        let _ = CloseHandle(token);
        result
    }
}

fn exit_windows(flags: EXIT_WINDOWS_FLAGS) -> Result<()> {
    // SAFETY: sem ponteiros; não força o fechamento dos apps (sem EWX_FORCE).
    unsafe { ExitWindowsEx(flags, SHTDN_REASON_MAJOR_OTHER | SHTDN_REASON_FLAG_PLANNED) }
}

/// Executa a ação. Sair, reiniciar e desligar pedem aos apps para fechar (sem forçar).
pub fn run(a: PowerAction) -> Result<()> {
    match a {
        // SAFETY: sem parâmetros.
        PowerAction::Lock => unsafe { LockWorkStation() },
        PowerAction::Sleep => {
            // SAFETY: sem ponteiros. Suspender (não hibernar), sem forçar, com eventos de despertar.
            let ok = unsafe { SetSuspendState(false, false, false) };
            if ok { Ok(()) } else { Err(windows::core::Error::from_win32()) }
        }
        PowerAction::SignOut => exit_windows(EWX_LOGOFF),
        PowerAction::Restart => enable_shutdown_privilege().and_then(|_| exit_windows(EWX_REBOOT)),
        PowerAction::Shutdown => enable_shutdown_privilege().and_then(|_| exit_windows(EWX_POWEROFF)),
    }
}

/// Tempo desde que o Windows ligou.
pub fn uptime() -> Duration {
    // SAFETY: sem parâmetros.
    Duration::from_millis(unsafe { GetTickCount64() })
}

fn display_name_win32() -> Option<String> {
    let mut buf = vec![0u16; 256];
    let mut len = buf.len() as u32;
    // SAFETY: buffer local com o tamanho informado em `len`.
    let ok = unsafe { GetUserNameExW(NameDisplay, Some(PWSTR(buf.as_mut_ptr())), &mut len) };
    (ok && len > 0).then(|| String::from_utf16_lossy(&buf[..len as usize])).filter(|s| !s.trim().is_empty())
}

/// Nome de exibição e foto (64 px) da conta. Chamar fora da thread de UI: a API do Windows é
/// assíncrona e pode levar alguns ms. Sem acesso às informações da conta, cai para o nome do
/// Win32 e depois para `USERNAME`, sem foto.
pub fn user_info() -> (String, Option<Image>) {
    let user = User::FindAllAsync().and_then(|op| op.get()).ok().and_then(|users| users.GetAt(0).ok());
    let name = user
        .as_ref()
        .and_then(|u| u.GetPropertyAsync(&KnownUserProperties::DisplayName().ok()?).ok()?.get().ok())
        .and_then(|v| v.cast::<IReference<HSTRING>>().ok()?.Value().ok())
        .map(|s| s.to_string())
        .filter(|s| !s.trim().is_empty())
        .or_else(display_name_win32)
        .or_else(|| std::env::var("USERNAME").ok())
        .unwrap_or_default();
    let picture = user
        .and_then(|u| u.GetPictureAsync(UserPictureSize::Size64x64).ok()?.get().ok())
        .and_then(|reference| crate::imaging::decode(&reference, 64, true).ok().flatten());
    (name, picture)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn uptime_is_positive() {
        assert!(uptime() > std::time::Duration::ZERO);
    }

    #[test]
    fn user_has_a_name() {
        let (name, _) = user_info();
        assert!(!name.trim().is_empty());
    }
}
