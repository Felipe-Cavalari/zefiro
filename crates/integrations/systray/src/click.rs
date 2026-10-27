//! Um clique no painel vira as mesmas mensagens que a barra de tarefas manda ao app, na mensagem de
//! callback dele e conforme a versão que ele pediu (`NIM_SETVERSION`).

pub const WM_LBUTTONDOWN: u32 = 0x201;
pub const WM_LBUTTONUP: u32 = 0x202;
pub const WM_RBUTTONDOWN: u32 = 0x204;
pub const WM_RBUTTONUP: u32 = 0x205;
pub const WM_CONTEXTMENU: u32 = 0x7B;
/// `NIN_SELECT` (`WM_USER`): o "clique" da versão 4.
pub const NIN_SELECT: u32 = 0x400;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Button {
    Left,
    Right,
}

/// Pares (`wParam`, `lParam`) a postar, em ordem, na mensagem de callback do app. `cursor` é a
/// posição na tela, usada pela versão 4 para o app abrir o menu ali.
pub fn messages(version: u32, uid: u32, button: Button, cursor: (i16, i16)) -> Vec<(usize, isize)> {
    let seq: &[u32] = match (version >= 4, button) {
        (true, Button::Left) => &[WM_LBUTTONDOWN, WM_LBUTTONUP, NIN_SELECT],
        (true, Button::Right) => &[WM_RBUTTONDOWN, WM_RBUTTONUP, WM_CONTEXTMENU],
        (false, Button::Left) => &[WM_LBUTTONDOWN, WM_LBUTTONUP],
        (false, Button::Right) => &[WM_RBUTTONDOWN, WM_RBUTTONUP],
    };
    let xy = (cursor.0 as u16 as usize) | ((cursor.1 as u16 as usize) << 16);
    seq.iter()
        .map(|&m| if version >= 4 { (xy, (m | (uid << 16)) as isize) } else { (uid as usize, m as isize) })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    const UID: u32 = 7;

    fn xy(x: i16, y: i16) -> usize {
        (x as u16 as usize) | ((y as u16 as usize) << 16)
    }

    fn lp(msg: u32) -> isize {
        (msg | (UID << 16)) as isize
    }

    #[test]
    fn v4_left() {
        let w = xy(100, -5);
        assert_eq!(messages(4, UID, Button::Left, (100, -5)), [(w, lp(WM_LBUTTONDOWN)), (w, lp(WM_LBUTTONUP)), (w, lp(NIN_SELECT))]);
    }

    #[test]
    fn v4_right() {
        let w = xy(3, 4);
        assert_eq!(messages(4, UID, Button::Right, (3, 4)), [(w, lp(WM_RBUTTONDOWN)), (w, lp(WM_RBUTTONUP)), (w, lp(WM_CONTEXTMENU))]);
    }

    #[test]
    fn old_left() {
        assert_eq!(messages(0, UID, Button::Left, (1, 1)), [(7, WM_LBUTTONDOWN as isize), (7, WM_LBUTTONUP as isize)]);
    }

    #[test]
    fn old_right() {
        assert_eq!(messages(3, UID, Button::Right, (1, 1)), [(7, WM_RBUTTONDOWN as isize), (7, WM_RBUTTONUP as isize)]);
    }

    #[test]
    fn message_values() {
        assert_eq!((WM_LBUTTONDOWN, WM_LBUTTONUP, WM_RBUTTONDOWN, WM_RBUTTONUP, WM_CONTEXTMENU, NIN_SELECT), (0x201, 0x202, 0x204, 0x205, 0x7B, 0x400));
    }
}
