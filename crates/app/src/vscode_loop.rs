//! Projetos recentes do VS Code e do Cursor no laço de eventos (o mesmo código, um `Editor` por painel): leitura ao abrir o painel (numa thread), busca
//! digitada direto no painel, rolagem pela roda, ↑ ↓ para escolher e abrir no clique ou no Enter.

use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use windows::core::Result;
use zefiro_bar::controller::PanelKind;
use zefiro_bar::vscode::{RecentRow, Search, VISIBLE, edit_query, hovered_row, matches, max_scroll, vscode_panel};
use zefiro_integration_usage::format::format_short;
use zefiro_integration_vscode::{Editor, Kind, Recent, open_args, read_recents, wsl_distro};
use zefiro_platform::geometry::{Point, Rect};
use zefiro_platform_windows::{app_icon, system, window};
use zefiro_ui::paint::DisplayList;

use crate::event_loop::{Loop, USER_EDITOR_ICON, USER_VSCODE};

const MAX: usize = 20;

/// O que a thread de leitura devolve: de qual editor, a lista e o executável dele.
type Loaded = (Editor, Vec<Recent>, Option<PathBuf>);

#[derive(Default)]
pub struct VsCodeRuntime {
    /// `None` = lendo.
    recents: Option<Vec<Recent>>,
    /// Resultados das threads de leitura, pegos no `USER_VSCODE` (uma leitura antiga do outro editor pode
    /// chegar depois da nova; só vale a do editor aberto).
    incoming: Arc<Mutex<Vec<Loaded>>>,
    /// Qual editor o painel aberto mostra.
    editor: Editor,
    /// O executável do editor aberto.
    code: Option<PathBuf>,
    scroll: usize,
    /// Área de clique de cada linha e o índice do projeto em `recents`.
    hits: Vec<(Rect, usize)>,
    /// As mesmas áreas com a posição em `shown` (para o hover escolher a linha).
    rows: Vec<(Rect, usize)>,
    query: String,
    /// Projetos que passam na busca (índices em `recents`), na ordem mostrada.
    shown: Vec<usize>,
    /// Linha escolhida (posição em `shown`); só aparece depois de buscar ou usar as setas.
    selected: usize,
    navigated: bool,
    /// Resultado da thread que lê o ícone do Cursor (`Some(None)` = sem Cursor), pego no `USER_EDITOR_ICON`.
    icon_incoming: Arc<Mutex<Option<Option<zefiro_platform::image::Image>>>>,
    /// A leitura do ícone já foi pedida (e deu certo, ou ainda está em andamento).
    icon_started: bool,
}

/// Lado, em px, do ícone do chip lido do executável: o chip desenha menor e o painter reduz.
const ICON_PX: u32 = 64;

fn storage_dir(editor: Editor) -> PathBuf {
    editor.storage_dir(&PathBuf::from(std::env::var_os("APPDATA").unwrap_or_default()))
}

