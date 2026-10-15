//! Explorer reiniciado, monitores e DPI mudando, preferências do Windows.

use windows::core::Result;
use zefiro_platform_windows::window::{self, WinEvent};
use zefiro_platform_windows::{island_zorder, monitors, system};

use crate::bar_view::BarView;
use crate::event_loop::{Loop, TIMER_DISPLAY};
use crate::panel_view::PanelView;

const ABN_POSCHANGED: u32 = 1;

pub fn handle(l: &mut Loop, e: WinEvent) -> Result<bool> {
    match e {
        WinEvent::TaskbarCreated => {
            tracing::info!("Explorer reiniciado: registrando as AppBars de novo");
            for b in &mut l.bars {
                b.register_appbar(&l.ctx);
            }
            island_zorder::raise_island();
        }
        WinEvent::DisplayChanged | WinEvent::DpiChanged { .. } => window::set_timer(TIMER_DISPLAY, 400),
        WinEvent::Timer { id: TIMER_DISPLAY } => {
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
        }
        WinEvent::SettingChanged => {
            l.ctx.theme.animations = system::animations_enabled();
            l.apply_bars(false)?;
        }
        WinEvent::AppBar { notification: ABN_POSCHANGED, .. } => {
            for b in &l.bars {
                b.reposition_appbar();
            }
        }
        _ => {}
    }
    Ok(true)
}
