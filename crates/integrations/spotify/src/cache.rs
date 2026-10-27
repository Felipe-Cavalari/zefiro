//! Capas em memória: no máximo `COVERS` (cerca de 3 MB a 200 %), saindo as usadas há mais tempo.

/// Capas guardadas pelo serviço.
pub const COVERS: usize = 150;

pub struct CoverCache<T> { cap: usize, entries: Vec<(String, T)> }

impl<T> CoverCache<T> {
    pub fn new(cap: usize) -> Self {
        Self { cap, entries: Vec::new() }
    }

    /// A capa lida passa a ser a mais recente.
    pub fn get(&mut self, url: &str) -> Option<&T> {
        let i = self.entries.iter().position(|(u, _)| u == url)?;
        let entry = self.entries.remove(i);
        self.entries.push(entry);
        self.entries.last().map(|(_, v)| v)
    }

    pub fn insert(&mut self, url: String, value: T) {
        self.entries.retain(|(u, _)| *u != url);
        if self.entries.len() >= self.cap {
            self.entries.remove(0);
        }
        self.entries.push((url, value));
    }

    pub fn clear(&mut self) {
        self.entries.clear();
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}
