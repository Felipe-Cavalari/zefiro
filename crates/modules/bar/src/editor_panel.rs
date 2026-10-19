//! Painel de edição: réplica da barra com Início, Centro e Fim lado a lado (como o "Personalizar
//! barra de ferramentas" do Mac), prateleira com os itens fora da barra e botões com hover e aperto.
//! Arrastar um chip entre as seções (ou para a prateleira) só muda o rascunho ao soltar.

use zefiro_core::config::LayoutMode;
use zefiro_platform::color::Rgba;
use zefiro_platform::geometry::{Point, Rect, Size};
use zefiro_platform::text::{Align, Family, FontWeight, TextMeasurer, TextStyle};
use zefiro_resources::tokens::Tokens;
use zefiro_ui::paint::{DisplayList, DrawOp};
use crate::editor::{EditorSession, Section};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EditorAction {
    Monitor(usize), ToggleMode, SelectItem(String), Add(String), Remove,
    MoveSection(Section), MoveBefore, MoveAfter, Options, Back,
    ToggleSeconds, EditFormat, SystemFormat, Reload, Reapply, ResolveFile, ResolveDraft,
    Cancel, Save,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum EditorPage { #[default] Items, Options }

/// Onde um chip está ou pode ser solto.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Zone { Section(Section), Shelf }

/// O que está sob o mouse ou com o foco do teclado.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Target { Button(EditorAction), Chip(String) }

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ButtonKind { Plain, Primary, Segment { selected: bool }, Row }

#[derive(Debug, Clone)]
pub struct Button { pub action: EditorAction, pub rect: Rect, pub label: String, pub enabled: bool, pub kind: ButtonKind }

#[derive(Debug, Clone)]
pub struct ChipBox { pub id: String, pub label: String, pub rect: Rect, pub zone: Zone, pub locked: bool }

/// Tudo o que o painel desenha e onde, num tamanho de painel (px).
#[derive(Debug, Clone)]
pub struct EditorLayout {
    pub size: Size,
    pub buttons: Vec<Button>,
    pub zones: Vec<(Zone, Rect)>,
    pub chips: Vec<ChipBox>,
    labels: Vec<(Rect, String)>,
    empty_shelf: Option<Rect>,
    center_locked: bool,
    scale: f32,
}

/// Arraste de um chip dentro do painel.
#[derive(Debug, Clone)]
pub struct PanelDrag { pub id: String, pub origin: Point, pub point: Point, pub active: bool }

/// O que o painel precisa saber da sessão e do monitor para se montar.
pub struct EditorContext<'a> {
    pub session: &'a EditorSession,
    pub monitors: &'a [String],
    pub monitor: usize,
    /// Ilha ligada neste monitor: o Centro mostra a ilha travada e não aceita soltura.
    pub island_locked: bool,
    pub scale: f32,
    pub width: f32,
}

/// Chip da ilha no Centro: aparece, mas não se move.
pub const ISLAND_CHIP: &str = "__island";
/// Largura do painel em DIP (limitada à largura do monitor).
pub const WIDTH_DIP: f32 = 560.0;

const PAD: f32 = 16.0;
const CHIP_H: f32 = 28.0;
const GAP: f32 = 6.0;
const BUTTON_H: f32 = 32.0;
const LABEL_H: f32 = 16.0;

#[derive(Default)]
pub struct EditorPanel {
    pub page: EditorPage,
    pub selected: Option<String>,
    pub message: String,
    pub editing_format: bool,
    pub format_select_all: bool,
    /// Movimento por teclado do item selecionado (setas).
    pub moving: bool,
    pub focus: Option<Target>,
    pub hover: Option<Target>,
    pub pressed: Option<Target>,
    pub drag: Option<PanelDrag>,
}

fn has_options(id: &str) -> bool {
    id == "clock"
}

impl EditorPanel {
    pub fn layout(&self, cx: &EditorContext, m: &dyn TextMeasurer) -> EditorLayout {
        let s = |v: f32| v * cx.scale;
        let session = cx.session;
        let inner = cx.width - s(2.0 * PAD);
        let enabled = !session.saving;
        let mut out = EditorLayout { size: Size { w: cx.width, h: 0.0 }, buttons: Vec::new(), zones: Vec::new(), chips: Vec::new(), labels: Vec::new(), empty_shelf: None, center_locked: cx.island_locked, scale: cx.scale };
        let mut y = s(54.0);

        if cx.monitors.len() > 1 {
            let segments_w = inner * 0.62;
            let each = segments_w / cx.monitors.len() as f32;
            for (i, name) in cx.monitors.iter().enumerate() {
                out.buttons.push(Button { action: EditorAction::Monitor(i), rect: Rect::new(s(PAD) + each * i as f32, y, each, s(CHIP_H)), label: name.clone(), enabled, kind: ButtonKind::Segment { selected: i == cx.monitor } });
            }
            let shared = session.draft.bar.layout_mode == LayoutMode::Shared;
            let x = s(PAD) + segments_w + s(8.0);
            out.buttons.push(Button { action: EditorAction::ToggleMode, rect: Rect::new(x, y, cx.width - s(PAD) - x, s(CHIP_H)), label: if shared { "Mesmo layout em todos" } else { "Layout próprio" }.into(), enabled, kind: ButtonKind::Plain });
            y += s(CHIP_H + 14.0);
        }

        match self.page {
            EditorPage::Items => {
                let gap = s(8.0);
                let unit = (inner - 2.0 * gap) / 3.6;
                let widths = [unit, unit, unit * 1.6];
                let mut x = s(PAD);
                let zone_y = y + s(LABEL_H + 4.0);
                let mut heights = Vec::new();
                for (i, section) in Section::ALL.into_iter().enumerate() {
                    out.labels.push((Rect::new(x + s(4.0), y, widths[i], s(LABEL_H)), section.label().into()));
                    let mut ids: Vec<(String, bool)> = Vec::new();
                    if section == Section::Center && cx.island_locked {
                        ids.push((ISLAND_CHIP.into(), true));
                    }
                    ids.extend(section.items(session.sections()).iter().map(|id| (id.clone(), false)));
                    let zone = Rect::new(x, zone_y, widths[i], 0.0);
                    heights.push(flow(&mut out.chips, &ids, zone, Zone::Section(section), cx.scale, m));
                    out.zones.push((Zone::Section(section), zone));
                    x += widths[i] + gap;
                }
                let zone_h = heights.into_iter().fold(s(40.0), f32::max);
                for (_, r) in &mut out.zones {
                    r.h = zone_h;
                }
                y = zone_y + zone_h + s(14.0);

                out.labels.push((Rect::new(s(PAD + 4.0), y, inner, s(LABEL_H)), "Fora da barra · arraste para cá para remover".into()));
                let shelf_y = y + s(LABEL_H + 4.0);
                let present: Vec<&str> = session.sections().ids().collect();
                let ids: Vec<(String, bool)> = crate::catalog::ITEMS.iter().filter(|item| !present.contains(&item.id)).map(|item| (item.id.to_string(), false)).collect();
                let shelf = Rect::new(s(PAD), shelf_y, inner, 0.0);
                let shelf_h = flow(&mut out.chips, &ids, shelf, Zone::Shelf, cx.scale, m).max(s(40.0));
                if ids.is_empty() {
                    out.empty_shelf = Some(Rect::new(s(PAD), shelf_y, inner, shelf_h));
                }
                out.zones.push((Zone::Shelf, Rect::new(s(PAD), shelf_y, inner, shelf_h)));
                y = shelf_y + shelf_h;

                if let Some(id) = self.selected.as_deref().filter(|id| has_options(id)) {
                    y += s(10.0);
                    out.buttons.push(Button { action: EditorAction::Options, rect: Rect::new(s(PAD), y, inner, s(CHIP_H)), label: format!("Opções do {} →", crate::catalog::name(id)), enabled, kind: ButtonKind::Row });
                    y += s(CHIP_H);
                }
            }
            EditorPage::Options => {
                let clock = &session.draft.clock;
                let rows = [
                    (EditorAction::Back, "← Voltar às seções".to_string()),
                    (EditorAction::ToggleSeconds, format!("Segundos: {}", if clock.seconds { "ligados" } else { "desligados" })),
                    (EditorAction::EditFormat, format!("Formato: {}", if clock.format.is_empty() { "padrão do Windows" } else { &clock.format })),
                    (EditorAction::SystemFormat, "Usar formato do Windows".into()),
                ];
                for (action, label) in rows {
                    out.buttons.push(Button { action, rect: Rect::new(s(PAD), y, inner, s(BUTTON_H)), label, enabled, kind: ButtonKind::Row });
                    y += s(BUTTON_H + 4.0);
                }
            }
        }

        if session.conflict {
            let mut actions = vec![(EditorAction::Reload, "Recarregar arquivo"), (EditorAction::Reapply, "Reaplicar minhas alterações")];
            if self.message.starts_with("Conflito:") {
                actions.push((EditorAction::ResolveFile, "Manter a versão do arquivo"));
                actions.push((EditorAction::ResolveDraft, "Manter a minha versão"));
            }
            y += s(12.0);
            let half = (inner - s(8.0)) / 2.0;
            for (i, (action, label)) in actions.into_iter().enumerate() {
                let rect = Rect::new(s(PAD) + (half + s(8.0)) * (i % 2) as f32, y + s(BUTTON_H + 6.0) * (i / 2) as f32, half, s(BUTTON_H));
                out.buttons.push(Button { action, rect, label: label.into(), enabled: session.file_error.is_none() || i >= 2, kind: ButtonKind::Plain });
            }
            y = out.buttons.last().map_or(y, |b| b.rect.bottom());
        }

        y += s(18.0);
        let half = (inner - s(8.0)) / 2.0;
        out.buttons.push(Button { action: EditorAction::Cancel, rect: Rect::new(s(PAD), y, half, s(BUTTON_H)), label: "Cancelar".into(), enabled, kind: ButtonKind::Plain });
        out.buttons.push(Button { action: EditorAction::Save, rect: Rect::new(s(PAD) + half + s(8.0), y, half, s(BUTTON_H)), label: if session.saving { "Gravando…" } else { "Concluir" }.into(), enabled: enabled && !session.conflict, kind: ButtonKind::Primary });
        out.size.h = y + s(BUTTON_H + PAD);
        out
    }

    /// Botão ou chip sob o ponto. A ilha travada não é alvo.
    pub fn hit(layout: &EditorLayout, p: Point) -> Option<Target> {
        if let Some(b) = layout.buttons.iter().find(|b| b.rect.contains(p)) {
            return Some(Target::Button(b.action.clone()));
        }
        layout.chips.iter().find(|c| !c.locked && c.rect.contains(p)).map(|c| Target::Chip(c.id.clone()))
    }

    /// Seção (ou prateleira) que aceita soltar no ponto. Com a ilha, o Centro não aceita.
    pub fn zone_at(layout: &EditorLayout, p: Point) -> Option<Zone> {
        layout.zones.iter().find(|(z, r)| r.contains(p) && !(layout.center_locked && *z == Zone::Section(Section::Center))).map(|(z, _)| *z)
    }

    /// Posição de inserção na zona, contando só os outros chips (o arrastado sai de onde estava).
    pub fn drop_index(layout: &EditorLayout, zone: Zone, p: Point, dragged: &str) -> usize {
        let half = CHIP_H * layout.scale / 2.0;
        layout
            .chips
            .iter()
            .filter(|c| c.zone == zone && !c.locked && c.id != dragged)
            .filter(|c| {
                let cy = c.rect.y + c.rect.h / 2.0;
                cy < p.y - half || ((cy - p.y).abs() <= half && c.rect.center_x() < p.x)
            })
            .count()
    }

    /// Alvo do arraste em andamento: zona e posição.
    pub fn drag_target(&self, layout: &EditorLayout) -> Option<(Zone, usize)> {
        let d = self.drag.as_ref().filter(|d| d.active)?;
        let zone = Self::zone_at(layout, d.point)?;
        Some((zone, Self::drop_index(layout, zone, d.point, &d.id)))
    }

    /// Ordem do Tab: chips (menos a ilha) e depois os botões habilitados.
    pub fn focusables(layout: &EditorLayout) -> Vec<Target> {
        let chips = layout.chips.iter().filter(|c| !c.locked).map(|c| Target::Chip(c.id.clone()));
        let buttons = layout.buttons.iter().filter(|b| b.enabled).map(|b| Target::Button(b.action.clone()));
        chips.chain(buttons).collect()
    }

    pub fn navigate(&mut self, layout: &EditorLayout, delta: i32) {
        let all = Self::focusables(layout);
        if all.is_empty() {
            return;
        }
        let current = self.focus.as_ref().and_then(|f| all.iter().position(|t| t == f));
        let next = match current {
            Some(i) => (i as i32 + delta).rem_euclid(all.len() as i32) as usize,
            None if delta < 0 => all.len() - 1,
            None => 0,
        };
        self.focus = Some(all[next].clone());
    }

    pub fn paint(&self, layout: &EditorLayout, session: &EditorSession, tokens: &Tokens) -> DisplayList {
        let s = |v: f32| v * layout.scale;
        let mut list = DisplayList::new(layout.size);
        let inner = layout.size.w - s(2.0 * PAD);
        let text = |size: f32, weight: FontWeight| TextStyle { size: s(size), weight, family: Family::Text };
        list.push(DrawOp::Text { rect: Rect::new(s(PAD), s(12.0), inner, s(20.0)), text: "Editar barra".into(), style: text(14.0, FontWeight::SemiBold), color: tokens.text, align: Align::Leading });
        let message = if session.conflict && self.message.is_empty() { "A configuração mudou fora do editor" } else if self.message.is_empty() { "Arraste os itens entre as seções" } else { &self.message };
        list.push(DrawOp::Text { rect: Rect::new(s(PAD), s(32.0), inner, s(16.0)), text: message.into(), style: text(11.0, FontWeight::Regular), color: tokens.text_secondary, align: Align::Leading });
        for (rect, label) in &layout.labels {
            list.push(DrawOp::Text { rect: *rect, text: label.clone(), style: text(10.5, FontWeight::SemiBold), color: tokens.text_tertiary, align: Align::Leading });
        }

        let target = self.drag_target(layout);
        for (zone, rect) in &layout.zones {
            let over = target.is_some_and(|(z, _)| z == *zone);
            list.push(DrawOp::RoundRect { rect: *rect, radius: s(14.0), color: if over { tokens.accent_soft } else { tokens.raised.with_alpha(tokens.raised.a * 0.55) } });
            if over {
                list.push(DrawOp::RoundRectStroke { rect: rect.inset(s(0.75), s(0.75)), radius: s(14.0), color: tokens.accent, width: s(1.5) });
            }
        }
        if let Some(rect) = layout.empty_shelf {
            list.push(DrawOp::Text { rect, text: "Todos os itens estão na barra".into(), style: text(11.0, FontWeight::Regular), color: tokens.text_tertiary, align: Align::Center });
        }

        let dragging = self.drag.as_ref().filter(|d| d.active).map(|d| d.id.as_str());
        let chip_text = text(12.0, FontWeight::Regular);
        for c in &layout.chips {
            if c.locked {
                list.push(DrawOp::RoundRect { rect: c.rect, radius: c.rect.h / 2.0, color: tokens.accent_soft });
                list.push(DrawOp::Text { rect: c.rect, text: c.label.clone(), style: chip_text, color: tokens.accent, align: Align::Center });
                continue;
            }
            let target = Target::Chip(c.id.clone());
            let lit = self.hover.as_ref() == Some(&target) || self.focus.as_ref() == Some(&target);
            let ghosted = dragging == Some(c.id.as_str());
            let fill = if lit { tokens.raised_strong } else { tokens.raised };
            let alpha = if ghosted { 0.35 } else { 1.0 };
            list.push(DrawOp::RoundRect { rect: c.rect, radius: c.rect.h / 2.0, color: fill.with_alpha(fill.a * alpha) });
            if self.selected.as_deref() == Some(c.id.as_str()) && !ghosted {
                list.push(DrawOp::RoundRectStroke { rect: c.rect.inset(s(0.75), s(0.75)), radius: c.rect.h / 2.0, color: tokens.accent, width: s(1.5) });
            }
            list.push(DrawOp::Text { rect: c.rect, text: c.label.clone(), style: chip_text, color: tokens.text.with_alpha(tokens.text.a * alpha), align: Align::Center });
        }

        // Marcador de inserção e o chip que segue o mouse.
        if let (Some((zone, index)), Some(d)) = (target, self.drag.as_ref())
            && zone != Zone::Shelf
        {
            let others: Vec<&ChipBox> = layout.chips.iter().filter(|c| c.zone == zone && !c.locked && c.id != d.id).collect();
            let zone_rect = layout.zones.iter().find(|(z, _)| *z == zone).map(|(_, r)| *r).unwrap_or_default();
            let (x, y) = match (others.get(index), others.last()) {
                (Some(c), _) => (c.rect.x - s(4.0), c.rect.y),
                (None, Some(last)) => (last.rect.right() + s(2.0), last.rect.y),
                (None, None) => (zone_rect.x + s(6.0), zone_rect.y + s(6.0)),
            };
            list.push(DrawOp::RoundRect { rect: Rect::new(x, y, s(2.5), s(CHIP_H)), radius: s(1.25), color: tokens.accent });
        }
        if let Some(d) = self.drag.as_ref().filter(|d| d.active)
            && let Some(c) = layout.chips.iter().find(|c| c.id == d.id)
        {
            let ghost = Rect::new(d.point.x - c.rect.w / 2.0, d.point.y - c.rect.h / 2.0, c.rect.w, c.rect.h);
            list.push(DrawOp::RoundRect { rect: ghost, radius: ghost.h / 2.0, color: tokens.raised_strong });
            list.push(DrawOp::RoundRectStroke { rect: ghost.inset(s(0.75), s(0.75)), radius: ghost.h / 2.0, color: tokens.accent, width: s(1.5) });
            list.push(DrawOp::Text { rect: ghost, text: c.label.clone(), style: chip_text, color: tokens.text, align: Align::Center });
        }

        // Segmentos (monitores) dividem um trilho só.
        let segments: Vec<&Button> = layout.buttons.iter().filter(|b| matches!(b.kind, ButtonKind::Segment { .. })).collect();
        if let (Some(first), Some(last)) = (segments.first(), segments.last()) {
            let track = first.rect.union(last.rect);
            list.push(DrawOp::RoundRect { rect: track, radius: track.h / 2.0, color: tokens.raised });
        }
        let body = text(12.0, FontWeight::Regular);
        for b in &layout.buttons {
            let target = Target::Button(b.action.clone());
            let lit = b.enabled && (self.hover.as_ref() == Some(&target) || self.focus.as_ref() == Some(&target));
            let pressed = b.enabled && self.pressed.as_ref() == Some(&target);
            // Aperto: o botão encolhe um pouco enquanto o botão do mouse está embaixo.
            let rect = if pressed { b.rect.inset(s(1.5), s(1.5)) } else { b.rect };
            let radius = rect.h / 2.0;
            let mut color = if b.enabled { tokens.text } else { tokens.text_tertiary };
            match b.kind {
                ButtonKind::Segment { selected } => {
                    let cell = rect.inset(s(3.0), s(3.0));
                    if selected {
                        list.push(DrawOp::RoundRect { rect: cell, radius: cell.h / 2.0, color: tokens.raised_strong });
                    } else if lit || pressed {
                        list.push(DrawOp::RoundRect { rect: cell, radius: cell.h / 2.0, color: tokens.raised_strong.with_alpha(tokens.raised_strong.a * 0.5) });
                    }
                }
                ButtonKind::Primary if b.enabled => {
                    list.push(DrawOp::RoundRect { rect, radius, color: tokens.accent });
                    if lit || pressed {
                        let veil = if pressed { Rgba::BLACK.with_alpha(0.14) } else { Rgba::WHITE.with_alpha(0.14) };
                        list.push(DrawOp::RoundRect { rect, radius, color: veil });
                    }
                    color = on_accent(tokens.accent);
                }
                _ => {
                    let fill = if lit || pressed { tokens.raised_strong } else { tokens.raised };
                    list.push(DrawOp::RoundRect { rect, radius, color: fill });
                }
            }
            let (label_rect, align) = if b.kind == ButtonKind::Row { (rect.inset(s(14.0), 0.0), Align::Leading) } else { (rect.inset(s(8.0), 0.0), Align::Center) };
            let weight = if b.kind == ButtonKind::Primary { FontWeight::SemiBold } else { FontWeight::Regular };
            list.push(DrawOp::Text { rect: label_rect, text: b.label.clone(), style: TextStyle { weight, ..body }, color, align });
        }
        list
    }
}

/// Texto legível sobre o destaque: escuro em cores claras, branco nas escuras.
fn on_accent(accent: Rgba) -> Rgba {
    let luminance = 0.2126 * accent.r + 0.7152 * accent.g + 0.0722 * accent.b;
    if luminance > 0.55 { Rgba::rgb(0x1C, 0x1C, 0x1E) } else { Rgba::WHITE }
}

/// Distribui chips da esquerda para a direita dentro da zona, quebrando linha. Devolve a altura.
fn flow(out: &mut Vec<ChipBox>, ids: &[(String, bool)], zone: Rect, kind: Zone, scale: f32, m: &dyn TextMeasurer) -> f32 {
    let s = |v: f32| v * scale;
    let style = TextStyle { size: s(12.0), weight: FontWeight::Regular, family: Family::Text };
    let (left, right) = (zone.x + s(GAP), zone.right() - s(GAP));
    let (mut x, mut y) = (left, zone.y + s(GAP));
    for (id, locked) in ids {
        let label = if *locked { "Ilha · fixa".to_string() } else { crate::catalog::name(id).to_string() };
        let w = (m.measure(&label, &style).w + s(24.0)).max(s(44.0)).min(right - left);
        if x + w > right && x > left {
            x = left;
            y += s(CHIP_H + GAP);
        }
        out.push(ChipBox { id: id.clone(), label, rect: Rect::new(x, y, w, s(CHIP_H)), zone: kind, locked: *locked });
        x += w + s(GAP);
    }
    if ids.is_empty() { s(CHIP_H + 2.0 * GAP) } else { y + s(CHIP_H + GAP) - zone.y }
}
