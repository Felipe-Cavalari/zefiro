//! Chip `ia` (ADR-0028): o uso do Claude, do Codex e do Grok num chip só, que alterna entre as
//! contas, um slide por conta e origem ("Claude · Windows", "Grok · WSL"). Aqui ficam os slides, a
//! ordem que o usuário escolhe (`[ia] order` e `hidden`), o desenho de cada slide e a lista de
//! edição do painel. A animação do deslize fica no compositor (`crates/app/src/ai_carousel.rs`).

use zefiro_core::config::AiConfig;
use zefiro_integration_chat::agent::Agent;
use zefiro_integration_usage::origins::OriginKind;
use zefiro_platform::color::Rgba;
use zefiro_platform::geometry::{Rect, Size};
use zefiro_platform::text::{Align, Family, FontWeight, TextMeasurer, TextStyle};
use zefiro_resources::icons;
use zefiro_resources::tokens::Tokens;
use zefiro_ui::paint::{DisplayList, DrawOp};

use crate::chips::ChipContent;
use crate::devices::with_svg_icon;
use crate::usage::usage_chip;

pub const AI_CHIP: &str = "ia";

/// Glifos de reserva (Segoe Fluent Icons): os de Claude, Codex e Grok viram o ícone SVG da marca.
const GLYPH_CLAUDE: &str = "\u{E945}";
const GLYPH_CODEX: &str = "\u{E943}";
/// Robô: o lugar do ícone do Grok e o chip sem nenhuma conta.
pub const GLYPH_AI: &str = "\u{E99A}";

/// Uma conta de uma IA numa origem (a mesma conta no Windows e no WSL são dois slides).
#[derive(Debug, Clone, PartialEq)]
pub struct Slide {
    /// `claude:windows`, `grok:Ubuntu-24.04`, `codex:windows+Ubuntu-24.04`: vai para o `[ia]`.
    pub key: String,
    pub agent: Agent,
    /// "Windows", "WSL" ou "WSL · Ubuntu-24.04" (o rótulo dos painéis de uso).
    pub origin: String,
    /// "Win" ou "WSL" ao lado da %.
    pub tag: &'static str,
    pub percent: Option<f32>,
}

impl Slide {
    /// "Claude · Windows".
    pub fn name(&self) -> String {
        if self.origin.is_empty() { self.agent.label().to_string() } else { format!("{} · {}", self.agent.label(), self.origin) }
    }
}

/// Chave do slide: o agente e as origens da conta (`windows` ou o nome da distro).
pub fn slide_key(agent: Agent, origins: &[OriginKind]) -> String {
    let parts: Vec<&str> = origins.iter().map(|o| match o { OriginKind::Windows => "windows", OriginKind::Wsl(d) => d.as_str() }).collect();
    format!("{}:{}", agent.as_str(), parts.join("+"))
}

/// Chave do slide de uma conta numa origem: `codex:Ubuntu-24.04:1a2b3c4d`. O fim é um código curto
/// da conta (FNV-1a de 32 bits da chave dela, nunca o id): duas contas da mesma IA na mesma origem
/// são dois slides, e a chave de uma não muda quando a outra aparece.
pub fn account_slide_key(agent: Agent, origin: &OriginKind, account: &str) -> String {
    let hash = account.bytes().fold(0x811c_9dc5_u32, |h, b| (h ^ u32::from(b)).wrapping_mul(0x0100_0193));
    format!("{}:{hash:08x}", slide_key(agent, std::slice::from_ref(origin)))
}

/// "Win" (só Windows), "WSL" (só distros) ou vazio (a mesma conta nos dois).
pub fn origin_tag(origins: &[OriginKind]) -> &'static str {
    let windows = origins.contains(&OriginKind::Windows);
    let wsl = origins.iter().any(|o| matches!(o, OriginKind::Wsl(_)));
    match (windows, wsl) {
        (true, false) => "Win",
        (false, true) => "WSL",
        _ => "",
    }
}

