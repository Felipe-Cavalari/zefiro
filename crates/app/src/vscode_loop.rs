//! Projetos recentes do VS Code no laço de eventos: leitura ao abrir o painel (numa thread), busca
//! digitada direto no painel, rolagem pela roda, ↑ ↓ para escolher e abrir no clique ou no Enter.

use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use windows::core::Result;
use zefiro_bar::controller::PanelKind;
use zefiro_bar::vscode::{RecentRow, Search, VISIBLE, edit_query, matches, max_scroll, vscode_panel};
use zefiro_integration_usage::format::format_short;
use zefiro_integration_vscode::{Kind, Recent, open_args, read_recents, wsl_distro};
use zefiro_platform::geometry::{Point, Rect};
use zefiro_platform_windows::{system, window};
use zefiro_ui::paint::DisplayList;

use crate::event_loop::{Loop, USER_VSCODE};

const MAX: usize = 20;

/// O que a thread de leitura devolve: a lista e o `Code.exe`.
type Loaded = (Vec<Recent>, Option<PathBuf>);

#[derive(Default)]
pub struct VsCodeRuntime {
    /// `None` = lendo.
    recents: Option<Vec<Recent>>,
    /// Resultado da thread de leitura (lista e o `Code.exe`), pego no `USER_VSCODE`.
    incoming: Arc<Mutex<Option<Loaded>>>,
    code: Option<PathBuf>,
    scroll: usize,
    /// Área de clique de cada linha e o índice do projeto em `recents`.
    hits: Vec<(Rect, usize)>,
    query: String,
    /// Projetos que passam na busca (índices em `recents`), na ordem mostrada.
    shown: Vec<usize>,
    /// Linha escolhida (posição em `shown`); só aparece depois de buscar ou usar as setas.
    selected: usize,
    navigated: bool,
}

fn storage_dir() -> PathBuf {
    PathBuf::from(std::env::var_os("APPDATA").unwrap_or_default()).join(r"Code\User\workspaceStorage")
}

fn row(r: &Recent, offset: i32) -> RecentRow {
    let badge = match r.kind {
        Kind::Local => None,
        Kind::Wsl => Some("WSL"),
        Kind::Remote => Some("Remoto"),
        Kind::Workspace => Some("Workspace"),
    };
    let unix = r.modified.duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_secs() as i64);
    RecentRow { name: r.name.clone(), location: r.location.clone(), badge, date: format_short(unix, offset) }
}

impl Loop {
    /// Ao abrir o painel: lê a lista numa thread e procura o VS Code.
    pub fn vscode_prepare(&mut self, kind: PanelKind) {
        if kind != PanelKind::VsCode {
            return;
        }
        let v = &mut self.vscode;
        v.scroll = 0;
        v.recents = None;
        v.query.clear();
        v.selected = 0;
        v.navigated = false;
        let incoming = v.incoming.clone();
        let spawned = std::thread::Builder::new().name("zefiro-vscode".into()).spawn(move || {
            let code = system::find_code_exe();
            // Pastas do WSL só são conferidas com a distro rodando; parada, ficam (não dá para saber).
            let running = system::wsl_running_distros();
            let exists = |p: &std::path::Path| match wsl_distro(p) {
                Some(distro) if !running.iter().any(|r| r.eq_ignore_ascii_case(distro)) => true,
                _ => p.exists(),
            };
            let list = read_recents(&storage_dir(), &exists, MAX);
            if let Ok(mut slot) = incoming.lock() {
                *slot = Some((list, code));
            }
            window::post_user(USER_VSCODE);
        });
        if let Err(e) = spawned {
            tracing::warn!("não consegui ler os projetos do VS Code: {e}");
            v.recents = Some(Vec::new());
        }
    }

    /// A leitura terminou.
    pub fn vscode_loaded(&mut self) -> Result<()> {
        if let Some((list, code)) = self.vscode.incoming.lock().ok().and_then(|mut s| s.take()) {
            self.vscode.recents = Some(list);
            self.vscode.code = code;
        }
        if self.controller.open().is_some_and(|o| o.kind == PanelKind::VsCode) {
            self.redraw_open_panel()?;
        }
        Ok(())
    }

