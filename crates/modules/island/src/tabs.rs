//! Abas do painel expandido (roda do mouse troca, como na ilha C++): mídia, clima e monitor de
//! hardware, todas com 380 × 184 para a troca não mexer na forma do painel.

use zefiro_platform::color::Rgba;
use zefiro_platform::geometry::{Point, Rect, Size};
use zefiro_platform::text::{Align, Family, FontWeight, TextMeasurer, TextStyle};
use zefiro_resources::tokens::Tokens;
use zefiro_ui::paint::{DisplayList, DrawOp};

use crate::view::EXPANDED_DESIGN;
use crate::lyrics::{Lyrics, current_line, line_progress};
use crate::weather::{Weather, icon};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Tab { #[default] Media, Weather, Hardware, Lyrics }

/// Abas disponíveis agora, na ordem da ilha.
pub fn available(has_media: bool, has_weather: bool, has_lyrics: bool) -> Vec<Tab> {
    let mut tabs = Vec::new();
    if has_media {
        tabs.push(Tab::Media);
    }
    if has_weather {
        tabs.push(Tab::Weather);
    }
    tabs.push(Tab::Hardware);
    if has_media && has_lyrics {
        tabs.push(Tab::Lyrics);
    }
    tabs
}

/// Próxima aba pela roda (`delta` > 0 = para cima = anterior), sem dar a volta.
pub fn step(tabs: &[Tab], current: Tab, delta: i32) -> Tab {
    let i = tabs.iter().position(|t| *t == current).unwrap_or(0) as i32;
    let next = (i + if delta > 0 { -1 } else { 1 }).clamp(0, tabs.len() as i32 - 1);
    tabs[next as usize]
}

/// Carrossel das abas, como a ilha C++ (`DrawTabTransition`): uma posição contínua persegue a aba
/// escolhida com a mola 250/31 (assenta em ~0,29 s, sem balanço). Cada aba é desenhada pela
/// distância até essa posição (desliza 34 px, encolhe até 96,5 % e some), então a troca pode ser
/// interrompida e invertida no meio sem pular.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Carousel { pos: f32, vel: f32, target: f32, count: usize }

pub const TAB_STIFFNESS: f32 = 250.0;
pub const TAB_DAMPING: f32 = 31.0;
/// Distância entre abas vizinhas (px de conteúdo); na horizontal, porque os pontos ficam lado a lado.
pub const TAB_TRAVEL: f32 = 34.0;
const TAB_MIN_SCALE: f32 = 0.965;
const TAB_FADE_POWER: f32 = 1.4;

fn smoothstep(x: f32) -> f32 {
    let x = x.clamp(0.0, 1.0);
    x * x * (3.0 - 2.0 * x)
}

/// Uma aba visível num quadro: índice, opacidade, deslocamento (px de conteúdo) e escala.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TabLayer { pub index: usize, pub alpha: f32, pub dx: f32, pub scale: f32 }

impl Carousel {
    /// Sem animação: painel abrindo ou o número de abas mudou.
    pub fn snap(&mut self, index: usize, count: usize) {
        *self = Carousel { pos: index as f32, vel: 0.0, target: index as f32, count };
    }

    pub fn count(&self) -> usize {
        self.count
    }

    pub fn go(&mut self, index: usize) {
        self.target = index.min(self.count.saturating_sub(1)) as f32;
    }

    pub fn moving(&self) -> bool {
        self.pos != self.target || self.vel != 0.0
    }

    /// Avança `dt` segundos (`speed` = velocidade das animações do tema). Devolve se ainda se move.
    pub fn step(&mut self, dt: f32, speed: f32) -> bool {
        let max = self.count.saturating_sub(1) as f32;
        let mut remaining = dt.clamp(0.0, 0.05) * speed;
        while remaining > 0.0 && self.moving() {
            let h = remaining.min(0.004);
            self.vel += (-TAB_STIFFNESS * (self.pos - self.target) - TAB_DAMPING * self.vel) * h;
            self.pos += self.vel * h;
            // A primeira e a última aba são batentes: sem passar delas nem tremer.
            if self.pos < 0.0 {
                self.pos = 0.0;
                self.vel = self.vel.max(0.0);
            } else if self.pos > max {
                self.pos = max;
                self.vel = self.vel.min(0.0);
            }
            remaining -= h;
        }
        if (self.pos - self.target).abs() < 0.0008 && self.vel.abs() < 0.02 {
            self.pos = self.target;
            self.vel = 0.0;
        }
        self.moving()
    }

    /// Abas a desenhar agora (parado: só a atual, inteira).
    pub fn layers(&self) -> Vec<TabLayer> {
        (0..self.count)
            .filter_map(|index| {
                let d = index as f32 - self.pos;
                let eased = smoothstep(d.abs());
                let alpha = (1.0 - eased).powf(TAB_FADE_POWER);
                (alpha > 0.004).then_some(TabLayer { index, alpha, dx: d * TAB_TRAVEL, scale: 1.0 - (1.0 - TAB_MIN_SCALE) * eased })
            })
            .collect()
    }
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

/// Letras: linha anterior apagada, a atual grande com o karaokê (parte cantada acende da esquerda
/// para a direita) e as duas próximas apagadas. Sem sincronia, só o começo do texto.
pub fn lyrics_tab(l: &Lyrics, position_ms: i64, scale: f32, font_scale: f32, tokens: &Tokens, m: &dyn TextMeasurer) -> DisplayList {
    let px = |v: f32| v * scale;
    let fs = |v: f32| v * scale * font_scale;
    let mut list = panel(scale);
    let x = px(24.0);
    let w = list.size.w - 2.0 * x;
    let dim = text(fs(13.0), FontWeight::Regular);
    let line = |list: &mut DisplayList, s: &str, y: f32, h: f32, style: TextStyle, color: Rgba| list.push(DrawOp::Text { rect: Rect::new(x, y, w, h), text: s.to_string(), style, color, align: Align::Center });
    if !l.synced {
        for (i, t) in l.lines.iter().take(6).enumerate() {
            line(&mut list, &t.text, px(14.0) + i as f32 * px(24.0), px(22.0), dim, tokens.text_secondary);
        }
        return list;
    }
    let current = current_line(&l.lines, position_ms);
    let text_at = |i: usize| l.lines.get(i).map(|x| if x.text.is_empty() { "♪" } else { x.text.as_str() });
    if let Some(prev) = current.and_then(|i| i.checked_sub(1)).and_then(text_at) {
        line(&mut list, prev, px(16.0), px(22.0), dim, tokens.text_tertiary);
    }
    let big = text(fs(19.5), FontWeight::SemiBold);
    let main_y = px(50.0);
    let main_h = px(38.0);
    match current.and_then(|i| text_at(i).map(|t| (i, t))) {
        Some((i, t)) => {
            // Parte não cantada a 60 %, depois a parte cantada recortada até a frente da varredura.
            line(&mut list, t, main_y, main_h, big, tokens.text.with_alpha(0.6 * tokens.text.a));
            let tw = m.measure(t, &big).w.min(w);
            let start = x + (w - tw) / 2.0;
            let front = start + tw * line_progress(&l.lines, i, position_ms);
            list.push(DrawOp::PushClip(Rect::new(x, main_y, (front - x).max(0.0), main_h)));
            line(&mut list, t, main_y, main_h, big, tokens.accent);
            list.push(DrawOp::PopClip);
        }
        None => line(&mut list, "♪", main_y, main_h, big, tokens.text_secondary),
    }
    let next = current.map_or(0, |i| i + 1);
    for (k, y) in [(0, 98.0), (1, 124.0)] {
        if let Some(t) = text_at(next + k) {
            line(&mut list, t, px(y), px(22.0), dim, if k == 0 { tokens.text_secondary } else { tokens.text_tertiary });
        }
    }
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
        assert_eq!(available(true, true, false), vec![Tab::Media, Tab::Weather, Tab::Hardware]);
        assert_eq!(available(true, false, true), vec![Tab::Media, Tab::Hardware, Tab::Lyrics]);
        assert_eq!(available(false, false, true), vec![Tab::Hardware], "sem mídia, sem letra");
        assert_eq!(available(false, false, false), vec![Tab::Hardware]);
        let tabs = available(true, true, false);
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

#[cfg(test)]
mod lyrics_tests {
    use super::*;
    use crate::lyrics::{parse_lrc, parse_plain};
    use zefiro_platform::text::FixedWidthMeasurer;
    use zefiro_resources::theme::{BUILTIN, parse_theme};
    use zefiro_resources::tokens::derive_tokens;

    fn tokens() -> Tokens {
        derive_tokens(&parse_theme("obsidian", "o", BUILTIN[0].1).unwrap(), Rgba::rgb(0, 120, 215), None)
    }

    #[test]
    fn current_line_is_drawn_twice_with_the_sung_part_clipped() {
        let l = Lyrics { synced: true, lines: parse_lrc("[00:01.00]um\n[00:05.00]dois três\n[00:09.00]quatro\n[00:12.00]cinco\n") };
        let list = lyrics_tab(&l, 6_510, 1.0, 1.0, &tokens(), &FixedWidthMeasurer);
        let texts: Vec<&str> = list.ops.iter().filter_map(|o| if let DrawOp::Text { text, .. } = o { Some(text.as_str()) } else { None }).collect();
        assert_eq!(texts, ["um", "dois três", "dois três", "quatro", "cinco"]);
        let clip = list.ops.iter().find_map(|o| if let DrawOp::PushClip(r) = o { Some(*r) } else { None }).unwrap();
        // Metade da linha cantada: a frente fica no meio do texto centralizado.
        let tw = FixedWidthMeasurer.measure("dois três", &text(19.5, FontWeight::SemiBold)).w;
        assert!((clip.right() - (190.0 - tw / 2.0 + tw * 0.5)).abs() < 1.0, "{clip:?}");
        assert!(list.ops.contains(&DrawOp::PopClip));
    }

    #[test]
    fn before_the_first_line_and_plain_lyrics() {
        let l = Lyrics { synced: true, lines: parse_lrc("[00:10.00]um\n") };
        let list = lyrics_tab(&l, 0, 1.0, 1.0, &tokens(), &FixedWidthMeasurer);
        assert!(list.ops.iter().any(|o| matches!(o, DrawOp::Text { text, .. } if text == "♪")));
        let plain = Lyrics { synced: false, lines: parse_plain("a\nb") };
        assert_eq!(lyrics_tab(&plain, 0, 1.0, 1.0, &tokens(), &FixedWidthMeasurer).ops.len(), 2);
    }
}

#[cfg(test)]
mod carousel_tests {
    use super::*;

    #[test]
    fn settles_in_about_three_tenths_of_a_second_without_overshoot() {
        let mut c = Carousel::default();
        c.snap(0, 3);
        assert_eq!(c.layers(), vec![TabLayer { index: 0, alpha: 1.0, dx: 0.0, scale: 1.0 }]);
        c.go(1);
        let mut t = 0.0;
        let mut max_pos: f32 = 0.0;
        while c.step(1.0 / 120.0, 1.0) && t < 2.0 {
            t += 1.0 / 120.0;
            max_pos = max_pos.max(c.layers().iter().map(|l| l.index as f32 - l.dx / TAB_TRAVEL).fold(0.0, f32::max));
        }
        assert!((0.2..0.6).contains(&t), "{t}");
        assert!(max_pos <= 1.01, "sem passar da aba: {max_pos}");
        assert_eq!(c.layers().len(), 1);
    }

    #[test]
    fn mid_move_draws_both_tabs_sliding_shrinking_and_fading() {
        let mut c = Carousel::default();
        c.snap(0, 2);
        c.go(1);
        // Avança até cerca da metade do caminho.
        while c.layers().iter().all(|l| l.index == 0 || l.alpha < 0.2) {
            c.step(0.004, 1.0);
        }
        let layers = c.layers();
        assert_eq!(layers.len(), 2);
        let (old, new) = (layers[0], layers[1]);
        assert!(old.dx < 0.0 && new.dx > 0.0, "a antiga sai pela esquerda, a nova entra pela direita");
        assert!(old.scale < 1.0 && new.scale < 1.0 && old.alpha < 1.0);
    }

    #[test]
    fn edges_are_hard_stops_and_double_speed_is_faster() {
        let mut c = Carousel::default();
        c.snap(1, 2);
        c.go(5);
        assert!(!c.moving(), "já está na última");
        let time = |speed: f32| {
            let mut c = Carousel::default();
            c.snap(0, 2);
            c.go(1);
            let mut t = 0.0;
            while c.step(1.0 / 240.0, speed) {
                t += 1.0 / 240.0;
            }
            t
        };
        assert!(time(2.0) < time(1.0) * 0.6);
    }
}