/// Slides visíveis (na ordem escolhida) e ocultos.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Arranged { pub visible: Vec<Slide>, pub hidden: Vec<Slide> }

/// Os que estão em `order` primeiro, nessa ordem; os outros depois, na ordem em que chegaram
/// (Claude, Codex, Grok; Windows antes das distros). Os de `hidden` vão para os ocultos.
pub fn arrange(slides: Vec<Slide>, ai: &AiConfig) -> Arranged {
    let (hidden, mut visible): (Vec<Slide>, Vec<Slide>) = slides.into_iter().partition(|s| ai.hidden.contains(&s.key));
    let rank = |s: &Slide| ai.order.iter().position(|k| *k == s.key).unwrap_or(usize::MAX);
    visible.sort_by_key(rank);
    Arranged { visible, hidden }
}

/// Olho: tira do slide ou põe de volta no fim dos visíveis (`visible`, na ordem da tela). A ordem
/// gravada passa a ser a dos visíveis: uma chave sozinha no `order` iria para a frente.
pub fn toggle(ai: &mut AiConfig, visible: &[String], key: &str) {
    if let Some(i) = ai.hidden.iter().position(|k| k == key) {
        ai.hidden.remove(i);
        ai.order = visible.iter().filter(|k| *k != key).cloned().collect();
        ai.order.push(key.to_string());
    } else {
        ai.hidden.push(key.to_string());
    }
}

/// Setas: sobe ou desce entre os visíveis (`visible`, na ordem da tela). A ordem gravada passa a
/// ser a dos visíveis.
pub fn nudge(ai: &mut AiConfig, visible: &[String], key: &str, up: bool) {
    let mut keys = visible.to_vec();
    let Some(i) = keys.iter().position(|k| k == key) else { return };
    let j = if up { i.checked_sub(1) } else { (i + 1 < keys.len()).then_some(i + 1) };
    if let Some(j) = j {
        keys.swap(i, j);
        ai.order = keys;
    }
}

/// Um slide no chip: o ícone da IA, a % (como os chips de uso) e a origem ao lado, mais fraca.
/// `tag_w`: a largura da maior origem entre os slides, para todos terem a mesma largura.
pub fn slide_chip(slide: &Slide, tag_w: f32, chip_h: f32, font_px: f32, scale: f32, tokens: &Tokens, m: &dyn TextMeasurer) -> ChipContent {
    let glyph = match slide.agent {
        Agent::Claude => GLYPH_CLAUDE,
        Agent::Codex => GLYPH_CODEX,
        Agent::Grok => GLYPH_AI,
    };
    let mut chip = usage_chip(glyph, slide.percent, chip_h, font_px, scale, tokens, m);
    chip = match slide.agent {
        Agent::Claude => with_svg_icon(chip, icons::CLAUDE_CODE),
        Agent::Codex => with_svg_icon(chip, icons::CODEX),
        Agent::Grok => with_svg_icon(chip, icons::GROK),
    };
    if tag_w > 0.0 {
        let gap = (4.0 * scale).round();
        if !slide.tag.is_empty() {
            let style = tag_style(font_px);
            chip.list.ops.push(DrawOp::Text { rect: Rect::new(chip.content_w + gap, 0.0, tag_w, chip_h), text: slide.tag.into(), style, color: tokens.text_secondary, align: Align::Leading });
        }
        chip.content_w += gap + tag_w;
        chip.list.size.w = chip.content_w;
    }
    chip
}

fn tag_style(font_px: f32) -> TextStyle {
    TextStyle { size: (font_px * 0.78).round(), weight: FontWeight::SemiBold, family: Family::Text }
}

