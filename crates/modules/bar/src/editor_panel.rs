//! Painel de edição desenhado com os mesmos tokens e listas portáveis da barra.

use zefiro_core::config::LayoutMode;
use zefiro_platform::geometry::{Rect, Size};
use zefiro_platform::text::{Align, Family, FontWeight, TextStyle};
use zefiro_resources::tokens::Tokens;
use zefiro_ui::paint::{DisplayList, DrawOp};
use crate::editor::{EditorSession, Section};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EditorAction {
    Monitor(usize), ToggleMode, SelectSection(Section), SelectItem(String), Add(String), Remove,
    MoveSection(Section), MoveBefore, MoveAfter, Gallery, Options, Back,
    ToggleSeconds, EditFormat, SystemFormat, Reload, Reapply, ResolveFile, ResolveDraft,
    CopyMonitor(String), Cancel, Save,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum EditorPage { #[default] Items, Gallery, Options }

pub struct EditorRow { pub label: String, pub action: EditorAction, pub enabled: bool }

#[derive(Default)]
pub struct EditorPanel {
    pub focus: usize,
    pub scroll: usize,
    pub page: EditorPage,
    pub selected: Option<String>,
    pub section: Option<Section>,
    pub message: String,
    pub editing_format: bool,
    pub format_select_all: bool,
    pub moving: bool,
}

impl EditorPanel {
    pub fn rows(&self, session: &EditorSession, monitor_names: &[String]) -> Vec<EditorRow> {
        let mut rows = Vec::new();
        let mut row = |label: String, action, enabled| rows.push(EditorRow { label, action, enabled: enabled && !session.saving });
        if session.conflict {
            row("Recarregar arquivo".into(), EditorAction::Reload, true);
            row("Reaplicar minhas alterações".into(), EditorAction::Reapply, true);
        }
        match self.page {
            EditorPage::Items => {
                for (index, name) in monitor_names.iter().enumerate() { row(format!("Monitor {} · {name}", index + 1), EditorAction::Monitor(index), true); }
                row(if session.draft.bar.layout_mode == LayoutMode::Shared { "Layout: mesmo em todos → usar próprio" } else { "Layout: próprio por monitor → usar selecionado em todos" }.into(), EditorAction::ToggleMode, true);
                for section in Section::ALL {
                    row(format!("{}{}", if self.section == Some(section) { "● " } else { "" }, section.label()), EditorAction::SelectSection(section), true);
                    for id in section.items(session.sections()) {
                        let selected = self.selected.as_deref() == Some(id);
                        row(format!("  {}{}", if selected { "● " } else { "" }, crate::catalog::name(id)), EditorAction::SelectItem(id.clone()), true);
                    }
                }
                row("+ Adicionar item".into(), EditorAction::Gallery, true);
                row("Opções do item selecionado".into(), EditorAction::Options, self.selected.is_some());
                row("Remover item selecionado".into(), EditorAction::Remove, self.selected.is_some());
                row("Mover antes".into(), EditorAction::MoveBefore, self.selected.is_some());
                row("Mover depois".into(), EditorAction::MoveAfter, self.selected.is_some());
                for section in Section::ALL { row(format!("Mover para {}", section.label()), EditorAction::MoveSection(section), self.selected.is_some()); }
                for id in session.draft.bar.monitors.keys().filter(|id| Some(id.as_str()) != session.monitor_id.as_deref()) {
                    row(format!("Copiar layout salvo: {id}"), EditorAction::CopyMonitor(id.clone()), session.draft.bar.layout_mode == LayoutMode::PerMonitor && session.monitor_id.is_some());
                }
            }
            EditorPage::Gallery => {
                row("← Voltar à organização".into(), EditorAction::Back, true);
                for item in crate::catalog::ITEMS {
                    let present = session.sections().ids().any(|id| id == item.id);
                    row(format!("{} · {}", item.name, if present { "Já adicionado" } else { item.description }), EditorAction::Add(item.id.into()), !present);
                }
            }
            EditorPage::Options => {
                row("← Voltar à organização".into(), EditorAction::Back, true);
                if self.selected.as_deref() == Some("clock") {
                    row(format!("Segundos: {}", if session.draft.clock.seconds { "ligados" } else { "desligados" }), EditorAction::ToggleSeconds, true);
                    row(format!("Formato: {}", if session.draft.clock.format.is_empty() { "padrão do Windows" } else { &session.draft.clock.format }), EditorAction::EditFormat, true);
                    row("Usar formato do Windows".into(), EditorAction::SystemFormat, true);
                }
            }
        }
        rows.push(EditorRow { label: "Cancelar".into(), action: EditorAction::Cancel, enabled: !session.saving });
        rows.push(EditorRow { label: if session.saving { "Gravando…" } else { "Concluir" }.into(), action: EditorAction::Save, enabled: !session.saving && !session.conflict });
        rows
    }

    pub fn navigate(&mut self, delta: i32, len: usize, visible: usize) {
        if len == 0 { return; }
        self.focus = (self.focus as i32 + delta).rem_euclid(len as i32) as usize;
        let body = len.saturating_sub(2);
        if self.focus < body {
            if self.focus < self.scroll { self.scroll = self.focus; }
            if self.focus >= self.scroll + visible { self.scroll = self.focus + 1 - visible; }
        }
    }

    pub fn visible_count(scale: f32, height: f32) -> usize { ((height / scale - 112.0).max(32.0) / 32.0).floor() as usize }

    pub fn boxes(&self, session: &EditorSession, names: &[String], scale: f32, width: f32, height: f32) -> Vec<(EditorAction, Rect)> {
        let rows = self.rows(session, names);
        let count = Self::visible_count(scale, height);
        let body = rows.len().saturating_sub(2);
        let mut boxes = Vec::new();
        for (i, row) in rows.iter().take(body).skip(self.scroll).take(count).enumerate() {
            boxes.push((row.action.clone(), Rect::new(12.0 * scale, (48.0 + 32.0 * i as f32) * scale, width - 24.0 * scale, 32.0 * scale)));
        }
        let footer_y = height - 44.0 * scale;
        let button_width = ((width - 36.0 * scale) / 2.0).max(28.0 * scale);
        boxes.push((EditorAction::Cancel, Rect::new(12.0 * scale, footer_y, button_width, 32.0 * scale)));
        boxes.push((EditorAction::Save, Rect::new(width / 2.0 + 6.0 * scale, footer_y, button_width, 32.0 * scale)));
        boxes
    }

    pub fn paint(&self, session: &EditorSession, names: &[String], scale: f32, size: Size, tokens: &Tokens) -> DisplayList {
        let rows = self.rows(session, names);
        let mut list = DisplayList::new(size);
        let style = TextStyle { size: 12.0 * scale, weight: FontWeight::Regular, family: Family::Text };
        list.push(DrawOp::Text { rect: Rect::new(16.0 * scale, 4.0 * scale, size.w - 32.0 * scale, 24.0 * scale), text: "Editar barra · Início / Centro / Fim".into(), style: TextStyle { weight: FontWeight::SemiBold, ..style }, color: tokens.text, align: Align::Leading });
        let message = if session.conflict { "A configuração mudou fora do editor" } else { &self.message };
        list.push(DrawOp::Text { rect: Rect::new(16.0 * scale, 26.0 * scale, size.w - 32.0 * scale, 20.0 * scale), text: message.into(), style: TextStyle { size: 10.0 * scale, ..style }, color: tokens.text_secondary, align: Align::Leading });
        for (action, rect) in self.boxes(session, names, scale, size.w, size.h) {
            let Some((index, row)) = rows.iter().enumerate().find(|(_, row)| row.action == action) else { continue };
            let focused = self.focus == index;
            if focused || action == EditorAction::Save || action == EditorAction::Cancel {
                list.push(DrawOp::RoundRect { rect, radius: 8.0 * scale, color: if focused { tokens.raised_strong } else { tokens.raised } });
            }
            if focused { list.push(DrawOp::RoundRectStroke { rect: rect.inset(scale, scale), radius: 7.0 * scale, color: tokens.accent, width: 1.5 * scale }); }
            list.push(DrawOp::Text { rect: rect.inset(8.0 * scale, 0.0), text: row.label.chars().take(90).collect(), style, color: if row.enabled { tokens.text } else { tokens.text_tertiary }, align: Align::Leading });
        }
        list
    }
}
