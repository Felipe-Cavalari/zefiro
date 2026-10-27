//! Chip Bandeja no laço de eventos (ADR-0023): o host da bandeja sobe e desce com o chip, o painel
//! mostra a grade, e clique (ou Enter) abre o app enquanto o botão direito (ou a tecla de menu)
//! abre o menu dele. O painel fecha antes de o clique chegar ao app.

use std::sync::Arc;
use windows::Win32::UI::Input::KeyboardAndMouse::{VK_APPS, VK_DOWN, VK_LEFT, VK_RETURN, VK_RIGHT, VK_UP};
use windows::core::Result;
use zefiro_bar::controller::PanelKind;
use zefiro_bar::systray::{COLUMNS, Cell, Nav, TrayLayout, TrayView, cell_at, display_name, max_scroll, step, tray_panel, visible_rows};
use zefiro_integration_systray::click::Button;
use zefiro_platform::geometry::Point;
use zefiro_platform_windows::tray_host::{self, TrayHost, TrayItem};
use zefiro_platform_windows::window;
use zefiro_ui::paint::DisplayList;

use crate::event_loop::{Loop, USER_TRAY};

/// Ícones convertidos em 32 px (16 DIP a 200%).
const ICON_PX: u32 = 32;
/// Folga (DIP) entre o fim do painel e a borda de baixo do monitor.
const BOTTOM_GAP: f32 = 16.0;

#[derive(Default)]
pub struct SystrayRuntime {
    host: Option<TrayHost>,
    items: Vec<TrayItem>,
    /// Nome de cada ícone (`display_name`), na ordem de `items`.
    names: Vec<String>,
    hover: Option<usize>,
    /// Ícone escolhido pelo teclado.
    selected: Option<usize>,
    /// Primeira linha visível.
    scroll: usize,
    /// Altura máxima do último desenho (DIP), para a rolagem.
    max_h: f32,
    layout: Option<TrayLayout>,
    /// Processo do app cujo menu (clique direito) está aberto: o painel fica atrás dele.
    menu_pid: Option<u32>,
    menu_hook: Option<window::MenuHook>,
}

/// O que pode encerrar o menu de um ícone.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MenuSignal {
    /// Um menu fechou (escolheu uma opção, Esc ou clicou fora).
    Ended,
    /// Esse processo veio para o primeiro plano.
    Foreground(u32),
}

/// O menu do app acabou: o painel fecha (se não tiver o foco de volta).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MenuClosed;

impl SystrayRuntime {
    /// Troca os ícones mantendo o hover e a seleção no mesmo ícone (pela chave, não pela posição).
    fn set_items(&mut self, items: Vec<TrayItem>) {
        let follow = |i: Option<usize>| i.and_then(|i| self.items.get(i)).and_then(|old| items.iter().position(|n| n.key == old.key));
        // O hover só fica se o ícone continua sob o mouse; a seleção do teclado segue o ícone.
        let hover = self.hover.filter(|&i| follow(Some(i)) == Some(i));
        self.selected = follow(self.selected);
        self.hover = hover;
        self.names = items.iter().map(|i| display_name(&i.tip, &i.exe)).collect();
        self.items = items;
    }
}

impl SystrayRuntime {
    /// O painel está atrás do menu de um app e não fecha ao perder o foco.
    pub fn holds_panel(&self) -> bool {
        self.menu_pid.is_some()
    }

    fn menu_signal(&mut self, signal: MenuSignal) -> Option<MenuClosed> {
        let pid = self.menu_pid?;
        if signal == MenuSignal::Foreground(pid) {
            return None;
        }
        self.menu_pid = None;
        self.menu_hook = None;
        Some(MenuClosed)
    }

    /// Roda: uma linha por passo; o hover sai (a célula sob o mouse mudou).
    fn scroll_by(&mut self, delta: i32) {
        let max = max_scroll(self.items.len(), self.max_h);
        self.scroll = if delta > 0 { self.scroll.saturating_sub(1) } else { (self.scroll + 1).min(max) };
        self.hover = None;
    }
}

impl Loop {
    fn systray_open(&self) -> bool {
        self.controller.open().is_some_and(|o| o.kind == PanelKind::Tray)
    }

    /// O host só existe com o chip em algum layout.
    pub fn systray_services(&mut self) {
        let wanted = self.ctx.config.bar.uses("tray");
        let s = &mut self.systray;
        match (wanted, s.host.is_some()) {
            (true, false) => match TrayHost::start(Arc::new(|| window::post_user(USER_TRAY)), ICON_PX) {
                Ok(host) => {
                    s.host = Some(host);
                    // O ícone do Zefiro entrou antes do host e não responde ao pedido do próprio
                    // Zefiro: ele vai de novo, agora passando pelo host.
                    crate::tray_loop::show_icon();
                }
                Err(e) => tracing::warn!("bandeja indisponível: {e}"),
            },
            (false, true) => {
                s.host = None;
                s.set_items(Vec::new());
            }
            _ => {}
        }
    }