/// Os desenhos dos slides, todos da mesma largura (a maior), e essa largura. Sem nenhum slide, um
/// só: o robô com "—".
pub fn slide_chips(slides: &[Slide], chip_h: f32, font_px: f32, scale: f32, tokens: &Tokens, m: &dyn TextMeasurer) -> (Vec<DisplayList>, f32) {
    if slides.is_empty() {
        let c = usage_chip(GLYPH_AI, None, chip_h, font_px, scale, tokens, m);
        return (vec![c.list], c.content_w);
    }
    let tag_w = slides.iter().filter(|s| !s.tag.is_empty()).map(|s| m.measure(s.tag, &tag_style(font_px)).w.ceil()).fold(0.0, f32::max);
    let chips: Vec<ChipContent> = slides.iter().map(|s| slide_chip(s, tag_w, chip_h, font_px, scale, tokens, m)).collect();
    let w = chips.iter().map(|c| c.content_w).fold(0.0, f32::max);
    let lists = chips
        .into_iter()
        .map(|mut c| {
            c.list.size = Size { w, h: chip_h };
            c.list
        })
        .collect();
    (lists, w)
}

/// O que um clique no painel do chip faz.
#[derive(Debug, Clone, PartialEq)]
pub enum AiHit { Edit, Done, Toggle(String), Up(String), Down(String) }

/// O lápis do painel normal, à esquerda do botão de atualizar.
pub fn pencil_rect(scale: f32) -> Rect {
    let r = crate::usage::refresh_button_rect(scale);
    Rect::new(r.x - (36.0 * scale).round(), r.y, r.w, r.h)
}

const GLYPH_PENCIL: &str = "\u{E70F}";
const GLYPH_EYE: &str = "\u{E7B3}";
const GLYPH_EYE_OFF: &str = "\u{ED1A}";
const GLYPH_UP: &str = "\u{E70E}";
const GLYPH_DOWN: &str = "\u{E70D}";
const WIDTH: f32 = 340.0;
const ROW_H: f32 = 36.0;
const ROWS_Y: f32 = 62.0;
const HIDDEN_ALPHA: f32 = 0.4;

/// O lápis no painel normal (o painel de uso já tem o resto). `hovered`: o mouse em cima.
pub fn pencil_ops(scale: f32, hovered: bool, tokens: &Tokens) -> Vec<DrawOp> {
    let r = pencil_rect(scale);
    let icons = TextStyle { size: 12.0 * scale, weight: FontWeight::Regular, family: Family::Icons };
    vec![
        DrawOp::RoundRect { rect: r, radius: r.h / 2.0, color: if hovered { tokens.raised_strong } else { tokens.raised } },
        DrawOp::Text { rect: r, text: GLYPH_PENCIL.into(), style: icons, color: if hovered { tokens.text } else { tokens.text_secondary }, align: Align::Center },
    ]
}

