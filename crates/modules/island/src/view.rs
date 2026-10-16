//! O que a ilha desenha: a pílula recolhida (ocioso, mídia, espiando) e o painel expandido da mídia.
//!
//! Medidas em "px de conteúdo" do Dynamic Island for Windows (§3.2 e §6 das notas de porte). A
//! pílula recolhida escala pela altura da barra (como `[Zefiro] PillHeight` fazia no fork); o painel
//! expandido escala só pelo DPI.

use zefiro_platform::color::Rgba;
use zefiro_platform::geometry::{Point, Rect, Size};
use zefiro_platform::image::Image;
use zefiro_platform::text::{Align, Family, FontWeight, TextMeasurer, TextStyle};
use zefiro_resources::tokens::Tokens;
use zefiro_ui::paint::{DisplayList, DrawOp};

use crate::media::{MediaSession, Playback};
use crate::state::Activity;
use crate::timer::{FocusTimer, TimerKind};

/// Alturas de projeto de cada estado recolhido.
pub const MEDIA_DESIGN: Size = Size { w: 150.0, h: 44.0 };
pub const PEEK_DESIGN: Size = Size { w: 320.0, h: 44.0 };
pub const IDLE_DESIGN_H: f32 = 36.0;
pub const EXPANDED_DESIGN: Size = Size { w: 380.0, h: 184.0 };
/// Reserva estável no centro da barra: a maior pílula recolhida (espiando), para os chips das
/// laterais não pularem quando a ilha muda de estado.
pub const RESERVE_DESIGN_W: f32 = PEEK_DESIGN.w + 60.0;

const ICON_PREVIOUS: &str = "\u{E892}";
const ICON_NEXT: &str = "\u{E893}";
const ICON_PLAY: &str = "\u{E768}";
const ICON_PAUSE: &str = "\u{E769}";

#[derive(Debug, Clone, PartialEq)]
pub enum Collapsed { Idle, Media, Peek, Activity(Activity), Timer(FocusTimer) }

/// Aviso de volume/brilho: 300 × 54 (px de conteúdo), ícone, barra e porcentagem.
pub const OSD_DESIGN: Size = Size { w: 300.0, h: 54.0 };

const ICON_MUTE: &str = "\u{E74F}";
const ICON_VOLUME: [&str; 4] = ["\u{E992}", "\u{E993}", "\u{E994}", "\u{E995}"];
const ICON_BRIGHTNESS: &str = "\u{E706}";
const ICON_COPY: &str = "\u{E8C8}";
const ICON_KEYBOARD: &str = "\u{E765}";
const ICON_QUIET: &str = "\u{E708}";
const ICON_BELL: &str = "\u{EA8F}";

const ICON_USB: &str = "\u{E88E}";
const ICON_BLUETOOTH: &str = "\u{E702}";
const ICON_HEADPHONE: &str = "\u{E7F6}";
const ICON_MOUSE: &str = "\u{E962}";

/// Ícone pelo nome do dispositivo (fones, mouse, teclado ou Bluetooth genérico).
fn bluetooth_icon(name: &str) -> &'static str {
    let n = name.to_lowercase();
    if ["buds", "headphone", "headset", "fone", "airpods", "earbuds", "wh-", "wf-", "jbl", "audio"].iter().any(|k| n.contains(k)) {
        ICON_HEADPHONE
    } else if n.contains("mouse") || n.contains("mx ") {
        ICON_MOUSE
    } else if n.contains("keyboard") || n.contains("teclado") || n.contains("keys") {
        ICON_KEYBOARD
    } else {
        ICON_BLUETOOTH
    }
}

const ICON_TIMER: &str = "\u{E916}";
const ICON_BREAK: &str = "\u{EC32}";

/// Timer de foco: 260 × 54, ícone, tempo restante e barra de progresso.
pub fn timer_pill(t: &FocusTimer, now: f64, pill_h: f32, font_scale: f32, tokens: &Tokens) -> PillContent {
    let k = pill_h / 54.0;
    let w = round2(260.0 * k);
    let mut list = DisplayList::new(Size { w, h: pill_h });
    let paused = !t.running();
    let color = if paused { tokens.text_tertiary } else { tokens.accent };
    let glyph = if t.kind == TimerKind::Focus { ICON_TIMER } else { ICON_BREAK };
    list.push(DrawOp::Text { rect: Rect::new(16.0 * k, 0.0, 26.0 * k, pill_h), text: glyph.into(), style: icon(18.0 * k), color, align: Align::Center });
    let time_w = 64.0 * k;
    list.push(DrawOp::Text { rect: Rect::new(52.0 * k, 0.0, time_w, pill_h), text: crate::timer::clock(t.remaining(now)), style: TextStyle { size: 15.0 * k * font_scale, weight: FontWeight::SemiBold, family: Family::Display }, color: tokens.text, align: Align::Leading });
    let track = Rect::new(52.0 * k + time_w + 8.0 * k, (pill_h - 6.0 * k) / 2.0, w - (52.0 * k + time_w + 8.0 * k) - 18.0 * k, 6.0 * k);
    list.push(DrawOp::RoundRect { rect: track, radius: track.h / 2.0, color: tokens.raised_strong });
    let p = t.progress(now);
    if p > 0.0 {
        list.push(DrawOp::RoundRect { rect: Rect::new(track.x, track.y, (track.w * p).max(track.h), track.h), radius: track.h / 2.0, color });
    }
    PillContent { size: Size { w, h: pill_h }, list, visualizer: None, dots: Vec::new() }
}

