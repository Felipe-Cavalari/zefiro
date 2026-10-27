//! O `lpData` de um `WM_COPYDATA` de ícone (`dwData = 1`). Os handles vêm em 32 bits mesmo no
//! Windows de 64. Os offsets foram conferidos com capturas reais (`tests/fixtures`); o Windows 11
//! manda 1484 bytes: os 964 do `NOTIFYICONDATA` de 32 bits e, depois, o caminho do executável.

/// `COPYDATASTRUCT::dwData` das mensagens de ícone.
pub const COPYDATA_NOTIFY: usize = 1;
/// Primeiros 4 bytes de toda mensagem de ícone.
pub const SIGNATURE: u32 = 0x3475_3423;

pub const NIF_MESSAGE: u32 = 0x1;
pub const NIF_ICON: u32 = 0x2;
pub const NIF_TIP: u32 = 0x4;
pub const NIF_STATE: u32 = 0x8;
pub const NIF_GUID: u32 = 0x20;
pub const NIS_HIDDEN: u32 = 0x1;

const TIP: usize = 32;
const TIP_LEN: usize = 128;
const STATE: usize = 288;
const VERSION: usize = 808;
const GUID: usize = 944;
const EXE: usize = 964;
const EXE_LEN: usize = 260;
/// O mínimo para ler até o GUID.
const MIN_LEN: usize = GUID + 16;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Op {
    #[default]
    Add,
    Modify,
    Delete,
    SetFocus,
    SetVersion,
}

/// Uma chamada do `Shell_NotifyIcon`, como chegou. `version` só vale no `NIM_SETVERSION`.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Notify {
    pub op: Op,
    pub hwnd: u32,
    pub uid: u32,
    pub flags: u32,
    pub callback: u32,
    pub hicon: u32,
    pub tip: String,
    pub state: u32,
    pub state_mask: u32,
    pub version: u32,
    pub guid: Option<[u8; 16]>,
    /// Caminho do executável do app (vazio em versões do Windows que não mandam).
    pub exe: String,
}

fn u32_at(b: &[u8], at: usize) -> u32 {
    b.get(at..at + 4).map_or(0, |s| u32::from_le_bytes([s[0], s[1], s[2], s[3]]))
}

/// Texto UTF-16 de `len` unidades a partir de `at`, até o primeiro zero.
fn text_at(b: &[u8], at: usize, len: usize) -> String {
    let end = (at + len * 2).min(b.len());
    let units: Vec<u16> = b.get(at..end).unwrap_or_default().as_chunks::<2>().0.iter().map(|c| u16::from_le_bytes(*c)).take_while(|&u| u != 0).collect();
    String::from_utf16_lossy(&units)
}

pub fn parse(bytes: &[u8]) -> Option<Notify> {
    if bytes.len() < MIN_LEN || u32_at(bytes, 0) != SIGNATURE {
        return None;
    }
    let op = match u32_at(bytes, 4) {
        0 => Op::Add,
        1 => Op::Modify,
        2 => Op::Delete,
        3 => Op::SetFocus,
        4 => Op::SetVersion,
        _ => return None,
    };
    let flags = u32_at(bytes, 20);
    let guid = (flags & NIF_GUID != 0).then(|| {
        let mut g = [0; 16];
        g.copy_from_slice(&bytes[GUID..GUID + 16]);
        g
    });
    Some(Notify {
        op,
        hwnd: u32_at(bytes, 12),
        uid: u32_at(bytes, 16),
        flags,
        callback: u32_at(bytes, 24),
        hicon: u32_at(bytes, 28),
        tip: text_at(bytes, TIP, TIP_LEN),
        state: u32_at(bytes, STATE),
        state_mask: u32_at(bytes, STATE + 4),
        version: u32_at(bytes, VERSION),
        guid,
        exe: text_at(bytes, EXE, EXE_LEN),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    const ADD_V4: &[u8] = include_bytes!("../tests/fixtures/nim_add_v4.bin");
    const ADD_GUID: &[u8] = include_bytes!("../tests/fixtures/nim_add_guid.bin");
    const SET_VERSION: &[u8] = include_bytes!("../tests/fixtures/nim_setversion.bin");
    const DELETE: &[u8] = include_bytes!("../tests/fixtures/nim_delete.bin");
    const ADD_EXE: &[u8] = include_bytes!("../tests/fixtures/nim_add_exe_path.bin");

    #[test]
    fn parses_captured_add() {
        let n = parse(ADD_V4).unwrap();
        assert_eq!(n.op, Op::Add);
        assert_eq!((n.hwnd, n.uid, n.flags, n.callback), (722_478, 1000, 0x87, 0x400));
        assert_eq!(n.tip, "Wallpaper Engine");
        assert!(n.exe.ends_with("wallpaper64.exe"), "{}", n.exe);
    }

    #[test]
    fn parses_set_version() {
        let n = parse(SET_VERSION).unwrap();
        assert_eq!(n.op, Op::SetVersion);
        assert_eq!((n.uid, n.version), (1, 4));
        assert_eq!(n.tip, "Windhawk");
    }

    #[test]
    fn parses_delete() {
        let n = parse(DELETE).unwrap();
        assert_eq!(n.op, Op::Delete);
        assert_eq!((n.hwnd, n.uid), (656_942, 1000));
    }

    #[test]
    fn guid_only_with_flag() {
        let with = parse(ADD_GUID).unwrap();
        assert_eq!(with.guid, Some([0x78, 0xae, 0x20, 0x78, 0xe3, 0x23, 0x29, 0x42, 0x82, 0xc1, 0xe4, 0x1c, 0xb6, 0x7d, 0x5b, 0x9c]));
        assert_eq!(parse(ADD_V4).unwrap().guid, None);
    }

    #[test]
    fn exe_path_is_read() {
        assert!(parse(ADD_EXE).unwrap().exe.ends_with("\\WhatsApp.Root.exe"));
    }

    #[test]
    fn short_or_unsigned_payload_is_none() {
        assert!(parse(&[0; 10]).is_none());
        let mut wrong = ADD_V4.to_vec();
        wrong[0] ^= 0xFF;
        assert!(parse(&wrong).is_none());
    }
}
