//! Painel do chat com o Claude: o campo de texto (edição com cursor, sem o EDIT do Windows), a
//! quebra de linha, a limpeza do Markdown e o desenho da conversa com rolagem.

use zefiro_integration_chat::conversation::{MAX_PROMPT, Role, State};
use zefiro_platform::geometry::{Point, Rect, Size};
use zefiro_platform::text::{Align, Family, FontWeight, TextMeasurer, TextStyle};
use zefiro_resources::tokens::Tokens;
use zefiro_ui::paint::{DisplayList, DrawOp};

// --- Campo -----------------------------------------------------------------------------------

/// Texto do campo e o cursor (índice em bytes, sempre numa fronteira de caractere).
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Field {
    pub text: String,
    pub caret: usize,
    /// Ctrl+A: tudo selecionado (digitar troca, Backspace apaga, Ctrl+C copia).
    pub all: bool,
    /// Primeira metade de um par de surrogates (emoji) esperando a segunda.
    high: Option<u16>,
}

/// O que uma tecla fez no campo.
#[derive(Debug, Clone, PartialEq)]
pub enum Edit { Nothing, Changed, Copy(String), Cut(String) }

/// Teclas de movimento e edição (`WM_KEYDOWN`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Key { Left, Right, WordLeft, WordRight, Home, End, Delete, NewLine }

fn is_word(c: char) -> bool {
    c.is_alphanumeric() || c == '_'
}

/// Texto colado ou digitado do jeito que o campo guarda: `\r\n` vira `\n`, tab vira espaços e
/// controles somem.
fn normalize(s: &str) -> String {
    s.replace("\r\n", "\n").replace('\r', "\n").replace('\t', "    ").chars().filter(|c| *c == '\n' || !c.is_control()).collect()
}

impl Field {
    pub fn clear(&mut self) {
        *self = Field::default();
    }

    pub fn is_blank(&self) -> bool {
        self.text.trim().is_empty()
    }

    fn insert(&mut self, s: &str) -> bool {
        if self.all {
            self.text.clear();
            self.caret = 0;
            self.all = false;
        }
        let room = MAX_PROMPT.saturating_sub(self.text.chars().count());
        let s: String = s.chars().take(room).collect();
        if s.is_empty() {
            return false;
        }
        self.text.insert_str(self.caret, &s);
        self.caret += s.len();
        true
    }

    fn prev(&self, i: usize) -> usize {
        self.text[..i].char_indices().next_back().map_or(0, |(p, _)| p)
    }

    fn next(&self, i: usize) -> usize {
        self.text[i..].chars().next().map_or(i, |c| i + c.len_utf8())
    }

    fn word_left(&self, mut i: usize) -> usize {
        while i > 0 && !self.text[..i].chars().next_back().is_some_and(is_word) {
            i = self.prev(i);
        }
        while i > 0 && self.text[..i].chars().next_back().is_some_and(is_word) {
            i = self.prev(i);
        }
        i
    }

    fn word_right(&self, mut i: usize) -> usize {
        while i < self.text.len() && !self.text[i..].chars().next().is_some_and(is_word) {
            i = self.next(i);
        }
        while i < self.text.len() && self.text[i..].chars().next().is_some_and(is_word) {
            i = self.next(i);
        }
        i
    }

    fn take_all(&mut self) -> String {
        self.all = false;
        self.caret = 0;
        std::mem::take(&mut self.text)
    }

    /// Um `WM_CHAR`: letras (e o par de surrogates de um emoji), Backspace, Ctrl+Backspace (apaga a
    /// palavra), Ctrl+A, Ctrl+C, Ctrl+X e Ctrl+V (cola `paste`). Enter, Tab e Esc são das teclas.
    pub fn char(&mut self, code: u32, paste: Option<&str>) -> Edit {
        let changed = |b: bool| if b { Edit::Changed } else { Edit::Nothing };
        match code {
            0xD800..=0xDBFF => {
                self.high = Some(code as u16);
                Edit::Nothing
            }
            0xDC00..=0xDFFF => match self.high.take() {
                Some(high) => changed(char::decode_utf16([high, code as u16]).next().and_then(Result::ok).is_some_and(|c| self.insert(&c.to_string()))),
                None => Edit::Nothing,
            },
            1 => {
                let had = self.all;
                self.all = !self.text.is_empty();
                changed(self.all != had)
            }
            3 if self.all => Edit::Copy(self.text.clone()),
            24 if self.all => Edit::Cut(self.take_all()),
            22 => changed(paste.is_some_and(|p| self.insert(&normalize(p)))),
            8 | 127 if self.all => changed(!self.take_all().is_empty()),
            8 => {
                if self.caret == 0 {
                    return Edit::Nothing;
                }
                let from = self.prev(self.caret);
                self.text.replace_range(from..self.caret, "");
                self.caret = from;
                Edit::Changed
            }
            127 => {
                let from = self.word_left(self.caret);
                if from == self.caret {
                    return Edit::Nothing;
                }
                self.text.replace_range(from..self.caret, "");
                self.caret = from;
                Edit::Changed
            }
            c if c >= 32 && c != 127 => changed(char::from_u32(c).is_some_and(|ch| self.insert(&ch.to_string()))),
            _ => Edit::Nothing,
        }
    }

