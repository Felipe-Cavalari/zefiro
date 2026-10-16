//! Gerações globais impedem que um fechamento antigo esconda uma janela reutilizada.

use std::sync::atomic::{AtomicUsize, Ordering};

#[derive(Default)]
pub struct PanelLifecycle { current: usize, open: bool, visible: bool }

impl PanelLifecycle {
    fn advance(&mut self) -> usize {
        static NEXT: AtomicUsize = AtomicUsize::new(1);
        self.current = NEXT.fetch_add(1, Ordering::Relaxed);
        self.current
    }
    pub fn open(&mut self) { self.advance(); self.open = true; self.visible = true; }
    pub fn close(&mut self) -> usize { self.open = false; self.advance() }
    pub fn settle(&mut self, ticket: usize) -> bool {
        if ticket != self.current || self.open || !self.visible { return false; }
        self.visible = false;
        true
    }
    pub fn visible(&self) -> bool { self.visible }
}
