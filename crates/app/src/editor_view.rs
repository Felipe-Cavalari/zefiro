//! Orquestra o editor: sessão portável, prévia real e mensagens do worker.

use windows::core::Result;
use zefiro_bar::editor::{EditorSession, Section};
use zefiro_bar::editor_panel::{EditorAction, EditorContext, EditorLayout, EditorPage, EditorPanel, PanelDrag, Target, WIDTH_DIP, Zone};
use zefiro_core::config::{LayoutMode, parse_config};
use zefiro_core::config_edit::{PatchChoice, PatchError};
use zefiro_integration_clock::format::ClockText;
use zefiro_platform::geometry::{Point, Rect};
use zefiro_platform_windows::window::{self, WinEvent};
use crate::{config_worker::{ConfigWorker, SaveError}, event_loop::{Loop, USER_EDITOR_IO, USER_CONFIG_CHANGED}};

pub struct EditorRuntime {
    pub session: EditorSession,
    pub ui: EditorPanel,
    pub monitor: usize,
    choices: Vec<(Vec<String>, PatchChoice)>,
    conflicts: Vec<Vec<String>>,
    drag: Option<Drag>,
    format_field: Option<zefiro_platform_windows::text_input::TextInput>,
    movement_ms: Vec<f64>,
    suspended: bool,
}

impl EditorRuntime {
    /// Guarda os campos em conflito e anuncia o primeiro; a mensagem "Conflito:" habilita os botões de resolver.
    fn record_conflict(&mut self, error: PatchError) {
        self.conflicts = error.fields;
        self.ui.message = if let Some(field) = self.conflicts.first() { format!("Conflito: {}", field.join(".")) } else { error.message };
    }
}

struct Drag { id: String, origin: Point, active: bool, target: Option<(Section, usize)> }

impl Loop {
    fn ensure_editor_worker(&mut self) -> bool {
        if self.editor_worker.is_none() {
            match ConfigWorker::new() {
                Ok(worker) => self.editor_worker = Some(worker),
                Err(error) => { tracing::error!("não consegui iniciar o worker de configuração: {error}"); return false; }
            }
        }
        true
    }

    pub fn start_editor(&mut self, monitor: usize) -> Result<()> {
        if self.editor.is_none() { self.editor_started = Some(std::time::Instant::now()); }
        self.close_panel_for_editor()?;
        if let Some(editor) = &mut self.editor {
            editor.suspended = false;
            editor.format_field = None;
            editor.ui.editing_format = false;
            editor.drag = None;
            window::release_capture();
            if editor.monitor != monitor { self.panels[editor.monitor].close(&self.ctx, false)?; }
        }
        if let Some(editor) = self.editor.as_mut() {
            editor.session.select_monitor(self.bars[monitor].monitor().identity.id.clone());
            editor.monitor = monitor;
            return self.redraw_editor();
        }
        if !self.ensure_editor_worker() { return Ok(()); }
        self.editor_generation += 1;
        self.editor_pending = Some(monitor);
        if let Some(worker) = &self.editor_worker { worker.read(self.editor_generation, self.ctx.config_path.clone()); }
        Ok(())
    }

    pub fn redraw_editor(&mut self) -> Result<()> {
        let selected = self.editor.as_ref().map(|editor| editor.monitor);
        for (index, bar) in self.bars.iter_mut().enumerate() { bar.show_edit_guides(&self.ctx, selected == Some(index))?; }
        let Some(editor) = self.editor.as_ref() else { return Ok(()) };
        if editor.suspended { return Ok(()); }
        let Some(bar) = self.bars.get(editor.monitor) else { return Ok(()) };
        let scale = bar.scale();
        let Some(layout) = self.editor_layout() else { return Ok(()) };
        let list = editor.ui.paint(&layout, &editor.session, &self.ctx.theme.tokens);
        let anchor = Rect::new(bar.monitor().rect.x + 8.0 * scale, bar.monitor().rect.y, 40.0 * scale, bar.band_bottom() - bar.monitor().rect.y);
        if self.panels[editor.monitor].open {
            self.panels[editor.monitor].resize(&self.ctx, anchor, bar.band_bottom(), &list)?;
        } else {
            self.panels[editor.monitor].open(&self.ctx, anchor, bar.band_bottom(), &list)?;
        }
        let (ox, oy) = self.panels[editor.monitor].content_origin();
        let field_rect = layout.buttons.iter().find(|b| b.action == EditorAction::EditFormat).map(|b| Rect::new(b.rect.x + ox, b.rect.y + oy, b.rect.w, b.rect.h));
        let owner = self.panels[editor.monitor].hwnd();
        if let Some(editor) = &mut self.editor {
            if editor.ui.editing_format && editor.ui.page == EditorPage::Options {
                if editor.format_field.is_none() && let Some(rect) = field_rect {
                    let field = zefiro_platform_windows::text_input::TextInput::new(owner, rect, &editor.session.draft.clock.format, self.ctx.theme.theme.colors.background, self.ctx.theme.tokens.text)?;
                    field.focus();
                    editor.format_field = Some(field);
                }
            } else { editor.format_field = None; }
        }
        Ok(())
    }