/// A lista de edição: os slides visíveis (na ordem do chip) e os ocultos, cada um com a %, as
/// setas (só nos visíveis) e o olho; "Concluir" no lugar do lápis.
pub fn slides_panel(arranged: &Arranged, scale: f32, tokens: &Tokens) -> (DisplayList, Vec<(Rect, AiHit)>) {
    let px = |v: f32| (v * scale).round();
    let text = |size: f32, weight: FontWeight| TextStyle { size: size * scale, weight, family: Family::Text };
    let icons = |size: f32| TextStyle { size: size * scale, weight: FontWeight::Regular, family: Family::Icons };
    let w = px(WIDTH);
    let mut ops = Vec::new();
    let mut hits = Vec::new();
    ops.push(DrawOp::Text { rect: Rect::new(px(20.0), px(14.0), w - px(130.0), px(22.0)), text: "Slides do chip".into(), style: text(13.0, FontWeight::SemiBold), color: tokens.text, align: Align::Leading });
    ops.push(DrawOp::Text { rect: Rect::new(px(20.0), px(34.0), w - px(40.0), px(16.0)), text: "O olho tira do slide · as setas ordenam".into(), style: text(11.0, FontWeight::Regular), color: tokens.text_tertiary, align: Align::Leading });
    let done = Rect::new(w - px(20.0 + 84.0), px(12.0), px(84.0), px(28.0));
    ops.push(DrawOp::RoundRect { rect: done, radius: done.h / 2.0, color: tokens.accent_soft });
    ops.push(DrawOp::Text { rect: done, text: "Concluir".into(), style: text(12.0, FontWeight::SemiBold), color: tokens.accent, align: Align::Center });
    hits.push((done, AiHit::Done));

    let mut y = px(ROWS_Y);
    if arranged.visible.is_empty() && arranged.hidden.is_empty() {
        ops.push(DrawOp::Text { rect: Rect::new(px(20.0), y + px(6.0), w - px(40.0), px(20.0)), text: "Nenhuma conta encontrada ainda.".into(), style: text(12.5, FontWeight::Regular), color: tokens.text, align: Align::Leading });
        return (DisplayList { size: Size { w, h: y + px(40.0) }, ops }, hits);
    }
    let row = |ops: &mut Vec<DrawOp>, hits: &mut Vec<(Rect, AiHit)>, y: f32, s: &Slide, hidden: bool, first: bool, last: bool| {
        let alpha = if hidden { HIDDEN_ALPHA } else { 1.0 };
        let fade = |c: Rgba| c.with_alpha(c.a * alpha);
        let r = Rect::new(px(20.0), y, w - px(40.0), px(ROW_H));
        let eye = Rect::new(r.right() - px(28.0), r.y + px(4.0), px(28.0), px(28.0));
        ops.push(DrawOp::Text { rect: eye, text: if hidden { GLYPH_EYE_OFF } else { GLYPH_EYE }.into(), style: icons(13.0), color: fade(tokens.text_secondary), align: Align::Center });
        hits.push((eye, AiHit::Toggle(s.key.clone())));
        let mut right = eye.x - px(4.0);
        if !hidden {
            for (glyph, enabled, hit, x) in [(GLYPH_DOWN, !last, AiHit::Down(s.key.clone()), right - px(26.0)), (GLYPH_UP, !first, AiHit::Up(s.key.clone()), right - px(52.0))] {
                let b = Rect::new(x, r.y + px(5.0), px(26.0), px(26.0));
                ops.push(DrawOp::Text { rect: b, text: glyph.into(), style: icons(11.0), color: if enabled { tokens.text_secondary } else { tokens.text_tertiary.with_alpha(tokens.text_tertiary.a * 0.5) }, align: Align::Center });
                if enabled {
                    hits.push((b, hit));
                }
            }
            right -= px(56.0);
        }
        let percent = s.percent.map_or_else(|| "—".to_string(), |p| format!("{p:.0}%"));
        let pr = Rect::new(right - px(44.0), r.y, px(40.0), r.h);
        ops.push(DrawOp::Text { rect: pr, text: percent, style: text(12.5, FontWeight::SemiBold), color: fade(tokens.text), align: Align::Trailing });
        ops.push(DrawOp::Text { rect: Rect::new(r.x, r.y, (pr.x - r.x - px(6.0)).max(0.0), r.h), text: s.name(), style: text(12.5, FontWeight::Regular), color: fade(tokens.text), align: Align::Leading });
    };
    let n = arranged.visible.len();
    for (i, s) in arranged.visible.iter().enumerate() {
        row(&mut ops, &mut hits, y, s, false, i == 0, i + 1 == n);
        y += px(ROW_H);
    }
    if !arranged.hidden.is_empty() {
        ops.push(DrawOp::Text { rect: Rect::new(px(20.0), y + px(8.0), w - px(40.0), px(16.0)), text: "Fora do slide".into(), style: text(10.5, FontWeight::SemiBold), color: tokens.text_tertiary, align: Align::Leading });
        y += px(28.0);
        for s in &arranged.hidden {
            row(&mut ops, &mut hits, y, s, true, false, false);
            y += px(ROW_H);
        }
    }
    (DisplayList { size: Size { w, h: y + px(10.0) }, ops }, hits)
}

