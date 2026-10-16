//! O que a ilha desenha: a pílula recolhida (ocioso, mídia, espiando) e o painel expandido da mídia.
//!
//! Medidas em "px de conteúdo" do Dynamic Island for Windows (§3.2 e §6 das notas de porte). A
//! pílula recolhida escala pela altura da barra (como `[Zefiro] PillHeight` fazia no fork); o painel
//! expandido escala só pelo DPI.

use zefiro_platform::color::Rgba;
use zefiro_platform::geometry::{Point, Rect, Size};
use zefiro_platform::text::{Align, Family, FontWeight, TextMeasurer, TextStyle};
use zefiro_resources::tokens::Tokens;
use zefiro_ui::paint::{DisplayList, DrawOp};

use crate::media::{MediaSession, Playback};

/// Alturas de projeto de cada estado recolhido.
pub const MEDIA_DESIGN: Size = Size { w: 150.0, h: 44.0 };
pub const PEEK_DESIGN: Size = Size { w: 320.0, h: 44.0 };
pub const IDLE_DESIGN_H: f32 = 36.0;
pub const EXPANDED_DESIGN: Size = Size { w: 380.0, h: 184.0 };
/// Reserva estável no centro da barra: a maior pílula recolhida (espiando), para os chips das
/// laterais não pularem quando a ilha muda de estado.
pub const RESERVE_DESIGN_W: f32 = PEEK_DESIGN.w + 40.0;

const ICON_PREVIOUS: &str = "\u{E892}";
const ICON_NEXT: &str = "\u{E893}";
const ICON_PLAY: &str = "\u{E768}";
const ICON_PAUSE: &str = "\u{E769}";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Collapsed { Idle, Media, Peek }

/// Conteúdo de uma pílula recolhida: tamanho, desenho e onde ficam as barras do visualizador.
#[derive(Debug, Clone, PartialEq)]
pub struct PillContent {
    pub size: Size,
    pub list: DisplayList,
    /// Área das barras do visualizador (coordenadas da pílula), quando há mídia.
    pub visualizer: Option<Rect>,
}

/// Quantas barras cabem na área do visualizador.
pub fn visualizer_bars(area: Rect, k: f32) -> usize {
    let pitch = 2.5 * k + 2.0 * k;
    ((area.w + 2.5 * k) / pitch).floor().clamp(3.0, 32.0) as usize
}

fn style(size: f32, weight: FontWeight) -> TextStyle {
    TextStyle { size, weight, family: Family::Text }
}

fn icon(size: f32) -> TextStyle {
    TextStyle { size, weight: FontWeight::Regular, family: Family::Icons }
}

fn round2(v: f32) -> f32 {
    (v / 2.0).ceil() * 2.0
}

/// Ocioso: só o relógio. Largura medida pelo texto (mín. 72, máx. 280), arredondada para 2 px.
pub fn idle_pill(clock: &str, pill_h: f32, font_scale: f32, tokens: &Tokens, m: &dyn TextMeasurer) -> PillContent {
    let k = pill_h / IDLE_DESIGN_H;
    let text = style(13.0 * k * font_scale, FontWeight::SemiBold);
    let tw = m.measure(clock, &text).w.ceil();
    let w = round2((tw + 28.0 * k).clamp(72.0 * k, 280.0 * k));
    let mut list = DisplayList::new(Size { w, h: pill_h });
    list.push(DrawOp::Text { rect: Rect::new(0.0, 0.0, w, pill_h), text: clock.into(), style: text, color: tokens.text, align: Align::Center });
    PillContent { size: Size { w, h: pill_h }, list, visualizer: None }
}

