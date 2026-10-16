//! Abas do painel expandido (roda do mouse troca, como na ilha C++): mídia, clima e monitor de
//! hardware, todas com 380 × 184 para a troca não mexer na forma do painel.

use zefiro_platform::color::Rgba;
use zefiro_platform::geometry::{Point, Rect, Size};
use zefiro_platform::text::{Align, Family, FontWeight, TextStyle};
use zefiro_resources::tokens::Tokens;
use zefiro_ui::paint::{DisplayList, DrawOp};

use crate::view::EXPANDED_DESIGN;
use crate::weather::{Weather, icon};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Tab { #[default] Media, Weather, Hardware }

/// Abas disponíveis agora, na ordem da ilha.
pub fn available(has_media: bool, has_weather: bool) -> Vec<Tab> {
    let mut tabs = Vec::new();
    if has_media {
        tabs.push(Tab::Media);
    }
    if has_weather {
        tabs.push(Tab::Weather);
    }
    tabs.push(Tab::Hardware);
    tabs
}

/// Próxima aba pela roda (`delta` > 0 = para cima = anterior), sem dar a volta.
pub fn step(tabs: &[Tab], current: Tab, delta: i32) -> Tab {
    let i = tabs.iter().position(|t| *t == current).unwrap_or(0) as i32;
    let next = (i + if delta > 0 { -1 } else { 1 }).clamp(0, tabs.len() as i32 - 1);
    tabs[next as usize]
}

#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Hardware {
    pub cpu: f32,
    pub gpu: Option<f32>,
    pub ram: f32,
    pub ram_used_gb: f32,
    pub ram_total_gb: f32,
    pub disk: f32,
    pub disk_free_gb: f32,
    pub net_down_bps: f64,
    pub net_up_bps: f64,
}

fn text(size: f32, weight: FontWeight) -> TextStyle {
    TextStyle { size, weight, family: Family::Text }
}

/// Cor da barra de carga: normal até 75 %, âmbar acima de 75 %, vermelho acima de 90 %.
pub fn load_color(pct: f32, tokens: &Tokens) -> Rgba {
    if pct > 90.0 {
        Rgba::rgb(0xFF, 0x45, 0x3A)
    } else if pct > 75.0 {
        Rgba::rgb(0xFF, 0x9F, 0x0A)
    } else {
        tokens.accent
    }
}

/// Pontos das abas no rodapé, a atual destacada.
pub fn tab_dots(list: &mut DisplayList, count: usize, index: usize, scale: f32, tokens: &Tokens) {
    if count < 2 {
        return;
    }
    let gap = 10.0 * scale;
    let y = list.size.h - 9.0 * scale;
    let x0 = list.size.w / 2.0 - gap * (count as f32 - 1.0) / 2.0;
    for i in 0..count {
        let color = if i == index { tokens.text } else { tokens.text_tertiary };
        list.push(DrawOp::Circle { center: Point { x: x0 + gap * i as f32, y }, radius: 2.5 * scale, color });
    }
}

fn panel(scale: f32) -> DisplayList {
    DisplayList::new(Size { w: EXPANDED_DESIGN.w * scale, h: EXPANDED_DESIGN.h * scale })
}

