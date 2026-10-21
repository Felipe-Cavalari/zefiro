//! Notificações e Não perturbe na barra: o serviço de notificações (compartilhado com a ilha) e os
//! chips que agem direto no clique.

use std::sync::Arc;
use windows::core::Result;
use zefiro_platform_windows::dnd;
use zefiro_platform_windows::notifications::{NotificationWatcher, open_notification_center};
use zefiro_platform_windows::window;

use crate::event_loop::{Loop, MouseButton, USER_DND, USER_NOTIFICATION};

#[derive(Default)]
pub struct SystemRuntime {
    /// Lido pela ilha (avisos) e pelo chip (contador).
    pub notifications: Option<NotificationWatcher>,
}

impl Loop {
    /// Liga ou desliga o serviço de notificações: a ilha embutida ou o chip precisam dele. O Não
    /// perturbe é lido sempre (uma inscrição WNF por processo, sem consulta).
    pub fn system_services(&mut self) {
        dnd::watch(Arc::new(|| window::post_user(USER_DND)));
        self.ctx.system.dnd = dnd::active();
        let wanted = self.ctx.config.shows_embedded_island() || self.ctx.config.bar.uses("notifications");
        match (wanted, self.system.notifications.is_some()) {
            (true, false) => {
                self.system.notifications = NotificationWatcher::start(Arc::new(|| window::post_user(USER_NOTIFICATION)))
                    .map_err(|e| tracing::warn!("notificações indisponíveis: {e}"))
                    .ok();
            }
            (false, true) => self.system.notifications = None,
            _ => {}
        }
    }

    /// O Não perturbe mudou (por aqui ou pelo Windows).
    pub fn dnd_changed(&mut self) -> Result<()> {
        let active = dnd::active();
        if active != self.ctx.system.dnd {
            self.ctx.system.dnd = active;
            self.apply_bars(true)?;
        }
        self.island_dnd_changed()
    }

    /// Clique na lua liga ou desliga o Não perturbe; o botão do meio abre as configurações. A lua e o
    /// aviso da ilha seguem o WNF, como numa troca feita pelo Windows.
    pub fn dnd_action(&mut self, button: MouseButton) -> Result<()> {
        match button {
            MouseButton::Left => {
                let on = !dnd::active();
                tracing::info!(on, "Não perturbe pelo chip");
                if let Err(e) = dnd::set(on) {
                    tracing::warn!("não consegui alternar o Não perturbe: {e}; abrindo as configurações");
                    dnd::open_settings();
                }
            }
            MouseButton::Middle => dnd::open_settings(),
        }
        Ok(())
    }

    /// Reavalia os serviços dos chips depois de mudar a configuração (arquivo ou modo de edição).
    pub fn refresh_services(&mut self) {
        self.system_services();
        self.usage_services();
        self.update_services();
        self.chat_services();
        // Mídia e clima seguem a ilha embutida e os chips do player e do clima.
        if let Err(e) = self.island_services() {
            tracing::warn!("não consegui atualizar os serviços da ilha: {e}");
        }
    }

    pub fn notification_count(&self) -> usize {
        self.system.notifications.as_ref().map_or(0, NotificationWatcher::count)
    }

    /// Clique no sino: esquerdo abre a central; meio limpa.
    pub fn notifications_action(&mut self, button: MouseButton) -> Result<()> {
        match button {
            MouseButton::Left => {
                if let Err(e) = open_notification_center() {
                    tracing::warn!("não consegui abrir a central de notificações: {e}");
                }
            }
            MouseButton::Middle => {
                if let Some(w) = &self.system.notifications {
                    tracing::info!("limpando as notificações");
                    w.clear();
                }
            }
        }
        Ok(())
    }
}