/// Tamanho de projeto de cada aviso (tabela §3.2 das notas de porte).
fn activity_design(a: &Activity) -> Size {
    match a {
        Activity::Clipboard { .. } => Size { w: 340.0, h: 56.0 },
        Activity::Lock { .. } => Size { w: 180.0, h: 42.0 },
        Activity::Device { .. } => Size { w: 240.0, h: 50.0 },
        Activity::Bluetooth { .. } => Size { w: 280.0, h: 54.0 },
        Activity::DoNotDisturb { .. } => Size { w: 220.0, h: 42.0 },
        Activity::Volume { .. } | Activity::Brightness { .. } => OSD_DESIGN,
        Activity::Notification(_) => Size { w: 360.0, h: 58.0 },
    }
}

pub fn activity_pill(a: &Activity, pill_h: f32, font_scale: f32, tokens: &Tokens) -> PillContent {
    let design = activity_design(a);
    let k = pill_h / design.h;
    let w = round2(design.w * k);
    let mut list = DisplayList::new(Size { w, h: pill_h });
    let icon_rect = Rect::new(16.0 * k, 0.0, 26.0 * k, pill_h);
    let text_x = icon_rect.right() + 10.0 * k;
    let text_w = (w - text_x - 16.0 * k).max(0.0);
    let primary = style(14.0 * k * font_scale, FontWeight::SemiBold);
    let secondary = style(12.0 * k * font_scale, FontWeight::Regular);
    let glyph = |list: &mut DisplayList, g: &str, color: Rgba| list.push(DrawOp::Text { rect: icon_rect, text: g.into(), style: icon(18.0 * k), color, align: Align::Center });
    let line = |list: &mut DisplayList, text: String, style: TextStyle, color: Rgba, y: f32, h: f32| list.push(DrawOp::Text { rect: Rect::new(text_x, y, text_w, h), text, style, color, align: Align::Leading });
    match a {
        Activity::Volume { .. } | Activity::Brightness { .. } => {
            let (g, level, dim) = match *a {
                Activity::Volume { level, muted } => (if muted || level <= 0.0 { ICON_MUTE } else { ICON_VOLUME[((level * 3.0).ceil() as usize).clamp(1, 3)] }, level, muted),
                Activity::Brightness { level } => (ICON_BRIGHTNESS, level, false),
                _ => unreachable!("tratado no braço acima"),
            };
            glyph(&mut list, g, tokens.text);
            let level = level.clamp(0.0, 1.0);
            let pct_w = 44.0 * k;
            let track = Rect::new(text_x, (pill_h - 6.0 * k) / 2.0, w - text_x - pct_w - 18.0 * k, 6.0 * k);
            list.push(DrawOp::RoundRect { rect: track, radius: track.h / 2.0, color: tokens.raised_strong });
            if level > 0.0 {
                list.push(DrawOp::RoundRect { rect: Rect::new(track.x, track.y, (track.w * level).max(track.h), track.h), radius: track.h / 2.0, color: if dim { tokens.text_tertiary } else { tokens.accent } });
            }
            let pct = format!("{}", (level * 100.0).round() as u32);
            list.push(DrawOp::Text { rect: Rect::new(w - pct_w - 14.0 * k, 0.0, pct_w, pill_h), text: pct, style: primary, color: tokens.text, align: Align::Trailing });
        }
        Activity::Clipboard { text } => {
            glyph(&mut list, ICON_COPY, tokens.accent);
            line(&mut list, "Copiado".into(), primary, tokens.text, pill_h * 0.12, pill_h * 0.42);
            line(&mut list, text.replace(['\r', '\n', '\t'], " "), secondary, tokens.text_secondary, pill_h * 0.5, pill_h * 0.38);
        }
        Activity::Lock { caps, on } => {
            glyph(&mut list, ICON_KEYBOARD, if *on { tokens.accent } else { tokens.text_secondary });
            let name = if *caps { "Caps Lock" } else { "Num Lock" };
            line(&mut list, format!("{name} {}", if *on { "ligado" } else { "desligado" }), primary, tokens.text, 0.0, pill_h);
        }
        Activity::Device { name, connected } | Activity::Bluetooth { name, connected } => {
            let g = if matches!(a, Activity::Device { .. }) { ICON_USB } else { bluetooth_icon(name) };
            glyph(&mut list, g, if *connected { tokens.accent } else { tokens.text_secondary });
            line(&mut list, name.clone(), primary, tokens.text, pill_h * 0.12, pill_h * 0.42);
            line(&mut list, if *connected { "Conectado" } else { "Desconectado" }.into(), secondary, tokens.text_secondary, pill_h * 0.5, pill_h * 0.38);
        }
        Activity::DoNotDisturb { on } => {
            glyph(&mut list, ICON_QUIET, if *on { tokens.accent } else { tokens.text_secondary });
            line(&mut list, format!("Não perturbe {}", if *on { "ligado" } else { "desligado" }), primary, tokens.text, 0.0, pill_h);
        }
        Activity::Notification(n) => {
            let art = 30.0 * k;
            let art_rect = Rect::new(16.0 * k, (pill_h - art) / 2.0, art, art);
            match &n.icon {
                Some(image) => list.push(DrawOp::Image { rect: art_rect, image: image.clone(), radius: 7.0 * k, opacity: 1.0 }),
                None => glyph(&mut list, ICON_BELL, tokens.accent),
            }
            line(&mut list, n.app.clone(), secondary, tokens.text_secondary, pill_h * 0.1, pill_h * 0.38);
            line(&mut list, n.text.clone(), primary, tokens.text, pill_h * 0.45, pill_h * 0.45);
        }
    }
    PillContent { size: Size { w, h: pill_h }, list, visualizer: None, dots: Vec::new() }
}