    /// O host avisou que os ícones mudaram.
    pub fn systray_changed(&mut self) -> Result<()> {
        if !self.systray_open() {
            return Ok(());
        }
        if let Some(items) = self.systray.host.as_ref().map(TrayHost::snapshot) {
            self.systray.set_items(items);
        }
        self.redraw_open_panel()
    }

    /// Ao abrir o painel: tira os órfãos e lê os ícones.
    pub fn systray_prepare(&mut self, kind: PanelKind) {
        if kind != PanelKind::Tray {
            return;
        }
        let s = &mut self.systray;
        let items = s.host.as_ref().map(|h| {
            h.prune();
            h.snapshot()
        });
        s.set_items(items.unwrap_or_default());
        s.hover = None;
        s.selected = None;
        s.scroll = 0;
    }

    pub fn systray_panel_list(&mut self, scale: f32) -> DisplayList {
        let max_h = self.controller.open().and_then(|o| self.bars.get(o.monitor)).map_or(600.0, |b| (b.monitor().rect.bottom() - b.band_bottom()) / scale - BOTTOM_GAP);
        let s = &mut self.systray;
        s.max_h = max_h;
        // O ícone escolhido pelo teclado fica sempre à vista.
        if let Some(row) = s.selected.map(|i| i / COLUMNS) {
            let rows = visible_rows(max_h);
            if row < s.scroll {
                s.scroll = row;
            } else if row >= s.scroll + rows {
                s.scroll = row + 1 - rows;
            }
        }
        s.scroll = s.scroll.min(max_scroll(s.items.len(), max_h));
        let cells: Vec<Cell> = s.items.iter().zip(&s.names).map(|(i, n)| Cell { name: n, image: i.image.as_ref() }).collect();
        let layout = tray_panel(&TrayView { cells: &cells, hover: s.hover, selected: s.selected, scroll: s.scroll, max_h }, scale, &self.ctx.theme.tokens);
        let list = layout.list.clone();
        s.layout = Some(layout);
        list
    }

    /// Esquerdo: fecha o painel (sem devolver o foco: quem ganha o primeiro plano é o app) e manda o
    /// clique. Direito: o painel fica atrás do menu do app, como no Windows, e fecha junto com ele.
    fn systray_activate(&mut self, index: usize, button: Button) -> Result<()> {
        let Some(key) = self.systray.items.get(index).map(|i| i.key) else { return Ok(()) };
        let cursor = tray_host::cursor();
        if button == Button::Left {
            self.close_panel_for_editor()?;
        }
        let pid = self.systray.host.as_ref().and_then(|h| h.click(&key, button, cursor));
        if button == Button::Right && pid.is_some() {
            self.systray.menu_pid = pid;
            self.systray.menu_hook = window::MenuHook::install();
        }
        Ok(())
    }

    /// Um menu fechou ou outro app veio para a frente: se era o menu do ícone, o painel fecha
    /// (a não ser que o foco tenha voltado para ele).
    pub fn systray_menu_signal(&mut self, signal: MenuSignal) -> Result<()> {
        if self.systray.menu_signal(signal).is_some() && !self.panels.iter().any(|p| p.open && p.has_focus()) {
            let fx = self.controller.handle(zefiro_bar::controller::PanelInput::FocusLost);
            self.run_effects(fx)?;
        }
        Ok(())
    }

    fn systray_cell_at(&self, p: Point) -> Option<usize> {
        self.systray.layout.as_ref().and_then(|l| cell_at(l, p))
    }

    /// Botão esquerdo solto no painel: abre o app.
    pub fn systray_up(&mut self, p: Point) -> Result<bool> {
        if !self.systray_open() {
            return Ok(false);
        }
        if let Some(i) = self.systray_cell_at(p) {
            self.systray_activate(i, Button::Left)?;
        }
        Ok(true)
    }

    /// Botão direito solto no painel: o menu do app.
    pub fn systray_right_up(&mut self, p: Point) -> Result<bool> {
        if !self.systray_open() {
            return Ok(false);
        }
        if let Some(i) = self.systray_cell_at(p) {
            self.systray_activate(i, Button::Right)?;
        }
        Ok(true)
    }

    pub fn systray_move(&mut self, p: Point) -> Result<()> {
        if !self.systray_open() {
            return Ok(());
        }
        let hover = self.systray_cell_at(p);
        if hover != self.systray.hover {
            self.systray.hover = hover;
            self.redraw_open_panel()?;
        }
        Ok(())
    }

