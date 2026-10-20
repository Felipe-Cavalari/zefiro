//! HUD de debug (`Ctrl+Alt+Shift+D`): quadros do DWM, memória, quando cada integração mandou dado
//! e os últimos avisos do log. Só desenha; quem mede é o app.

use zefiro_platform::color::Rgba;
use zefiro_platform::geometry::{Rect, Size};
use zefiro_platform::text::{Align, Family, FontWeight, TextStyle};
use zefiro_resources::tokens::Tokens;
use zefiro_ui::paint::{DisplayList, DrawOp};

/// Largura do HUD em DIP.
pub const WIDTH_DIP: f32 = 360.0;
const PAD: f32 = 14.0;
const LINE: f32 = 18.0;
/// Mensagens de aviso cortadas neste número de caracteres.
const MESSAGE_CHARS: usize = 52;

/// Quadros que o DWM compôs no último segundo.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Frames { pub per_second: f32, pub refresh_hz: f32, pub dropped: u64 }

#[derive(Debug, Clone, PartialEq)]
pub struct HudWarning { pub clock: String, pub error: bool, pub source: String, pub message: String }

#[derive(Debug, Clone, PartialEq, Default)]
pub struct HudSnapshot {
    pub frames: Option<Frames>,
    pub private_mb: f32,
    pub working_set_mb: f32,
    pub uptime_s: u64,
    /// Nome e segundos desde o último dado; `None` = ainda não mandou nada (não aparece).
    pub integrations: Vec<(String, Option<f64>)>,
    pub warnings: Vec<HudWarning>,
}

/// "agora", "há 12 s", "há 3 min", "há 2 h".
pub fn ago(secs: f64) -> String {
    let s = secs.max(0.0) as u64;
    match s {
        0 => "agora".into(),
        1..=59 => format!("há {s} s"),
        60..=3599 => format!("há {} min", s / 60),
        _ => format!("há {} h", s / 3600),
    }
}

fn uptime(s: u64) -> String {
    if s < 3600 { format!("{} min", s / 60) } else { format!("{} h {:02} min", s / 3600, s / 60 % 60) }
}

fn cut(text: &str, max: usize) -> String {
    let one_line = text.replace(['\r', '\n', '\t'], " ");
    if one_line.chars().count() <= max {
        return one_line;
    }
    let mut s: String = one_line.chars().take(max - 1).collect();
    s = s.trim_end().to_string();
    s.push('…');
    s
}

/// Cor de erro do HUD (vermelho da ilha); aviso usa o âmbar.
fn error_color() -> Rgba {
    Rgba::rgb(0xFF, 0x45, 0x3A)
}

fn warning_color() -> Rgba {
    Rgba::rgb(0xFF, 0x9F, 0x0A)
}

pub fn hud_paint(s: &HudSnapshot, scale: f32, tokens: &Tokens) -> DisplayList {
    let px = |v: f32| (v * scale).round();
    let label = TextStyle { size: 11.0 * scale, weight: FontWeight::SemiBold, family: Family::Text };
    let value = TextStyle { size: 11.0 * scale, weight: FontWeight::Regular, family: Family::Text };
    let w = px(WIDTH_DIP);
    let inner = w - 2.0 * px(PAD);
    let mut lines: Vec<(Option<String>, String, Rgba)> = Vec::new();
    let frames = match s.frames {
        Some(f) => format!("{:.0} q/s · monitor {:.0} Hz · {} perdidos", f.per_second, f.refresh_hz, f.dropped),
        None => "indisponível".into(),
    };
    lines.push((Some("Quadros".into()), frames, tokens.text));
    let mb = |v: f32| format!("{v:.1}").replace('.', ",");
    lines.push((Some("Memória".into()), format!("{} MB privada · {} MB working set", mb(s.private_mb), mb(s.working_set_mb)), tokens.text));
    lines.push((Some("Ligado".into()), format!("há {}", uptime(s.uptime_s)), tokens.text));
    let seen: Vec<_> = s.integrations.iter().filter_map(|(n, a)| a.map(|a| (n, a))).collect();
    if !seen.is_empty() {
        lines.push((None, "Integrações".into(), tokens.text_secondary));
        for (name, a) in seen {
            lines.push((Some(name.clone()), ago(a), tokens.text));
        }
    }
    if !s.warnings.is_empty() {
        lines.push((None, "Últimos avisos".into(), tokens.text_secondary));
        for warn in &s.warnings {
            let color = if warn.error { error_color() } else { warning_color() };
            lines.push((Some(warn.clock.clone()), cut(&format!("{} · {}", warn.source, warn.message), MESSAGE_CHARS), color));
        }
    }
    let h = 2.0 * px(PAD) + px(LINE) * (lines.len() as f32 + 1.0);
    let mut list = DisplayList::new(Size { w, h });
    list.push(DrawOp::RoundRect { rect: Rect::new(0.0, 0.0, w, h), radius: px(12.0), color: tokens.fill.with_alpha(0.94) });
    if let Some(border) = tokens.border {
        list.push(DrawOp::RoundRectStroke { rect: Rect::new(0.0, 0.0, w, h), radius: px(12.0), color: border, width: scale.max(1.0) });
    }
    let x = px(PAD);
    let mut y = px(PAD);
    list.push(DrawOp::Text { rect: Rect::new(x, y, inner, px(LINE)), text: "Zefiro · depuração".into(), style: label, color: tokens.text, align: Align::Leading });
    list.push(DrawOp::Text { rect: Rect::new(x, y, inner, px(LINE)), text: "Ctrl+Alt+Shift+D".into(), style: value, color: tokens.text_tertiary, align: Align::Trailing });
    let key_w = px(78.0);
    for (key, text, color) in lines {
        y += px(LINE);
        match key {
            Some(k) => {
                list.push(DrawOp::Text { rect: Rect::new(x, y, key_w, px(LINE)), text: k, style: value, color: tokens.text_secondary, align: Align::Leading });
                list.push(DrawOp::Text { rect: Rect::new(x + key_w, y, inner - key_w, px(LINE)), text, style: value, color, align: Align::Leading });
            }
            None => list.push(DrawOp::Text { rect: Rect::new(x, y, inner, px(LINE)), text, style: label, color, align: Align::Leading }),
        }
    }
    list
}

