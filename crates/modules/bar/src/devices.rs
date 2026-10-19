//! Alto-falante, microfone e brilho na barra: os chips (ícone + porcentagem) e os painéis (sliders,
//! mudo e troca de dispositivo), no estilo dos painéis da ilha.

use zefiro_platform::color::Rgba;
use zefiro_platform::geometry::{Point, Rect, Size};
use zefiro_platform::text::{Align, Family, FontWeight, TextMeasurer, TextStyle};
use zefiro_resources::tokens::Tokens;
use zefiro_ui::paint::{DisplayList, DrawOp};

use crate::chips::ChipContent;

pub const ICON_MUTE: &str = "\u{E74F}";
const ICON_VOLUME: [&str; 4] = ["\u{E992}", "\u{E993}", "\u{E994}", "\u{E995}"];
pub const ICON_MIC: &str = "\u{E720}";
pub const ICON_MIC_OFF: &str = "\u{EC54}";
pub const ICON_BRIGHTNESS: &str = "\u{E706}";

/// Ícone do alto-falante pelo nível (como o aviso da ilha).
pub fn speaker_icon(percent: u32, muted: bool) -> &'static str {
    if muted || percent == 0 { ICON_MUTE } else { ICON_VOLUME[(percent as f32 * 3.0 / 100.0).ceil().clamp(1.0, 3.0) as usize] }
}

pub fn mic_icon(muted: bool) -> &'static str {
    if muted { ICON_MIC_OFF } else { ICON_MIC }
}

/// Chip de som (`mic = false`) ou microfone a partir de `(porcentagem, mudo)`: ícone, texto e se
/// fica apagado. O mudo só apaga o chip; o % continua, para a largura não mudar.
pub fn volume_level(state: Option<(u32, bool)>, mic: bool) -> (&'static str, String, bool) {
    let (percent, muted) = state.unwrap_or((0, true));
    let glyph = if mic { mic_icon(muted) } else { speaker_icon(percent, muted) };
    (glyph, state.map(|_| format!("{percent}%")).unwrap_or_default(), muted)
}

fn icon_style(size: f32) -> TextStyle {
    TextStyle { size, weight: FontWeight::Regular, family: Family::Icons }
}

/// O que um chip de nível mostra: ícone, texto (`"42%"`, ou vazio) e se fica apagado (mudo,
/// indisponível).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Level<'a> { pub glyph: &'a str, pub text: &'a str, pub dim: bool }

/// Chip com ícone e texto.
pub fn level_chip(level: Level, chip_h: f32, font_px: f32, scale: f32, tokens: &Tokens, m: &dyn TextMeasurer) -> ChipContent {
    let Level { glyph, text, dim } = level;
    let icon_w = (font_px * 1.25).ceil();
    let gap = if text.is_empty() { 0.0 } else { (5.0 * scale).round() };
    let style = TextStyle { size: font_px, weight: FontWeight::SemiBold, family: Family::Text };
    let tw = if text.is_empty() { 0.0 } else { m.measure(text, &style).w.ceil() };
    // A largura reserva "100%": o painel aberto nasce do chip e não pode andar quando o nível muda.
    let slot = if text.is_empty() { 0.0 } else { tw.max(m.measure("100%", &style).w.ceil()) };
    let w = icon_w + gap + slot;
    let color = if dim { tokens.text_secondary } else { tokens.text };
    let mut ops = vec![DrawOp::Text { rect: Rect::new(0.0, 0.0, icon_w, chip_h), text: glyph.into(), style: icon_style(font_px * 1.05), color, align: Align::Center }];
    if !text.is_empty() {
        ops.push(DrawOp::Text { rect: Rect::new(icon_w + gap, 0.0, tw, chip_h), text: text.into(), style, color, align: Align::Leading });
    }
    ChipContent { content_w: w, list: DisplayList { size: Size { w, h: chip_h }, ops } }
}

/// Uma linha com slider: o volume do dispositivo padrão ou o brilho de um monitor.
#[derive(Debug, Clone, PartialEq)]
pub struct SliderRow {
    pub id: String,
    pub label: String,
    /// `None` = indisponível (monitor sem DDC/CI, sem dispositivo).
    pub percent: Option<u32>,
    /// `None` = sem botão de mudo (brilho).
    pub muted: Option<bool>,
    /// Ícone do botão de mudo (ou do item) para o estado atual.
    pub icon: &'static str,
}

