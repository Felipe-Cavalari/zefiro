//! Painel "Energia": cabeçalho com a conta e o tempo ligado, e os botões Bloquear, Suspender,
//! Sair, Reiniciar e Desligar. Sair, Reiniciar e Desligar pedem um segundo clique em até 3 s.

pub use zefiro_platform::power::PowerAction;
use zefiro_platform::geometry::{Point, Rect, Size};
use zefiro_platform::image::Image;
use zefiro_platform::text::{Align, Family, FontWeight, TextStyle};
use zefiro_resources::tokens::Tokens;
use zefiro_ui::paint::{DisplayList, DrawOp};

/// Janela do segundo clique.
const CONFIRM_S: f64 = 3.0;
/// Segundo clique antes disto é ignorado: um clique duplo não confirma.
const MIN_CONFIRM_S: f64 = 0.4;

/// Primeiro clique arma Sair/Reiniciar/Desligar; o segundo, em até 3 s, executa.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct ConfirmState { armed: Option<(PowerAction, f64)> }

fn needs_confirm(a: PowerAction) -> bool {
    matches!(a, PowerAction::SignOut | PowerAction::Restart | PowerAction::Shutdown)
}

impl ConfirmState {
    /// A ação a executar agora, se houver. `now` em segundos.
    pub fn click(&mut self, a: PowerAction, now: f64) -> Option<PowerAction> {
        if !needs_confirm(a) {
            self.armed = None;
            return Some(a);
        }
        match self.armed {
            Some((armed, at)) if armed == a && now - at < MIN_CONFIRM_S => None,
            Some((armed, at)) if armed == a && now - at <= CONFIRM_S => {
                self.armed = None;
                Some(a)
            }
            _ => {
                self.armed = Some((a, now));
                None
            }
        }
    }

    /// Desarma depois de 3 s. `true` se mudou (redesenhar).
    pub fn expire(&mut self, now: f64) -> bool {
        match self.armed {
            Some((_, at)) if now - at > CONFIRM_S => {
                self.armed = None;
                true
            }
            _ => false,
        }
    }

    pub fn cancel(&mut self) {
        self.armed = None;
    }

    pub fn armed(&self) -> Option<PowerAction> {
        self.armed.map(|(a, _)| a)
    }
}

/// "Ligado há 3 d 4 h", "Ligado há 5 h 12 min", "Ligado há 7 min".
pub fn format_uptime(secs: u64) -> String {
    let (d, h, m) = (secs / 86_400, secs % 86_400 / 3600, secs % 3600 / 60);
    match (d, h, m) {
        (0, 0, 0) => "Ligado há menos de 1 min".into(),
        (0, 0, m) => format!("Ligado há {m} min"),
        (0, h, m) => format!("Ligado há {h} h {m} min"),
        (d, h, _) => format!("Ligado há {d} d {h} h"),
    }
}

/// Iniciais do nome para o avatar sem foto ("Felipe Cavalari" → "FC").
pub fn initials(name: &str) -> String {
    let letters: String = name.split_whitespace().filter_map(|w| w.chars().next()).take(2).flat_map(char::to_uppercase).collect();
    if letters.is_empty() { "?".into() } else { letters }
}

const BUTTONS: [(PowerAction, &str, &str, &str); 5] = [
    (PowerAction::Lock, "\u{E72E}", "Bloquear", ""),
    (PowerAction::Sleep, "\u{E708}", "Suspender", ""),
    (PowerAction::SignOut, "\u{E748}", "Sair", "sair"),
    (PowerAction::Restart, "\u{E777}", "Reiniciar", "reiniciar"),
    (PowerAction::Shutdown, "\u{E7E8}", "Desligar", "desligar"),
];

const WIDTH: f32 = 320.0;