    pub fn systray_leave(&mut self) -> Result<()> {
        if self.systray_open() && self.systray.hover.take().is_some() {
            self.redraw_open_panel()?;
        }
        Ok(())
    }

    pub fn systray_wheel(&mut self, delta: i32) -> Result<bool> {
        if !self.systray_open() {
            return Ok(false);
        }
        self.systray.scroll_by(delta);
        self.redraw_open_panel()?;
        Ok(true)
    }

    /// Setas na grade, Enter abre e a tecla de menu mostra o menu do app. Esc fica com o laço.
    pub fn systray_key(&mut self, vk: u32) -> Result<bool> {
        if !self.systray_open() {
            return Ok(false);
        }
        let nav = match vk {
            v if v == VK_LEFT.0 as u32 => Some(Nav::Left),
            v if v == VK_RIGHT.0 as u32 => Some(Nav::Right),
            v if v == VK_UP.0 as u32 => Some(Nav::Up),
            v if v == VK_DOWN.0 as u32 => Some(Nav::Down),
            _ => None,
        };
        if let Some(nav) = nav {
            self.systray.selected = step(self.systray.selected, self.systray.items.len(), nav);
            self.redraw_open_panel()?;
            return Ok(true);
        }
        let button = match vk {
            v if v == VK_RETURN.0 as u32 => Button::Left,
            v if v == VK_APPS.0 as u32 => Button::Right,
            _ => return Ok(false),
        };
        if let Some(i) = self.systray.selected {
            self.systray_activate(i, button)?;
        }
        Ok(true)
    }

    pub fn systray_panel_closed(&mut self) {
        let s = &mut self.systray;
        s.hover = None;
        s.selected = None;
        s.layout = None;
        s.menu_pid = None;
        s.menu_hook = None;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use zefiro_integration_systray::model::Key;
    use zefiro_platform::image::Image;

    fn item(uid: u32, tip: &str, image: u8) -> TrayItem {
        TrayItem { key: Key::Window { hwnd: 1, uid }, tip: tip.into(), exe: String::new(), image: Image::new(1, 1, vec![image; 4]) }
    }

    #[test]
    fn icon_change_keeps_selection() {
        let mut r = SystrayRuntime::default();
        r.set_items(vec![item(1, "A", 0), item(2, "B", 0), item(3, "C", 0)]);
        r.selected = Some(1);
        r.hover = Some(2);
        r.set_items(vec![item(1, "A", 0), item(2, "B", 9), item(3, "C", 0)]);
        assert_eq!((r.selected, r.hover), (Some(1), Some(2)));
        r.set_items(vec![item(3, "C", 0), item(1, "A", 0)]);
        assert_eq!(r.selected, None, "B saiu");
        assert_eq!(r.names, ["C", "A"]);
    }

    #[test]
    fn hover_is_dropped_when_the_icon_under_the_mouse_moves() {
        let mut r = SystrayRuntime::default();
        r.set_items(vec![item(1, "A", 0), item(2, "B", 0), item(3, "C", 0)]);
        r.hover = Some(2);
        r.set_items(vec![item(1, "A", 0), item(2, "B", 0), item(3, "C", 9)]);
        assert_eq!(r.hover, Some(2), "mesmo lugar: continua");
        r.set_items(vec![item(3, "C", 0), item(1, "A", 0)]);
        assert_eq!(r.hover, None, "C mudou de lugar: o nome não pode ficar numa célula sem o mouse");
    }

    #[test]
    fn panel_stays_behind_the_app_menu() {
        let mut r = SystrayRuntime::default();
        assert!(!r.holds_panel());
        r.menu_pid = Some(42);
        assert!(r.holds_panel(), "o menu do app tirou o foco: o painel não fecha");
        assert_eq!(r.menu_signal(MenuSignal::Foreground(42)), None, "o próprio app no primeiro plano: o menu continua");
        assert!(r.holds_panel());
        assert_eq!(r.menu_signal(MenuSignal::Ended), Some(MenuClosed));
        assert!(!r.holds_panel());
        assert_eq!(r.menu_signal(MenuSignal::Ended), None, "sem menu aberto, nada a fazer");
    }

    #[test]
    fn another_app_in_front_ends_the_menu() {
        let mut r = SystrayRuntime { menu_pid: Some(42), ..Default::default() };
        assert_eq!(r.menu_signal(MenuSignal::Foreground(7)), Some(MenuClosed));
        assert!(!r.holds_panel());
    }

    #[test]
    fn scrolling_drops_hover() {
        let mut r = SystrayRuntime::default();
        r.set_items((0..40).map(|i| item(i, "x", 0)).collect());
        r.max_h = 200.0;
        r.hover = Some(3);
        r.scroll_by(-120);
        assert_eq!((r.scroll, r.hover), (1, None));
        r.scroll_by(120);
        assert_eq!(r.scroll, 0);
    }
}
