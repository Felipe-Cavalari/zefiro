//! Sessão portável de edição. Alterações são prévias até a confirmação da persistência.

use zefiro_core::config::{Config, LayoutMode, MonitorConfig, Sections};
use zefiro_core::config_edit::ConfigPatch;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Section { Start, Center, End }

impl Section {
    pub const ALL: [Self; 3] = [Self::Start, Self::Center, Self::End];
    pub fn key(self) -> &'static str { match self { Self::Start => "start", Self::Center => "center", Self::End => "end" } }
    pub fn label(self) -> &'static str { match self { Self::Start => "Início", Self::Center => "Centro", Self::End => "Fim" } }
    pub fn items(self, sections: &Sections) -> &[String] { match self { Self::Start => &sections.start, Self::Center => &sections.center, Self::End => &sections.end } }
    pub fn items_mut(self, sections: &mut Sections) -> &mut Vec<String> { match self { Self::Start => &mut sections.start, Self::Center => &mut sections.center, Self::End => &mut sections.end } }
}

#[derive(Debug, Clone)]
pub struct EditorSession {
    pub confirmed: Config,
    pub draft: Config,
    pub original_text: String,
    pub latest_text: String,
    pub conflict: bool,
    pub file_error: Option<String>,
    pub monitor_id: Option<String>,
    pub generation: u64,
    pub saving: bool,
    original: Config,
    moving: Option<Sections>,
}

impl EditorSession {
    pub fn new(config: Config, text: String) -> Self {
        Self { confirmed: config.clone(), draft: config.clone(), original: config, original_text: text.clone(), latest_text: text, conflict: false, file_error: None, monitor_id: None, generation: 0, saving: false, moving: None }
    }

    pub fn select_monitor(&mut self, id: Option<String>) { self.cancel_move(); self.monitor_id = id; }
    pub fn sections(&self) -> &Sections { self.draft.bar.sections_for(self.monitor_id.as_deref()) }

    fn sections_mut(&mut self) -> Result<&mut Sections, String> {
        if self.saving { return Err("Aguarde o fim da gravação".into()); }
        if self.draft.bar.layout_mode == LayoutMode::Shared { return Ok(&mut self.draft.bar.sections); }
        let id = self.monitor_id.as_ref().ok_or("Identificação indisponível; usando layout compartilhado")?;
        let base = self.draft.bar.sections.clone();
        Ok(&mut self.draft.bar.monitors.entry(id.clone()).or_insert(MonitorConfig { sections: base }).sections)
    }

    pub fn add(&mut self, id: &str, section: Section) -> Result<(), String> {
        if super::catalog::descriptor(id).is_none() { return Err("Item indisponível".into()); }
        if self.sections().ids().any(|item| item == id) { return Err("Já adicionado".into()); }
        if self.sections().ids().count() >= 64 { return Err("O layout já tem 64 itens; remova um antes de adicionar".into()); }
        section.items_mut(self.sections_mut()?).push(id.into());
        Ok(())
    }

    pub fn remove(&mut self, id: &str) -> Result<(), String> {
        let sections = self.sections_mut()?;
        for section in Section::ALL { section.items_mut(sections).retain(|item| item != id); }
        Ok(())
    }

    pub fn move_item(&mut self, id: &str, section: Section, index: usize) -> Result<(), String> {
        if !self.sections().ids().any(|item| item == id) { return Err("Item não encontrado".into()); }
        self.remove(id)?;
        let items = section.items_mut(self.sections_mut()?);
        items.insert(index.min(items.len()), id.into());
        Ok(())
    }

    pub fn begin_move(&mut self, id: &str) -> Result<(), String> {
        if self.moving.is_some() || self.saving { return Err("Outra operação está em andamento".into()); }
        if !self.sections().ids().any(|item| item == id) { return Err("Item não encontrado".into()); }
        self.moving = Some(self.sections().clone());
        Ok(())
    }

    pub fn finish_move(&mut self) { self.moving = None; }
    pub fn cancel_move(&mut self) {
        if let Some(before) = self.moving.take() && let Ok(sections) = self.sections_mut() { *sections = before; }
    }

    pub fn set_mode(&mut self, mode: LayoutMode, connected: &[String]) {
        if self.saving || mode == self.draft.bar.layout_mode { return; }
        self.cancel_move();
        if mode == LayoutMode::Shared {
            self.draft.bar.sections = self.sections().clone();
        } else {
            for id in connected {
                self.draft.bar.monitors.entry(id.clone()).or_insert_with(|| MonitorConfig { sections: self.draft.bar.sections.clone() });
            }
        }
        self.draft.bar.layout_mode = mode;
    }

    pub fn copy_monitor_layout(&mut self, source: &str) -> Result<(), String> {
        let sections = self.draft.bar.monitors.get(source).ok_or("Monitor de origem não encontrado")?.sections.clone();
        *self.sections_mut()? = sections;
        Ok(())
    }

    pub fn external_change(&mut self, config: Config, text: String) {
        self.file_error = None;
        self.conflict = text != self.original_text;
        self.confirmed = config;
        self.latest_text = text;
    }

    pub fn reload(&mut self) {
        if self.file_error.is_some() || self.saving { return; }
        self.draft = self.confirmed.clone();
        self.original = self.confirmed.clone();
        self.original_text = self.latest_text.clone();
        self.conflict = false;
        self.moving = None;
    }

    pub fn external_invalid(&mut self, text: String, message: String) {
        self.latest_text = text;
        self.file_error = Some(message);
        self.conflict = true;
    }

    pub fn patch(&self) -> ConfigPatch {
        let mut patch = ConfigPatch::default();
        if self.original.bar.layout_mode != self.draft.bar.layout_mode {
            patch.set_string(&["bar", "layout_mode"], if self.draft.bar.layout_mode == LayoutMode::Shared { "shared" } else { "per_monitor" });
        }
        patch_sections(&mut patch, &["bar", "sections"], Some(&self.original.bar.sections), &self.draft.bar.sections);
        for (id, entry) in &self.draft.bar.monitors {
            let old = self.original.bar.monitors.get(id).map(|entry| &entry.sections);
            patch_sections(&mut patch, &["bar", "monitors", id, "sections"], old, &entry.sections);
        }
        if self.original.clock.seconds != self.draft.clock.seconds { patch.set_bool(&["clock", "seconds"], self.draft.clock.seconds); }
        if self.original.clock.format != self.draft.clock.format { patch.set_string(&["clock", "format"], &self.draft.clock.format); }
        patch
    }
}

fn patch_sections(patch: &mut ConfigPatch, prefix: &[&str], old: Option<&Sections>, new: &Sections) {
    for section in Section::ALL {
        if old.is_none_or(|old| section.items(old) != section.items(new)) {
            let mut path = prefix.to_vec();
            path.push(section.key());
            patch.set_strings(&path, section.items(new));
        }
    }
}