    /// Uma tecla de movimento ou edição. `true` se o campo mudou (texto ou cursor).
    pub fn key(&mut self, key: Key) -> bool {
        let before = (self.caret, self.all, self.text.len());
        if self.all && key != Key::NewLine {
            match key {
                Key::Delete => {
                    self.take_all();
                }
                Key::Left | Key::WordLeft | Key::Home => self.caret = 0,
                _ => self.caret = self.text.len(),
            }
            self.all = false;
            return true;
        }
        match key {
            Key::Left => self.caret = self.prev(self.caret),
            Key::Right => self.caret = self.next(self.caret),
            Key::WordLeft => self.caret = self.word_left(self.caret),
            Key::WordRight => self.caret = self.word_right(self.caret),
            Key::Home => self.caret = self.text[..self.caret].rfind('\n').map_or(0, |i| i + 1),
            Key::End => self.caret = self.text[self.caret..].find('\n').map_or(self.text.len(), |i| self.caret + i),
            Key::Delete => {
                let to = self.next(self.caret);
                self.text.replace_range(self.caret..to, "");
            }
            Key::NewLine => {
                self.insert("\n");
            }
        }
        before != (self.caret, self.all, self.text.len())
    }

    /// ↑ / ↓ entre as linhas quebradas do campo, mantendo mais ou menos a coluna. `false` na
    /// primeira (↑) ou na última (↓) linha.
    pub fn vertical(&mut self, lines: &[Line], up: bool, style: &TextStyle, m: &dyn TextMeasurer) -> bool {
        self.all = false;
        let at = caret_line(lines, self.caret);
        let Some(target) = (if up { at.checked_sub(1) } else { Some(at + 1).filter(|&i| i < lines.len()) }) else { return false };
        let x = m.measure(&self.text[lines[at].start..self.caret], style).w;
        let line = lines[target];
        let mut best = line.start;
        for (i, _) in self.text[line.start..line.end].char_indices().skip(1).chain(std::iter::once((line.end - line.start, ' '))) {
            let w = m.measure(&self.text[line.start..line.start + i], style).w;
            if w > x {
                let prev = m.measure(&self.text[line.start..best], style).w;
                if w - x < x - prev {
                    best = line.start + i;
                }
                break;
            }
            best = line.start + i;
        }
        self.caret = best;
        true
    }
}

// --- Quebra de linha -------------------------------------------------------------------------

/// Uma linha quebrada: o pedaço `start..end` do texto (sem o `\n`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Line { pub start: usize, pub end: usize }

/// Quebra `text` em linhas de até `width`: por palavra, e por letra quando a palavra sozinha não
/// cabe. `\n` sempre quebra. Texto vazio é uma linha vazia.
pub fn wrap(text: &str, width: f32, style: &TextStyle, m: &dyn TextMeasurer) -> Vec<Line> {
    let mut lines = Vec::new();
    let mut offset = 0;
    for para in text.split('\n') {
        wrap_paragraph(para, offset, width, style, m, &mut lines);
        offset += para.len() + 1;
    }
    lines
}

fn wrap_paragraph(para: &str, base: usize, width: f32, style: &TextStyle, m: &dyn TextMeasurer, out: &mut Vec<Line>) {
    let mut start = 0;
    let mut line_w = 0.0;
    let mut i = 0;
    let bytes = para.as_bytes();
    if para.is_empty() {
        out.push(Line { start: base, end: base });
        return;
    }
    while i < para.len() {
        // Um "token": a palavra e os espaços depois dela.
        let word_end = para[i..].find(' ').map_or(para.len(), |p| i + p);
        let mut token_end = word_end;
        while token_end < para.len() && bytes[token_end] == b' ' {
            token_end += 1;
        }
        let word_w = m.measure(&para[i..word_end], style).w;
        if line_w > 0.0 && line_w + word_w > width {
            out.push(Line { start: base + start, end: base + i });
            start = i;
            line_w = 0.0;
        }
        if word_w > width {
            // Palavra maior que a linha: quebra por letra.
            let mut piece = i;
            let mut piece_w = line_w;
            for (j, c) in para[i..word_end].char_indices() {
                let cw = m.measure(c.encode_utf8(&mut [0; 4]), style).w;
                if piece_w + cw > width && i + j > piece {
                    out.push(Line { start: base + start, end: base + i + j });
                    start = i + j;
                    piece = i + j;
                    piece_w = 0.0;
                }
                piece_w += cw;
            }
            line_w = piece_w + m.measure(&para[word_end..token_end], style).w;
        } else {
            line_w += m.measure(&para[i..token_end], style).w;
        }
        i = token_end;
    }
    out.push(Line { start: base + start, end: base + para.len() });
}

/// Linha em que o cursor está. Numa quebra sem `\n`, o cursor fica no começo da linha de baixo.
pub fn caret_line(lines: &[Line], caret: usize) -> usize {
    lines.iter().rposition(|l| l.start <= caret).unwrap_or(0)
}

/// Linhas de uma mensagem já quebradas, com a largura de cada uma (para o balão do usuário).
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Wrapped { pub lines: Vec<String>, pub widths: Vec<f32> }

pub fn wrap_text(text: &str, width: f32, style: &TextStyle, m: &dyn TextMeasurer) -> Wrapped {
    let mut out = Wrapped::default();
    for l in wrap(text, width, style, m) {
        let s = text[l.start..l.end].trim_end().to_string();
        out.widths.push(m.measure(&s, style).w);
        out.lines.push(s);
    }
    out
}