/// `day_names`: rótulos dos 3 dias da previsão (ex.: "Hoje", "Amanhã", "dom").
pub fn weather_tab(w: &Weather, day_names: &[String; 3], scale: f32, font_scale: f32, tokens: &Tokens) -> DisplayList {
    let px = |v: f32| v * scale;
    let fs = |v: f32| v * scale * font_scale;
    let mut list = panel(scale);
    let left = px(24.0);
    list.push(DrawOp::Text { rect: Rect::new(left, px(16.0), px(60.0), px(60.0)), text: icon(w.code).into(), style: text(fs(36.0), FontWeight::Regular), color: tokens.text, align: Align::Center });
    list.push(DrawOp::Text { rect: Rect::new(px(92.0), px(14.0), px(120.0), px(44.0)), text: format!("{}°", w.temp_c), style: TextStyle { size: fs(34.0), weight: FontWeight::SemiBold, family: Family::Display }, color: tokens.text, align: Align::Leading });
    list.push(DrawOp::Text { rect: Rect::new(px(92.0), px(56.0), px(264.0), px(18.0)), text: w.description.clone(), style: text(fs(12.5), FontWeight::Regular), color: tokens.text_secondary, align: Align::Leading });
    list.push(DrawOp::Text { rect: Rect::new(px(200.0), px(20.0), px(156.0), px(18.0)), text: w.city.clone(), style: text(fs(13.0), FontWeight::SemiBold), color: tokens.text, align: Align::Trailing });
    let details = format!("Sensação {}° · Umidade {}% · Vento {} km/h", w.feels_like_c, w.humidity, w.wind_kmh);
    list.push(DrawOp::Text { rect: Rect::new(left, px(84.0), px(332.0), px(18.0)), text: details, style: text(fs(11.5), FontWeight::Regular), color: tokens.text_tertiary, align: Align::Leading });
    // Previsão: 3 cartões.
    let card_w = px(104.0);
    for (i, d) in w.days.iter().take(3).enumerate() {
        let r = Rect::new(left + i as f32 * (card_w + px(10.0)), px(110.0), card_w, px(54.0));
        list.push(DrawOp::RoundRect { rect: r, radius: px(12.0), color: tokens.raised });
        list.push(DrawOp::Text { rect: Rect::new(r.x + px(10.0), r.y + px(6.0), px(60.0), px(18.0)), text: day_names[i].clone(), style: text(fs(11.5), FontWeight::SemiBold), color: tokens.text_secondary, align: Align::Leading });
        list.push(DrawOp::Text { rect: Rect::new(r.right() - px(36.0), r.y + px(4.0), px(28.0), px(22.0)), text: icon(d.code).into(), style: text(fs(15.0), FontWeight::Regular), color: tokens.text, align: Align::Center });
        list.push(DrawOp::Text { rect: Rect::new(r.x + px(10.0), r.y + px(26.0), r.w - px(20.0), px(22.0)), text: format!("{}° / {}°", d.max_c, d.min_c), style: text(fs(13.0), FontWeight::SemiBold), color: tokens.text, align: Align::Leading });
    }
    list
}

fn bytes_per_s(v: f64) -> String {
    let (n, unit) = if v >= 1e6 { (v / 1e6, "MB/s") } else if v >= 1e3 { (v / 1e3, "KB/s") } else { (v, "B/s") };
    if n >= 100.0 || unit == "B/s" { format!("{n:.0} {unit}") } else { format!("{n:.1} {unit}") }
}