/// Mídia recolhida: capa redonda à esquerda e visualizador à direita. Espiando: capa, "título —
/// artista" e o visualizador, numa pílula mais larga.
pub fn media_pill(s: &MediaSession, peek: bool, pill_h: f32, font_scale: f32, tokens: &Tokens) -> PillContent {
    let design = if peek { PEEK_DESIGN } else { MEDIA_DESIGN };
    let k = pill_h / design.h;
    let w = round2(design.w * k);
    let pad = 6.0 * k;
    let art = pill_h - 2.0 * pad;
    let mut list = DisplayList::new(Size { w, h: pill_h });
    let art_rect = Rect::new(pad, pad, art, art);
    match &s.cover {
        Some(image) => list.push(DrawOp::Image { rect: art_rect, image: image.clone(), radius: art / 2.0, opacity: 1.0 }),
        None => {
            list.push(DrawOp::RoundRect { rect: art_rect, radius: art / 2.0, color: tokens.raised_strong });
            list.push(DrawOp::Text { rect: art_rect, text: "\u{E8D6}".into(), style: icon(art * 0.5), color: tokens.text_secondary, align: Align::Center });
        }
    }
    let vis_w = 46.0 * k;
    let vis = Rect::new(w - 14.0 * k - vis_w, pill_h * 0.25, vis_w, pill_h * 0.5);
    if peek {
        let x = art_rect.right() + 10.0 * k;
        let text_w = (vis.x - 10.0 * k - x).max(0.0);
        list.push(DrawOp::Text { rect: Rect::new(x, 0.0, text_w, pill_h), text: s.headline(), style: style(13.0 * k * font_scale, FontWeight::SemiBold), color: tokens.text, align: Align::Leading });
    }
    PillContent { size: Size { w, h: pill_h }, list, visualizer: Some(vis) }
}

#[derive(Debug, Clone, PartialEq)]
pub enum IslandAction { Previous, PlayPause, Next, Seek(f64), Source(String) }

/// Painel expandido da mídia: desenho e áreas de clique (coordenadas do painel).
#[derive(Debug, Clone, PartialEq)]
pub struct ExpandedView { pub list: DisplayList, pub hits: Vec<(Rect, IslandAction)> }

fn time_text(s: f64) -> String {
    let s = s.max(0.0).round() as u64;
    if s >= 3600 { format!("{}:{:02}:{:02}", s / 3600, s / 60 % 60, s % 60) } else { format!("{}:{:02}", s / 60, s % 60) }
}

