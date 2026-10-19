//! Notificações e Não perturbe na barra: o serviço de notificações (compartilhado com a ilha) e os
//! chips que agem direto no clique.

use std::sync::Arc;
use windows::core::Result;
use zefiro_platform_windows::notifications::{NotificationWatcher, open_notification_center};
use zefiro_platform_windows::window;

use crate::event_loop::{Loop, MouseButton, USER_NOTIFICATION};

#[derive(Default)]
pub struct SystemRuntime {
    /// Lido pela ilha (avisos) e pelo chip (contador).
    pub notifications: Option<NotificationWatcher>,
}

impl Loop {
    /// Algum layout mostra o chip `id`?
    fn layout_has(&self, id: &str) -> bool {
        let bar = &self.ctx.config.bar;
        bar.sections.ids().any(|i| i == id) || bar.monitors.values().any(|m| m.sections.ids().any(|i| i == id))
    }

    /// Liga ou desliga o serviço de notificações: a ilha embutida ou o chip precisam dele.
    pub fn system_services(&mut self) {
        let wanted = self.ctx.config.island.embedded() || self.layout_has("notifications");
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
            MouseButton::Right => {}
        }
        Ok(())
    }
}
