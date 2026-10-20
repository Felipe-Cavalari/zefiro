//! Perfis do Windows Terminal no laço de eventos: leitura ao abrir o painel (numa thread), ↑ ↓ e a
//! roda para escolher, clique ou Enter para abrir o perfil. O lápis entra na edição: arrastar
//! ordena ou esconde, o olho esconde e mostra, Concluir (ou fechar o painel) grava em `[terminal]`
//! e Esc desfaz.

use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use windows::core::Result;
use zefiro_bar::controller::PanelKind;
use zefiro_bar::terminal::{Arrangement, DragView, PanelLayout, PanelView, ProfileRow, VISIBLE, drop_slot, max_scroll, terminal_panel};
use zefiro_core::config::{TerminalConfig, set_terminal};
use zefiro_integration_terminal::{LoadError, Profile, arrange, icon_candidates, open_args, read_profiles, saved_lists};
use zefiro_platform::geometry::Point;
use zefiro_platform::image::Image;
use zefiro_platform_windows::{system, system_events, window, windows_terminal};
use zefiro_ui::paint::DisplayList;

use crate::event_loop::{Loop, USER_TERMINAL};

/// Ícones decodificados em 36 px (18 DIP a 200%).
const ICON_PX: u32 = 36;
/// Distância (DIP) que o mouse anda com o botão apertado antes de virar arraste.
const DRAG_START: f32 = 4.0;

/// O que a thread de leitura devolve: os perfis (ou o erro), os ícones e o `wt.exe`.
type Loaded = (std::result::Result<Vec<Profile>, LoadError>, Vec<Option<Image>>, Option<PathBuf>);

#[derive(Default)]
pub struct TerminalRuntime {
    /// `None` = lendo.
    profiles: Option<std::result::Result<Vec<Profile>, LoadError>>,
    rows: Vec<ProfileRow>,
    wt: Option<PathBuf>,
    /// Resultado da thread de leitura, pego no `USER_TERMINAL`.
    incoming: Arc<Mutex<Option<Loaded>>>,
    arrangement: Arrangement,
    scroll: usize,
    /// Perfil escolhido pelo teclado.
    selected: Option<usize>,
    /// Perfil sob o mouse.
    hover: Option<usize>,
    /// Último desenho, para os cliques.
    layout: Option<PanelLayout>,
    editing: bool,
    /// Arranjo ao entrar na edição, para o Esc.
    before_edit: Arrangement,
    /// Botão apertado numa linha da edição: o perfil e onde.
    press: Option<(usize, Point)>,
    drag: Option<DragView>,
}

fn env_dir(name: &str) -> PathBuf {
    PathBuf::from(std::env::var_os(name).unwrap_or_default())
}

impl Loop {
    fn terminal_open(&self) -> bool {
        self.controller.open().is_some_and(|o| o.kind == PanelKind::Terminal)
    }

    /// Ao abrir o painel: lê os perfis, os ícones e procura o `wt.exe` numa thread.
    pub fn terminal_prepare(&mut self, kind: PanelKind) {
        if kind != PanelKind::Terminal {
            return;
        }
        let t = &mut self.terminal;
        t.profiles = None;
        t.rows.clear();
        t.scroll = 0;
        t.selected = None;
        t.hover = None;
        t.editing = false;
        let incoming = t.incoming.clone();
        let spawned = std::thread::Builder::new().name("zefiro-terminal".into()).spawn(move || {
            let profiles = read_profiles(&env_dir("LOCALAPPDATA"), &env_dir("PROGRAMDATA"));
            let package = windows_terminal::package_dir();
            let candidates: Vec<Vec<PathBuf>> = profiles.iter().flatten().map(|p| icon_candidates(p, package.as_deref())).collect();
            let icons = windows_terminal::load_icons(&candidates, ICON_PX);
            if let Ok(mut slot) = incoming.lock() {
                *slot = Some((profiles, icons, windows_terminal::wt_exe()));
            }
            window::post_user(USER_TERMINAL);
        });
        if let Err(e) = spawned {
            tracing::warn!("não consegui ler os perfis do Windows Terminal: {e}");
            t.profiles = Some(Err(LoadError::Unreadable(e.to_string())));
        }
    }