#[cfg(test)]
mod tests {
    use super::*;
    use zefiro_resources::theme::{BUILTIN, parse_theme};
    use zefiro_resources::tokens::derive_tokens;

    fn tokens() -> Tokens {
        derive_tokens(&parse_theme("obsidian", "o", BUILTIN[0].1).unwrap(), Rgba::rgb(0, 120, 215), None)
    }

    fn texts(l: &DisplayList) -> Vec<String> {
        l.ops.iter().filter_map(|o| if let DrawOp::Text { text, .. } = o { Some(text.clone()) } else { None }).collect()
    }

    fn snapshot() -> HudSnapshot {
        HudSnapshot {
            frames: Some(Frames { per_second: 59.6, refresh_hz: 144.0, dropped: 2 }),
            private_mb: 39.72,
            working_set_mb: 93.6,
            uptime_s: 3 * 3600 + 5 * 60,
            integrations: vec![("Mídia".into(), Some(2.4)), ("Clima".into(), None), ("Claude".into(), Some(300.0))],
            warnings: vec![HudWarning { clock: "14:03".into(), error: true, source: "island_loop".into(), message: "clima indisponível: ".to_string() + &"x".repeat(80) }],
        }
    }

    #[test]
    fn ago_reads_naturally() {
        assert_eq!(ago(0.4), "agora");
        assert_eq!(ago(12.0), "há 12 s");
        assert_eq!(ago(185.0), "há 3 min");
        assert_eq!(ago(7300.0), "há 2 h");
    }

    #[test]
    fn hud_shows_frames_memory_uptime_and_seen_integrations() {
        let t = texts(&hud_paint(&snapshot(), 1.0, &tokens()));
        assert!(t.contains(&"60 q/s · monitor 144 Hz · 2 perdidos".to_string()), "{t:?}");
        assert!(t.contains(&"39,7 MB privada · 93,6 MB working set".to_string()), "{t:?}");
        assert!(t.contains(&"há 3 h 05 min".to_string()), "{t:?}");
        assert!(t.contains(&"Mídia".to_string()) && t.contains(&"há 2 s".to_string()));
        assert!(t.contains(&"há 5 min".to_string()), "Claude");
        assert!(!t.contains(&"Clima".to_string()), "sem dado, não aparece");
    }

    #[test]
    fn warnings_are_cut_and_colored_by_level() {
        let l = hud_paint(&snapshot(), 1.0, &tokens());
        let warning = l.ops.iter().find_map(|o| match o {
            DrawOp::Text { text, color, .. } if text.starts_with("island_loop") => Some((text.clone(), *color)),
            _ => None,
        });
        let (text, color) = warning.expect("aviso desenhado");
        assert!(text.ends_with('…') && text.chars().count() <= MESSAGE_CHARS, "{text}");
        assert_eq!(color, error_color());
    }

    #[test]
    fn without_frames_or_warnings_the_hud_is_shorter() {
        let full = hud_paint(&snapshot(), 1.0, &tokens());
        let bare = hud_paint(&HudSnapshot { frames: None, ..Default::default() }, 1.0, &tokens());
        assert!(texts(&bare).contains(&"indisponível".to_string()));
        assert!(bare.size.h < full.size.h);
        assert_eq!(bare.size.w, WIDTH_DIP);
    }
}