/// Conteúdo de uma pílula recolhida: tamanho, desenho e onde ficam as barras do visualizador.
#[derive(Debug, Clone, PartialEq)]
pub struct PillContent {
    pub size: Size,
    pub list: DisplayList,
    /// Área das barras do visualizador (coordenadas da pílula), quando há mídia.
    pub visualizer: Option<Rect>,
    /// Pontos de privacidade (centro, raio, cor), desenhados e pulsados pelo compositor.
    pub dots: Vec<(Point, f32, Rgba)>,
}

/// Câmera e microfone em uso.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Privacy { pub mic: bool, pub cam: bool }

/// Cores da ilha: laranja `#FF9500` (microfone) e verde `#34C759` (câmera).
pub fn mic_color() -> Rgba {
    Rgba::rgb(0xFF, 0x95, 0x00)
}

pub fn cam_color() -> Rgba {
    Rgba::rgb(0x34, 0xC7, 0x59)
}

/// Alarga a pílula à direita e põe os pontos (câmera antes do microfone), como a ilha reserva
/// espaço para o ponto na ponta direita.
pub fn with_privacy(mut c: PillContent, p: Privacy) -> PillContent {
    let colors: Vec<Rgba> = [(p.cam, cam_color()), (p.mic, mic_color())].into_iter().filter(|(on, _)| *on).map(|(_, c)| c).collect();
    if colors.is_empty() {
        return c;
    }
    let k = c.size.h / 44.0;
    let step = (11.0 * k).max(7.0);
    let radius = (4.0 * k).max(2.5);
    let start = c.size.w - 4.0 * k;
    for (i, color) in colors.iter().enumerate() {
        c.dots.push((Point { x: (start + step * (i as f32 + 0.5)).round(), y: (c.size.h / 2.0).round() }, radius, *color));
    }
    c.size.w = round2(start + step * colors.len() as f32 + 8.0 * k);
    c.list.size.w = c.size.w;
    c
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
/// Com clima: relógio, divisor de 1 px (margem vertical de 9) e "ícone temperatura°", espaço de 9 px.
pub fn idle_pill(clock: &str, weather: Option<(&str, i32)>, pill_h: f32, font_scale: f32, tokens: &Tokens, m: &dyn TextMeasurer) -> PillContent {
    let k = pill_h / IDLE_DESIGN_H;
    let text = style(13.0 * k * font_scale, FontWeight::SemiBold);
    let tw = m.measure(clock, &text).w.ceil();
    let gap = 9.0 * k;
    let weather_text = weather.map(|(icon, t)| format!("{icon} {t}°"));
    let ww = weather_text.as_ref().map_or(0.0, |s| m.measure(s, &text).w.ceil());
    let content = if weather_text.is_some() { tw + 2.0 * gap + 1.0 + ww } else { tw };
    let w = round2((content + 28.0 * k).clamp(72.0 * k, 280.0 * k));
    let mut list = DisplayList::new(Size { w, h: pill_h });
    let x = ((w - content) / 2.0).round();
    match weather_text {
        Some(wt) => {
            list.push(DrawOp::Text { rect: Rect::new(x, 0.0, tw, pill_h), text: clock.into(), style: text, color: tokens.text, align: Align::Leading });
            let dx = x + tw + gap;
            list.push(DrawOp::RoundRect { rect: Rect::new(dx, 9.0 * k, 1.0, pill_h - 18.0 * k), radius: 0.0, color: tokens.hairline });
            list.push(DrawOp::Text { rect: Rect::new(dx + 1.0 + gap, 0.0, ww + 2.0, pill_h), text: wt, style: text, color: tokens.text_secondary, align: Align::Leading });
        }
        None => list.push(DrawOp::Text { rect: Rect::new(0.0, 0.0, w, pill_h), text: clock.into(), style: text, color: tokens.text, align: Align::Center }),
    }
    PillContent { size: Size { w, h: pill_h }, list, visualizer: None, dots: Vec::new() }
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
    // 64 % da altura: com fala (nível 0,1–0,3) as barras ainda se mexem.
    let vis = Rect::new(w - 14.0 * k - vis_w, pill_h * 0.18, vis_w, pill_h * 0.64);
    if peek {
        let x = art_rect.right() + 10.0 * k;
        let text_w = (vis.x - 10.0 * k - x).max(0.0);
        list.push(DrawOp::Text { rect: Rect::new(x, 0.0, text_w, pill_h), text: s.headline(), style: style(13.0 * k * font_scale, FontWeight::SemiBold), color: tokens.text, align: Align::Leading });
    }
    PillContent { size: Size { w, h: pill_h }, list, visualizer: Some(vis), dots: Vec::new() }
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

/// Uma fonte na barra de fontes.
#[derive(Debug, Clone, PartialEq)]
pub struct SourceSlot { pub id: String, pub name: String, pub icon: Option<Image>, pub playing: bool }

/// Estado das animações do painel num instante (o app avança o tempo e redesenha).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ExpandedAnim {
    /// Posição do indicador da barra de fontes, em posições (fracionária durante o deslize de 0,30 s).
    pub dock_pos: Option<f32>,
    /// Troca de fonte: opacidade e deslocamento horizontal (px de conteúdo) do bloco capa + textos + tempo.
    pub swap_alpha: f32,
    pub swap_dx: f32,
}

impl Default for ExpandedAnim {
    fn default() -> Self {
        ExpandedAnim { dock_pos: None, swap_alpha: 1.0, swap_dx: 0.0 }
    }
}

/// Medidas da barra de fontes (§6.2): x = 20, altura 30, posições de 26 com 2 de espaço e 2 de borda.
pub const DOCK_LEFT: f32 = 20.0;
const DOCK_HEIGHT: f32 = 30.0;
const DOCK_SLOT: f32 = 26.0;
const DOCK_GAP: f32 = 2.0;
const DOCK_PAD: f32 = 2.0;
const DOCK_ICON: f32 = 16.0;

/// Posições da barra: até 3 fontes; com mais, a primeira, a atual e "+N" (`None`).
pub fn dock_slots(count: usize, active: usize) -> Vec<Option<usize>> {
    if count < 2 {
        Vec::new()
    } else if count <= 3 {
        (0..count).map(Some).collect()
    } else {
        vec![Some(0), Some(if active >= 2 { active } else { 1 }), None]
    }
}

fn dock_slot_center(i: f32, scale: f32) -> f32 {
    (DOCK_LEFT + DOCK_PAD + i * (DOCK_SLOT + DOCK_GAP) + DOCK_SLOT / 2.0) * scale
}

/// Ops com opacidade multiplicada e deslocadas na horizontal (grupo da troca de fonte).
fn faded(ops: Vec<DrawOp>, alpha: f32, dx: f32) -> Vec<DrawOp> {
    transformed(ops, alpha, dx, 1.0, Point::default())
}

/// Ops com opacidade multiplicada, escaladas em torno de `center` e deslocadas `dx` na horizontal
/// (troca de fonte e carrossel das abas). O texto também escala, então nada "pula" no fim.
pub fn transformed(ops: Vec<DrawOp>, alpha: f32, dx: f32, scale: f32, center: Point) -> Vec<DrawOp> {
    if alpha >= 0.999 && dx.abs() < 0.01 && (scale - 1.0).abs() < 1e-4 {
        return ops;
    }
    let pt = |p: Point| Point { x: center.x + (p.x - center.x) * scale + dx, y: center.y + (p.y - center.y) * scale };
    let mv = |r: Rect| {
        let o = pt(Point { x: r.x, y: r.y });
        Rect::new(o.x, o.y, r.w * scale, r.h * scale)
    };
    let a = |c: Rgba| c.with_alpha(c.a * alpha);
    ops.into_iter()
        .map(|op| match op {
            DrawOp::RoundRect { rect, radius, color } => DrawOp::RoundRect { rect: mv(rect), radius: radius * scale, color: a(color) },
            DrawOp::RoundRectStroke { rect, radius, color, width } => DrawOp::RoundRectStroke { rect: mv(rect), radius: radius * scale, color: a(color), width: width * scale },
            DrawOp::Circle { center: c, radius, color } => DrawOp::Circle { center: pt(c), radius: radius * scale, color: a(color) },
            DrawOp::Text { rect, text, style, color, align } => DrawOp::Text { rect: mv(rect), text, style: TextStyle { size: style.size * scale, ..style }, color: a(color), align },
            DrawOp::Image { rect, image, radius, opacity } => DrawOp::Image { rect: mv(rect), image, radius: radius * scale, opacity: opacity * alpha },
            DrawOp::PushClip(rect) => DrawOp::PushClip(mv(rect)),
            DrawOp::PopClip => DrawOp::PopClip,
        })
        .collect()
}

/// `sources`: todas as sessões, na ordem estável do SMTC.
pub fn expanded(s: &MediaSession, sources: &[SourceSlot], now_ms: i64, scale: f32, font_scale: f32, tokens: &Tokens, anim: ExpandedAnim) -> ExpandedView {
    let px = |v: f32| v * scale;
    let fs = |v: f32| v * scale * font_scale;
    let size = Size { w: px(EXPANDED_DESIGN.w), h: px(EXPANDED_DESIGN.h) };
    let mut list = DisplayList::new(size);
    let mut hits = Vec::new();
    let playing = s.playback == Playback::Playing;

    // ── Grupo da troca de fonte: capa, textos e linha do tempo ──────────────
    let mut group = DisplayList::new(size);
    let art = Rect::new(px(24.0), px(20.0), px(64.0), px(64.0));
    match &s.cover {
        Some(image) => group.push(DrawOp::Image { rect: art, image: image.clone(), radius: px(16.0), opacity: 1.0 }),
        None => {
            group.push(DrawOp::RoundRect { rect: art, radius: px(16.0), color: tokens.accent.with_alpha(0.24) });
            match &s.app_icon {
                Some(icon) => group.push(DrawOp::Image { rect: art.inset(px(11.0), px(11.0)), image: icon.clone(), radius: px(6.0), opacity: 0.95 }),
                None => group.push(DrawOp::Text { rect: art, text: "\u{E8D6}".into(), style: icon(px(26.0)), color: tokens.text_secondary, align: Align::Center }),
            }
        }
    }
    // Selo do app no canto da capa (raio 9,5, como a ilha C++).
    if s.cover.is_some()
        && let Some(icon) = &s.app_icon
    {
        let c = Point { x: art.right() - px(13.5), y: art.bottom() - px(12.5) };
        group.push(DrawOp::Circle { center: c, radius: px(11.0), color: tokens.fill.with_alpha(1.0) });
        group.push(DrawOp::Image { rect: Rect::new(c.x - px(9.5), c.y - px(9.5), px(19.0), px(19.0)), image: icon.clone(), radius: px(9.5), opacity: 0.98 });
    }
    let tx = art.right() + px(14.0);
    let tw = size.w - tx - px(24.0);
    let title = if s.title.is_empty() { s.app_name.clone() } else { s.title.clone() };
    group.push(DrawOp::Text { rect: Rect::new(tx, px(22.0), tw, px(22.0)), text: title, style: style(fs(15.0), FontWeight::SemiBold), color: tokens.text, align: Align::Leading });
    let sub = [s.artist.as_str(), s.album.as_str()].into_iter().filter(|t| !t.is_empty()).collect::<Vec<_>>().join(" · ");
    group.push(DrawOp::Text { rect: Rect::new(tx, px(44.0), tw, px(18.0)), text: sub, style: style(fs(12.5), FontWeight::Regular), color: tokens.text_secondary, align: Align::Leading });
    let source_line = if playing { format!("Tocando no {}", s.app_name) } else { format!("Pausado no {}", s.app_name) };
    group.push(DrawOp::Text { rect: Rect::new(tx, px(62.0), tw, px(16.0)), text: source_line, style: style(fs(11.0), FontWeight::Regular), color: tokens.text_tertiary, align: Align::Leading });

    // Linha do tempo (estilo fino) em y = 114, margens 24 + 48; marcador branco com aro de destaque.
    if s.duration_s > 0.0 {
        let pos = s.position_at(now_ms);
        let track = Rect::new(px(72.0), px(114.0) - px(2.0), size.w - px(144.0), px(4.0));
        let frac = (pos / s.duration_s).clamp(0.0, 1.0) as f32;
        group.push(DrawOp::RoundRect { rect: track, radius: px(2.0), color: tokens.raised_strong });
        group.push(DrawOp::RoundRect { rect: Rect::new(track.x, track.y, track.w * frac, track.h), radius: px(2.0), color: tokens.accent });
        let thumb = Point { x: track.x + track.w * frac, y: track.y + track.h / 2.0 };
        let r = px(4.5);
        group.push(DrawOp::Circle { center: thumb, radius: r, color: Rgba::WHITE.with_alpha(0.98) });
        group.push(DrawOp::RoundRectStroke { rect: Rect::new(thumb.x - r - px(0.8), thumb.y - r - px(0.8), 2.0 * r + px(1.6), 2.0 * r + px(1.6)), radius: r + px(0.8), color: tokens.accent.with_alpha(0.85), width: px(1.6) });
        let label = style(fs(11.0), FontWeight::Regular);
        group.push(DrawOp::Text { rect: Rect::new(px(20.0), px(104.0), px(46.0), px(20.0)), text: time_text(pos), style: label, color: tokens.text_secondary, align: Align::Trailing });
        group.push(DrawOp::Text { rect: Rect::new(size.w - px(66.0), px(104.0), px(46.0), px(20.0)), text: time_text(s.duration_s), style: label, color: tokens.text_secondary, align: Align::Leading });
        if s.can_seek && anim.swap_alpha >= 0.999 {
            // Clique ±14 px na vertical; a fração é calculada por quem trata o clique.
            hits.push((Rect::new(track.x, px(100.0), track.w, px(28.0)), IslandAction::Seek(-1.0)));
        }
    }
    list.ops.extend(faded(group.ops, anim.swap_alpha, px(anim.swap_dx)));

    // ── Controles em y = 148, espaçamento 64; raios 16 e 22 (+6 de folga no clique) ──
    let cy = px(148.0);
    let cx = size.w / 2.0;
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

    // ── Barra de fontes (2 ou mais), parada durante a troca ──────────────────
    let active = sources.iter().position(|x| x.id == s.id).unwrap_or(0);
    let slots = dock_slots(sources.len(), active);
    if !slots.is_empty() {
        let n = slots.len() as f32;
        let card = Rect::new(px(DOCK_LEFT), cy - px(DOCK_HEIGHT / 2.0), px(DOCK_PAD * 2.0 + n * DOCK_SLOT + (n - 1.0) * DOCK_GAP), px(DOCK_HEIGHT));
        list.push(DrawOp::RoundRect { rect: card, radius: card.h / 2.0, color: tokens.raised });
        let target = slots.iter().position(|x| *x == Some(active)).unwrap_or(0) as f32;
        let pos = anim.dock_pos.unwrap_or(target).clamp(0.0, n - 1.0);
        // Indicador: círculo cheio com aro de destaque, deslizando entre as posições.
        let ic = dock_slot_center(pos, scale);
        let ir = px(DOCK_SLOT / 2.0);
        list.push(DrawOp::Circle { center: Point { x: ic, y: cy }, radius: ir, color: tokens.raised_strong });
        list.push(DrawOp::RoundRectStroke { rect: Rect::new(ic - ir, cy - ir, 2.0 * ir, 2.0 * ir), radius: ir, color: tokens.accent, width: px(1.5) });
        for (i, slot) in slots.iter().enumerate() {
            let c = Point { x: dock_slot_center(i as f32, scale), y: cy };
            let slot_rect = Rect::new(c.x - px(13.0), cy - px(13.0), px(26.0), px(26.0));
            let lit = (1.0 - (pos - i as f32).abs()).clamp(0.0, 1.0);
            let opacity = 0.5 + 0.5 * lit;
            match slot.and_then(|k| sources.get(k)) {
                Some(src) => {
                    match &src.icon {
                        Some(icon) => list.push(DrawOp::Image { rect: Rect::new(c.x - px(DOCK_ICON / 2.0), c.y - px(DOCK_ICON / 2.0), px(DOCK_ICON), px(DOCK_ICON)), image: icon.clone(), radius: px(DOCK_ICON / 2.0), opacity }),
                        None => {
                            let initial = src.name.chars().next().map(|ch| ch.to_uppercase().to_string()).unwrap_or_default();
                            list.push(DrawOp::Text { rect: slot_rect, text: initial, style: style(fs(11.0), FontWeight::Bold), color: tokens.text.with_alpha(tokens.text.a * opacity), align: Align::Center });
                        }
                    }
                    if src.playing {
                        // Ponto verde de 7 px no canto, com aro da cor da barra.
                        let d = Point { x: c.x + px(9.5), y: c.y + px(9.5) };
                        list.push(DrawOp::Circle { center: d, radius: px(5.0), color: tokens.fill.with_alpha(1.0) });
                        list.push(DrawOp::Circle { center: d, radius: px(3.5), color: Rgba { r: 0.19, g: 0.83, b: 0.38, a: 1.0 } });
                    }
                    hits.push((slot_rect.inset(-px(1.0), -px(2.0)), IslandAction::Source(src.id.clone())));
                }
                None => {
                    let more = sources.len().saturating_sub(2);
                    list.push(DrawOp::Text { rect: slot_rect, text: format!("+{more}"), style: style(fs(11.0), FontWeight::Bold), color: tokens.text_secondary, align: Align::Center });
                    // "+N" mostra a próxima fonte fora da barra.
                    if let Some(next) = (2..sources.len()).map(|k| (active + k - 1) % sources.len()).find(|k| *k != 0 && *k != active) {
                        hits.push((slot_rect.inset(-px(1.0), -px(2.0)), IslandAction::Source(sources[next].id.clone())));
                    }
                }
            }
        }
    }
    ExpandedView { list, hits }
}

/// Posição (índice) da fonte `id` na barra de fontes, para animar o indicador.
pub fn dock_index(sources: &[SourceSlot], active: &str, id: &str) -> Option<usize> {
    let a = sources.iter().position(|s| s.id == active).unwrap_or(0);
    let k = sources.iter().position(|s| s.id == id)?;
    let slots = dock_slots(sources.len(), a);
    slots.iter().position(|x| *x == Some(k)).or(Some(slots.len().saturating_sub(1)))
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
        let short = idle_pill("12:00", None, 36.0, 1.0, &tokens(), &FixedWidthMeasurer);
        assert_eq!(short.size.w, 72.0);
        let long = idle_pill(&"x".repeat(80), None, 36.0, 1.0, &tokens(), &FixedWidthMeasurer);
        assert_eq!(long.size.w, 280.0);
        assert_eq!(long.size.w % 2.0, 0.0);
        let with_weather = idle_pill("12:00", Some(("☀️", 21)), 36.0, 1.0, &tokens(), &FixedWidthMeasurer);
        assert!(with_weather.size.w > short.size.w);
        let texts: Vec<_> = with_weather.list.ops.iter().filter_map(|o| if let DrawOp::Text { text, .. } = o { Some(text.as_str()) } else { None }).collect();
        assert_eq!(texts, ["12:00", "☀️ 21°"]);
    }

    #[test]
    fn expanded_controls_and_timeline_are_clickable() {
        let s = session("a", Playback::Playing);
        let v = expanded(&s, &[], 1_000, 1.0, 1.0, &tokens(), ExpandedAnim::default());
        assert_eq!(v.list.size, Size { w: 380.0, h: 184.0 });
        assert_eq!(hit(&v, &s, Point { x: 190.0, y: 148.0 }), Some(IslandAction::PlayPause));
        assert_eq!(hit(&v, &s, Point { x: 126.0, y: 148.0 }), Some(IslandAction::Previous));
        assert_eq!(hit(&v, &s, Point { x: 254.0, y: 150.0 }), Some(IslandAction::Next));
        // Metade da linha do tempo (72 … 308) = metade dos 200 s.
        assert_eq!(hit(&v, &s, Point { x: 190.0, y: 114.0 }), Some(IslandAction::Seek(100.0)));
        assert_eq!(hit(&v, &s, Point { x: 5.0, y: 5.0 }), None);
    }

    fn slot(id: &str, name: &str) -> SourceSlot {
        SourceSlot { id: id.into(), name: name.into(), icon: None, playing: false }
    }

    #[test]
    fn dock_uses_app_icons_indicator_and_playing_dot() {
        let s = session("a", Playback::Playing);
        let icon = Image::new(1, 1, vec![0, 0, 255, 255]).unwrap();
        let sources = [SourceSlot { icon: Some(icon.clone()), playing: true, ..slot("a", "Spotify") }, slot("b", "Brave")];
        let v = expanded(&s, &sources, 0, 1.0, 1.0, &tokens(), ExpandedAnim::default());
        // Ícone do app no lugar da letra, opaco na atual; a outra fonte sem ícone mostra a inicial a 50 %.
        assert!(v.list.ops.iter().any(|o| matches!(o, DrawOp::Image { image, opacity, .. } if *image == icon && (*opacity - 1.0).abs() < 1e-6)));
        assert!(v.list.ops.iter().any(|o| matches!(o, DrawOp::Text { text, color, .. } if text == "B" && (color.a - tokens().text.a * 0.5).abs() < 1e-3)));
        // Ponto verde de quem está tocando e o aro de destaque do indicador na posição 0.
        assert!(v.list.ops.iter().any(|o| matches!(o, DrawOp::Circle { center, .. } if (center.x - 44.5).abs() < 0.01 && (center.y - 157.5).abs() < 0.01)));
        assert!(v.list.ops.iter().any(|o| matches!(o, DrawOp::RoundRectStroke { rect, color, .. } if (rect.x - 22.0).abs() < 0.01 && *color == tokens().accent)));
        // Indicador no meio do deslize.
        let mid = expanded(&s, &sources, 0, 1.0, 1.0, &tokens(), ExpandedAnim { dock_pos: Some(0.5), ..Default::default() });
        assert!(mid.list.ops.iter().any(|o| matches!(o, DrawOp::RoundRectStroke { rect, .. } if (rect.x - 36.0).abs() < 0.01)));
    }

    #[test]
    fn more_than_three_sources_show_first_active_and_more() {
        assert_eq!(dock_slots(5, 3), vec![Some(0), Some(3), None]);
        assert_eq!(dock_slots(5, 1), vec![Some(0), Some(1), None]);
        assert_eq!(dock_slots(3, 2), vec![Some(0), Some(1), Some(2)]);
        assert!(dock_slots(1, 0).is_empty());
        let sources: Vec<_> = ["a", "b", "c", "d"].iter().map(|x| slot(x, x)).collect();
        let s = session("a", Playback::Playing);
        let v = expanded(&s, &sources, 0, 1.0, 1.0, &tokens(), ExpandedAnim::default());
        assert!(v.list.ops.iter().any(|o| matches!(o, DrawOp::Text { text, .. } if text == "+2")));
        assert_eq!(dock_index(&sources, "a", "d"), Some(2), "fora da barra: vai para o +N");
        assert_eq!(dock_index(&sources, "a", "b"), Some(1));
    }

    #[test]
    fn swap_fades_and_shifts_cover_texts_and_timeline_but_not_controls() {
        let s = session("a", Playback::Paused);
        let v = expanded(&s, &[], 0, 1.0, 1.0, &tokens(), ExpandedAnim { swap_alpha: 0.5, swap_dx: -7.0, ..Default::default() });
        let title = v.list.ops.iter().find_map(|o| if let DrawOp::Text { text, rect, color, .. } = o { (text == "a música").then_some((*rect, *color)) } else { None }).unwrap();
        assert_eq!(title.0.x, 102.0 - 7.0);
        assert!((title.1.a - tokens().text.a * 0.5).abs() < 1e-3);
        assert!(v.list.ops.iter().any(|o| matches!(o, DrawOp::Text { text, .. } if text == "Pausado no a")));
        assert!(!v.hits.iter().any(|(_, a)| matches!(a, IslandAction::Seek(_))), "sem busca no meio da troca");
        assert!(v.hits.iter().any(|(_, a)| *a == IslandAction::PlayPause), "controles ficam");
    }

    #[test]
    fn sources_bar_appears_with_two_or_more_sessions() {
        let s = session("a", Playback::Playing);
        let one = expanded(&s, &[slot("a", "Spotify")], 0, 1.0, 1.0, &tokens(), ExpandedAnim::default());
        assert!(!one.hits.iter().any(|(_, a)| matches!(a, IslandAction::Source(_))));
        let two = expanded(&s, &[slot("a", "Spotify"), slot("b", "Brave")], 0, 1.0, 1.0, &tokens(), ExpandedAnim::default());
        assert_eq!(hit(&two, &s, Point { x: 61.0, y: 148.0 }), Some(IslandAction::Source("b".into())));
    }

    #[test]
    fn disabled_buttons_are_not_clickable_and_time_formats() {
        let mut s = session("a", Playback::Paused);
        s.can_next = false;
        let v = expanded(&s, &[], 0, 1.0, 1.0, &tokens(), ExpandedAnim::default());
        assert_eq!(hit(&v, &s, Point { x: 254.0, y: 148.0 }), None);
        assert_eq!(time_text(65.4), "1:05");
        assert_eq!(time_text(3725.0), "1:02:05");
        assert_eq!(visualizer_bars(Rect::new(0.0, 0.0, 46.0, 10.0), 1.0), 10);
    }
}

