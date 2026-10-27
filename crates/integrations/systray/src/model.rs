//! Os ícones da bandeja, na ordem em que chegaram. `I` é a imagem já convertida (o host troca o
//! `HICON` por ela na hora; o handle nunca é guardado).

use crate::wire::{NIF_GUID, NIF_ICON, NIF_MESSAGE, NIF_STATE, NIF_TIP, NIS_HIDDEN, Notify, Op};

/// Quem é o ícone: o GUID quando o app manda um, senão a janela e o id.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Key {
    Guid([u8; 16]),
    Window { hwnd: u32, uid: u32 },
}

impl Key {
    pub fn of(n: &Notify) -> Key {
        match n.guid {
            Some(g) if n.flags & NIF_GUID != 0 => Key::Guid(g),
            _ => Key::Window { hwnd: n.hwnd, uid: n.uid },
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Icon<I> {
    pub key: Key,
    pub hwnd: u32,
    pub uid: u32,
    pub callback: u32,
    pub version: u32,
    pub tip: String,
    pub exe: String,
    pub hidden: bool,
    pub image: Option<I>,
}

#[derive(Debug, Clone)]
pub struct TrayModel<I> {
    icons: Vec<Icon<I>>,
}

impl<I> Default for TrayModel<I> {
    fn default() -> Self {
        Self { icons: Vec::new() }
    }
}

impl<I: Clone> TrayModel<I> {
    /// Aplica uma chamada; `image` é o `hIcon` já convertido (só usado com `NIF_ICON`). Diz se a
    /// lista mudou.
    pub fn apply(&mut self, n: &Notify, image: Option<I>) -> bool {
        let key = Key::of(n);
        let at = self.icons.iter().position(|i| i.key == key);
        match (n.op, at) {
            (Op::Add, None) => {
                let mut icon = Icon { key, hwnd: n.hwnd, uid: n.uid, callback: 0, version: 0, tip: String::new(), exe: n.exe.clone(), hidden: false, image: None };
                update(&mut icon, n, image);
                self.icons.push(icon);
                true
            }
            (Op::Add | Op::Modify, Some(i)) => {
                update(&mut self.icons[i], n, image);
                true
            }
            (Op::Delete, Some(i)) => {
                self.icons.remove(i);
                true
            }
            (Op::SetVersion, Some(i)) => {
                self.icons[i].version = n.version;
                true
            }
            _ => false,
        }
    }

    /// Os que aparecem no painel, na ordem de chegada.
    pub fn visible(&self) -> Vec<&Icon<I>> {
        self.icons.iter().filter(|i| !i.hidden).collect()
    }

    pub fn get(&self, key: &Key) -> Option<&Icon<I>> {
        self.icons.iter().find(|i| i.key == *key)
    }

    /// Tira os ícones cuja janela não existe mais (o app morreu sem `NIM_DELETE`).
    pub fn prune(&mut self, alive: impl Fn(u32) -> bool) -> bool {
        let before = self.icons.len();
        self.icons.retain(|i| alive(i.hwnd));
        self.icons.len() != before
    }
}

/// Só os campos marcados em `uFlags` mudam.
fn update<I>(icon: &mut Icon<I>, n: &Notify, image: Option<I>) {
    if n.flags & NIF_MESSAGE != 0 {
        icon.callback = n.callback;
    }
    if n.flags & NIF_TIP != 0 {
        icon.tip.clone_from(&n.tip);
    }
    if n.flags & NIF_ICON != 0 {
        icon.image = image;
    }
    if n.flags & NIF_STATE != 0 && n.state_mask & NIS_HIDDEN != 0 {
        icon.hidden = n.state & NIS_HIDDEN != 0;
    }
    if icon.exe.is_empty() {
        icon.exe.clone_from(&n.exe);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::wire::{NIF_GUID, NIF_ICON, NIF_MESSAGE, NIF_STATE, NIF_TIP, NIS_HIDDEN, Notify, Op};

    fn n(op: Op, hwnd: u32, uid: u32, flags: u32, tip: &str) -> Notify {
        Notify { op, hwnd, uid, flags, callback: 0x400, tip: tip.into(), ..Default::default() }
    }

    fn tips(m: &TrayModel<u8>) -> Vec<&str> {
        m.visible().iter().map(|i| i.tip.as_str()).collect()
    }

    #[test]
    fn add_then_modify_keeps_order_and_updates_tip() {
        let mut m = TrayModel::default();
        assert!(m.apply(&n(Op::Add, 1, 0, NIF_TIP | NIF_ICON, "A"), Some(1)));
        assert!(m.apply(&n(Op::Add, 2, 0, NIF_TIP, "B"), None));
        assert!(m.apply(&n(Op::Modify, 1, 0, NIF_TIP, "A2"), None));
        assert_eq!(tips(&m), ["A2", "B"]);
    }

    #[test]
    fn modify_only_touches_flagged_fields() {
        let mut m = TrayModel::default();
        m.apply(&n(Op::Add, 1, 0, NIF_TIP | NIF_ICON | NIF_MESSAGE, "A"), Some(7));
        m.apply(&n(Op::Modify, 1, 0, NIF_TIP, "A2"), Some(9));
        let icon = &m.visible()[0];
        assert_eq!(icon.image, Some(7));
        assert_eq!(icon.callback, 0x400);
        m.apply(&n(Op::Modify, 1, 0, NIF_ICON, "ignorada"), Some(9));
        assert_eq!(m.visible()[0].image, Some(9));
        assert_eq!(m.visible()[0].tip, "A2");
    }

    #[test]
    fn modify_of_unknown_icon_is_ignored() {
        let mut m: TrayModel<u8> = TrayModel::default();
        assert!(!m.apply(&n(Op::Modify, 1, 0, NIF_TIP | NIF_ICON, "A"), Some(1)));
        assert!(m.visible().is_empty());
    }

    #[test]
    fn add_of_known_icon_acts_as_modify() {
        let mut m = TrayModel::default();
        m.apply(&n(Op::Add, 1, 0, NIF_TIP, "A"), None::<u8>);
        m.apply(&n(Op::Add, 1, 0, NIF_TIP, "A2"), None);
        assert_eq!(tips(&m), ["A2"]);
    }

    #[test]
    fn delete_removes() {
        let mut m = TrayModel::default();
        m.apply(&n(Op::Add, 1, 0, NIF_TIP, "A"), None::<u8>);
        assert!(m.apply(&n(Op::Delete, 1, 0, 0, ""), None));
        assert!(m.visible().is_empty());
        assert!(!m.apply(&n(Op::Delete, 1, 0, 0, ""), None));
    }

    #[test]
    fn hidden_state_hides() {
        let mut m = TrayModel::default();
        let hide = Notify { state: NIS_HIDDEN, state_mask: NIS_HIDDEN, ..n(Op::Add, 1, 0, NIF_TIP | NIF_STATE, "A") };
        m.apply(&hide, None::<u8>);
        assert!(m.visible().is_empty());
        let show = Notify { state: 0, state_mask: NIS_HIDDEN, ..n(Op::Modify, 1, 0, NIF_STATE, "") };
        m.apply(&show, None);
        assert_eq!(tips(&m), ["A"]);
    }

    #[test]
    fn state_without_flag_is_ignored() {
        let mut m = TrayModel::default();
        let add = Notify { state: NIS_HIDDEN, state_mask: NIS_HIDDEN, ..n(Op::Add, 1, 0, NIF_TIP, "A") };
        m.apply(&add, None::<u8>);
        assert_eq!(tips(&m), ["A"]);
    }

    #[test]
    fn guid_key_wins_over_window() {
        let mut m = TrayModel::default();
        let g = Some([5; 16]);
        m.apply(&Notify { guid: g, ..n(Op::Add, 1, 3, NIF_TIP | NIF_GUID, "A") }, None::<u8>);
        m.apply(&Notify { guid: g, ..n(Op::Modify, 1, 4, NIF_TIP | NIF_GUID, "A2") }, None);
        assert_eq!(tips(&m), ["A2"]);
        assert_eq!(m.visible()[0].key, Key::Guid([5; 16]));
    }

    #[test]
    fn set_version_is_stored() {
        let mut m = TrayModel::default();
        m.apply(&Notify { version: 4, ..n(Op::Add, 1, 0, NIF_TIP, "A") }, None::<u8>);
        assert_eq!(m.visible()[0].version, 0, "o uVersion de um ADD é lixo");
        m.apply(&Notify { version: 4, ..n(Op::SetVersion, 1, 0, 0, "") }, None);
        assert_eq!(m.visible()[0].version, 4);
    }

    #[test]
    fn exe_is_kept() {
        let mut m = TrayModel::default();
        m.apply(&Notify { exe: "C:\\x\\Discord.exe".into(), ..n(Op::Add, 1, 0, 0, "") }, None::<u8>);
        assert_eq!(m.visible()[0].exe, "C:\\x\\Discord.exe");
    }

    #[test]
    fn prune_drops_icons_whose_window_is_gone() {
        let mut m = TrayModel::default();
        m.apply(&n(Op::Add, 1, 0, NIF_TIP, "A"), None::<u8>);
        m.apply(&n(Op::Add, 2, 0, NIF_TIP, "B"), None);
        assert!(m.prune(|hwnd| hwnd != 1));
        assert_eq!(tips(&m), ["B"]);
        assert!(!m.prune(|_| true));
    }

    #[test]
    fn get_finds_by_key() {
        let mut m = TrayModel::default();
        m.apply(&n(Op::Add, 9, 2, NIF_TIP, "A"), None::<u8>);
        assert_eq!(m.get(&Key::Window { hwnd: 9, uid: 2 }).map(|i| i.tip.as_str()), Some("A"));
        assert!(m.get(&Key::Window { hwnd: 9, uid: 3 }).is_none());
    }
}