/// Um dispositivo da lista de troca (saídas ou entradas).
#[derive(Debug, Clone, PartialEq)]
pub struct DeviceItem { pub id: String, pub name: String, pub current: bool }

#[derive(Debug, Clone, PartialEq)]
pub enum DeviceAction { ToggleMute(String), Device(String) }

/// Desenho do painel, os trilhos dos sliders (para arrastar) e as áreas de clique.
#[derive(Debug, Clone, PartialEq)]
pub struct DevicePanel { pub list: DisplayList, pub sliders: Vec<(String, Rect)>, pub hits: Vec<(Rect, DeviceAction)> }

const WIDTH: f32 = 340.0;
const ROW_H: f32 = 56.0;
const DEVICE_H: f32 = 32.0;
const MAX_DEVICES: usize = 8;

/// Painel com título, uma linha de slider por item e, se houver, a lista de dispositivos.
pub fn device_panel(title: &str, rows: &[SliderRow], devices: &[DeviceItem], scale: f32, tokens: &Tokens) -> DevicePanel {
    let px = |v: f32| (v * scale).round();
    let shown = devices.len().min(MAX_DEVICES);
    let devices_h = if shown == 0 { 0.0 } else { 30.0 + shown as f32 * DEVICE_H };
    let h = 44.0 + rows.len().max(1) as f32 * ROW_H + devices_h + 10.0;
    let size = Size { w: px(WIDTH), h: px(h) };
    let mut list = DisplayList::new(size);
    let mut sliders = Vec::new();
    let mut hits = Vec::new();
    let text = |size: f32, weight: FontWeight| TextStyle { size: size * scale, weight, family: Family::Text };
    list.push(DrawOp::Text { rect: Rect::new(px(20.0), px(14.0), size.w - px(40.0), px(22.0)), text: title.into(), style: text(13.0, FontWeight::SemiBold), color: tokens.text_secondary, align: Align::Leading });
    for (i, row) in rows.iter().enumerate() {
        let top = px(44.0 + i as f32 * ROW_H);
        list.push(DrawOp::Text { rect: Rect::new(px(20.0), top, size.w - px(40.0), px(18.0)), text: row.label.clone(), style: text(12.0, FontWeight::Regular), color: tokens.text_secondary, align: Align::Leading });
        let cy = top + px(34.0);
        let mut left = px(20.0);
        if let Some(muted) = row.muted {
            let r = Rect::new(px(16.0), cy - px(15.0), px(30.0), px(30.0));
            list.push(DrawOp::Circle { center: Point { x: r.x + r.w / 2.0, y: cy }, radius: px(15.0), color: if muted { tokens.accent_soft } else { tokens.raised } });
            list.push(DrawOp::Text { rect: r, text: row.icon.into(), style: icon_style(px(15.0)), color: if muted { tokens.accent } else { tokens.text }, align: Align::Center });
            hits.push((r, DeviceAction::ToggleMute(row.id.clone())));
            left = r.right() + px(12.0);
        } else {
            list.push(DrawOp::Text { rect: Rect::new(px(16.0), cy - px(15.0), px(30.0), px(30.0)), text: row.icon.into(), style: icon_style(px(15.0)), color: tokens.text_secondary, align: Align::Center });
            left = px(58.0).max(left);
        }
        let track = Rect::new(left, cy - px(2.5), size.w - left - px(70.0), px(5.0));
        match row.percent {
            Some(p) => {
                let frac = p.min(100) as f32 / 100.0;
                let dim = row.muted == Some(true);
                list.push(DrawOp::RoundRect { rect: track, radius: track.h / 2.0, color: tokens.raised_strong });
                list.push(DrawOp::RoundRect { rect: Rect::new(track.x, track.y, (track.w * frac).max(track.h), track.h), radius: track.h / 2.0, color: if dim { tokens.text_tertiary } else { tokens.accent } });
                let thumb = Point { x: track.x + track.w * frac, y: cy };
                list.push(DrawOp::Circle { center: thumb, radius: px(7.0), color: Rgba::WHITE.with_alpha(0.98) });
                list.push(DrawOp::Text { rect: Rect::new(size.w - px(62.0), cy - px(11.0), px(44.0), px(22.0)), text: format!("{p}%"), style: text(13.0, FontWeight::SemiBold), color: tokens.text, align: Align::Trailing });
                // O trilho clicável é mais alto que o desenho (±14 px), para acertar fácil.
                sliders.push((row.id.clone(), Rect::new(track.x, cy - px(14.0), track.w, px(28.0))));
            }
            None => list.push(DrawOp::Text { rect: Rect::new(left, cy - px(11.0), size.w - left - px(20.0), px(22.0)), text: "Indisponível".into(), style: text(12.0, FontWeight::Regular), color: tokens.text_tertiary, align: Align::Leading }),
        }
    }
    if shown > 0 {
        let top = px(44.0 + rows.len().max(1) as f32 * ROW_H);
        list.push(DrawOp::RoundRect { rect: Rect::new(px(20.0), top, size.w - px(40.0), px(1.0)), radius: 0.0, color: tokens.hairline });
        list.push(DrawOp::Text { rect: Rect::new(px(20.0), top + px(6.0), size.w - px(40.0), px(20.0)), text: "Dispositivo".into(), style: text(11.5, FontWeight::SemiBold), color: tokens.text_tertiary, align: Align::Leading });
        for (i, d) in devices.iter().take(shown).enumerate() {
            let r = Rect::new(px(10.0), top + px(30.0 + i as f32 * DEVICE_H), size.w - px(20.0), px(DEVICE_H));
            if d.current {
                list.push(DrawOp::RoundRect { rect: r, radius: px(10.0), color: tokens.raised });
            }
            let dot = Point { x: r.x + px(18.0), y: r.y + r.h / 2.0 };
            list.push(DrawOp::Circle { center: dot, radius: px(6.0), color: if d.current { tokens.accent } else { tokens.raised_strong } });
            if d.current {
                list.push(DrawOp::Circle { center: dot, radius: px(2.5), color: Rgba::WHITE });
            }
            list.push(DrawOp::Text { rect: Rect::new(r.x + px(34.0), r.y, r.w - px(42.0), r.h), text: d.name.clone(), style: text(12.5, if d.current { FontWeight::SemiBold } else { FontWeight::Regular }), color: if d.current { tokens.text } else { tokens.text_secondary }, align: Align::Leading });
            if !d.current {
                hits.push((r, DeviceAction::Device(d.id.clone())));
            }
        }
    }
    DevicePanel { list, sliders, hits }
}