    pub fn vscode_panel_list(&mut self, scale: f32) -> DisplayList {
        let offset = zefiro_integration_clock::windows::utc_offset_seconds();
        let v = &mut self.vscode;
        v.shown = match &v.recents {
            Some(list) => list.iter().enumerate().filter(|(_, r)| matches(&v.query, &r.name, &r.location)).map(|(i, _)| i).collect(),
            None => Vec::new(),
        };
        v.selected = v.selected.min(v.shown.len().saturating_sub(1));
        // A linha escolhida fica sempre à vista.
        if v.selected < v.scroll {
            v.scroll = v.selected;
        } else if v.selected >= v.scroll + VISIBLE {
            v.scroll = v.selected + 1 - VISIBLE;
        }
        let (rows, message): (Vec<RecentRow>, Option<&str>) = match (&v.recents, &v.code) {
            (None, _) => (Vec::new(), Some("Carregando…")),
            (Some(_), None) => (Vec::new(), Some("VS Code não encontrado")),
            (Some(list), Some(_)) => (v.shown.iter().map(|&i| row(&list[i], offset)).collect(), None),
        };
        let lit = (v.navigated || !v.query.is_empty()).then_some(v.selected);
        let search = Search { query: &v.query, selected: lit, measure: &self.ctx.text };
        let (list, hits, _) = vscode_panel(&rows, v.scroll, message, &search, scale, &self.ctx.theme.tokens);
        v.hits = hits.into_iter().map(|(r, i)| (r, v.shown[i])).collect();
        list
    }

    /// Tecla digitada com o painel aberto: edita a busca, filtra e volta ao primeiro resultado.
    pub fn vscode_char(&mut self, code: u32) -> Result<()> {
        if !self.controller.open().is_some_and(|o| o.kind == PanelKind::VsCode) {
            return Ok(());
        }
        let paste = (code == 22).then(zefiro_platform_windows::system_events::clipboard_text).flatten();
        if edit_query(&mut self.vscode.query, code, paste.as_deref()) {
            self.vscode.selected = 0;
            self.vscode.scroll = 0;
            self.redraw_open_panel()?;
        }
        Ok(())
    }

    /// Teclas com o painel do VS Code aberto. `true` se tratou: Esc limpa a busca (vazia, fecha o
    /// painel como sempre), ↑ ↓ escolhem a linha e Enter abre.
    pub fn vscode_key(&mut self, vk: u32) -> Result<bool> {
        if !self.controller.open().is_some_and(|o| o.kind == PanelKind::VsCode) {
            return Ok(false);
        }
        let v = &mut self.vscode;
        match vk {
            0x1B if !v.query.is_empty() => {
                v.query.clear();
                v.selected = 0;
                v.scroll = 0;
                self.redraw_open_panel()?;
                Ok(true)
            }
            0x26 | 0x28 => {
                if v.navigated || !v.query.is_empty() {
                    v.selected = if vk == 0x26 { v.selected.saturating_sub(1) } else { (v.selected + 1).min(v.shown.len().saturating_sub(1)) };
                }
                v.navigated = true;
                self.redraw_open_panel()?;
                Ok(true)
            }
            0x0D => {
                if let Some(&index) = v.shown.get(v.selected) {
                    self.vscode_open(index)?;
                }
                Ok(true)
            }
            0x09 => Ok(true),
            _ => Ok(false),
        }
    }

    fn vscode_open(&mut self, index: usize) -> Result<()> {
        let target = self.vscode.recents.as_ref().and_then(|list| list.get(index).cloned());
        if let (Some(r), Some(code)) = (target, self.vscode.code.clone()) {
            tracing::info!(projeto = %r.name, "abrindo projeto no VS Code");
            if let Err(e) = system::spawn_detached(&code, &open_args(&r)) {
                tracing::warn!("não consegui abrir o VS Code: {e}");
            }
            self.close_open_panel()?;
        }
        Ok(())
    }

    /// Roda sobre o painel. `true` se tratou.
    pub fn vscode_wheel(&mut self, delta: i32) -> Result<bool> {
        if !self.controller.open().is_some_and(|o| o.kind == PanelKind::VsCode) {
            return Ok(false);
        }
        let count = self.vscode.shown.len();
        let v = &mut self.vscode;
        v.scroll = if delta > 0 { v.scroll.saturating_sub(1) } else { (v.scroll + 1).min(max_scroll(count)) };
        self.redraw_open_panel()?;
        Ok(true)
    }

    /// Clique no painel: abre o projeto e fecha o painel. `true` se o painel é o do VS Code.
    pub fn vscode_click(&mut self, p: Point) -> Result<bool> {
        if !self.controller.open().is_some_and(|o| o.kind == PanelKind::VsCode) {
            return Ok(false);
        }
        if let Some(index) = self.vscode.hits.iter().find(|(r, _)| r.contains(p)).map(|(_, i)| *i) {
            self.vscode_open(index)?;
        }
        Ok(true)
    }
}