/// `sources`: (id, nome) de todas as sessões, na ordem estável do SMTC.
pub fn expanded(s: &MediaSession, sources: &[(String, String)], now_ms: i64, scale: f32, font_scale: f32, tokens: &Tokens) -> ExpandedView {
    let px = |v: f32| v * scale;
    let fs = |v: f32| v * scale * font_scale;
    let size = Size { w: px(EXPANDED_DESIGN.w), h: px(EXPANDED_DESIGN.h) };
    let mut list = DisplayList::new(size);
    let mut hits = Vec::new();

    // Capa 64 × 64 em (24, 20).
    let art = Rect::new(px(24.0), px(20.0), px(64.0), px(64.0));
    match &s.cover {
        Some(image) => list.push(DrawOp::Image { rect: art, image: image.clone(), radius: px(16.0), opacity: 1.0 }),
        None => {
            list.push(DrawOp::RoundRect { rect: art, radius: px(16.0), color: tokens.raised_strong });
            list.push(DrawOp::Text { rect: art, text: "\u{E8D6}".into(), style: icon(px(26.0)), color: tokens.text_secondary, align: Align::Center });
        }
    }
    let tx = art.right() + px(14.0);
    let tw = size.w - tx - px(24.0);
    let title = if s.title.is_empty() { s.app_name.clone() } else { s.title.clone() };
    list.push(DrawOp::Text { rect: Rect::new(tx, px(22.0), tw, px(22.0)), text: title, style: style(fs(15.0), FontWeight::SemiBold), color: tokens.text, align: Align::Leading });
    let sub = [s.artist.as_str(), s.album.as_str()].into_iter().filter(|t| !t.is_empty()).collect::<Vec<_>>().join(" · ");
    list.push(DrawOp::Text { rect: Rect::new(tx, px(44.0), tw, px(18.0)), text: sub, style: style(fs(12.5), FontWeight::Regular), color: tokens.text_secondary, align: Align::Leading });
    list.push(DrawOp::Text { rect: Rect::new(tx, px(62.0), tw, px(16.0)), text: s.app_name.clone(), style: style(fs(11.0), FontWeight::Regular), color: tokens.text_tertiary, align: Align::Leading });

    // Linha do tempo (estilo fino) em y = 114, margens 24 + 48.
    if s.duration_s > 0.0 {
        let pos = s.position_at(now_ms);
        let track = Rect::new(px(72.0), px(114.0) - px(2.0), size.w - px(144.0), px(4.0));
        let frac = (pos / s.duration_s).clamp(0.0, 1.0) as f32;
        list.push(DrawOp::RoundRect { rect: track, radius: px(2.0), color: tokens.raised_strong });
        list.push(DrawOp::RoundRect { rect: Rect::new(track.x, track.y, track.w * frac, track.h), radius: px(2.0), color: tokens.accent });
        list.push(DrawOp::Circle { center: Point { x: track.x + track.w * frac, y: track.y + track.h / 2.0 }, radius: px(5.0), color: tokens.text });
        let label = style(fs(11.0), FontWeight::Regular);
        list.push(DrawOp::Text { rect: Rect::new(px(20.0), px(104.0), px(46.0), px(20.0)), text: time_text(pos), style: label, color: tokens.text_secondary, align: Align::Trailing });
        list.push(DrawOp::Text { rect: Rect::new(size.w - px(66.0), px(104.0), px(46.0), px(20.0)), text: time_text(s.duration_s), style: label, color: tokens.text_secondary, align: Align::Leading });
        if s.can_seek {
            // Clique ±14 px na vertical; a fração é calculada por quem trata o clique.
            hits.push((Rect::new(track.x, px(100.0), track.w, px(28.0)), IslandAction::Seek(-1.0)));
        }
    }

    // Controles em y = 148, espaçamento 64; raios 16 e 22 (+6 de folga no clique).
    let cy = px(148.0);
    let cx = size.w / 2.0;
    let playing = s.playback == Playback::Playing;
    let buttons = [
        (cx - px(64.0), 16.0, ICON_PREVIOUS, IslandAction::Previous, s.can_previous),
        (cx, 22.0, if playing { ICON_PAUSE } else { ICON_PLAY }, IslandAction::PlayPause, s.can_play_pause),
        (cx + px(64.0), 16.0, ICON_NEXT, IslandAction::Next, s.can_next),
    ];
    for (x, r, glyph, action, enabled) in buttons {
        let r = px(r);
        let rect = Rect::new(x - r, cy - r, 2.0 * r, 2.0 * r);
        if action == IslandAction::PlayPause {
            list.push(DrawOp::Circle { center: Point { x, y: cy }, radius: r, color: tokens.raised_strong });
        }
        let color = if enabled { tokens.text } else { tokens.text_tertiary };
        list.push(DrawOp::Text { rect, text: glyph.into(), style: icon(px(if action == IslandAction::PlayPause { 18.0 } else { 16.0 })), color, align: Align::Center });
        if enabled {
            hits.push((rect.inset(-px(6.0), -px(6.0)), action));
        }
    }

    // Barra de fontes (2 ou mais): x = 20, altura 30, posições de 26 px, até 3.
    if sources.len() >= 2 {
        for (i, (id, name)) in sources.iter().take(3).enumerate() {
            let rect = Rect::new(px(20.0) + i as f32 * px(28.0), cy - px(13.0), px(26.0), px(26.0));
            let current = *id == s.id;
            list.push(DrawOp::Circle { center: Point { x: rect.x + rect.w / 2.0, y: cy }, radius: px(13.0), color: if current { tokens.accent_soft } else { tokens.raised } });
            let initial = name.chars().next().map(|c| c.to_uppercase().to_string()).unwrap_or_default();
            list.push(DrawOp::Text { rect, text: initial, style: style(fs(11.0), FontWeight::SemiBold), color: if current { tokens.accent } else { tokens.text_secondary }, align: Align::Center });
            hits.push((rect, IslandAction::Source(id.clone())));
        }
    }
    ExpandedView { list, hits }
}

/// Ação no ponto `p` do painel. O clique na linha do tempo vira `Seek(segundos)`.
pub fn hit(view: &ExpandedView, s: &MediaSession, p: Point) -> Option<IslandAction> {
    let (rect, action) = view.hits.iter().find(|(r, _)| r.contains(p))?;
    Some(match action {
        IslandAction::Seek(_) => {

            let frac = ((p.x - rect.x) / rect.w.max(1.0)).clamp(0.0, 1.0) as f64;
            IslandAction::Seek(frac * s.duration_s)
        }
        other => other.clone(),
    })
}

