//! Ocultar automaticamente (`[bar] autohide`, spec 2026-10-22-barra-limpa): depois de cada evento,
//! cada barra recebe a posição do cursor (pelo hook de mouse), os painéis abertos e a tela cheia, e
//! desce ou sobe pela regra de `zefiro_ui::autohide`.

use windows::core::Result;
use zefiro_platform::geometry::Rect;
use zefiro_platform_windows::{bar_context, monitors, window};
use zefiro_ui::autohide::{Inputs, covered_band, top_triggers};

use crate::event_loop::{Loop, TIMER_AUTOHIDE};

#[derive(Default)]
pub struct AutoHideRuntime {
    on: bool,
    /// `[bar] display` com que as barras foram criadas.
    display: zefiro_core::config::BarDisplay,
    /// Retângulos de todos os monitores (com ou sem barra): decidem onde o topo é borda de verdade.
    screens: Vec<Rect>,
    timer: bool,
}

impl Loop {
    /// Aplica "Ocultar automaticamente" e "Só no monitor principal" se o que está valendo for diferente
    /// da config. Compara com o aplicado, não com a config anterior: o Editar barra põe o rascunho em
    /// `ctx.config` durante a prévia, então ao Concluir a config "nova" já é igual à de antes.
    pub fn clean_bar_sync(&mut self) -> Result<()> {
        if self.ctx.config.bar.display != self.autohide.display {
            self.autohide.display = self.ctx.config.bar.display;
            // Recria as barras pelo mesmo caminho da troca de monitores.
            window::set_timer(crate::event_loop::TIMER_DISPLAY, 1);
        }
        if self.ctx.config.autohide() != self.autohide.on {
            self.autohide_sync()?;
        }
        Ok(())
    }

    /// Início, config ou monitores mudaram: liga ou desliga, refaz a reserva da AppBar e relê os monitores.
    pub fn autohide_sync(&mut self) -> Result<()> {
        let on = self.ctx.config.autohide();
        self.autohide.display = self.ctx.config.bar.display;
        if self.ctx.config.bar.autohide && !on {
            tracing::warn!("ocultar automaticamente ignorado: indisponível com a ilha externa");
        }
        self.autohide.screens = if on { monitors::list().into_iter().map(|m| m.rect).collect() } else { Vec::new() };
        for b in &mut self.bars {
            b.sync_appbar(&self.ctx);
        }
        if !on {
            for b in &mut self.bars {
                bar_context::unwatch_autohide(b.hwnd());
                b.auto = Default::default();
                b.set_retracted(&self.ctx, false, true)?;
            }
            self.island_visibility_changed();
            self.autohide_timer(None);
        }
        if on != self.autohide.on {
            tracing::info!(ligado = on, "ocultar automaticamente");
        }
        self.autohide.on = on;
        self.autohide_update()
    }

    /// Um passo da regra em cada barra. Barato: roda depois de todo evento enquanto a opção está ligada.
    pub fn autohide_update(&mut self) -> Result<()> {
        if !self.autohide.on {
            return Ok(());
        }
        let now = self.ctx.epoch.elapsed().as_millis() as u64;
        let open = self.controller.open().map(|o| o.monitor);
        let editing = self.editor.as_ref().map(|e| e.monitor);
        let (mut wake, mut changed): (Option<u64>, bool) = (None, false);
        for i in 0..self.bars.len() {
            let b = &self.bars[i];
            let monitor = b.monitor().rect;
            let others: Vec<Rect> = self.autohide.screens.iter().filter(|r| **r != monitor).copied().collect();
            // A faixa muda com a altura e o formato; atualizar mantém o que o hook já sabe do cursor.
            let triggers = top_triggers(monitor, &others, covered_band(b.monitor().dpi));
            bar_context::watch_autohide(b.hwnd(), triggers, b.band_on_screen());
            let (at_edge, over_bar) = bar_context::autohide_cursor(b.hwnd());
            let inputs = Inputs { at_edge, over_bar, held: open == Some(i) || editing == Some(i), fullscreen: b.is_hidden() };
            let step = self.bars[i].auto.step(inputs, now);
            if let Some(ms) = step.wake_in_ms {
                wake = Some(wake.map_or(ms, |w| w.min(ms)));
            }
            let was = self.bars[i].is_retracted();
            // Escondida por tela cheia, recolhe sem animar: ao sair da tela cheia ela não pisca.
            self.bars[i].set_retracted(&self.ctx, !step.visible, !inputs.fullscreen)?;
            if was != self.bars[i].is_retracted() {
                tracing::debug!(monitor = i, desceu = step.visible, "ocultar automaticamente");
                changed = true;
            }
        }
        if changed {
            self.island_visibility_changed();
        }
        self.autohide_timer(wake);
        Ok(())
    }

    /// "Mostrar a barra" na bandeja: a barra de `bar` desce como se o cursor tivesse ido ao topo.
    pub fn autohide_reveal(&mut self, bar: usize) -> Result<()> {
        if self.autohide.on && let Some(b) = self.bars.get_mut(bar) {
            b.auto.reveal();
        }
        self.autohide_update()
    }

    fn autohide_timer(&mut self, wake: Option<u64>) {
        match wake {
            Some(ms) => {
                window::set_timer(TIMER_AUTOHIDE, ms as u32);
                self.autohide.timer = true;
            }
            None if self.autohide.timer => {
                window::kill_timer(TIMER_AUTOHIDE);
                self.autohide.timer = false;
            }
            None => {}
        }
    }
}