/// Desenho e as áreas de clique de cada botão.
pub fn power_panel(user: &str, picture: Option<&Image>, uptime: &str, armed: Option<PowerAction>, scale: f32, tokens: &Tokens) -> (DisplayList, Vec<(Rect, PowerAction)>) {
    let px = |v: f32| (v * scale).round();
    let text = |size: f32, weight: FontWeight| TextStyle { size: size * scale, weight, family: Family::Text };
    let icon = |size: f32| TextStyle { size: size * scale, weight: FontWeight::Regular, family: Family::Icons };
    let (bw, bh, gap) = (px(88.0), px(64.0), px(8.0));
    let grid_top = px(76.0);
    let size = Size { w: px(WIDTH), h: grid_top + 2.0 * bh + gap + px(40.0) };
    let mut list = DisplayList::new(size);
    let mut hits = Vec::new();
    // Cabeçalho: foto (ou iniciais), nome e tempo ligado.
    let avatar = Rect::new(px(20.0), px(18.0), px(40.0), px(40.0));
    match picture {
        Some(img) => list.push(DrawOp::Image { rect: avatar, image: img.clone(), radius: avatar.w / 2.0, opacity: 1.0 }),
        None => {
            list.push(DrawOp::Circle { center: Point { x: avatar.x + avatar.w / 2.0, y: avatar.y + avatar.h / 2.0 }, radius: avatar.w / 2.0, color: tokens.accent_soft });
            list.push(DrawOp::Text { rect: avatar, text: initials(user), style: text(14.0, FontWeight::SemiBold), color: tokens.accent, align: Align::Center });
        }
    }
    list.push(DrawOp::Text { rect: Rect::new(px(72.0), px(18.0), size.w - px(92.0), px(20.0)), text: user.into(), style: text(13.0, FontWeight::SemiBold), color: tokens.text, align: Align::Leading });
    list.push(DrawOp::Text { rect: Rect::new(px(72.0), px(38.0), size.w - px(92.0), px(18.0)), text: uptime.into(), style: text(11.5, FontWeight::Regular), color: tokens.text_tertiary, align: Align::Leading });
    for (i, (action, glyph, label, verb)) in BUTTONS.iter().enumerate() {
        let (col, row) = (i % 3, i / 3);
        let r = Rect::new(px(20.0) + col as f32 * (bw + gap), grid_top + row as f32 * (bh + gap), bw, bh);
        let is_armed = armed == Some(*action);
        list.push(DrawOp::RoundRect { rect: r, radius: px(12.0), color: if is_armed { tokens.accent_soft } else { tokens.raised } });
        list.push(DrawOp::Text { rect: Rect::new(r.x, r.y + px(10.0), r.w, px(24.0)), text: (*glyph).into(), style: icon(18.0), color: if is_armed { tokens.accent } else { tokens.text }, align: Align::Center });
        let caption = if is_armed { "Confirmar" } else { label };
        list.push(DrawOp::Text { rect: Rect::new(r.x, r.y + px(38.0), r.w, px(18.0)), text: caption.into(), style: text(11.5, FontWeight::Regular), color: if is_armed { tokens.accent } else { tokens.text_secondary }, align: Align::Center });
        if is_armed {
            let hint = Rect::new(px(20.0), grid_top + 2.0 * bh + gap + px(10.0), size.w - px(40.0), px(18.0));
            list.push(DrawOp::Text { rect: hint, text: format!("Clique de novo para {verb}"), style: text(11.5, FontWeight::Regular), color: tokens.accent, align: Align::Leading });
        }
        hits.push((r, *action));
    }
    (list, hits)
}

#[cfg(test)]
mod tests {
    use super::*;
    use zefiro_platform::color::Rgba;
    use zefiro_resources::theme::{BUILTIN, parse_theme};
    use zefiro_resources::tokens::derive_tokens;

    fn tokens() -> Tokens {
        derive_tokens(&parse_theme("obsidian", "o", BUILTIN[0].1).unwrap(), Rgba::rgb(0, 120, 215), None)
    }

    fn texts(l: &DisplayList) -> Vec<String> {
        l.ops.iter().filter_map(|o| if let DrawOp::Text { text, .. } = o { Some(text.clone()) } else { None }).collect()
    }

    #[test]
    fn lock_and_sleep_act_on_first_click() {
        let mut c = ConfirmState::default();
        assert_eq!(c.click(PowerAction::Lock, 0.0), Some(PowerAction::Lock));
        assert_eq!(c.click(PowerAction::Sleep, 0.0), Some(PowerAction::Sleep));
        assert_eq!(c.armed(), None);
    }