// --- Markdown --------------------------------------------------------------------------------

/// O pouco de Markdown que escapa do prompt vira texto simples: negrito e crases somem, `#` dos
/// títulos sai, itens com `-`, `*` ou `+` viram `•` e as linhas de cerca (```) somem.
pub fn plain_text(text: &str) -> String {
    let mut out = Vec::new();
    for line in text.split('\n') {
        let trimmed = line.trim_start();
        if trimmed.starts_with("```") {
            continue;
        }
        let indent = &line[..line.len() - trimmed.len()];
        let body = if let Some(rest) = trimmed.strip_prefix("- ").or_else(|| trimmed.strip_prefix("* ")).or_else(|| trimmed.strip_prefix("+ ")) {
            format!("{indent}• {rest}")
        } else if trimmed.starts_with('#') && trimmed.trim_start_matches('#').starts_with(' ') {
            trimmed.trim_start_matches('#').trim_start().to_string()
        } else {
            line.to_string()
        };
        out.push(body.replace("**", "").replace("__", "").replace('`', ""));
    }
    out.join("\n")
}

// --- Chip ------------------------------------------------------------------------------------

/// Chip do chat: o ícone e, com `badge` (respondendo ou resposta não lida), um ponto de destaque
/// no canto de cima do ícone.
pub fn chat_chip(mut chip: crate::chips::ChipContent, badge: bool, chip_h: f32, scale: f32, tokens: &Tokens) -> crate::chips::ChipContent {
    if badge {
        let r = 3.0 * scale;
        chip.list.ops.push(DrawOp::Circle { center: Point { x: chip.content_w - r * 0.5, y: chip_h * 0.5 - 5.0 * scale }, radius: r, color: tokens.accent });
    }
    chip
}

// --- Painel ----------------------------------------------------------------------------------

pub const WIDTH: f32 = 420.0;
/// Área da conversa: cresce até isto e depois rola.
pub const MAX_AREA: f32 = 460.0;
const TEXT: f32 = 12.5;
const LINE: f32 = 18.0;
const PAD_X: f32 = 10.0;
const PAD_Y: f32 = 7.0;
/// Largura máxima do balão do usuário.
const USER_MAX: f32 = 300.0;
const FIELD_LINES: usize = 6;
const AREA_Y: f32 = 50.0;
const EMPTY_H: f32 = 84.0;

const GLYPH_SEND: &str = "\u{E724}";
const GLYPH_STOP: &str = "\u{E71A}";
const GLYPH_COPY: &str = "\u{E8C8}";
const GLYPH_CHECK: &str = "\u{E73E}";
const GLYPH_NEW: &str = "\u{E710}";

pub fn text_style(scale: f32) -> TextStyle {
    TextStyle { size: TEXT * scale, weight: FontWeight::Regular, family: Family::Text }
}

/// Largura do texto de uma mensagem (para quebrar antes de desenhar).
pub fn text_width(role: Role, scale: f32) -> f32 {
    let inner = WIDTH - 24.0 - 2.0 * PAD_X;
    (match role {
        Role::User => USER_MAX - 2.0 * PAD_X,
        Role::Claude => inner,
    }) * scale
}

/// Largura do texto do campo.
pub fn field_text_width(scale: f32) -> f32 {
    (WIDTH - 20.0 - 12.0 - 46.0) * scale
}

/// Uma mensagem como o painel desenha.
pub struct MessageView<'a> { pub role: Role, pub state: State, pub wrapped: &'a Wrapped, pub note: Option<&'a str> }

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Hit { Send, Stop, NewChat, Copy(usize), Field }

pub struct ChatView<'a> {
    pub messages: &'a [MessageView<'a>],
    pub field: &'a Field,
    /// Linhas do campo, quebradas em [`field_text_width`].
    pub field_lines: &'a [Line],
    /// Ao lado do título: a origem ("Windows", "Ubuntu-24.04") ou o que está acontecendo.
    pub subtitle: Option<&'a str>,
    /// Pixels rolados a partir do fim (0 = acompanhando).
    pub scroll: f32,
    pub hover: Option<Hit>,
    /// Mensagem recém-copiada (o ícone vira um visto).
    pub copied: Option<usize>,
}

pub struct ChatLayout {
    pub list: DisplayList,
    pub hits: Vec<(Rect, Hit)>,
    /// Rolagem máxima (pixels).
    pub max_scroll: f32,
    /// Altura de uma linha de texto (pixels), para o passo da roda.
    pub line: f32,
}

impl ChatLayout {
    pub fn hit(&self, p: Point) -> Option<Hit> {
        self.hits.iter().find(|(r, _)| r.contains(p)).map(|(_, h)| *h)
    }
}

/// Altura de uma mensagem (balão + rodapé).
fn message_h(m: &MessageView, scale: f32, note_lines: usize) -> f32 {
    if bubbleless(m) {
        return (note_lines as f32 * 16.0 + 4.0) * scale;
    }
    let lines = if m.state.busy() && m.wrapped.lines.iter().all(|l| l.is_empty()) { 1 } else { m.wrapped.lines.len().max(1) };
    let mut h = lines as f32 * LINE + 2.0 * PAD_Y;
    if m.role == Role::Claude {
        h += note_lines as f32 * 16.0;
        if !m.state.busy() {
            h += 22.0;
        }
    }
    h * scale
}

