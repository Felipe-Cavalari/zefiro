//! Chip Comandos no laço de eventos (ADR-0029): lê a biblioteca ao abrir o painel, busca digitada
//! direto no painel, ↑ ↓ e Enter, clique e roda; copiar põe o comando na área de transferência e
//! fecha o painel.

use windows::core::Result;
use zefiro_bar::commands::{Action, CommandRow, PanelState, PanelView, ToolEntry, commands_panel};
use zefiro_bar::controller::PanelKind;
use zefiro_bar::vscode::hovered_row;
use zefiro_core::config::set_value;
use zefiro_integration_commands::{Library, builtin, merge, read_user_dir, terms};
use zefiro_platform::geometry::{Point, Rect};
use zefiro_platform_windows::system_events;
use zefiro_ui::paint::DisplayList;

use crate::event_loop::Loop;

/// Glifo das ferramentas que não trazem o próprio (`icon` no TOML).
const GENERIC_ICON: char = '\u{E8A5}';

#[derive(Default)]
pub struct CommandsRuntime {
    library: Library,
    tools: Vec<ToolEntry>,
    state: PanelState,
    /// O `cmd` de cada resultado mostrado, na ordem da tela.
    shown: Vec<String>,
    rows: Vec<(Rect, usize)>,
    header: Option<Rect>,
    items: Vec<(Rect, usize)>,
}

impl Loop {
    fn commands_open(&self) -> bool {
        self.controller.open().is_some_and(|o| o.kind == PanelKind::Commands)
    }

    /// Ao abrir o painel: lê a pasta do usuário (poucos arquivos pequenos, com limite), junta ao pacote
    /// embutido e volta à última ferramenta escolhida.
    pub fn commands_prepare(&mut self, kind: PanelKind) {
        if kind != PanelKind::Commands {
            return;
        }
        let dir = self.ctx.config_path.parent().map(|p| p.join("commands")).unwrap_or_default();
        let (user, warnings) = read_user_dir(&dir);
        for warning in warnings {
            tracing::warn!("comandos: {warning}");
        }
        let c = &mut self.commands;
        c.library = merge(builtin(), user);
        c.tools = c.library.tools.iter().map(|t| ToolEntry { id: t.id.clone(), name: t.name.clone(), icon: t.icon.unwrap_or(GENERIC_ICON) }).collect();
        c.state = PanelState::open(&self.ctx.config.commands.last, &c.tools);
        c.shown.clear();
    }

    pub fn commands_panel_list(&mut self, scale: f32) -> DisplayList {
        let c = &mut self.commands;
        let hits = c.library.search(c.state.tool.as_deref(), &c.state.query);
        // Busca vazia (ou só palavras vazias): o título diz o que a lista é.
        let heading = terms(&c.state.query).is_empty().then(|| if hits.iter().all(|h| h.command.popular) { "Mais usados" } else { "Comandos" });
        let rows: Vec<CommandRow> = hits.iter().map(|h| CommandRow { tool: h.tool.name.clone(), cmd: h.command.cmd.clone(), what: h.command.what.clone() }).collect();
        c.shown = rows.iter().map(|r| r.cmd.clone()).collect();
        c.state.selected = c.state.selected.min(rows.len().saturating_sub(1));
        let layout = commands_panel(&PanelView { tools: &c.tools, state: &c.state, rows: &rows, heading, measure: &self.ctx.text }, scale, &self.ctx.theme.tokens);
        c.rows = layout.rows;
        c.header = Some(layout.header);
        c.items = layout.dropdown_items;
        layout.list
    }

    /// Mouse sobre o painel: a linha (ou o item do dropdown) sob ele ganha o destaque.
    pub fn commands_move(&mut self, p: Point) -> Result<()> {
        if !self.commands_open() {
            return Ok(());
        }
        let c = &mut self.commands;
        let changed = if c.state.dropdown.is_some() {
            match c.items.iter().find(|(r, _)| r.contains(p)).map(|&(_, i)| i) {
                Some(i) if c.state.dropdown != Some(i) => {
                    c.state.dropdown = Some(i);
                    true
                }
                _ => false,
            }
        } else {
            match hovered_row(&c.rows, p) {
                Some(row) if row != c.state.selected || !c.state.navigated => {
                    c.state.selected = row;
                    c.state.navigated = true;
                    true
                }
                _ => false,
            }
        };
        if changed {
            self.redraw_open_panel()?;
        }
        Ok(())
    }