    /// O nome que o Windows dá é quase sempre "Generic PnP Monitor"; a posição diz mais.
    fn editor_monitor_names(&self) -> Vec<String> {
        self.bars.iter().enumerate().map(|(i, bar)| if bar.monitor().primary { "Principal".into() } else { format!("Monitor {}", i + 1) }).collect()
    }

    /// Layout do painel do editor no monitor dele (tamanho e posição de chips, seções e botões).
    fn editor_layout(&self) -> Option<EditorLayout> {
        let editor = self.editor.as_ref()?;
        let bar = self.bars.get(editor.monitor)?;
        let scale = bar.scale();
        let names = self.editor_monitor_names();
        let cx = EditorContext {
            session: &editor.session,
            monitors: &names,
            monitor: editor.monitor,
            island_locked: bar.monitor().primary && editor.session.draft.island.external(),
            scale,
            width: (WIDTH_DIP * scale).min(bar.monitor().rect.w - 16.0 * scale),
        };
        Some(editor.ui.layout(&cx, &self.ctx.text))
    }

    /// Ponto do evento na área do conteúdo do painel do editor, se o evento veio desse painel.
    fn editor_panel_point(&self, hwnd: isize, x: f32, y: f32) -> Option<Point> {
        let editor = self.editor.as_ref()?;
        let index = self.panel_index(hwnd).filter(|i| *i == editor.monitor)?;
        let (ox, oy) = self.panels[index].content_origin();
        Some(Point { x: x - ox, y: y - oy })
    }

    fn apply_editor_preview(&mut self) -> Result<()> {
        if let Some(editor) = &self.editor { self.ctx.config = editor.session.draft.clone(); }
        self.apply_bars(true)?;
        self.schedule_clock();
        self.redraw_editor()
    }

    fn finish_editor(&mut self, saved: Option<String>) -> Result<()> {
        window::release_capture();
        for bar in &mut self.bars { bar.clear_drag()?; bar.show_edit_guides(&self.ctx, false)?; }
        if let Some(mut editor) = self.editor.take() {
            if !editor.movement_ms.is_empty() {
                editor.movement_ms.sort_by(f64::total_cmp);
                let p95 = editor.movement_ms[(editor.movement_ms.len() * 95).div_ceil(100) - 1];
                tracing::info!(amostras = editor.movement_ms.len(), p95_ms = p95, "processamento de movimento do editor");
            }
            editor.format_field = None;
            self.ctx.config = match saved {
                Some(text) => parse_config(&text).unwrap_or(editor.session.confirmed),
                None => editor.session.confirmed,
            };
            if let Some(panel) = self.panels.get_mut(editor.monitor) { panel.close(&self.ctx, true)?; }
        }
        self.editor_pending = None;
        self.editor_generation += 1;
        self.editor_worker = None;
        let (id, dir) = (self.ctx.config.theme.clone(), self.ctx.themes_dir.clone());
        self.ctx.theme.reload(&id, &dir);
        self.ctx.text.set_family(&self.ctx.theme.theme.font.family);
        // Chips adicionados ou removidos no editor ligam ou desligam os serviços deles.
        self.refresh_services();
        self.apply_bars(true)?;
        self.schedule_clock();
        // Ocultar automaticamente e só no principal valem ao concluir (a prévia não os aplica).
        self.clean_bar_sync()?;
        window::post_user(USER_CONFIG_CHANGED);
        Ok(())
    }