    /// A leitura terminou.
    pub fn terminal_loaded(&mut self) -> Result<()> {
        if let Some((profiles, icons, wt)) = self.terminal.incoming.lock().ok().and_then(|mut s| s.take()) {
            if let Err(LoadError::Unreadable(e)) = &profiles {
                tracing::warn!("não consegui ler as configurações do Windows Terminal: {e}");
            }
            let t = &mut self.terminal;
            t.rows = profiles.iter().flatten().zip(icons.into_iter().chain(std::iter::repeat(None))).map(|(p, icon)| ProfileRow { name: p.name.clone(), icon, default: p.default }).collect();
            t.profiles = Some(profiles);
            t.wt = wt;
            self.terminal_rearrange();
        }
        if self.terminal_open() {
            self.redraw_open_panel()?;
        }
        Ok(())
    }

    /// Ordem e ocultos a partir do `[terminal]` do config.
    fn terminal_rearrange(&mut self) {
        let config = &self.ctx.config.terminal;
        let t = &mut self.terminal;
        if let Some(Ok(profiles)) = &t.profiles {
            let (visible, hidden) = arrange(profiles, &config.order, &config.hidden);
            t.arrangement = Arrangement { visible, hidden };
        }
    }

    pub fn terminal_panel_list(&mut self, scale: f32) -> DisplayList {
        let t = &mut self.terminal;
        let message = match (&t.profiles, &t.wt) {
            (None, _) => Some("Carregando…"),
            (Some(Err(LoadError::Unreadable(_))), _) => Some("Não consegui ler as configurações do Windows Terminal"),
            (Some(Err(LoadError::NotInstalled)), _) | (Some(Ok(_)), None) => Some("Windows Terminal não encontrado"),
            (Some(Ok(_)), Some(_)) => None,
        };
        // O perfil escolhido fica sempre à vista.
        if let Some(i) = t.selected.and_then(|p| t.arrangement.visible.iter().position(|&v| v == p)) {
            if i < t.scroll {
                t.scroll = i;
            } else if i >= t.scroll + VISIBLE {
                t.scroll = i + 1 - VISIBLE;
            }
        }
        t.scroll = t.scroll.min(max_scroll(t.arrangement.visible.len()));
        let view = PanelView { rows: &t.rows, arrangement: &t.arrangement, editing: t.editing, message, selected: t.selected, hover: t.hover, drag: t.drag.as_ref(), scroll: t.scroll };
        let layout = terminal_panel(&view, scale, &self.ctx.theme.tokens);
        let list = layout.list.clone();
        t.layout = Some(layout);
        list
    }

    fn terminal_launch(&mut self, profile: usize) -> Result<()> {
        let t = &self.terminal;
        if let (Some(Ok(profiles)), Some(wt)) = (&t.profiles, &t.wt)
            && let Some(p) = profiles.get(profile)
        {
            tracing::info!(perfil = %p.name, "abrindo perfil no Windows Terminal");
            if let Err(e) = system::spawn_detached(wt, &open_args(p)) {
                tracing::warn!("não consegui abrir o Windows Terminal: {e}");
            }
        }
        self.close_open_panel()
    }