    #[test]
    fn destructive_actions_need_second_click_within_3s() {
        for a in [PowerAction::SignOut, PowerAction::Restart, PowerAction::Shutdown] {
            let mut c = ConfirmState::default();
            assert_eq!(c.click(a, 10.0), None);
            assert_eq!(c.armed(), Some(a));
            assert_eq!(c.click(a, 12.9), Some(a));
            assert_eq!(c.armed(), None);
        }
    }

    #[test]
    fn second_click_after_3s_rearms() {
        let mut c = ConfirmState::default();
        assert_eq!(c.click(PowerAction::Restart, 0.0), None);
        assert!(c.expire(3.1));
        assert_eq!(c.armed(), None);
        assert_eq!(c.click(PowerAction::Restart, 3.2), None, "arma de novo, não executa");
        assert!(!c.expire(4.0));
    }

    #[test]
    fn arming_another_action_never_runs_the_first() {
        let mut c = ConfirmState::default();
        assert_eq!(c.click(PowerAction::Restart, 0.0), None);
        assert_eq!(c.click(PowerAction::Shutdown, 1.0), None);
        assert_eq!(c.armed(), Some(PowerAction::Shutdown));
        assert_eq!(c.click(PowerAction::Shutdown, 1.5), Some(PowerAction::Shutdown));
    }

    #[test]
    fn lock_while_armed_disarms_and_locks() {
        let mut c = ConfirmState::default();
        c.click(PowerAction::Shutdown, 0.0);
        assert_eq!(c.click(PowerAction::Lock, 0.5), Some(PowerAction::Lock));
        assert_eq!(c.armed(), None);
    }

    #[test]
    fn fast_double_click_does_not_confirm() {
        // Um clique duplo em "Desligar" não pode desligar: o segundo clique logo depois é ignorado.
        let mut c = ConfirmState::default();
        assert_eq!(c.click(PowerAction::Shutdown, 0.0), None);
        assert_eq!(c.click(PowerAction::Shutdown, 0.2), None);
        assert_eq!(c.armed(), Some(PowerAction::Shutdown));
        assert_eq!(c.click(PowerAction::Shutdown, 0.5), Some(PowerAction::Shutdown));
    }

    #[test]
    fn cancel_disarms() {
        let mut c = ConfirmState::default();
        c.click(PowerAction::SignOut, 0.0);
        c.cancel();
        assert_eq!(c.armed(), None);
        assert_eq!(c.click(PowerAction::SignOut, 0.5), None);
    }

    #[test]
    fn uptime_formats() {
        assert_eq!(format_uptime(3 * 86400 + 4 * 3600 + 59), "Ligado há 3 d 4 h");
        assert_eq!(format_uptime(5 * 3600 + 12 * 60), "Ligado há 5 h 12 min");
        assert_eq!(format_uptime(7 * 60 + 5), "Ligado há 7 min");
        assert_eq!(format_uptime(20), "Ligado há menos de 1 min");
    }

    #[test]
    fn initials_from_name() {
        assert_eq!(initials("Felipe Cavalari"), "FC");
        assert_eq!(initials("felip"), "F");
        assert_eq!(initials(""), "?");
    }

    #[test]
    fn panel_shows_buttons_and_armed_label() {
        let (list, hits) = power_panel("Felipe Cavalari", None, "Ligado há 7 min", None, 1.0, &tokens());
        let t = texts(&list);
        for s in ["Felipe Cavalari", "FC", "Ligado há 7 min", "Bloquear", "Suspender", "Sair", "Reiniciar", "Desligar"] {
            assert!(t.contains(&s.to_string()), "falta {s}");
        }
        assert_eq!(hits.len(), 5);
        assert_eq!(list.size.w, 320.0);
        let (armed, _) = power_panel("Felipe Cavalari", None, "", Some(PowerAction::Restart), 1.0, &tokens());
        assert!(texts(&armed).contains(&"Clique de novo para reiniciar".to_string()));
        assert!(!texts(&armed).contains(&"Reiniciar".to_string()));
    }
}
