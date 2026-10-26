//! Ícone do Zefiro na bandeja: o clique traz a barra de volta (ou abre o menu do Z) e o botão
//! direito abre um menu curto, útil quando a barra sumiu.

use windows::Win32::UI::WindowsAndMessaging::PostQuitMessage;
use windows::core::Result;
use zefiro_bar::controller::PanelInput;
use zefiro_island::menu::MenuEntry;
use zefiro_platform_windows::{island_zorder, popup_menu, system, tray, window};

use crate::event_loop::{Loop, USER_ISLAND};

const WM_LBUTTONUP: u32 = 0x0202;
const WM_RBUTTONUP: u32 = 0x0205;

const SHOW: u32 = 1;
const EDIT: u32 = 2;
const ISLAND: u32 = 3;
const LOGS: u32 = 4;
const EXIT: u32 = 5;
const AUTOSTART: u32 = 6;

/// Põe (ou repõe, depois de o Explorer reiniciar ou o tema mudar) o ícone na bandeja.
pub fn show_icon() {
    if let Err(e) = tray::add(system::taskbar_dark()) {
        tracing::warn!("não consegui pôr o ícone na bandeja: {e}");
    }
}

fn item(label: &str, id: u32) -> MenuEntry {
    MenuEntry { label: label.into(), id, enabled: true, ..Default::default() }
}

/// Liga ou desliga o início com o Windows (menu do Z e da bandeja).
pub fn toggle_autostart() {
    let on = !system::autostart_enabled();
    match system::set_autostart(on) {
        Ok(()) => tracing::info!(on, "iniciar com o Windows"),
        Err(e) => tracing::warn!("não consegui mudar o início com o Windows: {e}"),
    }
}

impl Loop {
    fn primary_bar(&self) -> usize {
        self.bars.iter().position(|b| b.monitor().primary).unwrap_or(0)
    }

    pub fn tray_event(&mut self, mouse: u32) -> Result<()> {
        match mouse {
            WM_LBUTTONUP => {
                if !self.reveal_bars() {
                    let fx = self.controller.handle(PanelInput::ChipClick { monitor: self.primary_bar(), chip: "zefiro".into() });
                    self.run_effects(fx)?;
                }
                Ok(())
            }
            WM_RBUTTONUP => self.tray_menu(),
            _ => Ok(()),
        }
    }

    /// Traz de volta as barras escondidas (tela cheia). `true` se havia alguma.
    fn reveal_bars(&mut self) -> bool {
        let mut revealed = false;
        for b in self.bars.iter_mut().filter(|b| b.is_hidden()) {
            b.set_hidden(false);
            revealed = true;
        }
        if revealed {
            tracing::info!("barra trazida de volta pela bandeja");
            self.island_visibility_changed();
            island_zorder::raise_island();
        }
        revealed
    }

    fn tray_menu(&mut self) -> Result<()> {
        let entries = [
            item("Mostrar a barra", SHOW),
            item("Editar barra", EDIT),
            item("Reiniciar a ilha", ISLAND),
            item("Abrir pasta de logs", LOGS),
            MenuEntry { checked: system::autostart_enabled(), ..item("Iniciar com o Windows", AUTOSTART) },
            MenuEntry { separator: true, ..Default::default() },
            item("Sair", EXIT),
        ];
        let primary = self.primary_bar();
        match popup_menu::show(self.bars[primary].hwnd(), &entries)? {
            Some(SHOW) => {
                self.reveal_bars();
                // Com ocultar automaticamente, a do principal desce e sobe pela regra normal.
                self.autohide_reveal(primary)?;
            }
            Some(EDIT) => self.start_editor(primary)?,
            Some(ISLAND) => window::post_user(USER_ISLAND),
            Some(LOGS) => {
                let dir = zefiro_core::observability::log_dir_from(std::env::var_os("LOCALAPPDATA"), std::env::var_os("XDG_STATE_HOME"), std::env::var_os("HOME"));
                system::open_folder(&dir);
            }
            Some(AUTOSTART) => toggle_autostart(),
            // SAFETY: encerra o laço de mensagens desta thread pelo caminho normal (tira as AppBars).
            Some(EXIT) => unsafe { PostQuitMessage(0) },
            _ => {}
        }
        Ok(())
    }
}