/// O editor de um tipo de painel.
fn editor_of(kind: PanelKind) -> Option<Editor> {
    match kind {
        PanelKind::VsCode => Some(Editor::Code),
        PanelKind::Cursor => Some(Editor::Cursor),
        _ => None,
    }
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
    /// Com o chip `cursor` na barra, lê o ícone do `Cursor.exe` uma vez, numa thread (o chip mostra o
    /// glifo até ele chegar). O ícone é o do próprio executável do usuário.
    pub fn editor_icon_services(&mut self) {
        if !self.ctx.config.bar.uses("cursor") || self.vscode.icon_started {
            return;
        }
        self.vscode.icon_started = true;
        let incoming = self.vscode.icon_incoming.clone();
        let spawned = std::thread::Builder::new().name("zefiro-editor-icon".into()).spawn(move || {
            let icon = system::find_editor_exe(Editor::Cursor).and_then(|exe| app_icon::file_icon(&exe, ICON_PX));
            if let Ok(mut slot) = incoming.lock() {
                *slot = Some(icon);
            }
            window::post_user(USER_EDITOR_ICON);
        });
        if let Err(e) = spawned {
            tracing::warn!("não consegui ler o ícone do Cursor: {e}");
        }
    }

    /// O ícone chegou: a barra é refeita com ele.
    pub fn editor_icon_loaded(&mut self) -> Result<()> {
        let Some(icon) = self.vscode.icon_incoming.lock().ok().and_then(|mut s| s.take()) else { return Ok(()) };
        if icon.is_none() {
            tracing::info!("ícone do Cursor indisponível (Cursor não encontrado); o chip usa o glifo");
            // Um Cursor instalado depois é achado na próxima vez que a configuração for salva.
            self.vscode.icon_started = false;
        }
        self.ctx.cursor_icon = icon;
        self.apply_bars(false)
    }

    /// O editor do painel aberto, se o painel aberto é de um deles.
    fn open_editor(&self) -> Option<Editor> {
        self.controller.open().and_then(|o| editor_of(o.kind))
    }

    /// Ao abrir o painel: lê a lista numa thread e procura o editor.
    pub fn vscode_prepare(&mut self, kind: PanelKind) {
        let Some(editor) = editor_of(kind) else { return };
        let v = &mut self.vscode;
        v.editor = editor;
        v.scroll = 0;
        v.recents = None;
        v.query.clear();
        v.selected = 0;
        v.navigated = false;
        let incoming = v.incoming.clone();
        let spawned = std::thread::Builder::new().name("zefiro-vscode".into()).spawn(move || {
            let code = system::find_editor_exe(editor);
            // Pastas do WSL só são conferidas com a distro rodando; parada, ficam (não dá para saber).
            let running = system::wsl_running_distros();
            let exists = |p: &std::path::Path| match wsl_distro(p) {
                Some(distro) if !running.iter().any(|r| r.eq_ignore_ascii_case(distro)) => true,
                _ => p.exists(),
            };
            let list = read_recents(&storage_dir(editor), &exists, MAX);
            if let Ok(mut slot) = incoming.lock() {
                slot.push((editor, list, code));
            }
            window::post_user(USER_VSCODE);
        });
        if let Err(e) = spawned {
            tracing::warn!("não consegui ler os projetos do {}: {e}", editor.name());
            v.recents = Some(Vec::new());
        }
    }

    /// A leitura terminou.
    pub fn vscode_loaded(&mut self) -> Result<()> {
        let arrived: Vec<Loaded> = self.vscode.incoming.lock().map(|mut s| std::mem::take(&mut *s)).unwrap_or_default();
        if let Some((_, list, code)) = arrived.into_iter().rev().find(|(editor, ..)| *editor == self.vscode.editor) {
            self.vscode.recents = Some(list);
            self.vscode.code = code;
        }
        if self.open_editor().is_some() {
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
        let missing = format!("{} não encontrado", v.editor.name());
        let (rows, message): (Vec<RecentRow>, Option<&str>) = match (&v.recents, &v.code) {
            (None, _) => (Vec::new(), Some("Carregando…")),
            (Some(_), None) => (Vec::new(), Some(missing.as_str())),
            (Some(list), Some(_)) => (v.shown.iter().map(|&i| row(&list[i], offset)).collect(), None),
        };
        let lit = (v.navigated || !v.query.is_empty()).then_some(v.selected);
        let search = Search { query: &v.query, selected: lit, measure: &self.ctx.text };
        let (list, hits, _) = vscode_panel(v.editor.name(), &rows, v.scroll, message, &search, scale, &self.ctx.theme.tokens);
        v.hits = hits.iter().map(|&(r, i)| (r, v.shown[i])).collect();
        v.rows = hits;
        list
    }

    /// Mouse sobre o painel: a linha sob ele ganha o destaque (só redesenha quando muda).
    pub fn vscode_move(&mut self, p: Point) -> Result<()> {
        if self.open_editor().is_none() {
            return Ok(());
        }
        let v = &mut self.vscode;
        if let Some(row) = hovered_row(&v.rows, p)
            && (row != v.selected || !v.navigated)
        {
            v.selected = row;
            v.navigated = true;
            self.redraw_open_panel()?;
        }
        Ok(())
    }

    /// Tecla digitada com o painel aberto: edita a busca, filtra e volta ao primeiro resultado.
    pub fn vscode_char(&mut self, code: u32) -> Result<()> {
        if self.open_editor().is_none() {
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
        if self.open_editor().is_none() {
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
            tracing::info!(projeto = %r.name, editor = self.vscode.editor.name(), "abrindo projeto no editor");
            if let Err(e) = system::spawn_detached(&code, &open_args(&r)) {
                tracing::warn!("não consegui abrir o {}: {e}", self.vscode.editor.name());
            }
            self.close_open_panel()?;
        }
        Ok(())
    }

    /// Roda sobre o painel. `true` se tratou.
    pub fn vscode_wheel(&mut self, delta: i32) -> Result<bool> {
        if self.open_editor().is_none() {
            return Ok(false);
        }
        let count = self.vscode.shown.len();
        let v = &mut self.vscode;
        v.scroll = if delta > 0 { v.scroll.saturating_sub(1) } else { (v.scroll + 1).min(max_scroll(count)) };
        self.redraw_open_panel()?;
        Ok(true)
    }

    /// Clique no painel: abre o projeto e fecha o painel. `true` se o painel é o do VS Code ou o do Cursor.
    pub fn vscode_click(&mut self, p: Point) -> Result<bool> {
        if self.open_editor().is_none() {
            return Ok(false);
        }
        if let Some(index) = self.vscode.hits.iter().find(|(r, _)| r.contains(p)).map(|(_, i)| *i) {
            self.vscode_open(index)?;
        }
        Ok(true)
    }
}