pub fn chat_panel(view: &ChatView, scale: f32, tokens: &Tokens, m: &dyn TextMeasurer) -> ChatLayout {
    let px = |v: f32| (v * scale).round();
    let text = |size: f32, weight: FontWeight| TextStyle { size: size * scale, weight, family: Family::Text };
    let icons = |size: f32| TextStyle { size: size * scale, weight: FontWeight::Regular, family: Family::Icons };
    let body = text_style(scale);
    let note_style = text(11.0, FontWeight::Regular);
    let line = LINE * scale;
    let w = px(WIDTH);
    let mut ops = Vec::new();
    let mut hits = Vec::new();

    // Cabeçalho.
    let title_style = text(13.0, FontWeight::SemiBold);
    let title_w = m.measure("Claude", &title_style).w.ceil();
    ops.push(DrawOp::Text { rect: Rect::new(px(20.0), px(14.0), title_w + px(2.0), px(22.0)), text: "Claude".into(), style: title_style, color: tokens.text, align: Align::Leading });
    if let Some(sub) = view.subtitle {
        ops.push(DrawOp::Text { rect: Rect::new(px(20.0) + title_w + px(8.0), px(16.0), w - px(170.0) - title_w, px(20.0)), text: sub.into(), style: text(11.0, FontWeight::Regular), color: tokens.text_tertiary, align: Align::Leading });
    }
    if !view.messages.is_empty() {
        let label = "Nova conversa";
        let ls = text(11.5, FontWeight::SemiBold);
        let bw = m.measure(label, &ls).w.ceil() + px(36.0);
        let r = Rect::new(w - px(14.0) - bw, px(12.0), bw, px(28.0));
        ops.push(DrawOp::RoundRect { rect: r, radius: r.h / 2.0, color: if view.hover == Some(Hit::NewChat) { tokens.raised_strong } else { tokens.raised } });
        ops.push(DrawOp::Text { rect: Rect::new(r.x + px(10.0), r.y, px(14.0), r.h), text: GLYPH_NEW.into(), style: icons(11.0), color: tokens.text_secondary, align: Align::Center });
        ops.push(DrawOp::Text { rect: Rect::new(r.x + px(28.0), r.y, r.w - px(34.0), r.h), text: label.into(), style: ls, color: tokens.text_secondary, align: Align::Leading });
        hits.push((r, Hit::NewChat));
    }

    // Conversa.
    let note_width = text_width(Role::Claude, scale);
    let notes: Vec<Vec<String>> = view.messages.iter().map(|msg| msg.note.map(|n| wrap_text(n, note_width, &note_style, m).lines).unwrap_or_default()).collect();
    let gap = px(8.0);
    let heights: Vec<f32> = view.messages.iter().zip(&notes).map(|(msg, n)| message_h(msg, scale, n.len())).collect();
    let content_h: f32 = heights.iter().sum::<f32>() + gap * view.messages.len().saturating_sub(1) as f32;
    let area_h = if view.messages.is_empty() { px(EMPTY_H) } else { content_h.min(px(MAX_AREA)) };
    let area = Rect::new(0.0, px(AREA_Y), w, area_h);
    let max_scroll = (content_h - area_h).max(0.0);
    let scroll = view.scroll.clamp(0.0, max_scroll);

    if view.messages.is_empty() {
        ops.push(DrawOp::Text { rect: Rect::new(px(20.0), area.y + px(18.0), w - px(40.0), px(20.0)), text: "Pergunte qualquer coisa ao Claude".into(), style: text(13.0, FontWeight::Regular), color: tokens.text_secondary, align: Align::Center });
        ops.push(DrawOp::Text { rect: Rect::new(px(20.0), area.y + px(42.0), w - px(40.0), px(18.0)), text: "Enter envia · Shift+Enter quebra a linha".into(), style: text(11.0, FontWeight::Regular), color: tokens.text_tertiary, align: Align::Center });
    } else {
        ops.push(DrawOp::PushClip(area));
        let mut y = area.y + area_h - content_h + scroll;
        for (i, msg) in view.messages.iter().enumerate() {
            let h = heights[i];
            if y + h >= area.y && y <= area.bottom() {
                draw_message(&mut ops, &mut hits, view, i, msg, &notes[i], y, area, scale, tokens);
            }
            y += h + gap;
        }
        ops.push(DrawOp::PopClip);
        // Indício de que há mais acima ou abaixo.
        if scroll < max_scroll {
            ops.push(DrawOp::RoundRect { rect: Rect::new(px(12.0), area.y, w - px(24.0), px(1.0)), radius: 0.0, color: tokens.hairline });
        }
        if scroll > 0.0 {
            ops.push(DrawOp::RoundRect { rect: Rect::new(px(12.0), area.bottom() - px(1.0), w - px(24.0), px(1.0)), radius: 0.0, color: tokens.hairline });
        }
    }

    // Campo.
    let lines = view.field_lines;
    let shown = lines.len().clamp(1, FIELD_LINES);
    let at = caret_line(lines, view.field.caret);
    let first = (at + 1).saturating_sub(FIELD_LINES).min(lines.len().saturating_sub(shown));
    let field = Rect::new(px(10.0), area.bottom() + px(10.0), w - px(20.0), shown as f32 * line + px(2.0 * 9.0));
    ops.push(DrawOp::RoundRect { rect: field, radius: px(14.0), color: tokens.raised });
    hits.push((field, Hit::Field));
    let tx = field.x + px(12.0);
    let ty = field.y + px(9.0);
    let tw = field_text_width(scale);
    let busy = view.messages.last().is_some_and(|m| m.role == Role::Claude && m.state.busy());
    if view.field.text.is_empty() {
        ops.push(DrawOp::Text { rect: Rect::new(tx, ty, tw, line), text: "Pergunte ao Claude…".into(), style: body, color: tokens.text_tertiary, align: Align::Leading });
    }
    for (row, l) in lines.iter().enumerate().skip(first).take(shown) {
        let y = ty + (row - first) as f32 * line;
        let s = &view.field.text[l.start..l.end];
        if view.field.all && !s.is_empty() {
            ops.push(DrawOp::RoundRect { rect: Rect::new(tx - px(1.0), y + px(1.0), m.measure(s, &body).w + px(2.0), line - px(2.0)), radius: px(3.0), color: tokens.accent_soft });
        }
        if !s.is_empty() {
            ops.push(DrawOp::Text { rect: Rect::new(tx, y, tw + px(4.0), line), text: s.into(), style: body, color: tokens.text, align: Align::Leading });
        }
    }
    if !view.field.all {
        let l = lines.get(at).copied().unwrap_or(Line { start: 0, end: 0 });
        let cx = tx + m.measure(&view.field.text[l.start..view.field.caret.max(l.start)], &body).w.min(tw);
        let cy = ty + (at.saturating_sub(first)) as f32 * line;
        ops.push(DrawOp::RoundRect { rect: Rect::new(cx, cy + px(2.0), px(1.5), line - px(4.0)), radius: px(0.75), color: tokens.accent });
    }
    let button = Rect::new(field.right() - px(6.0 + 30.0), field.bottom() - px(6.0 + 30.0), px(30.0), px(30.0));
    let (glyph, hit, lit) = if busy { (GLYPH_STOP, Hit::Stop, true) } else { (GLYPH_SEND, Hit::Send, !view.field.is_blank()) };
    let hovered = view.hover == Some(hit);
    let fill = if lit { if hovered { tokens.accent } else { tokens.accent.with_alpha(tokens.accent.a * 0.85) } } else { tokens.raised_strong };
    ops.push(DrawOp::RoundRect { rect: button, radius: button.h / 2.0, color: fill });
    let fg = if lit { if tokens.accent.relative_luminance() > 0.5 { zefiro_platform::color::Rgba::BLACK } else { zefiro_platform::color::Rgba::WHITE } } else { tokens.text_tertiary };
    ops.push(DrawOp::Text { rect: button, text: glyph.into(), style: icons(12.0), color: fg, align: Align::Center });
    hits.insert(0, (button, hit));

    let size = Size { w, h: field.bottom() + px(10.0) };
    ChatLayout { list: DisplayList { size, ops }, hits, max_scroll, line }
}