/// O que está sob `p` no painel de edição.
pub fn hit_at(hits: &[(Rect, AiHit)], p: zefiro_platform::geometry::Point) -> Option<AiHit> {
    hits.iter().find(|(r, _)| r.contains(p)).map(|(_, h)| h.clone())
}

#[cfg(test)]
mod tests {
    use super::*;
    use zefiro_platform::geometry::Point;
    use zefiro_platform::text::FixedWidthMeasurer;
    use zefiro_resources::theme::{BUILTIN, parse_theme};
    use zefiro_resources::tokens::derive_tokens;

    fn tokens() -> Tokens {
        derive_tokens(&parse_theme("obsidian", "o", BUILTIN[0].1).unwrap(), Rgba::rgb(0, 120, 215), None)
    }

    fn wsl() -> OriginKind {
        OriginKind::Wsl("Ubuntu-24.04".into())
    }

    fn slide(agent: Agent, origins: &[OriginKind], percent: Option<f32>) -> Slide {
        Slide { key: slide_key(agent, origins), agent, origin: String::new(), tag: origin_tag(origins), percent }
    }

    fn slides() -> Vec<Slide> {
        vec![slide(Agent::Claude, &[OriginKind::Windows], Some(41.0)), slide(Agent::Claude, &[wsl()], Some(3.0)), slide(Agent::Codex, &[OriginKind::Windows, wsl()], Some(1.0)), slide(Agent::Grok, &[wsl()], Some(25.0))]
    }

    fn keys(s: &[Slide]) -> Vec<&str> {
        s.iter().map(|s| s.key.as_str()).collect()
    }

    #[test]
    fn keys_and_tags_say_where_the_account_is() {
        assert_eq!(slide_key(Agent::Claude, &[OriginKind::Windows]), "claude:windows");
        assert_eq!(slide_key(Agent::Codex, &[OriginKind::Windows, wsl()]), "codex:windows+Ubuntu-24.04");
        assert_eq!(slide_key(Agent::Grok, &[wsl()]), "grok:Ubuntu-24.04");
        // Uma conta por slide: duas contas do Codex no mesmo WSL não podem ter a mesma chave.
        let (a, b) = (account_slide_key(Agent::Codex, &wsl(), "conta-a"), account_slide_key(Agent::Codex, &wsl(), "conta-b"));
        assert!(a.starts_with("codex:Ubuntu-24.04:") && a.len() == "codex:Ubuntu-24.04:".len() + 8, "{a}");
        assert_ne!(a, b);
        assert_eq!(a, account_slide_key(Agent::Codex, &wsl(), "conta-a"), "a mesma em toda partida");
        assert!(!a.contains("conta-a"), "nunca o id da conta");
        assert_eq!((origin_tag(&[OriginKind::Windows]), origin_tag(&[wsl()]), origin_tag(&[OriginKind::Windows, wsl()])), ("Win", "WSL", ""));
        let mut s = slide(Agent::Grok, &[wsl()], None);
        s.origin = "WSL".into();
        assert_eq!(s.name(), "Grok · WSL");
    }

    #[test]
    fn order_and_hidden_come_from_the_config() {
        let all = arrange(slides(), &AiConfig::default());
        assert_eq!(keys(&all.visible), ["claude:windows", "claude:Ubuntu-24.04", "codex:windows+Ubuntu-24.04", "grok:Ubuntu-24.04"], "sem config, todas na ordem natural");
        let ai = AiConfig { order: vec!["grok:Ubuntu-24.04".into(), "sumiu:x".into()], hidden: vec!["claude:Ubuntu-24.04".into()], interval: 5 };
        let a = arrange(slides(), &ai);
        assert_eq!(keys(&a.visible), ["grok:Ubuntu-24.04", "claude:windows", "codex:windows+Ubuntu-24.04"]);
        assert_eq!(keys(&a.hidden), ["claude:Ubuntu-24.04"]);
    }

