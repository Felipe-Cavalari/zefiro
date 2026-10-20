//! Perfis do Windows Terminal no laço de eventos: leitura ao abrir o painel (numa thread), ↑ ↓ e a
//! roda para escolher, clique ou Enter para abrir o perfil.

use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use windows::core::Result;
use zefiro_bar::controller::PanelKind;
use zefiro_bar::terminal::{Arrangement, PanelLayout, PanelView, ProfileRow, VISIBLE, max_scroll, terminal_panel};
use zefiro_integration_terminal::{LoadError, Profile, arrange, icon_candidates, open_args, read_profiles};
use zefiro_platform::geometry::Point;
use zefiro_platform::image::Image;
use zefiro_platform_windows::{system, window, windows_terminal};
use zefiro_ui::paint::DisplayList;

use crate::event_loop::{Loop, USER_TERMINAL};

/// Ícones decodificados em 36 px (18 DIP a 200%).
const ICON_PX: u32 = 36;

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
    /// Último desenho, para os cliques.
    layout: Option<PanelLayout>,
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
        let view = PanelView { rows: &t.rows, arrangement: &t.arrangement, editing: false, message, selected: t.selected, hover: None, drag: None, scroll: t.scroll };
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

    /// Teclas com o painel do Terminal aberto. `true` se tratou: ↑ ↓ escolhem e Enter abre.
    pub fn terminal_key(&mut self, vk: u32) -> Result<bool> {
        if !self.terminal_open() {
            return Ok(false);
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

    /// Botão solto no painel: abre o perfil da linha. `true` se o painel é o do Terminal.
    pub fn terminal_up(&mut self, p: Point) -> Result<bool> {
        if !self.terminal_open() {
            return Ok(false);
        }
        let hit = self.terminal.layout.as_ref().and_then(|l| l.rows.iter().find(|(r, _)| r.contains(p)).map(|(_, i)| *i));
        if let Some(profile) = hit {
            self.terminal_launch(profile)?;
        }
        Ok(true)
    }

    pub fn terminal_panel_closed(&mut self) {
        self.terminal.layout = None;
    }
}