/// Cor das barras do visualizador.
pub fn visualizer_color(tokens: &Tokens) -> Rgba {
    tokens.accent
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::media::{Playback, session};
    use zefiro_platform::color::Rgba;
    use zefiro_platform::text::FixedWidthMeasurer;
    use zefiro_resources::theme::{BUILTIN, parse_theme};
    use zefiro_resources::tokens::derive_tokens;

    fn tokens() -> Tokens {
        derive_tokens(&parse_theme("obsidian", "o", BUILTIN[0].1).unwrap(), Rgba::rgb(0, 120, 215), None)
    }

    #[test]
    fn collapsed_media_scales_with_the_bar_height() {
        let full = media_pill(&session("a", Playback::Playing), false, 44.0, 1.0, &tokens());
        assert_eq!(full.size, Size { w: 150.0, h: 44.0 });
        let compact = media_pill(&session("a", Playback::Playing), false, 28.0, 1.0, &tokens());
        assert_eq!(compact.size.h, 28.0);
        assert_eq!(compact.size.w, 96.0, "150 × 28/44 = 95,45 → múltiplo de 2");
        let vis = compact.visualizer.unwrap();
        assert!(vis.right() <= compact.size.w && vis.x > 28.0, "{vis:?}");
    }

    #[test]
    fn peek_is_wider_and_shows_the_headline() {
        let p = media_pill(&session("a", Playback::Playing), true, 44.0, 1.0, &tokens());
        assert_eq!(p.size.w, 320.0);
        assert!(p.list.ops.iter().any(|o| matches!(o, DrawOp::Text { text, .. } if text == "a música — Artista")));
        const { assert!(RESERVE_DESIGN_W >= PEEK_DESIGN.w) };
    }

    #[test]
    fn idle_width_follows_the_clock_text_within_limits() {
        let short = idle_pill("12:00", 36.0, 1.0, &tokens(), &FixedWidthMeasurer);
        assert_eq!(short.size.w, 72.0);
        let long = idle_pill(&"x".repeat(80), 36.0, 1.0, &tokens(), &FixedWidthMeasurer);
        assert_eq!(long.size.w, 280.0);
        assert_eq!(long.size.w % 2.0, 0.0);
    }

    #[test]
    fn expanded_controls_and_timeline_are_clickable() {
        let s = session("a", Playback::Playing);
        let v = expanded(&s, &[], 1_000, 1.0, 1.0, &tokens());
        assert_eq!(v.list.size, Size { w: 380.0, h: 184.0 });
        assert_eq!(hit(&v, &s, Point { x: 190.0, y: 148.0 }), Some(IslandAction::PlayPause));
        assert_eq!(hit(&v, &s, Point { x: 126.0, y: 148.0 }), Some(IslandAction::Previous));
        assert_eq!(hit(&v, &s, Point { x: 254.0, y: 150.0 }), Some(IslandAction::Next));
        // Metade da linha do tempo (72 … 308) = metade dos 200 s.
        assert_eq!(hit(&v, &s, Point { x: 190.0, y: 114.0 }), Some(IslandAction::Seek(100.0)));
        assert_eq!(hit(&v, &s, Point { x: 5.0, y: 5.0 }), None);
    }

    #[test]
    fn sources_bar_appears_with_two_or_more_sessions() {
        let s = session("a", Playback::Playing);
        let one = expanded(&s, &[("a".into(), "Spotify".into())], 0, 1.0, 1.0, &tokens());
        assert!(!one.hits.iter().any(|(_, a)| matches!(a, IslandAction::Source(_))));
        let two = expanded(&s, &[("a".into(), "Spotify".into()), ("b".into(), "Brave".into())], 0, 1.0, 1.0, &tokens());
        assert_eq!(hit(&two, &s, Point { x: 61.0, y: 148.0 }), Some(IslandAction::Source("b".into())));
    }

    #[test]
    fn disabled_buttons_are_not_clickable_and_time_formats() {
        let mut s = session("a", Playback::Paused);
        s.can_next = false;
        let v = expanded(&s, &[], 0, 1.0, 1.0, &tokens());
        assert_eq!(hit(&v, &s, Point { x: 254.0, y: 148.0 }), None);
        assert_eq!(time_text(65.4), "1:05");
        assert_eq!(time_text(3725.0), "1:02:05");
        assert_eq!(visualizer_bars(Rect::new(0.0, 0.0, 46.0, 10.0), 1.0), 10);
    }
}