    #[test]
    fn eye_and_arrows_edit_the_config() {
        let mut ai = AiConfig::default();
        let shown = |ai: &AiConfig| -> Vec<String> { keys(&arrange(slides(), ai).visible).into_iter().map(String::from).collect() };
        toggle(&mut ai, &shown(&AiConfig::default()), "claude:windows");
        assert_eq!(ai.hidden, ["claude:windows"]);
        let now = shown(&ai);
        toggle(&mut ai, &now, "claude:windows");
        assert!(ai.hidden.is_empty());
        assert_eq!(shown(&ai).last().map(String::as_str), Some("claude:windows"), "volta no fim dos que estão no slide");
        ai = AiConfig::default();
        let visible = shown(&ai);
        nudge(&mut ai, &visible, "grok:Ubuntu-24.04", true);
        assert_eq!(ai.order.last().map(String::as_str), Some("codex:windows+Ubuntu-24.04"));
        assert_eq!(keys(&arrange(slides(), &ai).visible)[2], "grok:Ubuntu-24.04");
        let before = ai.clone();
        nudge(&mut ai, &visible, "claude:windows", true);
        assert_eq!(ai, before, "o primeiro não sobe");
    }

    #[test]
    fn every_slide_has_the_same_width_and_shows_its_origin() {
        let (lists, w) = slide_chips(&slides(), 28.0, 12.0, 1.0, &tokens(), &FixedWidthMeasurer);
        assert_eq!(lists.len(), 4);
        assert!(lists.iter().all(|l| l.size.w == w && l.size.h == 28.0));
        let texts = |l: &DisplayList| l.ops.iter().filter_map(|o| if let DrawOp::Text { text, .. } = o { Some(text.clone()) } else { None }).collect::<Vec<_>>();
        assert!(texts(&lists[0]).contains(&"Win".to_string()) && texts(&lists[0]).contains(&"41%".to_string()));
        assert!(matches!(lists[0].ops[0], DrawOp::Svg { .. }), "o ícone do Claude");
        assert!(!texts(&lists[2]).iter().any(|t| t == "Win" || t == "WSL"), "a mesma conta nos dois lados");
        assert!(matches!(lists[3].ops[0], DrawOp::Svg { svg, .. } if svg == icons::GROK), "o logo do Grok");
        let (empty, _) = slide_chips(&[], 28.0, 12.0, 1.0, &tokens(), &FixedWidthMeasurer);
        assert_eq!(empty.len(), 1, "sem conta, o robô com o traço");
    }

    #[test]
    fn edit_list_has_eyes_and_arrows() {
        let ai = AiConfig { hidden: vec!["claude:Ubuntu-24.04".into()], ..AiConfig::default() };
        let (list, hits) = slides_panel(&arrange(slides(), &ai), 1.0, &tokens());
        assert_eq!(list.size.w, 340.0);
        let toggles = hits.iter().filter(|(_, h)| matches!(h, AiHit::Toggle(_))).count();
        assert_eq!(toggles, 4, "um olho por slide, oculto ou não");
        assert!(!hits.iter().any(|(_, h)| *h == AiHit::Up("claude:windows".into())), "o primeiro não sobe");
        assert!(!hits.iter().any(|(_, h)| *h == AiHit::Down("grok:Ubuntu-24.04".into())), "o último não desce");
        assert!(hits.iter().any(|(_, h)| *h == AiHit::Down("claude:windows".into())));
        assert!(!hits.iter().any(|(_, h)| matches!(h, AiHit::Up(k) | AiHit::Down(k) if k == "claude:Ubuntu-24.04")), "oculto não tem setas");
        let done = hits.iter().find(|(_, h)| *h == AiHit::Done).map(|(r, _)| *r).unwrap();
        assert_eq!(hit_at(&hits, Point { x: done.x + 2.0, y: done.y + 2.0 }), Some(AiHit::Done));
        assert!(pencil_rect(1.0).right() <= crate::usage::refresh_button_rect(1.0).x, "o lápis antes do atualizar");
    }
}