/// Resposta que terminou sem texto (só o aviso do erro): sem balão.
fn bubbleless(m: &MessageView) -> bool {
    m.role == Role::Claude && !m.state.busy() && m.wrapped.lines.iter().all(|l| l.is_empty())
}

#[allow(clippy::too_many_arguments)]
fn draw_message(ops: &mut Vec<DrawOp>, hits: &mut Vec<(Rect, Hit)>, view: &ChatView, index: usize, msg: &MessageView, note: &[String], y: f32, area: Rect, scale: f32, tokens: &Tokens) {
    let px = |v: f32| (v * scale).round();
    let body = text_style(scale);
    let line = LINE * scale;
    let w = px(WIDTH);
    let note_style = TextStyle { size: 11.0 * scale, weight: FontWeight::Regular, family: Family::Text };
    if bubbleless(msg) {
        for (row, n) in note.iter().enumerate() {
            ops.push(DrawOp::Text { rect: Rect::new(px(16.0), y + px(2.0) + row as f32 * px(16.0), w - px(32.0), px(16.0)), text: n.clone(), style: note_style, color: tokens.text_secondary, align: Align::Leading });
        }
        return;
    }
    let waiting = msg.state.busy() && msg.wrapped.lines.iter().all(|l| l.is_empty());
    let lines = if waiting { 1 } else { msg.wrapped.lines.len().max(1) };
    let text_w = if msg.role == Role::User { msg.wrapped.widths.iter().copied().fold(0.0, f32::max).ceil() } else { text_width(Role::Claude, scale) };
    let bubble_w = (text_w + px(2.0 * PAD_X)).min(w - px(24.0));
    let bubble_h = lines as f32 * line + px(2.0 * PAD_Y);
    let x = if msg.role == Role::User { w - px(12.0) - bubble_w } else { px(12.0) };
    let bubble = Rect::new(x, y, bubble_w, bubble_h);
    let (fill, color) = match msg.role {
        Role::User => (tokens.accent_soft, tokens.text),
        Role::Claude => (tokens.raised, if msg.state == State::Failed { tokens.text_secondary } else { tokens.text }),
    };
    ops.push(DrawOp::RoundRect { rect: bubble, radius: px(12.0), color: fill });
    let tx = bubble.x + px(PAD_X);
    if waiting {
        let label = if msg.state == State::Thinking { "Pensando…" } else { "…" };
        ops.push(DrawOp::Text { rect: Rect::new(tx, bubble.y + px(PAD_Y), bubble.w - px(2.0 * PAD_X), line), text: label.into(), style: body, color: tokens.text_tertiary, align: Align::Leading });
    } else {
        for (row, s) in msg.wrapped.lines.iter().enumerate() {
            let ly = bubble.y + px(PAD_Y) + row as f32 * line;
            if ly + line < area.y || ly > area.bottom() || s.is_empty() {
                continue;
            }
            ops.push(DrawOp::Text { rect: Rect::new(tx, ly, bubble.w - px(2.0 * PAD_X) + px(4.0), line), text: s.clone(), style: body, color, align: Align::Leading });
        }
    }
    if msg.role != Role::Claude {
        return;
    }
    let mut fy = bubble.bottom() + px(2.0);
    for n in note {
        ops.push(DrawOp::Text { rect: Rect::new(px(22.0), fy, w - px(44.0), px(16.0)), text: n.clone(), style: note_style, color: tokens.text_tertiary, align: Align::Leading });
        fy += px(16.0);
    }
    if !msg.state.busy() && !msg.wrapped.lines.iter().all(|l| l.is_empty()) {
        let r = Rect::new(px(16.0), fy, px(26.0), px(20.0));
        let copied = view.copied == Some(index);
        let hovered = view.hover == Some(Hit::Copy(index));
        if hovered {
            ops.push(DrawOp::RoundRect { rect: r, radius: px(6.0), color: tokens.raised });
        }
        let icon = TextStyle { size: 11.0 * scale, weight: FontWeight::Regular, family: Family::Icons };
        ops.push(DrawOp::Text { rect: r, text: if copied { GLYPH_CHECK } else { GLYPH_COPY }.into(), style: icon, color: if copied { tokens.accent } else { tokens.text_tertiary }, align: Align::Center });
        if r.y >= area.y && r.bottom() <= area.bottom() {
            hits.push((r, Hit::Copy(index)));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use zefiro_platform::color::Rgba;
    use zefiro_platform::text::FixedWidthMeasurer;
    use zefiro_resources::theme::{BUILTIN, parse_theme};
    use zefiro_resources::tokens::derive_tokens;

    fn tokens() -> Tokens {
        derive_tokens(&parse_theme("obsidian", "o", BUILTIN[0].1).unwrap(), Rgba::rgb(0, 120, 215), None)
    }

    /// Com o medidor fixo, cada caractere tem 10 px.
    fn style() -> TextStyle {
        TextStyle { size: 10.0 / 0.55, weight: FontWeight::Regular, family: Family::Text }
    }

    fn typed(s: &str) -> Field {
        let mut f = Field::default();
        for c in s.encode_utf16() {
            if c == 10 {
                f.key(Key::NewLine);
            } else {
                f.char(c as u32, None);
            }
        }
        f
    }

    fn texts(l: &DisplayList) -> Vec<String> {
        l.ops.iter().filter_map(|o| if let DrawOp::Text { text, .. } = o { Some(text.clone()) } else { None }).collect()
    }

    #[test]
    fn typing_accents_and_emoji() {
        let f = typed("Olá, ação 🌬️!");
        assert_eq!(f.text, "Olá, ação 🌬️!");
        assert_eq!(f.caret, f.text.len());
        let mut lone = Field::default();
        assert_eq!(lone.char(0xDC00, None), Edit::Nothing, "metade solta de um par é ignorada");
        assert_eq!(lone.char(13, None), Edit::Nothing, "Enter é das teclas");
        assert_eq!(lone.char(9, None), Edit::Nothing);
        assert!(lone.text.is_empty());
    }

    #[test]
    fn backspace_and_word_delete() {
        let mut f = typed("bom dia, mundo");
        assert_eq!(f.char(8, None), Edit::Changed);
        assert_eq!(f.text, "bom dia, mund");
        assert_eq!(f.char(127, None), Edit::Changed);
        assert_eq!(f.text, "bom dia, ");
        f.char(127, None);
        assert_eq!(f.text, "bom ");
        let mut e = typed("é");
        e.char(8, None);
        assert_eq!((e.text.as_str(), e.caret), ("", 0), "apaga o caractere inteiro");
        assert_eq!(e.char(8, None), Edit::Nothing);
    }

    #[test]
    fn caret_moves_and_inserts_in_the_middle() {
        let mut f = typed("ab cd");
        assert!(f.key(Key::WordLeft));
        assert_eq!(f.caret, 3);
        f.char('X' as u32, None);
        assert_eq!(f.text, "ab Xcd");
        assert!(f.key(Key::Home));
        assert_eq!(f.caret, 0);
        assert!(!f.key(Key::Left), "no começo não anda");
        assert!(f.key(Key::Delete));
        assert_eq!(f.text, "b Xcd");
        assert!(f.key(Key::End));
        assert_eq!(f.caret, f.text.len());
        f.key(Key::WordLeft);
        f.key(Key::WordRight);
        assert_eq!(f.caret, f.text.len());
    }

    #[test]
    fn newline_and_home_end_per_line() {
        let mut f = typed("um");
        assert!(f.key(Key::NewLine));
        f.char('d' as u32, None);
        f.char('o' as u32, None);
        assert_eq!(f.text, "um\ndo");
        f.key(Key::Home);
        assert_eq!(f.caret, 3);
        f.key(Key::Left);
        f.key(Key::Home);
        assert_eq!(f.caret, 0);
        f.key(Key::End);
        assert_eq!(f.caret, 2);
    }

    #[test]
    fn paste_normalizes_and_respects_the_limit() {
        let mut f = typed("x");
        assert_eq!(f.char(22, Some("a\r\nb\tc\u{7}")), Edit::Changed);
        assert_eq!(f.text, "xa\nb    c");
        assert_eq!(f.char(22, None), Edit::Nothing, "área vazia");
        let mut big = Field::default();
        big.char(22, Some(&"a".repeat(MAX_PROMPT + 50)));
        assert_eq!(big.text.chars().count(), MAX_PROMPT);
        assert_eq!(big.char('b' as u32, None), Edit::Nothing, "cheio");
    }

    #[test]
    fn select_all_then_copy_cut_replace_or_delete() {
        let mut f = typed("abc");
        assert_eq!(f.char(3, None), Edit::Nothing, "Ctrl+C sem seleção não copia");
        assert_eq!(f.char(1, None), Edit::Changed);
        assert_eq!(f.char(3, None), Edit::Copy("abc".into()));
        f.char('z' as u32, None);
        assert_eq!((f.text.as_str(), f.all), ("z", false), "digitar troca tudo");
        f.char(1, None);
        assert_eq!(f.char(24, None), Edit::Cut("z".into()));
        assert!(f.text.is_empty());
        let mut g = typed("abc");
        g.char(1, None);
        g.char(8, None);
        assert!(g.text.is_empty());
        let mut h = typed("abc");
        h.char(1, None);
        assert!(h.key(Key::Left));
        assert_eq!((h.caret, h.all), (0, false), "seta tira a seleção");
    }

    #[test]
    fn wraps_by_word_by_letter_and_by_newline() {
        let m = FixedWidthMeasurer;
        let st = style();
        let t = "aaa bbb ccc";
        let lines = wrap(t, 75.0, &st, &m);
        let s: Vec<&str> = lines.iter().map(|l| &t[l.start..l.end]).collect();
        assert_eq!(s, ["aaa bbb ", "ccc"]);
        let t = "abcdefghij";
        let s: Vec<String> = wrap(t, 40.0, &st, &m).iter().map(|l| t[l.start..l.end].to_string()).collect();
        assert_eq!(s, ["abcd", "efgh", "ij"], "palavra maior que a linha quebra por letra");
        let t = "um\n\ndois";
        let s: Vec<String> = wrap(t, 500.0, &st, &m).iter().map(|l| t[l.start..l.end].to_string()).collect();
        assert_eq!(s, ["um", "", "dois"]);
        assert_eq!(wrap("", 100.0, &st, &m), [Line { start: 0, end: 0 }]);
    }

    #[test]
    fn wrapped_text_drops_trailing_spaces_and_measures() {
        let w = wrap_text("aaa bbb ccc", 75.0, &style(), &FixedWidthMeasurer);
        assert_eq!(w.lines, ["aaa bbb", "ccc"]);
        assert_eq!(w.widths.iter().map(|x| x.round()).collect::<Vec<_>>(), [70.0, 30.0]);
    }

    #[test]
    fn caret_line_prefers_the_next_line_at_a_soft_break() {
        let lines = [Line { start: 0, end: 8 }, Line { start: 8, end: 11 }];
        assert_eq!(caret_line(&lines, 3), 0);
        assert_eq!(caret_line(&lines, 8), 1);
        assert_eq!(caret_line(&lines, 11), 1);
    }

    #[test]
    fn up_and_down_keep_the_column() {
        let m = FixedWidthMeasurer;
        let st = style();
        let mut f = typed("abcdef\nxy\nlmnopq");
        let lines = wrap(&f.text, 500.0, &st, &m);
        f.caret = 4; // "abcd|ef"
        assert!(f.vertical(&lines, false, &st, &m));
        assert_eq!(f.caret, 9, "linha curta: vai para o fim dela");
        assert!(f.vertical(&lines, false, &st, &m));
        assert_eq!(f.caret, 12, "coluna 2 da terceira linha");
        assert!(!f.vertical(&lines, false, &st, &m), "última linha");
        f.caret = 1;
        assert!(!f.vertical(&lines, true, &st, &m), "primeira linha");
    }

    #[test]
    fn markdown_becomes_plain_text() {
        let md = "# Título\nUm **negrito** e `código`.\n- item\n  * sub\n```rust\nlet x = 1;\n```";
        assert_eq!(plain_text(md), "Título\nUm negrito e código.\n• item\n  • sub\nlet x = 1;");
        assert_eq!(plain_text("#hashtag e 2*3"), "#hashtag e 2*3");
    }

    fn wrapped(s: &str) -> Wrapped {
        wrap_text(s, text_width(Role::Claude, 1.0), &text_style(1.0), &FixedWidthMeasurer)
    }

    fn panel(messages: &[MessageView], field: &Field, scroll: f32) -> ChatLayout {
        let lines = wrap(&field.text, field_text_width(1.0), &text_style(1.0), &FixedWidthMeasurer);
        let view = ChatView { messages, field, field_lines: &lines, subtitle: Some("Windows"), scroll, hover: None, copied: None };
        chat_panel(&view, 1.0, &tokens(), &FixedWidthMeasurer)
    }

    #[test]
    fn empty_conversation_shows_the_hint_and_no_new_chat() {
        let l = panel(&[], &Field::default(), 0.0);
        let t = texts(&l.list);
        for s in ["Claude", "Windows", "Pergunte qualquer coisa ao Claude", "Pergunte ao Claude…", GLYPH_SEND] {
            assert!(t.contains(&s.to_string()), "falta {s}");
        }
        assert!(l.hits.iter().all(|(_, h)| *h != Hit::NewChat));
        assert!(l.hits.iter().any(|(_, h)| *h == Hit::Send));
        assert_eq!(l.list.size.w, WIDTH);
    }

    #[test]
    fn conversation_with_waiting_reply_shows_stop_and_thinking() {
        let q = wrapped("Qual a capital do Peru?");
        let a = Wrapped::default();
        let msgs = [MessageView { role: Role::User, state: State::Done, wrapped: &q, note: None }, MessageView { role: Role::Claude, state: State::Thinking, wrapped: &a, note: None }];
        let l = panel(&msgs, &Field::default(), 0.0);
        let t = texts(&l.list);
        assert!(t.contains(&"Qual a capital do Peru?".to_string()) && t.contains(&"Pensando…".to_string()) && t.contains(&"Nova conversa".to_string()));
        assert!(l.hits.iter().any(|(_, h)| *h == Hit::Stop) && !l.hits.iter().any(|(_, h)| *h == Hit::Send));
        assert!(!l.hits.iter().any(|(_, h)| matches!(h, Hit::Copy(_))), "sem copiar enquanto responde");
    }

    #[test]
    fn failed_reply_without_text_is_just_the_note() {
        let a = Wrapped { lines: vec![String::new()], widths: vec![0.0] };
        let msgs = [MessageView { role: Role::Claude, state: State::Failed, wrapped: &a, note: Some("Entre no Claude Code.") }];
        let l = panel(&msgs, &Field::default(), 0.0);
        assert!(texts(&l.list).contains(&"Entre no Claude Code.".to_string()));
        assert!(!l.hits.iter().any(|(_, h)| matches!(h, Hit::Copy(_))));
        let bubbles = l.list.ops.iter().filter(|o| matches!(o, DrawOp::RoundRect { rect, color, .. } if *color == tokens().raised && rect.x == 12.0)).count();
        assert_eq!(bubbles, 0, "sem balão vazio");
    }

    #[test]
    fn chip_badge_is_a_dot() {
        let chip = || crate::chips::ChipContent { content_w: 20.0, list: DisplayList::new(Size { w: 20.0, h: 30.0 }) };
        assert!(chat_chip(chip(), false, 30.0, 1.0, &tokens()).list.ops.is_empty());
        assert!(matches!(chat_chip(chip(), true, 30.0, 1.0, &tokens()).list.ops[..], [DrawOp::Circle { .. }]));
    }

    #[test]
    fn finished_reply_has_copy_and_note() {
        let a = wrapped("Lima.");
        let msgs = [MessageView { role: Role::Claude, state: State::Stopped, wrapped: &a, note: Some("Resposta interrompida.") }];
        let l = panel(&msgs, &Field::default(), 0.0);
        assert!(texts(&l.list).contains(&"Resposta interrompida.".to_string()));
        assert!(l.hits.iter().any(|(_, h)| *h == Hit::Copy(0)));
    }

    #[test]
    fn long_conversation_scrolls_and_culls() {
        let text = (0..60).map(|i| format!("linha {i}")).collect::<Vec<_>>().join("\n");
        let a = wrapped(&text);
        let msgs = [MessageView { role: Role::Claude, state: State::Done, wrapped: &a, note: None }];
        let bottom = panel(&msgs, &Field::default(), 0.0);
        assert!(bottom.max_scroll > 0.0);
        assert!(bottom.list.size.h < AREA_Y + MAX_AREA + 120.0, "a área não passa do máximo");
        let t = texts(&bottom.list);
        assert!(t.contains(&"linha 59".to_string()) && !t.contains(&"linha 0".to_string()), "acompanha o fim e não desenha o que está fora");
        let top = panel(&msgs, &Field::default(), 1.0e6);
        let t = texts(&top.list);
        assert!(t.contains(&"linha 0".to_string()) && !t.contains(&"linha 59".to_string()), "rolagem presa no máximo");
        assert_eq!(top.list.size, bottom.list.size);
    }

    #[test]
    fn field_grows_to_six_lines_then_scrolls() {
        let one = panel(&[], &typed("oi"), 0.0);
        let three = panel(&[], &typed("a\nb\nc"), 0.0);
        let many = panel(&[], &typed("1\n2\n3\n4\n5\n6\n7\n8"), 0.0);
        assert!(three.list.size.h > one.list.size.h);
        assert_eq!(many.list.size.h - one.list.size.h, 5.0 * LINE);
        let t = texts(&many.list);
        assert!(t.contains(&"8".to_string()) && !t.contains(&"1".to_string()), "o cursor (no fim) fica à vista");
        assert!(!texts(&one.list).contains(&"Pergunte ao Claude…".to_string()));
    }

    #[test]
    fn hit_finds_buttons() {
        let l = panel(&[], &typed("oi"), 0.0);
        let send = l.hits.iter().find(|(_, h)| *h == Hit::Send).unwrap().0;
        assert_eq!(l.hit(Point { x: send.x + 2.0, y: send.y + 2.0 }), Some(Hit::Send), "o botão ganha do campo embaixo dele");
        assert_eq!(l.hit(Point { x: 1.0, y: 1.0 }), None);
    }
}