    /// Teclas com o painel do Terminal aberto. `true` se tratou: ↑ ↓ escolhem e Enter abre. Na
    /// edição: Ctrl+↑/↓ move, Espaço esconde ou mostra, Enter conclui e Esc desfaz.
    pub fn terminal_key(&mut self, vk: u32) -> Result<bool> {
        if !self.terminal_open() {
            return Ok(false);
        }
        if self.terminal.editing {
            return self.terminal_edit_key(vk);
        }
        let t = &mut self.terminal;
        let rows = &t.arrangement.visible;
        match vk {
            0x26 | 0x28 => {
                let at = t.selected.and_then(|p| rows.iter().position(|&r| r == p));
                let next = match (at, vk == 0x26) {
                    (None, _) => 0,
                    (Some(i), true) => i.saturating_sub(1),
                    (Some(i), false) => (i + 1).min(rows.len().saturating_sub(1)),
                };
                t.selected = rows.get(next).copied();
                self.redraw_open_panel()?;
                Ok(true)
            }
            0x0D => {
                if let Some(p) = t.selected {
                    self.terminal_launch(p)?;
                }
                Ok(true)
            }
            _ => Ok(false),
        }
    }

    fn terminal_edit_key(&mut self, vk: u32) -> Result<bool> {
        let t = &mut self.terminal;
        let rows = t.arrangement.rows();
        let at = t.selected.and_then(|p| rows.iter().position(|&r| r == p));
        match vk {
            0x26 | 0x28 if system_events::ctrl_down() => {
                if let Some(p) = t.selected {
                    t.arrangement.nudge(p, vk == 0x26);
                }
            }
            0x26 | 0x28 => {
                let next = match (at, vk == 0x26) {
                    (None, _) => 0,
                    (Some(i), true) => i.saturating_sub(1),
                    (Some(i), false) => (i + 1).min(rows.len().saturating_sub(1)),
                };
                t.selected = rows.get(next).copied();
            }
            0x20 => {
                if let Some(p) = t.selected {
                    t.arrangement.toggle(p);
                }
            }
            0x0D => self.terminal_finish_edit(),
            0x1B => {
                t.arrangement = std::mem::take(&mut t.before_edit);
                t.editing = false;
                self.terminal_drop_gesture();
            }
            _ => return Ok(true),
        }
        self.redraw_open_panel()?;
        Ok(true)
    }

    /// Sai da edição gravando.
    fn terminal_finish_edit(&mut self) {
        self.terminal_save();
        self.terminal.editing = false;
        self.terminal_drop_gesture();
    }

    /// Grava ordem e ocultos em `[terminal]` se mudaram.
    fn terminal_save(&mut self) {
        let t = &self.terminal;
        let Some(Ok(profiles)) = &t.profiles else { return };
        let current = &self.ctx.config.terminal;
        let (old_visible, old_hidden) = arrange(profiles, &current.order, &current.hidden);
        if old_visible == t.arrangement.visible && old_hidden == t.arrangement.hidden {
            return;
        }
        let (order, hidden) = saved_lists(profiles, &t.arrangement.visible, &t.arrangement.hidden, &current.hidden);
        let wanted = TerminalConfig { order, hidden };
        match set_terminal(&self.ctx.config_path, &wanted) {
            Ok(()) => {
                tracing::info!(visiveis = wanted.order.len(), ocultos = wanted.hidden.len(), "perfis do Terminal gravados");
                self.ctx.config.terminal = wanted;
            }
            Err(e) => tracing::warn!("não consegui gravar os perfis do Terminal: {e}"),
        }
    }

    /// Esquece o botão apertado e o arraste.
    fn terminal_drop_gesture(&mut self) {
        let t = &mut self.terminal;
        let pressed = t.press.take().is_some();
        let dragging = t.drag.take().is_some();
        if pressed || dragging {
            window::release_capture();
        }
    }

    /// Roda sobre o painel. `true` se tratou.
    pub fn terminal_wheel(&mut self, delta: i32) -> Result<bool> {
        if !self.terminal_open() {
            return Ok(false);
        }
        let t = &mut self.terminal;
        let max = max_scroll(t.arrangement.visible.len());
        t.scroll = if delta > 0 { t.scroll.saturating_sub(1) } else { (t.scroll + 1).min(max) };
        self.redraw_open_panel()?;
        Ok(true)
    }

    fn terminal_row_at(&self, p: Point) -> Option<usize> {
        self.terminal.layout.as_ref().and_then(|l| l.rows.iter().find(|(r, _)| r.contains(p)).map(|(_, i)| *i))
    }