    fn editor_io(&mut self) -> Result<()> {
        let replies = self.editor_worker.as_ref().map(ConfigWorker::replies).unwrap_or_default();
        for reply in replies {
            if reply.generation != self.editor_generation { continue; }
            if reply.saved {
                match reply.result {
                    Ok(text) => { self.finish_editor(Some(text))?; }
                    Err(error) => if let Some(editor) = &mut self.editor {
                        editor.session.saving = false;
                        match error {
                            SaveError::Conflict(text) => {
                                match parse_config(&text) {
                                    Ok(config) => editor.session.external_change(config, text),
                                    Err(error) => editor.session.external_invalid(text, error.message),
                                }
                                editor.session.conflict = true;
                                editor.ui.message = "O arquivo mudou durante a gravação; reconcilie as alterações".into();
                            }
                            SaveError::Io(message) => editor.ui.message = format!("Não foi possível gravar: {message}"),
                        }
                        self.redraw_editor()?;
                    },
                }
                continue;
            }
            match reply.result {
                Ok(text) => match parse_config(&text) {
                    Ok(config) => {
                        if let Some(monitor) = self.editor_pending.take() {
                            let monitor = monitor.min(self.bars.len().saturating_sub(1));
                            let mut session = EditorSession::new(config, text);
                            session.generation = reply.generation;
                            if let Some(bar) = self.bars.get(monitor) { session.select_monitor(bar.monitor().identity.id.clone()); }
                            self.editor = Some(EditorRuntime { session, ui: EditorPanel::default(), monitor, choices: Vec::new(), conflicts: Vec::new(), drag: None, format_field: None, movement_ms: Vec::new(), suspended: false });
                            let prepared = std::time::Instant::now();
                            let preparation_ms = self.editor_started.map_or(0.0, |started| started.elapsed().as_secs_f64() * 1000.0);
                            self.apply_editor_preview()?;
                            if let Some(started) = self.editor_started.take() { tracing::info!(ms = started.elapsed().as_secs_f64() * 1000.0, preparation_ms, desenho_ms = prepared.elapsed().as_secs_f64() * 1000.0, "editor pronto para composição"); }
                        } else if let Some(editor) = &mut self.editor {
                            if text != editor.session.latest_text {
                                editor.session.external_change(config, text);
                                editor.choices.clear();
                                editor.conflicts.clear();
                                editor.ui.message = "A configuração mudou fora do editor".into();
                            }
                            self.redraw_editor()?;
                        }
                    }
                    Err(error) => {
                        if let Some(editor) = &mut self.editor { editor.session.external_invalid(text, error.message.clone()); editor.ui.message = format!("Arquivo inválido: {}", error.message); }
                        tracing::warn!("configuração inválida ao editar: {}", error.message);
                        if let Some(monitor) = self.editor_pending.take() && let Some(panel) = self.panels.get(monitor) { panel.restore_focus(); }
                        self.redraw_editor()?;
                    }
                },
                Err(error) => {
                    if let Some(editor) = &mut self.editor { editor.session.conflict = true; editor.ui.message = format!("Não foi possível ler o arquivo: {error:?}"); }
                    tracing::warn!("não consegui ler a configuração para editar: {error:?}");
                    if let Some(monitor) = self.editor_pending.take() && let Some(panel) = self.panels.get(monitor) { panel.restore_focus(); }
                    self.redraw_editor()?;
                }
            }
        }
        Ok(())
    }

    fn reapply_editor(editor: &mut EditorRuntime) {
        if editor.session.file_error.is_some() { return; }
        match editor.session.patch().apply(&editor.session.original_text, &editor.session.latest_text, &editor.choices) {
            Ok(text) => if let Ok(config) = parse_config(&text) {
                editor.session.rebase_onto_latest(config);
                // As escolhas valiam para a base antiga; depois do rebase já estão no rascunho.
                editor.choices.clear();
                editor.conflicts.clear();
                editor.ui.message = "Alterações reaplicadas; confira e conclua".into();
            },
            Err(error) => editor.record_conflict(error),
        }
    }

