//! Painel "Energia" no laço de eventos: conta e tempo ligado, confirmação por segundo clique e as
//! ações. Sair, reiniciar e desligar fecham a barra antes (a AppBar é liberada) e só então rodam.

use std::sync::{Arc, Mutex};
use windows::Win32::UI::WindowsAndMessaging::PostQuitMessage;
use windows::core::Result;
use zefiro_bar::controller::PanelKind;
use zefiro_bar::power::{ConfirmState, PowerAction, format_uptime, power_panel};
use zefiro_platform::geometry::{Point, Rect};
use zefiro_platform::image::Image;
use zefiro_platform_windows::{power, window};
use zefiro_ui::paint::DisplayList;

use crate::event_loop::{Loop, TIMER_POWER, USER_POWER};

type UserInfo = (String, Option<Image>);

#[derive(Default)]
pub struct PowerRuntime {
    confirm: ConfirmState,
    user: Option<UserInfo>,
    incoming: Arc<Mutex<Option<UserInfo>>>,
    hits: Vec<(Rect, PowerAction)>,
    /// Ação que roda depois que a barra sair.
    pending: Option<PowerAction>,
}

fn dry_run() -> bool {
    std::env::var("ZEFIRO_POWER_DRY_RUN").is_ok_and(|v| v == "1")
}

/// Roda a ação (ou só registra, com `ZEFIRO_POWER_DRY_RUN=1`).
fn execute(a: PowerAction) {
    if dry_run() {
        tracing::info!(?a, "ação de energia (simulada)");
        return;
    }
    tracing::info!(?a, "ação de energia");
    if let Err(e) = power::run(a) {
        tracing::warn!(?a, "ação de energia falhou: {e}");
    }
}

/// Chamado por `run` depois que as barras saíram.
pub fn run_pending(l: &mut Loop) {
    if let Some(a) = l.power.pending.take() {
        execute(a);
    }
}

impl Loop {
    /// Ao abrir o painel: nome e foto da conta numa thread (só na primeira vez).
    pub fn power_prepare(&mut self, kind: PanelKind) {
        if kind != PanelKind::Power || self.power.user.is_some() {
            return;
        }
        let incoming = self.power.incoming.clone();
        let spawned = std::thread::Builder::new().name("zefiro-user".into()).spawn(move || {
            let info = power::user_info();
            if let Ok(mut slot) = incoming.lock() {
                *slot = Some(info);
            }
            window::post_user(USER_POWER);
        });
        if let Err(e) = spawned {
            tracing::warn!("não consegui ler a conta do Windows: {e}");
        }
    }

    pub fn power_user_loaded(&mut self) -> Result<()> {
        if let Some(info) = self.power.incoming.lock().ok().and_then(|mut s| s.take()) {
            self.power.user = Some(info);
        }
        if self.controller.open().is_some_and(|o| o.kind == PanelKind::Power) {
            self.redraw_open_panel()?;
        }
        Ok(())
    }

    pub fn power_panel_list(&mut self, scale: f32) -> DisplayList {
        let uptime = format_uptime(power::uptime().as_secs());
        let fallback = std::env::var("USERNAME").unwrap_or_default();
        let (name, picture) = self.power.user.as_ref().map_or((fallback.as_str(), None), |(n, p)| (n.as_str(), p.as_ref()));
        let (list, hits) = power_panel(name, picture, &uptime, self.power.confirm.armed(), scale, &self.ctx.theme.tokens);
        self.power.hits = hits;
        list
    }

    /// Clique no painel. `true` se o painel é o de Energia.
    pub fn power_click(&mut self, p: Point) -> Result<bool> {
        if !self.controller.open().is_some_and(|o| o.kind == PanelKind::Power) {
            return Ok(false);
        }
        let Some(action) = self.power.hits.iter().find(|(r, _)| r.contains(p)).map(|(_, a)| *a) else { return Ok(true) };
        match self.power.confirm.click(action, self.ctx.now()) {
            None => {
                // Armou: desarma sozinho em 3 s.
                window::set_timer(TIMER_POWER, 3100);
                self.redraw_open_panel()?;
            }
            Some(a @ (PowerAction::Lock | PowerAction::Sleep)) => {
                self.close_open_panel()?;
                execute(a);
            }
            Some(a) => {
                // Sai como no "Sair" do menu; a ação roda depois que as AppBars saírem.
                self.power.pending = Some(a);
                // SAFETY: encerra o laço de mensagens desta thread.
                unsafe { PostQuitMessage(0) };
            }
        }
        Ok(true)
    }

    pub fn power_timer(&mut self) -> Result<()> {
        window::kill_timer(TIMER_POWER);
        let now = self.ctx.now();
        if self.power.confirm.expire(now) && self.controller.open().is_some_and(|o| o.kind == PanelKind::Power) {
            self.redraw_open_panel()?;
        }
        Ok(())
    }

    /// Painel fechou (Esc, clique fora, outro chip): desarma.
    pub fn power_panel_closed(&mut self) {
        self.power.confirm.cancel();
    }
}
