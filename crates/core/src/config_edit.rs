//! Patches de configuração: mescla por campo, sem perder comentários ou chaves futuras.

use toml_edit::{DocumentMut, Item, Value};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PatchChoice { File, Draft }

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PatchError { pub fields: Vec<Vec<String>>, pub message: String }

#[derive(Debug, Clone, Default)]
pub struct ConfigPatch { fields: Vec<(Vec<String>, Item)> }

fn error(message: impl ToString) -> PatchError {
    PatchError { fields: Vec::new(), message: message.to_string() }
}

fn field<'a>(root: &'a toml::Value, path: &[String]) -> Option<&'a toml::Value> {
    path.iter().try_fold(root, |table, key| table.get(key))
}

impl ConfigPatch {
    pub fn is_empty(&self) -> bool { self.fields.is_empty() }

    fn set(&mut self, path: &[&str], item: Item) {
        assert!(!path.is_empty(), "o caminho deve conter uma chave");
        let path: Vec<_> = path.iter().map(|s| (*s).to_string()).collect();
        if let Some((_, existing)) = self.fields.iter_mut().find(|(keys, _)| keys == &path) {
            *existing = item;
        } else {
            self.fields.push((path, item));
        }
    }

    pub fn set_strings(&mut self, path: &[&str], values: &[String]) {
        let array: toml_edit::Array = values.iter().map(String::as_str).collect();
        self.set(path, Item::Value(Value::Array(array)));
    }

    pub fn set_bool(&mut self, path: &[&str], value: bool) {
        self.set(path, toml_edit::value(value));
    }

    pub fn set_string(&mut self, path: &[&str], value: &str) {
        self.set(path, toml_edit::value(value));
    }

    /// `choices` pertence à revisão `latest`; se ela mudar, o chamador pede nova reconciliação.
    pub fn apply(&self, base: &str, latest: &str, choices: &[(Vec<String>, PatchChoice)]) -> Result<String, PatchError> {
        crate::config::parse_config(latest).map_err(|e| error(e.message))?;
        let original: toml::Value = toml::from_str(base).map_err(error)?;
        let current: toml::Value = toml::from_str(latest).map_err(error)?;
        let mut doc: DocumentMut = latest.parse().map_err(error)?;
        let mut conflicts = Vec::new();
        for (keys, item) in &self.fields {
            let changed = field(&original, keys) != field(&current, keys);
            let choice = choices.iter().find(|(p, _)| p == keys).map(|(_, c)| *c);
            if changed && choice.is_none() {
                conflicts.push(keys.clone());
                continue;
            }
            if changed && choice == Some(PatchChoice::File) { continue; }
            let (last, parents) = keys.split_last().expect("validado em set");
            let mut table = doc.as_table_mut();
            for key in parents {
                let entry = table.entry(key).or_insert(toml_edit::table());
                table = entry.as_table_mut().ok_or_else(|| error(format!("`{key}` não é uma tabela")))?;
            }
            let mut new = item.clone();
            if let (Some(old), Some(value)) = (table.get(last).and_then(Item::as_value), new.as_value_mut()) {
                *value.decor_mut() = old.decor().clone();
            }
            table.insert(last, new);
        }
        if !conflicts.is_empty() {
            return Err(PatchError { fields: conflicts, message: "A configuração mudou nos mesmos campos; escolha qual versão manter".into() });
        }
        let text = doc.to_string();
        crate::config::parse_config(&text).map_err(|e| error(e.message))?;
        Ok(text)
    }
}