    pub fn editor_action(&mut self, action: EditorAction) -> Result<()> {
        let Some(mut editor) = self.editor.take() else { return Ok(()) };
        if editor.session.saving { self.editor = Some(editor); return Ok(()); }
        let connected = self.bars.iter().filter_map(|bar| bar.monitor().identity.id.clone()).collect::<Vec<_>>();
        let mut error = None;
        match action {
            EditorAction::Cancel => { self.editor = Some(editor); return self.finish_editor(None); }
            EditorAction::Save => {
                if let Err(message) = self.ctx.clock.validate_format(&editor.session.draft.clock.format) { error = Some(message); }
                else if !editor.session.conflict {
                    match editor.session.patch().apply(&editor.session.original_text, &editor.session.latest_text, &editor.choices) {
                        Ok(output) => {
                            self.editor_generation += 1;
                            editor.session.saving = true;
                            if let Some(worker) = &self.editor_worker { worker.save(self.editor_generation, self.ctx.config_path.clone(), editor.session.latest_text.clone(), output); }
                        }
                        Err(failure) => {
                            // Com campos em conflito, mostra os botões de resolver em vez de só a mensagem.
                            if !failure.fields.is_empty() { editor.session.conflict = true; }
                            editor.record_conflict(failure);
                        }
                    }
                }
            }
            EditorAction::Monitor(index) => if let Some(bar) = self.bars.get(index) {
                editor.format_field = None;
                editor.ui.editing_format = false;
                editor.drag = None;
                window::release_capture();
                self.panels[editor.monitor].close(&self.ctx, false)?;
                editor.session.select_monitor(bar.monitor().identity.id.clone());
                editor.monitor = index;
                editor.ui.message = if bar.monitor().identity.id.is_none() { "Identificação indisponível; usando layout compartilhado".into() } else { "Monitor selecionado".into() };
            },
            EditorAction::ToggleMode => {
                let mode = if editor.session.draft.bar.layout_mode == LayoutMode::Shared { LayoutMode::PerMonitor } else { LayoutMode::Shared };
                editor.session.set_mode(mode, &connected);
                editor.ui.message = if mode == LayoutMode::Shared { "O layout selecionado será usado em todos; layouts próprios preservados".into() } else { "Layouts próprios recuperados; novas telas recebem cópia do compartilhado".into() };
            }
            EditorAction::SelectItem(id) => { editor.ui.selected = Some(id); editor.ui.message = "Arraste para mover; Espaço move pelo teclado".into(); }
            EditorAction::Add(id) => {
                error = editor.session.add(&id, Section::End).err();
                editor.ui.page = EditorPage::Items;
                editor.ui.selected = Some(id);
            }
            EditorAction::Remove => if let Some(id) = editor.ui.selected.take() { error = editor.session.remove(&id).err(); },
            EditorAction::MoveSection(section) => if let Some(id) = &editor.ui.selected {
                let len = section.items(editor.session.sections()).len();
                error = editor.session.move_item(id, section, len).err();
                if section == Section::Center && self.bars[editor.monitor].monitor().primary && editor.session.draft.island.external() {
                    editor.ui.message = "Oculto neste monitor enquanto a ilha estiver ligada".into();
                }
            },
            EditorAction::MoveBefore | EditorAction::MoveAfter => if let Some(id) = &editor.ui.selected
                && let Some((section, index)) = Section::ALL.into_iter().find_map(|s| s.items(editor.session.sections()).iter().position(|item| item == id).map(|i| (s, i))) {
                    let index = if action == EditorAction::MoveBefore { index.saturating_sub(1) } else { index + 1 };
                    error = editor.session.move_item(id, section, index).err();
            },
            EditorAction::Options => {
                editor.ui.page = EditorPage::Options; editor.ui.focus = None; editor.ui.hover = None;
                editor.ui.message = "Aplica-se a todos os relógios".into();
            }
            EditorAction::Back => { editor.ui.page = EditorPage::Items; editor.ui.focus = None; editor.ui.hover = None; editor.ui.editing_format = false; editor.ui.message.clear(); }
            EditorAction::ToggleSeconds => editor.session.draft.clock.seconds = !editor.session.draft.clock.seconds,
            EditorAction::ToggleAutohide => {
                editor.session.toggle_autohide();
                editor.ui.message = if editor.session.draft.bar.autohide { "Ao concluir, a barra só aparece com o mouse no topo da tela".into() } else { "Ao concluir, a barra fica sempre visível".into() };
            }
            EditorAction::TogglePrimaryOnly => {
                editor.session.toggle_primary_only();
                editor.ui.message = if editor.session.draft.bar.display == zefiro_core::config::BarDisplay::Principal { "Ao concluir, só o monitor principal terá barra; os layouts dos outros ficam guardados".into() } else { "Ao concluir, todos os monitores terão barra".into() };
            }
            EditorAction::EditFormat => { editor.ui.editing_format = true; editor.ui.format_select_all = true; editor.ui.message = "Digite o padrão de data do Windows; Enter confirma, Ctrl+A seleciona tudo".into(); }
            EditorAction::SystemFormat => { editor.session.draft.clock.format.clear(); editor.ui.editing_format = false; }
            EditorAction::Reload => { editor.session.reload(); editor.choices.clear(); editor.conflicts.clear(); editor.ui.message = "Configuração recarregada".into(); }
            EditorAction::Reapply => Self::reapply_editor(&mut editor),
            EditorAction::ResolveFile | EditorAction::ResolveDraft => {
                if let Some(field) = editor.conflicts.first().cloned() {
                    editor.choices.push((field, if action == EditorAction::ResolveFile { PatchChoice::File } else { PatchChoice::Draft }));
                    Self::reapply_editor(&mut editor);
                }
            }
        }
        if let Some(error) = error { editor.ui.message = error; }
        if let Err(error) = self.ctx.clock.validate_format(&editor.session.draft.clock.format) { editor.ui.message = error; }
        self.editor = Some(editor);
        self.apply_editor_preview()
    }

