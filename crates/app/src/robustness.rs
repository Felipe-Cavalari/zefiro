//! Explorer reiniciado, monitores e DPI mudando, preferências do Windows.

use windows::core::Result;
use zefiro_platform_windows::window::{self, WinEvent};
use zefiro_platform_windows::{island_zorder, monitors, system};

use crate::bar_view::BarView;
use crate::event_loop::{Loop, TIMER_DISPLAY, TIMER_FULLSCREEN};
use crate::panel_view::PanelView;

const ABN_POSCHANGED: u32 = 1;
const ABN_FULLSCREENAPP: u32 = 2;

pub fn handle(l: &mut Loop, e: WinEvent) -> Result<bool> {
    match e {
        WinEvent::TaskbarCreated => {
            tracing::info!("Explorer reiniciado: registrando as AppBars de novo");
            for b in &mut l.bars {
                b.register_appbar(&l.ctx);
            }
            // A bandeja nova não conhece o ícone antigo.
            crate::tray_loop::show_icon();
            island_zorder::raise_island();
            l.editor_topology_changed()?;
        }
        WinEvent::DisplayChanged | WinEvent::DpiChanged { .. } => window::set_timer(TIMER_DISPLAY, 400),
        WinEvent::Timer { id: TIMER_DISPLAY } => {
            l.prepare_editor_rebuild();
            window::kill_timer(TIMER_DISPLAY);
            tracing::info!("monitores mudaram: recriando as barras");
            // Os painéis são recriados abaixo; o controlador só precisa saber que nada está aberto.
            let _ = l.controller.handle(zefiro_bar::controller::PanelInput::ConfigChanged);
            for b in l.bars.drain(..) {
                b.destroy();
            }
            for p in l.panels.drain(..) {
                p.destroy();
            }
            for m in monitors::list() {
                let mut b = BarView::create(&l.ctx, &m)?;
                b.register_appbar(&l.ctx);
                l.panels.push(PanelView::create(&l.ctx, &m)?);
                l.bars.push(b);
            }
            island_zorder::raise_island();
            l.editor_topology_changed()?;
            // Monitores novos ou religados: o DDC/CI reabre os handles e relê o brilho.
            l.devices_displays_changed();
            // Jogo em tela cheia exclusiva troca a resolução: as barras novas nascem visíveis por cima dele.
            reconcile_fullscreen(l)?;
        }
        WinEvent::SettingChanged => {
            l.ctx.theme.animations = system::animations_enabled();
            // Tema claro/escuro da barra de tarefas: o ícone troca de cor.
            crate::tray_loop::show_icon();
            l.apply_bars(false)?;
        }
        // App em tela cheia (vídeo, jogo, apresentação): a barra daquele monitor sai da frente,
        // como a barra de tarefas; volta quando o app sai da tela cheia. O aviso do Windows chega uma
        // vez só, então a troca de foco e o temporizador (enquanto há barra escondida) também conferem.
        WinEvent::AppBar { notification: ABN_FULLSCREENAPP, .. } | WinEvent::ForegroundChanged | WinEvent::Timer { id: TIMER_FULLSCREEN } => reconcile_fullscreen(l)?,
        WinEvent::AppBar { notification: ABN_POSCHANGED, .. } => {
            for b in &l.bars {
                b.reposition_appbar();
            }
        }
        _ => {}
    }
    Ok(true)
}

/// A barra de um monitor fica escondida? `verdict` é o que a janela em primeiro plano diz sobre ele
/// (`system::fullscreen_verdict`). Sem veredito (foco em outro monitor), ela segue como está, a não
/// ser que o app que a escondeu já não cubra o monitor (fechou, minimizou, saiu da tela cheia).
fn next_hidden(hidden: bool, verdict: Option<bool>, owner_still_fullscreen: bool) -> bool {
    verdict.unwrap_or(hidden && owner_still_fullscreen)
}

/// Esconde ou traz de volta a barra de cada monitor conforme a janela em primeiro plano.
pub fn reconcile_fullscreen(l: &mut Loop) -> Result<()> {
    let (mut hid, mut revealed) = (false, false);
    for i in 0..l.bars.len() {
        let monitor = l.bars[i].monitor().rect;
        let (foreground, verdict) = system::foreground_verdict(monitor);
        let was = l.bars[i].is_hidden();
        let owner_on = was && system::window_fullscreen_on(l.bars[i].fullscreen_window(), monitor);
        match (was, next_hidden(was, verdict, owner_on)) {
            (false, true) => {
                if l.controller.open().is_some_and(|o| o.monitor == i) {
                    l.close_open_panel()?;
                }
                if l.editor.as_ref().is_some_and(|editor| editor.monitor == i) { l.suspend_editor(true)?; }
                tracing::info!(monitor = i, janela = %system::foreground_description(), "tela cheia: barra sai da frente");
                l.bars[i].hide_for_fullscreen(foreground);
                hid = true;
            }
            // O jogo voltou ao foco depois de outro app em tela cheia neste monitor: ele passa a ser o dono.
            (true, true) if verdict == Some(true) => l.bars[i].hide_for_fullscreen(foreground),
            (true, false) => {
                tracing::info!(monitor = i, "tela cheia acabou: barra de volta");
                l.bars[i].set_hidden(false);
                revealed = true;
            }
            _ => {}
        }
    }
    if hid || revealed {
        l.island_visibility_changed();
    }
    if revealed {
        island_zorder::raise_island();
        l.suspend_editor(false)?;
    }
    if l.bars.iter().any(|b| b.is_hidden()) {
        window::set_timer(TIMER_FULLSCREEN, 1000);
    } else {
        window::kill_timer(TIMER_FULLSCREEN);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_bar_stays_out_of_a_game_while_the_focus_is_elsewhere() {
        // Jogo em foco neste monitor: some; janela comum em foco nele: volta.
        assert!(next_hidden(false, Some(true), false));
        assert!(!next_hidden(true, Some(false), true));
        // Foco no outro monitor (Discord, navegador): o jogo ainda cobre este, a barra continua fora.
        assert!(next_hidden(true, None, true));
        // ...mas o jogo fechou ou saiu da tela cheia: a barra volta mesmo sem foco aqui.
        assert!(!next_hidden(true, None, false));
        // Barra visível e foco em outro monitor: nada muda.
        assert!(!next_hidden(false, None, false));
    }
}