#[cfg(test)]
mod osd_tests {
    use super::*;
    use zefiro_platform::color::Rgba;
    use zefiro_resources::theme::{BUILTIN, parse_theme};
    use zefiro_resources::tokens::derive_tokens;

    fn tokens() -> Tokens {
        derive_tokens(&parse_theme("obsidian", "o", BUILTIN[0].1).unwrap(), Rgba::rgb(0, 120, 215), None)
    }

    fn texts(p: &PillContent) -> Vec<String> {
        p.list.ops.iter().filter_map(|o| if let DrawOp::Text { text, .. } = o { Some(text.clone()) } else { None }).collect()
    }

    #[test]
    fn volume_pill_shows_icon_level_and_percent() {
        let p = activity_pill(&Activity::Volume { level: 0.42, muted: false }, 54.0, 1.0, &tokens());
        assert_eq!(p.size, Size { w: 300.0, h: 54.0 });
        assert_eq!(texts(&p), vec!["\u{E994}".to_string(), "42".into()]);
        let muted = activity_pill(&Activity::Volume { level: 0.42, muted: true }, 27.0, 1.0, &tokens());
        assert_eq!(muted.size.w, 150.0);
        assert_eq!(texts(&muted)[0], ICON_MUTE);
        let b = activity_pill(&Activity::Brightness { level: 1.2 }, 54.0, 1.0, &tokens());
        assert_eq!(texts(&b), vec![ICON_BRIGHTNESS.to_string(), "100".into()]);
    }
}

