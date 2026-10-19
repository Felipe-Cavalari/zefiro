//! Projetos recentes do VS Code no laço de eventos: leitura ao abrir o painel (numa thread),
//! rolagem pela roda e abrir o projeto no clique.

use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use windows::core::Result;
use zefiro_bar::controller::PanelKind;
use zefiro_bar::vscode::{RecentRow, max_scroll, vscode_panel};
use zefiro_integration_usage::format::format_short;
use zefiro_integration_vscode::{Kind, Recent, open_args, read_recents};
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
    hits: Vec<(Rect, usize)>,
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
        let incoming = v.incoming.clone();
        let spawned = std::thread::Builder::new().name("zefiro-vscode".into()).spawn(move || {
            let code = system::find_code_exe();
            let list = read_recents(&storage_dir(), &|p| p.exists(), MAX);
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
        let (rows, message): (Vec<RecentRow>, Option<&str>) = match (&v.recents, &v.code) {
            (None, _) => (Vec::new(), Some("Carregando…")),
            (Some(_), None) => (Vec::new(), Some("VS Code não encontrado")),
            (Some(list), Some(_)) => (list.iter().map(|r| row(r, offset)).collect(), None),
        };
        let (list, hits) = vscode_panel(&rows, v.scroll, message, scale, &self.ctx.theme.tokens);
        v.hits = hits;
        list
    }

    /// Roda sobre o painel. `true` se tratou.
    pub fn vscode_wheel(&mut self, delta: i32) -> Result<bool> {
        if !self.controller.open().is_some_and(|o| o.kind == PanelKind::VsCode) {
            return Ok(false);
        }
        let count = self.vscode.recents.as_ref().map_or(0, Vec::len);
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
        let hit = self.vscode.hits.iter().find(|(r, _)| r.contains(p)).map(|(_, i)| *i);
        let target = hit.and_then(|i| self.vscode.recents.as_ref()?.get(i).cloned());
        if let (Some(r), Some(code)) = (target, self.vscode.code.clone()) {
            tracing::info!(projeto = %r.name, "abrindo projeto no VS Code");
            if let Err(e) = system::spawn_detached(&code, &open_args(&r)) {
                tracing::warn!("não consegui abrir o VS Code: {e}");
            }
            self.close_open_panel()?;
        }
        Ok(true)
    }
}