pub fn hardware_tab(h: &Hardware, scale: f32, font_scale: f32, tokens: &Tokens) -> DisplayList {
    let px = |v: f32| v * scale;
    let fs = |v: f32| v * scale * font_scale;
    let mut list = panel(scale);
    let rows: Vec<(&str, f32, String)> = [
        Some(("CPU", h.cpu, format!("{:.0}%", h.cpu))),
        h.gpu.map(|g| ("GPU", g, format!("{g:.0}%"))),
        Some(("RAM", h.ram, format!("{:.1} / {:.0} GB", h.ram_used_gb, h.ram_total_gb))),
        Some(("Disco C:", h.disk, format!("{:.0} GB livres", h.disk_free_gb))),
    ]
    .into_iter()
    .flatten()
    .collect();
    let top = px(18.0);
    let row_h = px(30.0);
    for (i, (label, pct, value)) in rows.iter().enumerate() {
        let y = top + i as f32 * row_h;
        list.push(DrawOp::Text { rect: Rect::new(px(24.0), y, px(64.0), px(20.0)), text: (*label).into(), style: text(fs(12.0), FontWeight::SemiBold), color: tokens.text_secondary, align: Align::Leading });
        let track = Rect::new(px(92.0), y + px(8.0), px(150.0), px(5.0));
        list.push(DrawOp::RoundRect { rect: track, radius: track.h / 2.0, color: tokens.raised_strong });
        let frac = (pct / 100.0).clamp(0.0, 1.0);
        if frac > 0.0 {
            list.push(DrawOp::RoundRect { rect: Rect::new(track.x, track.y, (track.w * frac).max(track.h), track.h), radius: track.h / 2.0, color: load_color(*pct, tokens) });
        }
        list.push(DrawOp::Text { rect: Rect::new(px(250.0), y, px(106.0), px(20.0)), text: value.clone(), style: text(fs(12.0), FontWeight::Regular), color: tokens.text, align: Align::Trailing });
    }
    let net = format!("↓ {}   ↑ {}", bytes_per_s(h.net_down_bps), bytes_per_s(h.net_up_bps));
    list.push(DrawOp::Text { rect: Rect::new(px(24.0), top + rows.len() as f32 * row_h + px(2.0), px(332.0), px(20.0)), text: net, style: text(fs(12.0), FontWeight::Regular), color: tokens.text_secondary, align: Align::Leading });
    list
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::weather::Day;
    use zefiro_resources::theme::{BUILTIN, parse_theme};
    use zefiro_resources::tokens::derive_tokens;

    fn tokens() -> Tokens {
        derive_tokens(&parse_theme("obsidian", "o", BUILTIN[0].1).unwrap(), Rgba::rgb(0, 120, 215), None)
    }

    fn texts(l: &DisplayList) -> Vec<String> {
        l.ops.iter().filter_map(|o| if let DrawOp::Text { text, .. } = o { Some(text.clone()) } else { None }).collect()
    }

    #[test]
    fn tabs_follow_what_is_available_and_the_wheel_does_not_wrap() {
        assert_eq!(available(true, true), vec![Tab::Media, Tab::Weather, Tab::Hardware]);
        assert_eq!(available(false, false), vec![Tab::Hardware]);
        let tabs = available(true, true);
        assert_eq!(step(&tabs, Tab::Media, -120), Tab::Weather);
        assert_eq!(step(&tabs, Tab::Media, 120), Tab::Media);
        assert_eq!(step(&tabs, Tab::Hardware, -120), Tab::Hardware);
    }

    #[test]
    fn weather_tab_shows_reading_and_forecast() {
        let w = Weather { city: "Guarulhos".into(), temp_c: 21, feels_like_c: 20, humidity: 86, wind_kmh: 17, description: "Chuva fraca".into(), code: 176, days: vec![Day { max_c: 23, min_c: 19, code: 113 }] };
        let l = weather_tab(&w, &["Hoje".into(), "Amanhã".into(), "dom".into()], 1.0, 1.0, &tokens());
        assert_eq!(l.size, Size { w: 380.0, h: 184.0 });
        let t = texts(&l);
        for expected in ["21°", "Chuva fraca", "Guarulhos", "Sensação 20° · Umidade 86% · Vento 17 km/h", "Hoje", "23° / 19°"] {
            assert!(t.iter().any(|s| s == expected), "{expected} em {t:?}");
        }
    }

    #[test]
    fn hardware_tab_rows_colors_and_units() {
        let h = Hardware { cpu: 95.0, gpu: None, ram: 80.0, ram_used_gb: 12.8, ram_total_gb: 16.0, disk: 40.0, disk_free_gb: 300.0, net_down_bps: 1_500_000.0, net_up_bps: 900.0 };
        let l = hardware_tab(&h, 1.0, 1.0, &tokens());
        let t = texts(&l);
        assert!(!t.contains(&"GPU".to_string()), "sem GPU, sem linha");
        assert!(t.contains(&"12.8 / 16 GB".to_string()));
        assert!(t.contains(&"↓ 1.5 MB/s   ↑ 900 B/s".to_string()));
        assert_eq!(load_color(95.0, &tokens()), Rgba::rgb(0xFF, 0x45, 0x3A));
        assert_eq!(load_color(80.0, &tokens()), Rgba::rgb(0xFF, 0x9F, 0x0A));
        assert_eq!(load_color(10.0, &tokens()), tokens().accent);
    }
}