    /// Botão apertado: na edição, uma linha pode virar arraste. `true` se tratou.
    pub fn terminal_down(&mut self, panel: usize, p: Point) -> Result<bool> {
        if !self.terminal_open() || !self.terminal.editing {
            return Ok(false);
        }
        let Some(profile) = self.terminal_row_at(p) else { return Ok(false) };
        self.terminal.press = Some((profile, p));
        window::capture(self.panels[panel].hwnd());
        Ok(true)
    }

    /// Mouse andando no painel: arrasta (na edição) ou acende a linha sob ele. `true` se tratou.
    pub fn terminal_move(&mut self, p: Point) -> Result<bool> {
        if !self.terminal_open() {
            return Ok(false);
        }
        let scale = self.controller.open().map_or(1.0, |o| self.bars[o.monitor].scale());
        let t = &mut self.terminal;
        if let Some((profile, start)) = t.press {
            let far = (p.x - start.x).hypot(p.y - start.y) > DRAG_START * scale;
            if (t.drag.is_some() || far)
                && let Some(layout) = &t.layout
            {
                let slot = drop_slot(layout, &t.arrangement, profile, p);
                t.drag = Some(DragView { profile, point: p, slot });
                t.hover = None;
                self.redraw_open_panel()?;
            }
            return Ok(true);
        }
        let hover = self.terminal_row_at(p);
        if hover != self.terminal.hover {
            self.terminal.hover = hover;
            self.redraw_open_panel()?;
        }
        Ok(true)
    }

    /// Mouse saiu do painel: apaga a linha acesa.
    pub fn terminal_leave(&mut self) -> Result<()> {
        if self.terminal_open() && self.terminal.hover.take().is_some() {
            self.redraw_open_panel()?;
        }
        Ok(())
    }

    /// Botão solto no painel: lápis/Concluir, fim do arraste, olho ou abrir o perfil da linha.
    /// `true` se o painel é o do Terminal.
    pub fn terminal_up(&mut self, p: Point) -> Result<bool> {
        if !self.terminal_open() {
            return Ok(false);
        }
        let t = &mut self.terminal;
        let loaded = t.profiles.as_ref().is_some_and(|r| r.is_ok()) && t.wt.is_some();
        let on_pencil = loaded && t.drag.is_none() && t.layout.as_ref().is_some_and(|l| l.pencil.contains(p));
        if t.editing {
            if let Some(d) = t.drag.take() {
                t.arrangement.move_to(d.profile, d.slot);
            } else if let Some((profile, _)) = t.press
                && t.layout.as_ref().is_some_and(|l| l.eyes.iter().any(|(r, i)| *i == profile && r.contains(p)))
            {
                t.arrangement.toggle(profile);
            } else if on_pencil {
                self.terminal_finish_edit();
            }
            self.terminal_drop_gesture();
            self.redraw_open_panel()?;
            return Ok(true);
        }
        if on_pencil {
            t.before_edit = t.arrangement.clone();
            t.editing = true;
            t.selected = None;
            self.redraw_open_panel()?;
        } else if let Some(profile) = self.terminal_row_at(p) {
            self.terminal_launch(profile)?;
        }
        Ok(true)
    }

    /// O `[terminal]` do config mudou por fora: refaz a ordem, a menos que esteja editando.
    pub fn terminal_config_changed(&mut self) -> Result<()> {
        if self.terminal_open() && !self.terminal.editing {
            self.terminal_rearrange();
            self.redraw_open_panel()?;
        }
        Ok(())
    }

    /// Painel fechando: no meio da edição, grava (fechar não perde nada).
    pub fn terminal_panel_closed(&mut self) {
        if self.terminal.editing {
            self.terminal_finish_edit();
        }
        self.terminal_drop_gesture();
        self.terminal.layout = None;
        self.terminal.hover = None;
    }
}