#[cfg(test)]
mod privacy_tests {
    use super::*;

    fn pill(w: f32) -> PillContent {
        PillContent { size: Size { w, h: 44.0 }, list: DisplayList::new(Size { w, h: 44.0 }), visualizer: None, dots: Vec::new() }
    }

    #[test]
    fn no_dot_keeps_the_pill_and_each_dot_widens_it() {
        assert_eq!(with_privacy(pill(150.0), Privacy::default()), pill(150.0));
        let mic = with_privacy(pill(150.0), Privacy { mic: true, cam: false });
        assert_eq!(mic.dots.len(), 1);
        assert_eq!(mic.dots[0].2, mic_color());
        assert!(mic.size.w > 150.0 && mic.dots[0].0.x < mic.size.w && mic.dots[0].0.x > 146.0);
        let both = with_privacy(pill(150.0), Privacy { mic: true, cam: true });
        assert_eq!(both.dots.iter().map(|d| d.2).collect::<Vec<_>>(), vec![cam_color(), mic_color()]);
        assert!(both.size.w > mic.size.w && both.size.w % 2.0 == 0.0);
        assert_eq!(both.list.size.w, both.size.w);
    }
}

#[cfg(test)]
mod activity_tests {
    use super::*;
    use crate::state::Notice;
    use zefiro_platform::color::Rgba;
    use zefiro_resources::theme::{BUILTIN, parse_theme};
    use zefiro_resources::tokens::derive_tokens;