    /// Tecla digitada com o painel aberto: edita a busca.
    pub fn commands_char(&mut self, code: u32) -> Result<()> {
        if !self.commands_open() {
            return Ok(());
        }
        let paste = (code == 22).then(system_events::clipboard_text).flatten();
        if self.commands.state.on_char(code, paste.as_deref()) {
            self.redraw_open_panel()?;
        }
        Ok(())
    }

    /// Teclas com o painel aberto. `true` se tratou; o Esc final (nada a fechar ou limpar) fica para o
    /// tratamento de sempre, que fecha o painel e devolve o foco.
    pub fn commands_key(&mut self, vk: u32) -> Result<bool> {
        if !self.commands_open() {
            return Ok(false);
        }
        let c = &mut self.commands;
        match c.state.on_key(vk, c.shown.len(), &c.tools) {
            None | Some(Action::Close) => Ok(false),
            Some(action) => {
                self.commands_apply(action)?;
                Ok(true)
            }
        }
    }

    /// Roda sobre o painel. `true` se tratou.
    pub fn commands_wheel(&mut self, delta: i32) -> Result<bool> {
        if !self.commands_open() {
            return Ok(false);
        }
        let count = self.commands.shown.len();
        self.commands.state.wheel(delta, count);
        self.redraw_open_panel()?;
        Ok(true)
    }

    /// Clique no painel. `true` se o painel é o dos comandos.
    pub fn commands_click(&mut self, p: Point) -> Result<bool> {
        if !self.commands_open() {
            return Ok(false);
        }
        let c = &mut self.commands;
        let on_header = c.header.is_some_and(|h| h.contains(p));
        let action = if c.state.dropdown.is_some() {
            match c.items.iter().find(|(r, _)| r.contains(p)).map(|&(_, i)| i) {
                Some(index) => c.state.click_dropdown_item(index, &c.tools),
                None if on_header => {
                    c.state.click_header(&c.tools);
                    Action::Redraw
                }
                None => {
                    c.state.click_outside_dropdown();
                    Action::Redraw
                }
            }
        } else if on_header {
            c.state.click_header(&c.tools);
            Action::Redraw
        } else {
            match c.rows.iter().find(|(r, _)| r.contains(p)) {
                Some(&(_, index)) => Action::Copy(index),
                None => Action::None,
            }
        };
        self.commands_apply(action)?;
        Ok(true)
    }

    fn commands_apply(&mut self, action: Action) -> Result<()> {
        match action {
            Action::None => Ok(()),
            Action::Redraw => self.redraw_open_panel(),
            Action::Close => self.close_open_panel(),
            Action::SaveTool(id) => {
                // O recarregamento do config não fecha o painel (`Config::same_except_panels`).
                if let Err(e) = set_value(&self.ctx.config_path, &["commands", "last"], id.unwrap_or_default()) {
                    tracing::warn!("comandos: não consegui gravar a ferramenta escolhida: {e}");
                }
                self.redraw_open_panel()
            }
            Action::Copy(index) => self.commands_copy(index),
        }
    }

    /// Copia o comando (sem quebra de linha no fim) e fecha o painel.
    fn commands_copy(&mut self, index: usize) -> Result<()> {
        let (Some(cmd), Some(open)) = (self.commands.shown.get(index), self.controller.open()) else { return Ok(()) };
        let text = cmd.trim_end_matches(['\n', '\r']).to_string();
        if system_events::set_clipboard_text(self.panels[open.monitor].hwnd(), &text) {
            tracing::info!("comando copiado");
        } else {
            tracing::warn!("comandos: não consegui copiar para a área de transferência");
        }
        self.close_open_panel()
    }
}