    /// Soltou um chip arrastado no painel: move entre as seções, adiciona (vindo da prateleira) ou
    /// tira da barra (solto na prateleira). Fora das zonas, nada muda.
    fn editor_drop(&mut self, id: &str, target: Option<(Zone, usize)>) -> Result<()> {
        let Some(editor) = self.editor.as_mut() else { return Ok(()) };
        let present = editor.session.sections().ids().any(|item| item == id);
        let result = match target {
            None => Ok(()),
            Some((Zone::Shelf, _)) if present => editor.session.remove(id),
            Some((Zone::Shelf, _)) => Ok(()),
            Some((Zone::Section(section), index)) => {
                let added = if present { Ok(()) } else { editor.session.add(id, section) };
                added.and_then(|_| editor.session.move_item(id, section, index))
            }
        };
        editor.ui.message = match (result, target) {
            (Err(error), _) => error,
            (Ok(()), Some((Zone::Shelf, _))) => "Item tirado da barra".into(),
            (Ok(()), Some((Zone::Section(section), _))) => format!("Movido para {}", section.label()),
            (Ok(()), None) => String::new(),
        };
        editor.ui.selected = match target {
            Some((Zone::Section(_), _)) => Some(id.into()),
            _ => None,
        };
        self.apply_editor_preview()
    }

    pub fn editor_topology_changed(&mut self) -> Result<()> {
        if let Some(editor) = &mut self.editor {
            window::release_capture();
            editor.drag = None;
            editor.format_field = None;
            editor.ui.editing_format = false;
            editor.session.cancel_move();
            if let Some(index) = self.bars.iter().position(|bar| bar.monitor().identity.id == editor.session.monitor_id) {
                editor.monitor = index;
            } else {
                editor.monitor = 0;
                editor.session.select_monitor(self.bars.first().and_then(|bar| bar.monitor().identity.id.clone()));
            }
        }
        self.redraw_editor()
    }

    pub fn prepare_editor_rebuild(&mut self) {
        window::release_capture();
        if let Some(editor) = &mut self.editor {
            editor.format_field = None;
            editor.ui.editing_format = false;
            editor.drag = None;
            editor.session.cancel_move();
            self.ctx.config = editor.session.draft.clone();
        }
    }

    pub fn suspend_editor(&mut self, suspended: bool) -> Result<()> {
        self.prepare_editor_rebuild();
        if let Some(editor) = &mut self.editor {
            editor.suspended = suspended;
            if suspended {
                if let Some(panel) = self.panels.get_mut(editor.monitor) { panel.close(&self.ctx, false)?; }
            } else { self.redraw_editor()?; }
        }
        Ok(())
    }