impl DevicePanel {
    pub fn hit(&self, p: Point) -> Option<DeviceAction> {
        self.hits.iter().find(|(r, _)| r.contains(p)).map(|(_, a)| a.clone())
    }

    /// Slider sob o ponto (id da linha e o trilho, para converter a posição em %).
    pub fn slider_at(&self, p: Point) -> Option<(String, Rect)> {
        self.sliders.iter().find(|(_, r)| r.contains(p)).cloned()
    }

    pub fn slider(&self, id: &str) -> Option<Rect> {
        self.sliders.iter().find(|(i, _)| i == id).map(|(_, r)| *r)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use zefiro_platform::text::FixedWidthMeasurer;
    use zefiro_resources::theme::{BUILTIN, parse_theme};
    use zefiro_resources::tokens::derive_tokens;

    fn tokens() -> Tokens {
        derive_tokens(&parse_theme("obsidian", "o", BUILTIN[0].1).unwrap(), Rgba::rgb(0, 120, 215), None)
    }

    fn texts(l: &DisplayList) -> Vec<String> {
        l.ops.iter().filter_map(|o| if let DrawOp::Text { text, .. } = o { Some(text.clone()) } else { None }).collect()
    }

    #[test]
    fn icons_follow_level_and_mute() {
        assert_eq!(speaker_icon(0, false), ICON_MUTE);
        assert_eq!(speaker_icon(80, true), ICON_MUTE);
        assert_eq!(speaker_icon(10, false), ICON_VOLUME[1]);
        assert_eq!(speaker_icon(100, false), ICON_VOLUME[3]);
        assert_eq!(mic_icon(true), ICON_MIC_OFF);
    }

    #[test]
    fn chip_shows_icon_and_percent() {
        let c = level_chip(Level { glyph: ICON_BRIGHTNESS, text: "50%", dim: false }, 28.0, 12.0, 1.0, &tokens(), &FixedWidthMeasurer);
        assert_eq!(texts(&c.list), vec![ICON_BRIGHTNESS.to_string(), "50%".into()]);
        let only_icon = level_chip(Level { glyph: ICON_MIC, text: "", dim: true }, 28.0, 12.0, 1.0, &tokens(), &FixedWidthMeasurer);
        assert_eq!(only_icon.list.ops.len(), 1);
        assert!(only_icon.content_w < c.content_w);
    }

    #[test]
    fn chip_width_does_not_follow_the_level() {
        // O painel aberto nasce do chip: se a largura mudasse com o nível, ele andaria sob o cursor.
        let w = |text: &str| level_chip(Level { glyph: ICON_MUTE, text, dim: false }, 28.0, 12.0, 1.0, &tokens(), &FixedWidthMeasurer).content_w;
        assert_eq!(w("9%"), w("100%"));
        assert_eq!(w("68%"), w("100%"));
    }

    #[test]
    fn muted_volume_keeps_its_percent() {
        assert_eq!(volume_level(Some((94, true)), true), (ICON_MIC_OFF, "94%".to_string(), true));
        assert_eq!(volume_level(Some((94, false)), true), (ICON_MIC, "94%".to_string(), false));
        assert_eq!(volume_level(Some((68, true)), false), (ICON_MUTE, "68%".to_string(), true));
        assert_eq!(volume_level(None, false), (ICON_MUTE, String::new(), true));
    }

    #[test]
    fn audio_panel_has_mute_slider_and_devices() {
        let rows = [SliderRow { id: "out".into(), label: "Alto-falantes".into(), percent: Some(40), muted: Some(false), icon: speaker_icon(40, false) }];
        let devices = [DeviceItem { id: "a".into(), name: "Alto-falantes".into(), current: true }, DeviceItem { id: "b".into(), name: "Fones".into(), current: false }];
        let p = device_panel("Saída de som", &rows, &devices, 1.0, &tokens());
        assert_eq!(p.list.size.w, 340.0);
        let t = texts(&p.list);
        assert!(t.contains(&"40%".to_string()) && t.contains(&"Fones".to_string()) && t.contains(&"Dispositivo".to_string()));
        // Mudo, trilho e só o dispositivo que não é o atual clicável.
        assert_eq!(p.hit(Point { x: 31.0, y: 78.0 }), Some(DeviceAction::ToggleMute("out".into())));
        let (id, track) = p.slider_at(Point { x: 150.0, y: 78.0 }).unwrap();
        assert_eq!(id, "out");
        assert_eq!((track.x, track.right()), (58.0, 270.0));
        assert!(p.hits.iter().any(|(_, a)| *a == DeviceAction::Device("b".into())));
        assert!(!p.hits.iter().any(|(_, a)| *a == DeviceAction::Device("a".into())));
    }

    #[test]
    fn brightness_panel_lists_each_monitor_and_unavailable_ones() {
        let rows = [
            SliderRow { id: "\\\\.\\DISPLAY1".into(), label: "AOC 2702".into(), percent: Some(50), muted: None, icon: ICON_BRIGHTNESS },
            SliderRow { id: "\\\\.\\DISPLAY2".into(), label: "LG".into(), percent: None, muted: None, icon: ICON_BRIGHTNESS },
        ];
        let p = device_panel("Brilho", &rows, &[], 1.0, &tokens());
        assert_eq!(p.list.size.h, 44.0 + 2.0 * 56.0 + 10.0);
        assert_eq!(p.sliders.len(), 1);
        assert!(texts(&p.list).contains(&"Indisponível".to_string()));
        assert!(p.hits.is_empty(), "sem mudo no brilho");
    }
}
