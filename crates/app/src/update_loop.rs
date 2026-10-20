//! Atualização pelo GitHub Releases no laço de eventos (ADR-0014): liga a checagem, mostra a linha
//! "Atualizar para a x.y.z" no menu do Z e, depois de baixar e conferir, roda o instalador e sai.

use std::sync::Arc;
use windows::Win32::UI::WindowsAndMessaging::PostQuitMessage;
use windows::core::Result;
use zefiro_platform_windows::update::{UpdateService, UpdateState, run_installer};
use zefiro_platform_windows::window;

use crate::event_loop::{Loop, USER_UPDATE};

#[derive(Default)]
pub struct UpdateRuntime {
    service: Option<UpdateService>,
    /// Texto da linha do menu do Z (`None` = nada a mostrar).
    pub label: Option<String>,
}

/// O exe roda da instalação (`%LOCALAPPDATA%\Programs\Zefiro`), não de `target\release`.
fn installed_copy() -> bool {
    let (Ok(exe), Some(local)) = (std::env::current_exe(), std::env::var_os("LOCALAPPDATA")) else { return false };
    let install = std::path::PathBuf::from(local).join("Programs").join("Zefiro");
    exe.to_string_lossy().to_lowercase().starts_with(&install.to_string_lossy().to_lowercase())
}

/// Texto da linha do menu para cada estado.
fn label(state: &UpdateState) -> Option<String> {
    match state {
        UpdateState::Idle => None,
        UpdateState::Available(r) => Some(format!("Atualizar para a {}", r.version)),
        UpdateState::Downloading(r) | UpdateState::Ready(r, _) => Some(format!("Baixando a {}…", r.version)),
        UpdateState::Failed(..) => Some("Atualização falhou; tentar de novo".into()),
    }
}

impl Loop {
    /// Liga ou desliga a checagem conforme `[update] check`. Só a cópia instalada checa; o build de
    /// desenvolvimento, só com `ZEFIRO_UPDATE_CHECK=1`.
    pub fn update_services(&mut self) {
        let forced = std::env::var("ZEFIRO_UPDATE_CHECK").is_ok_and(|v| v == "1");
        let wanted = self.ctx.config.update.check && (installed_copy() || forced);
        match (wanted, self.update.service.is_some()) {
            (true, false) => {
                self.update.service = UpdateService::start(env!("CARGO_PKG_VERSION").into(), Arc::new(|| window::post_user(USER_UPDATE)))
                    .map_err(|e| tracing::warn!("checagem de atualização indisponível: {e}"))
                    .ok();
                tracing::info!("checagem de atualização ligada (GitHub Releases)");
            }
            (false, true) => {
                self.update.service = None;
                self.update.label = None;
            }
            _ => {}
        }
    }

    /// O serviço mudou de estado: atualiza o menu; pronto para instalar, roda o instalador e sai.
    pub fn update_changed(&mut self) -> Result<()> {
        let Some(state) = self.update.service.as_ref().map(UpdateService::state) else { return Ok(()) };
        if let UpdateState::Ready(r, path) = &state {
            if run_installer(path) {
                tracing::info!(versao = %r.version, "instalador da atualização aberto; saindo para ele trabalhar");
                // SAFETY: encerra o laço de mensagens desta thread pelo caminho normal (AppBars saem).
                unsafe { PostQuitMessage(0) };
                return Ok(());
            }
            tracing::warn!("o Windows não abriu o instalador {}", path.display());
            self.update.label = Some("Atualização falhou; tentar de novo".into());
        } else {
            self.update.label = label(&state);
        }
        self.redraw_open_panel()
    }

    /// Clique em "Atualizar para a x.y.z" (ou "tentar de novo").
    pub fn install_update(&mut self) -> Result<()> {
        if let Some(service) = &self.update.service {
            service.download();
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use zefiro_integration_update::{Asset, Release, Version};

    #[test]
    fn menu_label_follows_the_state() {
        let r = Release { version: Version(1, 1, 0), tag: "v1.1.0".into(), page: String::new(), asset: Asset { name: "Zefiro-1.1.0-setup.exe".into(), url: String::new(), size: 1, sha256: None } };
        assert_eq!(label(&UpdateState::Idle), None);
        assert_eq!(label(&UpdateState::Available(r.clone())).as_deref(), Some("Atualizar para a 1.1.0"));
        assert_eq!(label(&UpdateState::Downloading(r.clone())).as_deref(), Some("Baixando a 1.1.0…"));
        assert_eq!(label(&UpdateState::Failed(r, "x".into())).as_deref(), Some("Atualização falhou; tentar de novo"));
    }
}