    fn tokens() -> Tokens {
        derive_tokens(&parse_theme("obsidian", "o", BUILTIN[0].1).unwrap(), Rgba::rgb(0, 120, 215), None)
    }

    fn texts(p: &PillContent) -> Vec<String> {
        p.list.ops.iter().filter_map(|o| if let DrawOp::Text { text, .. } = o { Some(text.clone()) } else { None }).collect()
    }

    #[test]
    fn each_activity_has_the_island_size_and_text() {
        let t = tokens();
        let copy = activity_pill(&Activity::Clipboard { text: "linha 1\nlinha 2".into() }, 56.0, 1.0, &t);
        assert_eq!(copy.size, Size { w: 340.0, h: 56.0 });
        assert_eq!(texts(&copy)[1..], ["Copiado".to_string(), "linha 1 linha 2".into()]);
        let caps = activity_pill(&Activity::Lock { caps: true, on: true }, 42.0, 1.0, &t);
        assert_eq!(caps.size.w, 180.0);
        assert!(texts(&caps).contains(&"Caps Lock ligado".to_string()));
        let dnd = activity_pill(&Activity::DoNotDisturb { on: false }, 21.0, 1.0, &t);
        assert_eq!(dnd.size.w, 110.0);
        assert!(texts(&dnd).contains(&"Não perturbe desligado".to_string()));
        let note = activity_pill(&Activity::Notification(Notice { app: "WhatsApp".into(), text: "Vida - oi".into(), icon: None }), 58.0, 1.0, &t);
        assert_eq!(note.size.w, 360.0);
        assert_eq!(texts(&note)[1..], ["WhatsApp".to_string(), "Vida - oi".into()]);
    }
}