    pub fn editor_event(&mut self, event: WinEvent) -> Result<bool> {
        if event == (WinEvent::User { code: USER_EDITOR_IO }) { self.editor_io()?; return Ok(true); }
        if self.editor.is_none() { return Ok(false); }
        match event {
            WinEvent::User { code: USER_CONFIG_CHANGED } => {
                if !self.editor.as_ref().is_some_and(|e| e.session.saving) {
                    self.editor_generation += 1;
                    if let Some(worker) = &self.editor_worker { worker.read(self.editor_generation, self.ctx.config_path.clone()); }
                }
            }
            WinEvent::Close { hwnd } if self.panel_index(hwnd).is_some() => { self.editor_action(EditorAction::Cancel)?; }
            WinEvent::FocusLost { .. } => {
                if !self.panels.iter().any(|panel| panel.has_focus()) && !self.editor.as_ref().and_then(|editor| editor.format_field.as_ref()).is_some_and(|field| field.has_focus()) {
                    window::release_capture();
                    if let Some(editor) = &mut self.editor { editor.drag = None; editor.session.cancel_move(); editor.ui.moving = false; editor.ui.editing_format = false; editor.format_field = None; }
                    for bar in &mut self.bars { bar.clear_drag()?; }
                    self.apply_editor_preview()?;
                }
            }
            WinEvent::CaptureLost { .. } => { if let Some(editor) = &mut self.editor { editor.drag = None; editor.ui.drag = None; editor.ui.pressed = None; editor.session.cancel_move(); } for bar in &mut self.bars { bar.clear_drag()?; } self.apply_editor_preview()?; }
            WinEvent::RightUp { hwnd, .. } => if let Some(index) = self.bar_index(hwnd) { self.start_editor(index)?; },
            WinEvent::LeftDown { hwnd, x, y } if self.editor_panel_point(hwnd, x, y).is_some() => {
                let p = self.editor_panel_point(hwnd, x, y).expect("conferido acima");
                let Some(layout) = self.editor_layout() else { return Ok(true) };
                let monitor = self.editor.as_ref().map_or(0, |e| e.monitor);
                let editor = self.editor.as_mut().expect("sessão verificada");
                if editor.session.saving { return Ok(true); }
                match EditorPanel::hit(&layout, p) {
                    Some(Target::Chip(id)) => {
                        editor.ui.drag = Some(PanelDrag { id, origin: p, point: p, active: false });
                        window::capture(self.panels[monitor].hwnd());
                    }
                    Some(target @ Target::Button(_)) => editor.ui.pressed = Some(target),
                    None => return Ok(true),
                }
                self.redraw_editor()?;
            }
            WinEvent::MouseMove { hwnd, x, y } if self.editor_panel_point(hwnd, x, y).is_some() => {
                let p = self.editor_panel_point(hwnd, x, y).expect("conferido acima");
                let Some(layout) = self.editor_layout() else { return Ok(true) };
                let scale = self.bars[self.editor.as_ref().map_or(0, |e| e.monitor)].scale();
                let editor = self.editor.as_mut().expect("sessão verificada");
                let changed = if let Some(drag) = &mut editor.ui.drag {
                    drag.point = p;
                    drag.active |= zefiro_ui::edit_layout::starts_drag(drag.origin, p, scale);
                    let active = drag.active;
                    if active {
                        editor.ui.message = match editor.ui.drag_target(&layout) {
                            Some((Zone::Shelf, _)) => "Solte para tirar da barra".into(),
                            Some((Zone::Section(section), index)) => format!("Mover para {} · posição {}", section.label(), index + 1),
                            None => "Fora das seções: solte para cancelar".into(),
                        };
                    }
                    active
                } else {
                    let hover = EditorPanel::hit(&layout, p);
                    let changed = hover != editor.ui.hover;
                    editor.ui.hover = hover;
                    changed
                };
                if changed { self.redraw_editor()?; }
            }
            WinEvent::MouseLeave { hwnd } if self.editor.as_ref().is_some_and(|e| self.panel_index(hwnd) == Some(e.monitor)) => {
                let editor = self.editor.as_mut().expect("sessão verificada");
                if editor.ui.hover.take().is_some() && editor.ui.drag.is_none() { self.redraw_editor()?; }
            }
            WinEvent::LeftUp { hwnd, x, y } if self.editor_panel_point(hwnd, x, y).is_some() => {
                let p = self.editor_panel_point(hwnd, x, y).expect("conferido acima");
                let Some(layout) = self.editor_layout() else { return Ok(true) };
                let editor = self.editor.as_mut().expect("sessão verificada");
                let pressed = editor.ui.pressed.take();
                if let Some(drag) = editor.ui.drag.take() {
                    window::release_capture();
                    if !drag.active {
                        // Clique sem arrastar: seleciona (o Relógio ganha o botão de opções).
                        return self.editor_action(EditorAction::SelectItem(drag.id)).map(|_| true);
                    }
                    let target = EditorPanel::zone_at(&layout, p).map(|zone| (zone, EditorPanel::drop_index(&layout, zone, p, &drag.id)));
                    return self.editor_drop(&drag.id, target).map(|_| true);
                }
                match EditorPanel::hit(&layout, p) {
                    Some(Target::Button(action)) if pressed == Some(Target::Button(action.clone())) && layout.buttons.iter().any(|b| b.action == action && b.enabled) => self.editor_action(action)?,
                    _ => self.redraw_editor()?,
                }
            }
            WinEvent::LeftDown { hwnd, x, y } => {
                if let Some(index) = self.bar_index(hwnd) {
                    let Some(id) = self.bars[index].chip_at(x, y).filter(|id| !id.starts_with("__more")) else { return Ok(true) };
                    let editor = self.editor.as_mut().expect("sessão verificada");
                    if editor.monitor == index && !editor.session.saving {
                        editor.ui.selected = Some(id.clone());
                        editor.drag = Some(Drag { id, origin: Point { x, y }, active: false, target: None });
                        window::capture(self.bars[index].hwnd());
                        self.redraw_editor()?;
                    }
                }
            }
            WinEvent::MouseMove { hwnd, x, y } => {
                let started = std::time::Instant::now();
                if let Some(index) = self.bar_index(hwnd) {
                    let Some(mut editor) = self.editor.take() else { return Ok(true) };
                    let mut changed = false;
                    if let Some(drag) = &mut editor.drag {
                        if !drag.active && zefiro_ui::edit_layout::starts_drag(drag.origin, Point { x, y }, self.bars[index].scale()) {
                            drag.active = editor.session.begin_move(&drag.id).is_ok();
                        }
                        if drag.active {
                            let point = Point { x, y };
                            let blocked = self.bars[index].monitor().primary && editor.session.draft.island.external();
                            let section = self.bars[index].edit_regions().iter().position(|r| r.contains(point));
                            let target = section.filter(|s| !(*s == 1 && blocked)).map(|s| {
                                let section = Section::ALL[s];
                                let ids: Vec<_> = section.items(editor.session.sections()).iter().filter(|id| *id != &drag.id).collect();
                                let rects = ids.iter().filter_map(|id| self.bars[index].chip_rect(id)).collect::<Vec<_>>();
                                let position = zefiro_ui::edit_layout::insert_index(&rects, x + self.bars[index].monitor().rect.x);
                                (section, position)
                            });
                            if target != drag.target {
                                drag.target = target;
                                if let Some((section, position)) = target { let _ = editor.session.move_item(&drag.id, section, position); }
                                changed = true;
                            }
                            let marker = target.map(|(section, position)| {
                                let ids: Vec<_> = section.items(editor.session.sections()).iter().filter(|id| *id != &drag.id).collect();
                                let region = self.bars[index].edit_regions()[Section::ALL.iter().position(|s| *s == section).unwrap_or(0)];
                                let x = ids.get(position).and_then(|id| self.bars[index].chip_rect(id)).map_or(region.x, |r| r.x - self.bars[index].monitor().rect.x);
                                Rect::new(x, 2.0 * self.bars[index].scale(), 2.0 * self.bars[index].scale(), region.h - 4.0 * self.bars[index].scale())
                            });
                            self.bars[index].show_drag(&self.ctx, &drag.id, point, marker)?;
                            editor.ui.message = target.map_or("Destino inválido; solte para cancelar".into(), |(s, i)| format!("Mover para {} · posição {}", s.label(), i + 1));
                        }
                    }
                    self.editor = Some(editor);
                    if changed { self.apply_editor_preview()?; }
                    if let Some(editor) = &mut self.editor && editor.drag.as_ref().is_some_and(|drag| drag.active) && editor.movement_ms.len() < 4096 {
                        editor.movement_ms.push(started.elapsed().as_secs_f64() * 1000.0);
                    }
                }
            }
            WinEvent::LeftUp { hwnd, .. } => {
                if let Some(index) = self.bar_index(hwnd) {
                    let editor = self.editor.as_mut().expect("sessão verificada");
                    if editor.monitor == index {
                        if let Some(drag) = editor.drag.take() && drag.active {
                                if let Some((section, index)) = drag.target { let _ = editor.session.move_item(&drag.id, section, index); editor.session.finish_move(); }
                                else { editor.session.cancel_move(); }
                        }
                        window::release_capture();
                        self.bars[index].clear_drag()?;
                        self.apply_editor_preview()?;
                    }
                }
            }
            // O painel cresce com o conteúdo; não há o que rolar.
            WinEvent::Wheel { .. } => {}
            WinEvent::Key { vk, .. } => { self.editor_key(vk)?; }
            WinEvent::ControlChanged { control, .. } => {
                if let Some(editor) = &mut self.editor && !editor.session.saving
                    && let Some(field) = &editor.format_field && field.handle() == control {
                    editor.session.draft.clock.format = field.text();
                    self.apply_editor_preview()?;
                }
            }
            WinEvent::Character { code, .. } => {
                if let Some(editor) = &mut self.editor && editor.ui.editing_format && !editor.session.saving {
                    if code == 1 { editor.ui.format_select_all = true; }
                    else if code == 8 {
                        if editor.ui.format_select_all { editor.session.draft.clock.format.clear(); } else { editor.session.draft.clock.format.pop(); }
                        editor.ui.format_select_all = false;
                    } else if code >= 32 && let Some(character) = char::from_u32(code) {
                        if editor.ui.format_select_all { editor.session.draft.clock.format.clear(); }
                        editor.ui.format_select_all = false;
                        if editor.session.draft.clock.format.encode_utf16().count() < 1024 { editor.session.draft.clock.format.push(character); }
                    }
                    self.apply_editor_preview()?;
                }
            }
            _ => return Ok(false),
        }
        Ok(true)
    }

    fn editor_key(&mut self, vk: u32) -> Result<()> {
        let Some(editor) = &self.editor else { return Ok(()) };
        if editor.session.saving { return Ok(()); }
        if vk == 0x1B {
            let editor = self.editor.as_mut().expect("sessão verificada");
            if editor.drag.is_some() || editor.ui.moving {
                editor.drag = None; editor.ui.moving = false; editor.session.cancel_move(); window::release_capture();
                for bar in &mut self.bars { bar.clear_drag()?; }
                return self.apply_editor_preview();
            }
            if editor.ui.editing_format { editor.ui.editing_format = false; return self.redraw_editor(); }
            if editor.ui.page != EditorPage::Items { return self.editor_action(EditorAction::Back); }
            return self.editor_action(EditorAction::Cancel);
        }
        if editor.ui.editing_format {
            if vk == 0x0D || vk == 0x09 { self.editor.as_mut().expect("sessão verificada").ui.editing_format = false; return self.redraw_editor(); }
            return Ok(());
        }
        if (vk == 0x20 || vk == 0x0D) && !editor.ui.moving {
            match editor.ui.focus.clone() {
                Some(Target::Chip(id)) => {
                    let editor = self.editor.as_mut().expect("sessão verificada");
                    editor.ui.selected = Some(id.clone());
                    editor.ui.moving = editor.session.begin_move(&id).is_ok();
                    editor.ui.message = "Setas movem; Enter confirma; Esc cancela o movimento".into();
                    return self.redraw_editor();
                }
                Some(Target::Button(action)) => return self.editor_action(action),
                None if vk == 0x0D => return self.editor_action(EditorAction::Save),
                None => return Ok(()),
            }
        }
        if editor.ui.moving {
            match vk {
                0x0D => { let editor = self.editor.as_mut().expect("sessão verificada"); editor.ui.moving = false; editor.session.finish_move(); return self.redraw_editor(); }
                0x25 => return self.editor_action(EditorAction::MoveBefore),
                0x27 => return self.editor_action(EditorAction::MoveAfter),
                0x26 | 0x28 => {
                    let id = editor.ui.selected.as_deref().unwrap_or("");
                    let current = Section::ALL.iter().position(|s| s.items(editor.session.sections()).iter().any(|item| item == id)).unwrap_or(0);
                    let next = (current as i32 + if vk == 0x26 { -1 } else { 1 }).rem_euclid(3) as usize;
                    return self.editor_action(EditorAction::MoveSection(Section::ALL[next]));
                }
                _ => return Ok(()),
            }
        }
        if matches!(vk, 0x09 | 0x25 | 0x26 | 0x27 | 0x28) {
            let Some(layout) = self.editor_layout() else { return Ok(()) };
            let delta = if matches!(vk, 0x25 | 0x26) || (vk == 0x09 && window::shift_pressed()) { -1 } else { 1 };
            self.editor.as_mut().expect("sessão verificada").ui.navigate(&layout, delta);
            self.redraw_editor()?;
        }
        Ok(())
    }
}
