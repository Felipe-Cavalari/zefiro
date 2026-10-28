//! Painel do chat com o Claude: o campo de texto (edição com cursor, sem o EDIT do Windows), a
//! quebra de linha, a limpeza do Markdown e o desenho da conversa com rolagem.

use zefiro_integration_chat::agent::Agent;
use zefiro_integration_chat::conversation::{MAX_PROMPT, Role, State};
use zefiro_integration_chat::origin::Prefer;
use zefiro_platform::color::Rgba;
use zefiro_platform::geometry::{Point, Rect, Size};
use zefiro_platform::image::Image;
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
/// Topo da área (cabeçalho e seletores acima).
const AREA_Y: f32 = 78.0;
const EMPTY_H: f32 = 84.0;
/// Linha de uma escolha (origem, modelo).
const PICK_H: f32 = 32.0;
/// Linha de uma conversa antiga.
const HISTORY_H: f32 = 46.0;
/// Conversas antigas visíveis de uma vez (a roda rola).
pub const HISTORY_VISIBLE: usize = 9;
/// Título das páginas de escolha e de conversas.
const PAGE_TITLE_H: f32 = 32.0;

const GLYPH_SEND: &str = "\u{E724}";
const GLYPH_STOP: &str = "\u{E71A}";
const GLYPH_COPY: &str = "\u{E8C8}";
const GLYPH_CHECK: &str = "\u{E73E}";
const GLYPH_NEW: &str = "\u{E710}";
const GLYPH_HISTORY: &str = "\u{E81C}";
const GLYPH_CHEVRON: &str = "\u{E70D}";
const GLYPH_BACK: &str = "\u{E72B}";
const GLYPH_DELETE: &str = "\u{E74D}";
const GLYPH_REMOVE: &str = "\u{E711}";
const GLYPH_LIBRARY: &str = "\u{E8B9}";
const GLYPH_FOLDER: &str = "\u{E838}";
const GLYPH_CHAT: &str = "\u{E8BD}";
/// Biblioteca de imagens: colunas e fileiras à vista (a roda rola de uma fileira em uma).
pub const LIBRARY_COLUMNS: usize = 3;
pub const LIBRARY_ROWS: usize = 3;
const LIBRARY_GAP: f32 = 6.0;
/// Miniaturas no campo (quadradas) e altura das imagens no balão da pergunta.
const ATTACH: f32 = 56.0;
const PICTURE_H: f32 = 72.0;
const PICTURE_GAP: f32 = 6.0;
/// Altura máxima de uma imagem gerada no balão da resposta (ela ocupa a largura do balão).
pub const REPLY_PICTURE_H: f32 = 260.0;

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

/// Primeira conversa antiga máxima da rolagem.
pub fn history_max_scroll(count: usize) -> usize {
    count.saturating_sub(HISTORY_VISIBLE)
}

/// Primeira fileira máxima da rolagem da biblioteca.
pub fn library_max_scroll(count: usize) -> usize {
    count.div_ceil(LIBRARY_COLUMNS).saturating_sub(LIBRARY_ROWS)
}

/// Os quadros à vista da biblioteca com a rolagem `scroll` (em fileiras).
pub fn library_visible(count: usize, scroll: usize) -> std::ops::Range<usize> {
    let first = scroll.min(library_max_scroll(count)) * LIBRARY_COLUMNS;
    first..(first + LIBRARY_COLUMNS * LIBRARY_ROWS).min(count)
}

/// Um quadro da biblioteca: a miniatura (`None` = carregando) e se foi gerada ou colada.
pub struct LibraryTile { pub image: Option<Image>, pub generated: bool }

/// Uma mensagem como o painel desenha. `images`: as coladas na pergunta ou as geradas na resposta
/// (`None` = ainda carregando). `generating`: a resposta está gerando mais uma imagem.
pub struct MessageView<'a> { pub role: Role, pub state: State, pub wrapped: &'a Wrapped, pub note: Option<&'a str>, pub images: &'a [Option<Image>], pub generating: bool }

/// `rect` reduzido para caber em `cell` mantendo a proporção da imagem, centrado.
fn fit(image: &Image, cell: Rect) -> Rect {
    let s = (cell.w / image.width() as f32).min(cell.h / image.height() as f32);
    let (w, h) = (image.width() as f32 * s, image.height() as f32 * s);
    Rect::new(cell.x + (cell.w - w) / 2.0, cell.y + (cell.h - h) / 2.0, w, h)
}

/// Onde fica cada imagem no balão, a partir do canto do conteúdo. Na pergunta, numa fileira de
/// altura fixa (largura entre 40 e 160 DIP); na resposta, uma embaixo da outra na largura do
/// balão, com a altura limitada. Com `generating`, a última é o lugar da que está sendo gerada.
fn picture_cells(m: &MessageView, scale: f32) -> Vec<Rect> {
    let px = |v: f32| (v * scale).round();
    let count = m.images.len() + usize::from(m.generating);
    let mut cells = Vec::with_capacity(count);
    let mut at = 0.0;
    for i in 0..count {
        let image = m.images.get(i).and_then(Option::as_ref);
        let ratio = image.map(|i| i.width() as f32 / i.height().max(1) as f32);
        let cell = match m.role {
            Role::User => {
                let w = ratio.map_or(PICTURE_H, |r| (PICTURE_H * r).clamp(40.0, 160.0)) * scale;
                let r = Rect::new(at, 0.0, w, px(PICTURE_H));
                at += w + px(PICTURE_GAP);
                r
            }
            Role::Claude => {
                let full = text_width(Role::Claude, scale);
                let ratio = ratio.unwrap_or(1.5);
                let (mut w, mut h) = (full, (full / ratio).round());
                if h > px(REPLY_PICTURE_H) {
                    h = px(REPLY_PICTURE_H);
                    w = (h * ratio).round();
                }
                let r = Rect::new(0.0, at, w, h);
                at += h + px(PICTURE_GAP);
                r
            }
        };
        cells.push(cell);
    }
    cells
}

/// Largura e altura do bloco das imagens.
fn pictures_size(cells: &[Rect]) -> (f32, f32) {
    cells.iter().fold((0.0, 0.0), |(w, h), c| (w.max(c.right()), h.max(c.bottom())))
}

fn has_text(m: &MessageView) -> bool {
    !m.wrapped.lines.iter().all(|l| l.is_empty())
}

fn has_pictures(m: &MessageView) -> bool {
    !m.images.is_empty() || m.generating
}

/// Linhas de texto no balão: uma ("…", "Pensando…") enquanto nada chegou, nenhuma quando só há
/// imagens.
fn text_lines(m: &MessageView) -> usize {
    if has_text(m) {
        m.wrapped.lines.len().max(1)
    } else if has_pictures(m) {
        0
    } else {
        1
    }
}

/// Uma linha das conversas antigas: o título e "Codex · Ubuntu · 21/10 20:37".
#[derive(Debug, Clone, PartialEq)]
pub struct HistoryRow { pub title: String, pub meta: String, pub current: bool }

/// O que a área do painel mostra.
pub enum Page<'a> {
    Chat,
    /// Escolha da origem ou do modelo. `note`: "Procurando distros…", "Carregando modelos…".
    /// `prefer`: só na origem, a chave do que o automático tenta primeiro.
    Picker { title: &'a str, options: &'a [String], selected: Option<usize>, note: Option<&'a str>, prefer: Option<Prefer> },
    History { rows: &'a [HistoryRow], scroll: usize },
    /// Biblioteca de imagens: todos os quadros (só os à vista são desenhados); `scroll` em fileiras.
    Library { tiles: &'a [LibraryTile], scroll: usize },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Hit {
    Send,
    Stop,
    NewChat,
    History,
    Back,
    Agent(Agent),
    Origins,
    Models,
    Pick(usize),
    Prefer(Prefer),
    Open(usize),
    Delete(usize),
    Copy(usize),
    Unattach(usize),
    /// Imagem `.1` da mensagem `.0` (abre no visualizador).
    Picture(usize, usize),
    /// Botão da biblioteca no cabeçalho; na biblioteca, o quadro (abre a imagem), o botão da
    /// conversa de origem e "Abrir pasta".
    Library,
    Tile(usize),
    GoTo(usize),
    OpenFolder,
    Field,
}

pub struct ChatView<'a> {
    pub messages: &'a [MessageView<'a>],
    pub field: &'a Field,
    /// Linhas do campo, quebradas em [`field_text_width`].
    pub field_lines: &'a [Line],
    /// Imagens coladas esperando o envio (`None` = convertendo).
    pub attachments: &'a [Option<Image>],
    pub agent: Agent,
    /// Seletores: a origem ("Automático", "Windows", a distro) e o modelo.
    pub origin: &'a str,
    pub model: &'a str,
    /// Depois dos seletores: "iniciando…", "indisponível".
    pub status: Option<&'a str>,
    pub page: Page<'a>,
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
    let lines = text_lines(m);
    let (_, pictures_h) = pictures_size(&picture_cells(m, scale));
    let mut h = (lines as f32 * LINE + 2.0 * PAD_Y) * scale;
    if has_pictures(m) {
        h += pictures_h + if lines > 0 { PICTURE_GAP * scale } else { 0.0 };
    }
    if m.role == Role::Claude {
        h += note_lines as f32 * 16.0 * scale;
        if !m.state.busy() {
            h += 22.0 * scale;
        }
    }
    h
}

/// Cor de texto legível sobre o destaque.
fn on_accent(tokens: &Tokens) -> Rgba {
    if tokens.accent.relative_luminance() > 0.5 { Rgba::BLACK } else { Rgba::WHITE }
}

/// Cabeçalho: Claude | Codex | Grok, os botões de conversas antigas e de conversa nova, e embaixo os
/// seletores de origem e de modelo.
fn draw_header(ops: &mut Vec<DrawOp>, hits: &mut Vec<(Rect, Hit)>, view: &ChatView, scale: f32, tokens: &Tokens, m: &dyn TextMeasurer) {
    let px = |v: f32| (v * scale).round();
    let text = |size: f32, weight: FontWeight| TextStyle { size: size * scale, weight, family: Family::Text };
    let icons = |size: f32| TextStyle { size: size * scale, weight: FontWeight::Regular, family: Family::Icons };
    let w = px(WIDTH);
    let seg = text(12.0, FontWeight::SemiBold);
    let widths: Vec<f32> = Agent::ALL.iter().map(|a| m.measure(a.label(), &seg).w.ceil() + px(26.0)).collect();
    let track = Rect::new(px(14.0), px(12.0), widths.iter().sum::<f32>() + px(6.0), px(30.0));
    ops.push(DrawOp::RoundRect { rect: track, radius: track.h / 2.0, color: tokens.raised });
    let mut x = track.x + px(3.0);
    for (agent, sw) in Agent::ALL.iter().zip(&widths) {
        let r = Rect::new(x, track.y + px(3.0), *sw, track.h - px(6.0));
        let on = *agent == view.agent;
        if on {
            ops.push(DrawOp::RoundRect { rect: r, radius: r.h / 2.0, color: tokens.raised_strong });
        } else if view.hover == Some(Hit::Agent(*agent)) {
            ops.push(DrawOp::RoundRect { rect: r, radius: r.h / 2.0, color: tokens.raised });
        }
        ops.push(DrawOp::Text { rect: r, text: agent.label().into(), style: seg, color: if on { tokens.text } else { tokens.text_secondary }, align: Align::Center });
        hits.push((r, Hit::Agent(*agent)));
        x += sw;
    }
    for (i, (glyph, hit)) in [(GLYPH_NEW, Hit::NewChat), (GLYPH_HISTORY, Hit::History), (GLYPH_LIBRARY, Hit::Library)].into_iter().enumerate() {
        let r = Rect::new(w - px(14.0 + 30.0) - i as f32 * px(36.0), px(12.0), px(30.0), px(30.0));
        let lit = match hit {
            Hit::History => matches!(view.page, Page::History { .. }),
            Hit::Library => matches!(view.page, Page::Library { .. }),
            _ => false,
        };
        let fill = if lit { tokens.accent_soft } else if view.hover == Some(hit) { tokens.raised_strong } else { tokens.raised };
        ops.push(DrawOp::RoundRect { rect: r, radius: r.h / 2.0, color: fill });
        ops.push(DrawOp::Text { rect: r, text: glyph.into(), style: icons(12.0), color: if lit { tokens.accent } else { tokens.text_secondary }, align: Align::Center });
        hits.push((r, hit));
    }
    // Seletores.
    let ls = text(11.5, FontWeight::Regular);
    let mut x = px(14.0);
    for (label, hit) in [(view.origin, Hit::Origins), (view.model, Hit::Models)] {
        let tw = m.measure(label, &ls).w.ceil().min(px(150.0));
        let r = Rect::new(x, px(48.0), tw + px(32.0), px(24.0));
        let fill = if view.hover == Some(hit) { tokens.raised_strong } else { tokens.raised };
        ops.push(DrawOp::RoundRect { rect: r, radius: r.h / 2.0, color: fill });
        ops.push(DrawOp::Text { rect: Rect::new(r.x + px(10.0), r.y, tw + px(2.0), r.h), text: label.into(), style: ls, color: tokens.text_secondary, align: Align::Leading });
        ops.push(DrawOp::Text { rect: Rect::new(r.right() - px(20.0), r.y, px(12.0), r.h), text: GLYPH_CHEVRON.into(), style: icons(8.0), color: tokens.text_tertiary, align: Align::Center });
        hits.push((r, hit));
        x = r.right() + px(6.0);
    }
    if let Some(status) = view.status {
        ops.push(DrawOp::Text { rect: Rect::new(x + px(4.0), px(48.0), (w - x - px(18.0)).max(0.0), px(24.0)), text: status.into(), style: text(11.0, FontWeight::Regular), color: tokens.text_tertiary, align: Align::Leading });
    }
}

/// Título de uma página (com a seta de voltar).
fn draw_page_title(ops: &mut Vec<DrawOp>, hits: &mut Vec<(Rect, Hit)>, title: &str, hover: bool, scale: f32, tokens: &Tokens) -> f32 {
    let px = |v: f32| (v * scale).round();
    let w = px(WIDTH);
    let r = Rect::new(px(10.0), px(AREA_Y), w - px(20.0), px(PAGE_TITLE_H - 4.0));
    if hover {
        ops.push(DrawOp::RoundRect { rect: r, radius: px(8.0), color: tokens.raised });
    }
    ops.push(DrawOp::Text { rect: Rect::new(r.x + px(6.0), r.y, px(16.0), r.h), text: GLYPH_BACK.into(), style: TextStyle { size: 11.0 * scale, weight: FontWeight::Regular, family: Family::Icons }, color: tokens.text_secondary, align: Align::Center });
    ops.push(DrawOp::Text { rect: Rect::new(r.x + px(28.0), r.y, r.w - px(34.0), r.h), text: title.into(), style: TextStyle { size: 12.5 * scale, weight: FontWeight::SemiBold, family: Family::Text }, color: tokens.text, align: Align::Leading });
    hits.push((r, Hit::Back));
    px(AREA_Y + PAGE_TITLE_H)
}

pub fn chat_panel(view: &ChatView, scale: f32, tokens: &Tokens, m: &dyn TextMeasurer) -> ChatLayout {
    let px = |v: f32| (v * scale).round();
    let w = px(WIDTH);
    let mut ops = Vec::new();
    let mut hits = Vec::new();
    draw_header(&mut ops, &mut hits, view, scale, tokens, m);
    let line = LINE * scale;
    let text = |size: f32, weight: FontWeight| TextStyle { size: size * scale, weight, family: Family::Text };
    match &view.page {
        Page::Chat => chat_page(view, ops, hits, scale, tokens, m),
        Page::Picker { title, options, selected, note, prefer } => {
            let mut y = draw_page_title(&mut ops, &mut hits, title, view.hover == Some(Hit::Back), scale, tokens);
            for (i, option) in options.iter().enumerate() {
                let r = Rect::new(px(10.0), y, w - px(20.0), px(PICK_H));
                if view.hover == Some(Hit::Pick(i)) {
                    ops.push(DrawOp::RoundRect { rect: r, radius: px(8.0), color: tokens.raised });
                }
                let on = *selected == Some(i);
                if on {
                    ops.push(DrawOp::Text { rect: Rect::new(r.x + px(6.0), r.y, px(16.0), r.h), text: GLYPH_CHECK.into(), style: TextStyle { size: 11.0 * scale, weight: FontWeight::Regular, family: Family::Icons }, color: tokens.accent, align: Align::Center });
                }
                ops.push(DrawOp::Text { rect: Rect::new(r.x + px(28.0), r.y, r.w - px(34.0), r.h), text: option.clone(), style: text(12.5, if on { FontWeight::SemiBold } else { FontWeight::Regular }), color: tokens.text, align: Align::Leading });
                hits.push((r, Hit::Pick(i)));
                y += px(PICK_H);
            }
            if let Some(note) = note {
                ops.push(DrawOp::Text { rect: Rect::new(px(38.0), y + px(4.0), w - px(52.0), px(20.0)), text: (*note).into(), style: text(11.0, FontWeight::Regular), color: tokens.text_tertiary, align: Align::Leading });
                y += px(28.0);
            }
            if let Some(prefer) = prefer {
                y = draw_prefer(&mut ops, &mut hits, *prefer, view.hover, y, scale, tokens, m);
            }
            let size = Size { w, h: y + px(10.0) };
            ChatLayout { list: DisplayList { size, ops }, hits, max_scroll: 0.0, line }
        }
        Page::History { rows, scroll } => {
            let mut y = draw_page_title(&mut ops, &mut hits, "Conversas antigas", view.hover == Some(Hit::Back), scale, tokens);
            if rows.is_empty() {
                ops.push(DrawOp::Text { rect: Rect::new(px(38.0), y + px(4.0), w - px(52.0), px(20.0)), text: "Nenhuma conversa ainda.".into(), style: text(12.0, FontWeight::Regular), color: tokens.text_secondary, align: Align::Leading });
                y += px(32.0);
            }
            let scroll = (*scroll).min(history_max_scroll(rows.len()));
            for (i, row) in rows.iter().enumerate().skip(scroll).take(HISTORY_VISIBLE) {
                let r = Rect::new(px(10.0), y, w - px(20.0), px(HISTORY_H));
                let hovered = matches!(view.hover, Some(Hit::Open(j) | Hit::Delete(j)) if j == i);
                if hovered {
                    ops.push(DrawOp::RoundRect { rect: r, radius: px(10.0), color: tokens.raised });
                }
                if row.current {
                    ops.push(DrawOp::Circle { center: Point { x: r.x + px(14.0), y: r.y + px(15.0) }, radius: px(3.0), color: tokens.accent });
                }
                let text_w = r.w - px(28.0) - px(40.0);
                ops.push(DrawOp::Text { rect: Rect::new(r.x + px(28.0), r.y + px(5.0), text_w, px(20.0)), text: row.title.clone(), style: text(12.5, FontWeight::SemiBold), color: tokens.text, align: Align::Leading });
                ops.push(DrawOp::Text { rect: Rect::new(r.x + px(28.0), r.y + px(24.0), text_w, px(16.0)), text: row.meta.clone(), style: text(11.0, FontWeight::Regular), color: tokens.text_tertiary, align: Align::Leading });
                if hovered {
                    let d = Rect::new(r.right() - px(36.0), r.y + px(8.0), px(30.0), px(30.0));
                    if view.hover == Some(Hit::Delete(i)) {
                        ops.push(DrawOp::RoundRect { rect: d, radius: d.h / 2.0, color: tokens.raised_strong });
                    }
                    ops.push(DrawOp::Text { rect: d, text: GLYPH_DELETE.into(), style: TextStyle { size: 12.0 * scale, weight: FontWeight::Regular, family: Family::Icons }, color: tokens.text_secondary, align: Align::Center });
                }
                // A lixeira (só aparece com o mouse em cima) ganha da linha.
                hits.push((Rect::new(r.right() - px(36.0), r.y + px(8.0), px(30.0), px(30.0)), Hit::Delete(i)));
                hits.push((r, Hit::Open(i)));
                y += px(HISTORY_H);
            }
            let size = Size { w, h: y + px(10.0) };
            ChatLayout { list: DisplayList { size, ops }, hits, max_scroll: 0.0, line }
        }
        Page::Library { tiles, scroll } => {
            let y = library_page(&mut ops, &mut hits, tiles, *scroll, view.hover, scale, tokens, m);
            let size = Size { w, h: y + px(10.0) };
            ChatLayout { list: DisplayList { size, ops }, hits, max_scroll: 0.0, line }
        }
    }
}

/// A biblioteca de imagens: o título com "Abrir pasta" e a grade das fileiras à vista. Com o mouse
/// num quadro, o botão da conversa de origem aparece no canto dele. Devolve o fim da grade.
#[allow(clippy::too_many_arguments)]
fn library_page(ops: &mut Vec<DrawOp>, hits: &mut Vec<(Rect, Hit)>, tiles: &[LibraryTile], scroll: usize, hover: Option<Hit>, scale: f32, tokens: &Tokens, m: &dyn TextMeasurer) -> f32 {
    let px = |v: f32| (v * scale).round();
    let text = |size: f32, weight: FontWeight| TextStyle { size: size * scale, weight, family: Family::Text };
    let icons = |size: f32| TextStyle { size: size * scale, weight: FontWeight::Regular, family: Family::Icons };
    let w = px(WIDTH);
    let y = draw_page_title(ops, hits, "Imagens", hover == Some(Hit::Back), scale, tokens);
    // "Abrir pasta" no fim do título, por cima da seta de voltar.
    let label = "Abrir pasta";
    let ls = text(11.5, FontWeight::Regular);
    let bw = m.measure(label, &ls).w.ceil() + px(36.0);
    let folder = Rect::new(w - px(14.0) - bw, px(AREA_Y + 2.0), bw, px(PAGE_TITLE_H - 8.0));
    let fill = if hover == Some(Hit::OpenFolder) { tokens.raised_strong } else { tokens.raised };
    ops.push(DrawOp::RoundRect { rect: folder, radius: folder.h / 2.0, color: fill });
    ops.push(DrawOp::Text { rect: Rect::new(folder.x + px(8.0), folder.y, px(16.0), folder.h), text: GLYPH_FOLDER.into(), style: icons(11.0), color: tokens.text_secondary, align: Align::Center });
    ops.push(DrawOp::Text { rect: Rect::new(folder.x + px(28.0), folder.y, bw - px(32.0), folder.h), text: label.into(), style: ls, color: tokens.text_secondary, align: Align::Leading });
    hits.insert(0, (folder, Hit::OpenFolder));
    if tiles.is_empty() {
        ops.push(DrawOp::Text { rect: Rect::new(px(38.0), y + px(4.0), w - px(52.0), px(20.0)), text: "Nenhuma imagem ainda.".into(), style: text(12.0, FontWeight::Regular), color: tokens.text_secondary, align: Align::Leading });
        return y + px(32.0);
    }
    let gap = px(LIBRARY_GAP);
    let side = ((w - px(20.0) - gap * (LIBRARY_COLUMNS - 1) as f32) / LIBRARY_COLUMNS as f32).floor();
    let visible = library_visible(tiles.len(), scroll);
    let first = visible.start;
    let mut bottom = y;
    for i in visible {
        let (row, col) = ((i - first) / LIBRARY_COLUMNS, (i - first) % LIBRARY_COLUMNS);
        let cell = Rect::new(px(10.0) + col as f32 * (side + gap), y + row as f32 * (side + gap), side, side);
        let tile = &tiles[i];
        ops.push(DrawOp::RoundRect { rect: cell, radius: px(10.0), color: tokens.raised });
        if let Some(image) = &tile.image {
            ops.push(DrawOp::Image { rect: fit(image, cell), image: image.clone(), radius: px(8.0), opacity: 1.0 });
        }
        let tag = if tile.generated { "Gerada" } else { "Colada" };
        let ts = text(10.0, FontWeight::SemiBold);
        let tw = m.measure(tag, &ts).w.ceil() + px(12.0);
        let tr = Rect::new(cell.x + px(6.0), cell.bottom() - px(6.0 + 18.0), tw, px(18.0));
        ops.push(DrawOp::RoundRect { rect: tr, radius: tr.h / 2.0, color: tokens.raised_strong });
        ops.push(DrawOp::Text { rect: tr, text: tag.into(), style: ts, color: tokens.text_secondary, align: Align::Center });
        if matches!(hover, Some(Hit::Tile(j) | Hit::GoTo(j)) if j == i) {
            let go = Rect::new(cell.right() - px(6.0 + 26.0), cell.y + px(6.0), px(26.0), px(26.0));
            let on = hover == Some(Hit::GoTo(i));
            ops.push(DrawOp::Circle { center: Point { x: go.x + go.w / 2.0, y: go.y + go.h / 2.0 }, radius: go.w / 2.0, color: if on { tokens.accent } else { tokens.raised_strong } });
            ops.push(DrawOp::Text { rect: go, text: GLYPH_CHAT.into(), style: icons(11.0), color: if on { on_accent(tokens) } else { tokens.text }, align: Align::Center });
            hits.insert(0, (go, Hit::GoTo(i)));
        }
        hits.push((cell, Hit::Tile(i)));
        bottom = cell.bottom();
    }
    bottom
}

/// "No automático, primeiro: Windows | WSL", embaixo das origens. Devolve o fim da linha.
#[allow(clippy::too_many_arguments)]
fn draw_prefer(ops: &mut Vec<DrawOp>, hits: &mut Vec<(Rect, Hit)>, prefer: Prefer, hover: Option<Hit>, top: f32, scale: f32, tokens: &Tokens, m: &dyn TextMeasurer) -> f32 {
    let px = |v: f32| (v * scale).round();
    let w = px(WIDTH);
    let y = top + px(8.0);
    ops.push(DrawOp::RoundRect { rect: Rect::new(px(20.0), y, w - px(40.0), px(1.0)), radius: 0.0, color: tokens.hairline });
    let label = "No automático, primeiro:";
    let ls = TextStyle { size: 11.5 * scale, weight: FontWeight::Regular, family: Family::Text };
    let row = Rect::new(px(20.0), y + px(10.0), w - px(40.0), px(28.0));
    let lw = m.measure(label, &ls).w.ceil();
    ops.push(DrawOp::Text { rect: Rect::new(row.x + px(18.0), row.y, lw + px(2.0), row.h), text: label.into(), style: ls, color: tokens.text_secondary, align: Align::Leading });
    let seg = TextStyle { size: 11.5 * scale, weight: FontWeight::SemiBold, family: Family::Text };
    let widths: Vec<f32> = Prefer::ALL.iter().map(|p| m.measure(p.label(), &seg).w.ceil() + px(22.0)).collect();
    let track = Rect::new(row.x + px(18.0) + lw + px(10.0), row.y, widths.iter().sum::<f32>() + px(6.0), row.h);
    ops.push(DrawOp::RoundRect { rect: track, radius: track.h / 2.0, color: tokens.raised });
    let mut x = track.x + px(3.0);
    for (p, sw) in Prefer::ALL.iter().zip(&widths) {
        let r = Rect::new(x, track.y + px(3.0), *sw, track.h - px(6.0));
        let on = *p == prefer;
        if on {
            ops.push(DrawOp::RoundRect { rect: r, radius: r.h / 2.0, color: tokens.raised_strong });
        } else if hover == Some(Hit::Prefer(*p)) {
            ops.push(DrawOp::RoundRect { rect: r, radius: r.h / 2.0, color: tokens.raised });
        }
        ops.push(DrawOp::Text { rect: r, text: p.label().into(), style: seg, color: if on { tokens.text } else { tokens.text_secondary }, align: Align::Center });
        hits.push((r, Hit::Prefer(*p)));
        x += sw;
    }
    row.bottom()
}

/// A conversa, com rolagem, e o campo embaixo.
fn chat_page(view: &ChatView, mut ops: Vec<DrawOp>, mut hits: Vec<(Rect, Hit)>, scale: f32, tokens: &Tokens, m: &dyn TextMeasurer) -> ChatLayout {
    let px = |v: f32| (v * scale).round();
    let text = |size: f32, weight: FontWeight| TextStyle { size: size * scale, weight, family: Family::Text };
    let icons = |size: f32| TextStyle { size: size * scale, weight: FontWeight::Regular, family: Family::Icons };
    let body = text_style(scale);
    let note_style = text(11.0, FontWeight::Regular);
    let line = LINE * scale;
    let w = px(WIDTH);
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
        let hint = format!("Pergunte qualquer coisa ao {}", view.agent.label());
        ops.push(DrawOp::Text { rect: Rect::new(px(20.0), area.y + px(18.0), w - px(40.0), px(20.0)), text: hint, style: text(13.0, FontWeight::Regular), color: tokens.text_secondary, align: Align::Center });
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
    let attach_h = if view.attachments.is_empty() { 0.0 } else { px(ATTACH + 8.0) };
    let field = Rect::new(px(10.0), area.bottom() + px(10.0), w - px(20.0), shown as f32 * line + px(2.0 * 9.0) + attach_h);
    ops.push(DrawOp::RoundRect { rect: field, radius: px(14.0), color: tokens.raised });
    hits.push((field, Hit::Field));
    for (i, image) in view.attachments.iter().enumerate() {
        let cell = Rect::new(field.x + px(10.0) + i as f32 * px(ATTACH + 8.0), field.y + px(9.0), px(ATTACH), px(ATTACH));
        ops.push(DrawOp::RoundRect { rect: cell, radius: px(8.0), color: tokens.raised_strong });
        if let Some(image) = image {
            ops.push(DrawOp::Image { rect: fit(image, cell), image: image.clone(), radius: px(6.0), opacity: 1.0 });
        }
        let x = Rect::new(cell.right() - px(18.0), cell.y - px(4.0), px(22.0), px(22.0));
        let hovered = view.hover == Some(Hit::Unattach(i));
        ops.push(DrawOp::Circle { center: Point { x: x.x + x.w / 2.0, y: x.y + x.h / 2.0 }, radius: px(9.0), color: if hovered { tokens.accent } else { tokens.raised_strong } });
        ops.push(DrawOp::Text { rect: x, text: GLYPH_REMOVE.into(), style: icons(8.0), color: if hovered { on_accent(tokens) } else { tokens.text }, align: Align::Center });
        hits.insert(0, (x, Hit::Unattach(i)));
    }
    let tx = field.x + px(12.0);
    let ty = field.y + px(9.0) + attach_h;
    let tw = field_text_width(scale);
    let busy = view.messages.last().is_some_and(|m| m.role == Role::Claude && m.state.busy());
    if view.field.text.is_empty() {
        ops.push(DrawOp::Text { rect: Rect::new(tx, ty, tw, line), text: format!("Pergunte ao {}…", view.agent.label()), style: body, color: tokens.text_tertiary, align: Align::Leading });
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
    let attachments_ready = view.attachments.iter().all(Option::is_some);
    let (glyph, hit, lit) = if busy { (GLYPH_STOP, Hit::Stop, true) } else { (GLYPH_SEND, Hit::Send, attachments_ready && (!view.field.is_blank() || !view.attachments.is_empty())) };
    let hovered = view.hover == Some(hit);
    let fill = if lit { if hovered { tokens.accent } else { tokens.accent.with_alpha(tokens.accent.a * 0.85) } } else { tokens.raised_strong };
    ops.push(DrawOp::RoundRect { rect: button, radius: button.h / 2.0, color: fill });
    let fg = if lit { on_accent(tokens) } else { tokens.text_tertiary };
    ops.push(DrawOp::Text { rect: button, text: glyph.into(), style: icons(12.0), color: fg, align: Align::Center });
    hits.insert(0, (button, hit));

    let size = Size { w, h: field.bottom() + px(10.0) };
    ChatLayout { list: DisplayList { size, ops }, hits, max_scroll, line }
}

/// Resposta que terminou sem texto nem imagem (só o aviso do erro): sem balão.
fn bubbleless(m: &MessageView) -> bool {
    m.role == Role::Claude && !m.state.busy() && !has_text(m) && !has_pictures(m)
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
    let waiting = msg.state.busy() && !has_text(msg) && !has_pictures(msg);
    let lines = text_lines(msg);
    let cells = picture_cells(msg, scale);
    let (pictures_w, pictures_h) = pictures_size(&cells);
    let pictures_h = if has_pictures(msg) { pictures_h + if lines > 0 { px(PICTURE_GAP) } else { 0.0 } } else { 0.0 };
    let text_w = if msg.role == Role::User { msg.wrapped.widths.iter().copied().fold(0.0, f32::max).ceil() } else { text_width(Role::Claude, scale) };
    let bubble_w = (text_w.max(pictures_w) + px(2.0 * PAD_X)).min(w - px(24.0));
    let bubble_h = lines as f32 * line + px(2.0 * PAD_Y) + pictures_h;
    let x = if msg.role == Role::User { w - px(12.0) - bubble_w } else { px(12.0) };
    let bubble = Rect::new(x, y, bubble_w, bubble_h);
    let (fill, color) = match msg.role {
        Role::User => (tokens.accent_soft, tokens.text),
        Role::Claude => (tokens.raised, if msg.state == State::Failed { tokens.text_secondary } else { tokens.text }),
    };
    ops.push(DrawOp::RoundRect { rect: bubble, radius: px(12.0), color: fill });
    let tx = bubble.x + px(PAD_X);
    // Na pergunta as imagens vêm antes do texto; na resposta, depois.
    let (pictures_y, text_y) = match msg.role {
        Role::User => (bubble.y + px(PAD_Y), bubble.y + px(PAD_Y) + pictures_h),
        Role::Claude => (bubble.y + px(PAD_Y) + lines as f32 * line + if lines > 0 { px(PICTURE_GAP) } else { 0.0 }, bubble.y + px(PAD_Y)),
    };
    for (i, c) in cells.iter().enumerate() {
        let cell = Rect::new(tx + c.x, pictures_y + c.y, c.w, c.h);
        match msg.images.get(i) {
            Some(Some(image)) => {
                let shown = fit(image, cell);
                ops.push(DrawOp::Image { rect: shown, image: image.clone(), radius: px(8.0), opacity: 1.0 });
                // Só a parte visível da imagem recebe o clique.
                let top = shown.y.max(area.y);
                let bottom = shown.bottom().min(area.bottom());
                if bottom > top {
                    let visible = if top == shown.y && bottom == shown.bottom() { shown } else { Rect::new(shown.x, top, shown.w, bottom - top) };
                    hits.push((visible, Hit::Picture(index, i)));
                }
            }
            Some(None) => ops.push(DrawOp::RoundRect { rect: cell, radius: px(8.0), color: tokens.raised_strong }),
            None => {
                ops.push(DrawOp::RoundRect { rect: cell, radius: px(8.0), color: tokens.raised_strong });
                ops.push(DrawOp::Text { rect: cell, text: "Gerando imagem…".into(), style: TextStyle { size: 11.5 * scale, weight: FontWeight::Regular, family: Family::Text }, color: tokens.text_tertiary, align: Align::Center });
            }
        }
    }
    if waiting {
        let label = if msg.state == State::Thinking { "Pensando…" } else { "…" };
        ops.push(DrawOp::Text { rect: Rect::new(tx, text_y, bubble.w - px(2.0 * PAD_X), line), text: label.into(), style: body, color: tokens.text_tertiary, align: Align::Leading });
    } else {
        for (row, s) in msg.wrapped.lines.iter().enumerate() {
            let ly = text_y + row as f32 * line;
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

    fn view<'a>(messages: &'a [MessageView<'a>], field: &'a Field, lines: &'a [Line], page: Page<'a>) -> ChatView<'a> {
        ChatView { messages, field, field_lines: lines, attachments: &[], agent: Agent::Claude, origin: "Windows", model: "Sonnet", status: Some("iniciando…"), page, scroll: 0.0, hover: None, copied: None }
    }

    fn img(w: u32, h: u32) -> Image {
        Image::new(w, h, vec![255; (w * h * 4) as usize]).unwrap()
    }

    fn images(l: &ChatLayout) -> Vec<Rect> {
        l.list.ops.iter().filter_map(|o| if let DrawOp::Image { rect, .. } = o { Some(*rect) } else { None }).collect()
    }

    #[test]
    fn attachments_sit_in_the_field_and_can_be_removed() {
        let f = Field::default();
        let lines = wrap("", field_text_width(1.0), &text_style(1.0), &FixedWidthMeasurer);
        let plain = chat_panel(&view(&[], &f, &lines, Page::Chat), 1.0, &tokens(), &FixedWidthMeasurer);
        let atts = [Some(img(200, 100)), None];
        let mut v = view(&[], &f, &lines, Page::Chat);
        v.attachments = &atts;
        let l = chat_panel(&v, 1.0, &tokens(), &FixedWidthMeasurer);
        assert!(l.list.size.h > plain.list.size.h, "as miniaturas fazem o campo crescer");
        let shown = images(&l);
        assert_eq!(shown.len(), 1, "a que ainda está convertendo não tem imagem");
        assert!((shown[0].w / shown[0].h - 2.0).abs() < 0.05, "mantém a proporção");
        assert!(has(&l, Hit::Unattach(0)) && has(&l, Hit::Unattach(1)));
        let field = l.hits.iter().find(|(_, h)| *h == Hit::Field).unwrap().0;
        assert!(field.contains(Point { x: shown[0].x + 1.0, y: shown[0].y + 1.0 }), "dentro do campo");
        let x = l.hits.iter().find(|(_, h)| *h == Hit::Unattach(0)).unwrap().0;
        assert_eq!(l.hit(Point { x: x.x + x.w / 2.0, y: x.y + x.h / 2.0 }), Some(Hit::Unattach(0)), "o × ganha do campo");
        assert!(!has(&l, Hit::Stop));
        let send_lit = |l: &ChatLayout| {
            let r = l.hits.iter().find(|(_, h)| *h == Hit::Send).unwrap().0;
            l.list.ops.iter().any(|o| matches!(o, DrawOp::RoundRect { rect, color, .. } if *rect == r && *color != tokens().raised_strong))
        };
        assert!(!send_lit(&l), "imagem ainda convertendo: não envia");
        let ready = [Some(img(50, 50))];
        v.attachments = &ready;
        assert!(send_lit(&chat_panel(&v, 1.0, &tokens(), &FixedWidthMeasurer)), "só a imagem já basta para enviar");
    }

    #[test]
    fn question_images_show_in_the_bubble() {
        let text_only = wrapped("Que número é este?");
        let blank = Wrapped { lines: vec![String::new()], widths: vec![0.0] };
        let pics = [Some(img(160, 90))];
        let with_text = [MessageView { role: Role::User, state: State::Done, wrapped: &text_only, note: None, images: &pics, generating: false }];
        let only_image = [MessageView { role: Role::User, state: State::Done, wrapped: &blank, note: None, images: &pics, generating: false }];
        let no_image = [MessageView { role: Role::User, state: State::Done, wrapped: &text_only, note: None, images: &[], generating: false }];
        let a = panel(&with_text, &Field::default(), 0.0);
        let b = panel(&only_image, &Field::default(), 0.0);
        let c = panel(&no_image, &Field::default(), 0.0);
        assert_eq!(images(&a).len(), 1);
        assert!(texts(&a.list).contains(&"Que número é este?".to_string()));
        assert!(a.list.size.h > c.list.size.h, "a imagem ocupa espaço no balão");
        assert!(b.list.size.h < a.list.size.h, "só imagem: sem a linha de texto");
        let bubble = a.list.ops.iter().find_map(|o| if let DrawOp::RoundRect { rect, color, .. } = o { (*color == tokens().accent_soft).then_some(*rect) } else { None }).unwrap();
        let pic = images(&a)[0];
        assert!(bubble.contains(Point { x: pic.x + 1.0, y: pic.y + 1.0 }) && pic.right() <= bubble.right(), "a imagem fica dentro do balão");
    }

    fn reply<'a>(wrapped: &'a Wrapped, images: &'a [Option<Image>], state: State, generating: bool) -> MessageView<'a> {
        MessageView { role: Role::Claude, state, wrapped, note: None, images, generating }
    }

    fn hit_rect(l: &ChatLayout, hit: Hit) -> Option<Rect> {
        l.hits.iter().find(|(_, h)| *h == hit).map(|(r, _)| *r)
    }

    #[test]
    fn generated_images_fill_the_reply_bubble() {
        let text = wrapped("Pronto! Bonsai numa mesa gamer.");
        let wide = [Some(img(1536, 1024))];
        let tall = [Some(img(500, 1000))];
        let with = panel(&[reply(&text, &wide, State::Done, false)], &Field::default(), 0.0);
        let without = panel(&[reply(&text, &[], State::Done, false)], &Field::default(), 0.0);
        let pic = images(&with)[0];
        assert!((pic.w - text_width(Role::Claude, 1.0)).abs() < 1.0, "na largura do balão: {pic:?}");
        assert!((pic.w / pic.h - 1.5).abs() < 0.02, "mantém a proporção");
        assert!(with.list.size.h - without.list.size.h >= pic.h, "a imagem ocupa espaço");
        let bubble = with.list.ops.iter().find_map(|o| if let DrawOp::RoundRect { rect, color, .. } = o { (*color == tokens().raised && rect.x == 12.0).then_some(*rect) } else { None }).unwrap();
        assert!(bubble.contains(Point { x: pic.x + 1.0, y: pic.y + 1.0 }) && pic.right() <= bubble.right() && pic.bottom() <= bubble.bottom(), "dentro do balão");
        let line = with.list.ops.iter().find_map(|o| if let DrawOp::Text { rect, text, .. } = o { (text.starts_with("Pronto!")).then_some(*rect) } else { None }).unwrap();
        assert!(line.bottom() <= pic.y, "o texto vem antes da imagem");
        assert_eq!(hit_rect(&with, Hit::Picture(0, 0)), Some(pic), "clique abre a imagem");
        assert!(has(&with, Hit::Copy(0)));
        let t = images(&panel(&[reply(&text, &tall, State::Done, false)], &Field::default(), 0.0))[0];
        assert!(t.h <= REPLY_PICTURE_H && (t.w / t.h - 0.5).abs() < 0.02, "alta: altura limitada, proporção mantida");
    }

    #[test]
    fn reply_with_only_an_image_still_has_a_bubble() {
        let blank = Wrapped { lines: vec![String::new()], widths: vec![0.0] };
        let wide = [Some(img(300, 200))];
        let l = panel(&[reply(&blank, &wide, State::Done, false)], &Field::default(), 0.0);
        assert_eq!(images(&l).len(), 1, "não some como resposta sem texto");
        assert!(has(&l, Hit::Picture(0, 0)));
    }

    #[test]
    fn image_being_generated_has_a_placeholder() {
        let text = wrapped("Vou gerar a imagem.");
        let l = panel(&[reply(&text, &[], State::Streaming, true)], &Field::default(), 0.0);
        let plain = panel(&[reply(&text, &[], State::Streaming, false)], &Field::default(), 0.0);
        assert!(texts(&l.list).contains(&"Gerando imagem…".to_string()));
        assert!(l.list.size.h > plain.list.size.h + 100.0, "o lugar da imagem já fica reservado");
        assert!(!l.hits.iter().any(|(_, h)| matches!(h, Hit::Picture(..))), "nada para abrir ainda");
        let loading = [None];
        let l = panel(&[reply(&text, &loading, State::Done, false)], &Field::default(), 0.0);
        assert!(!texts(&l.list).contains(&"Gerando imagem…".to_string()) && !has(&l, Hit::Picture(0, 0)), "carregando do disco: só o lugar");
    }

    #[test]
    fn question_images_open_on_click() {
        let q = wrapped("e isto?");
        let pics = [Some(img(160, 90)), Some(img(90, 90))];
        let l = panel(&[MessageView { role: Role::User, state: State::Done, wrapped: &q, note: None, images: &pics, generating: false }], &Field::default(), 0.0);
        let shown = images(&l);
        assert_eq!(hit_rect(&l, Hit::Picture(0, 1)), Some(shown[1]));
        assert_eq!(l.hit(Point { x: shown[0].x + 2.0, y: shown[0].y + 2.0 }), Some(Hit::Picture(0, 0)));
    }

    fn panel(messages: &[MessageView], field: &Field, scroll: f32) -> ChatLayout {
        let lines = wrap(&field.text, field_text_width(1.0), &text_style(1.0), &FixedWidthMeasurer);
        let mut v = view(messages, field, &lines, Page::Chat);
        v.scroll = scroll;
        chat_panel(&v, 1.0, &tokens(), &FixedWidthMeasurer)
    }

    fn has(l: &ChatLayout, hit: Hit) -> bool {
        l.hits.iter().any(|(_, h)| *h == hit)
    }

    #[test]
    fn empty_conversation_shows_the_hint_header_and_pickers() {
        let l = panel(&[], &Field::default(), 0.0);
        let t = texts(&l.list);
        for s in ["Claude", "Codex", "Windows", "Sonnet", "iniciando…", "Pergunte qualquer coisa ao Claude", "Pergunte ao Claude…", GLYPH_SEND, GLYPH_HISTORY, GLYPH_NEW] {
            assert!(t.contains(&s.to_string()), "falta {s}");
        }
        for hit in [Hit::Send, Hit::NewChat, Hit::History, Hit::Origins, Hit::Models, Hit::Agent(Agent::Claude), Hit::Agent(Agent::Codex)] {
            assert!(has(&l, hit), "falta {hit:?}");
        }
        assert_eq!(l.list.size.w, WIDTH);
    }

    #[test]
    fn three_agents_fit_before_the_header_buttons() {
        let l = panel(&[], &Field::default(), 0.0);
        let rect = |hit: Hit| l.hits.iter().find(|(_, h)| *h == hit).map(|(r, _)| *r).unwrap();
        assert!(texts(&l.list).contains(&"Grok".to_string()));
        let grok = rect(Hit::Agent(Agent::Grok));
        assert!(rect(Hit::Agent(Agent::Codex)).right() <= grok.x, "Grok depois do Codex");
        let first_button = [Hit::NewChat, Hit::History, Hit::Library].map(|h| rect(h).x).into_iter().fold(f32::MAX, f32::min);
        assert!(grok.right() + 8.0 <= first_button, "{} vs {first_button}", grok.right());
    }

    #[test]
    fn codex_hint_uses_its_name() {
        let lines = wrap("", field_text_width(1.0), &text_style(1.0), &FixedWidthMeasurer);
        let f = Field::default();
        let mut v = view(&[], &f, &lines, Page::Chat);
        v.agent = Agent::Codex;
        let t = texts(&chat_panel(&v, 1.0, &tokens(), &FixedWidthMeasurer).list);
        assert!(t.contains(&"Pergunte ao Codex…".to_string()) && t.contains(&"Pergunte qualquer coisa ao Codex".to_string()));
    }

    #[test]
    fn picker_page_lists_options_with_a_check_and_no_field() {
        let lines = [Line { start: 0, end: 0 }];
        let f = Field::default();
        let options = vec!["Automático".to_string(), "Windows".to_string(), "Ubuntu-24.04".to_string()];
        let v = view(&[], &f, &lines, Page::Picker { title: "Origem do Claude", options: &options, selected: Some(2), note: Some("Procurando distros…"), prefer: None });
        let l = chat_panel(&v, 1.0, &tokens(), &FixedWidthMeasurer);
        let t = texts(&l.list);
        for s in ["Origem do Claude", "Ubuntu-24.04", "Procurando distros…", GLYPH_CHECK, GLYPH_BACK] {
            assert!(t.contains(&s.to_string()), "falta {s}");
        }
        assert!(has(&l, Hit::Pick(0)) && has(&l, Hit::Pick(2)) && has(&l, Hit::Back));
        assert!(!has(&l, Hit::Field) && !has(&l, Hit::Send), "sem campo nas escolhas");
        assert!(!has(&l, Hit::Prefer(Prefer::Wsl)), "sem a chave do automático no seletor de modelo");
    }

    #[test]
    fn origin_picker_has_the_auto_preference_switch() {
        let lines = [Line { start: 0, end: 0 }];
        let f = Field::default();
        let options = vec!["Automático".to_string(), "Windows".to_string()];
        let picker = |prefer| Page::Picker { title: "Origem do Codex", options: &options, selected: Some(0), note: None, prefer: Some(prefer) };
        let without = chat_panel(&view(&[], &f, &lines, Page::Picker { title: "Origem do Codex", options: &options, selected: Some(0), note: None, prefer: None }), 1.0, &tokens(), &FixedWidthMeasurer);
        let l = chat_panel(&view(&[], &f, &lines, picker(Prefer::Wsl)), 1.0, &tokens(), &FixedWidthMeasurer);
        let t = texts(&l.list);
        assert!(t.contains(&"No automático, primeiro:".to_string()) && t.contains(&"WSL".to_string()));
        assert!(has(&l, Hit::Prefer(Prefer::Windows)) && has(&l, Hit::Prefer(Prefer::Wsl)));
        assert!(l.list.size.h > without.list.size.h, "a chave ocupa uma linha a mais");
        let lit = |l: &ChatLayout, p: Prefer| {
            let r = l.hits.iter().find(|(_, h)| *h == Hit::Prefer(p)).unwrap().0;
            l.list.ops.iter().any(|o| matches!(o, DrawOp::RoundRect { rect, color, .. } if *rect == r && *color == tokens().raised_strong))
        };
        assert!(lit(&l, Prefer::Wsl) && !lit(&l, Prefer::Windows), "a escolhida fica acesa");
        let w = chat_panel(&view(&[], &f, &lines, picker(Prefer::Windows)), 1.0, &tokens(), &FixedWidthMeasurer);
        assert!(lit(&w, Prefer::Windows) && !lit(&w, Prefer::Wsl));
    }

    #[test]
    fn history_page_lists_conversations_and_scrolls() {
        let lines = [Line { start: 0, end: 0 }];
        let f = Field::default();
        let rows: Vec<HistoryRow> = (0..12).map(|i| HistoryRow { title: format!("pergunta {i}"), meta: format!("Codex · Windows · {i}"), current: i == 0 }).collect();
        let v = view(&[], &f, &lines, Page::History { rows: &rows, scroll: 0 });
        let l = chat_panel(&v, 1.0, &tokens(), &FixedWidthMeasurer);
        let t = texts(&l.list);
        assert!(t.contains(&"Conversas antigas".to_string()) && t.contains(&"pergunta 0".to_string()) && t.contains(&"Codex · Windows · 0".to_string()));
        assert!(!t.contains(&"pergunta 9".to_string()), "só {HISTORY_VISIBLE} por vez");
        assert!(has(&l, Hit::Open(0)) && has(&l, Hit::Delete(0)));
        let open = l.hits.iter().find(|(_, h)| *h == Hit::Open(0)).unwrap().0;
        let trash = l.hits.iter().find(|(_, h)| *h == Hit::Delete(0)).unwrap().0;
        assert_eq!(l.hit(Point { x: trash.x + 2.0, y: trash.y + 2.0 }), Some(Hit::Delete(0)), "a lixeira ganha da linha");
        assert_eq!(l.hit(Point { x: open.x + 30.0, y: open.y + 10.0 }), Some(Hit::Open(0)));
        let v = view(&[], &f, &lines, Page::History { rows: &rows, scroll: 99 });
        let l = chat_panel(&v, 1.0, &tokens(), &FixedWidthMeasurer);
        assert!(has(&l, Hit::Open(11)) && !has(&l, Hit::Open(2)), "rolagem presa no fim");
        assert_eq!(history_max_scroll(12), 3);
        let v = view(&[], &f, &lines, Page::History { rows: &[], scroll: 0 });
        assert!(texts(&chat_panel(&v, 1.0, &tokens(), &FixedWidthMeasurer).list).contains(&"Nenhuma conversa ainda.".to_string()));
    }

    #[test]
    fn header_has_the_library_button() {
        let l = panel(&[], &Field::default(), 0.0);
        assert!(has(&l, Hit::Library) && texts(&l.list).contains(&GLYPH_LIBRARY.to_string()));
        let library = hit_rect(&l, Hit::Library).unwrap();
        let history = hit_rect(&l, Hit::History).unwrap();
        assert!(library.right() <= history.x, "ao lado de Conversas antigas, sem encostar");
    }

    fn tiles(n: usize) -> Vec<LibraryTile> {
        (0..n).map(|i| LibraryTile { image: (i != 1).then(|| img(300, 200)), generated: i % 2 == 0 }).collect()
    }

    #[test]
    fn library_page_is_a_grid_with_tags_and_buttons() {
        let lines = [Line { start: 0, end: 0 }];
        let f = Field::default();
        let all = tiles(11);
        let v = view(&[], &f, &lines, Page::Library { tiles: &all, scroll: 0 });
        let l = chat_panel(&v, 1.0, &tokens(), &FixedWidthMeasurer);
        let t = texts(&l.list);
        for s in ["Imagens", "Gerada", "Colada", "Abrir pasta", GLYPH_BACK] {
            assert!(t.contains(&s.to_string()), "falta {s}");
        }
        assert!(has(&l, Hit::Back) && has(&l, Hit::OpenFolder) && !has(&l, Hit::Field), "sem campo na biblioteca");
        let shown = LIBRARY_COLUMNS * LIBRARY_ROWS;
        assert!(has(&l, Hit::Tile(shown - 1)) && !has(&l, Hit::Tile(shown)), "só {LIBRARY_ROWS} fileiras à vista");
        let (a, b, d) = (hit_rect(&l, Hit::Tile(0)).unwrap(), hit_rect(&l, Hit::Tile(1)).unwrap(), hit_rect(&l, Hit::Tile(3)).unwrap());
        assert!(b.x >= a.right() && (b.y - a.y).abs() < 0.5, "3 por fileira");
        assert!(d.y >= a.bottom() && (d.x - a.x).abs() < 0.5, "a 4ª abre a fileira de baixo");
        assert!((a.w - a.h).abs() < 0.5, "quadros quadrados");
        assert_eq!(images(&l).len(), shown - 1, "a que ainda carrega fica sem imagem");
        assert!(!has(&l, Hit::GoTo(0)), "o botão da conversa só aparece com o mouse em cima");
        let mut v = view(&[], &f, &lines, Page::Library { tiles: &all, scroll: 0 });
        v.hover = Some(Hit::Tile(4));
        let l = chat_panel(&v, 1.0, &tokens(), &FixedWidthMeasurer);
        let go = hit_rect(&l, Hit::GoTo(4)).unwrap();
        assert!(hit_rect(&l, Hit::Tile(4)).unwrap().contains(Point { x: go.x + 1.0, y: go.y + 1.0 }), "dentro do quadro");
        assert_eq!(l.hit(Point { x: go.x + 2.0, y: go.y + 2.0 }), Some(Hit::GoTo(4)), "o botão ganha do quadro");
        let v = view(&[], &f, &lines, Page::Library { tiles: &all, scroll: 99 });
        let l = chat_panel(&v, 1.0, &tokens(), &FixedWidthMeasurer);
        assert!(has(&l, Hit::Tile(10)) && has(&l, Hit::Tile(3)) && !has(&l, Hit::Tile(2)), "rolagem presa no fim");
        assert_eq!(library_max_scroll(11), 1);
        assert_eq!(library_max_scroll(9), 0);
        assert_eq!(library_visible(11, 1), 3..11);
        let v = view(&[], &f, &lines, Page::Library { tiles: &[], scroll: 0 });
        assert!(texts(&chat_panel(&v, 1.0, &tokens(), &FixedWidthMeasurer).list).contains(&"Nenhuma imagem ainda.".to_string()));
    }

    #[test]
    fn conversation_with_waiting_reply_shows_stop_and_thinking() {
        let q = wrapped("Qual a capital do Peru?");
        let a = Wrapped::default();
        let msgs = [MessageView { role: Role::User, state: State::Done, wrapped: &q, note: None, images: &[], generating: false }, MessageView { role: Role::Claude, state: State::Thinking, wrapped: &a, note: None, images: &[], generating: false }];
        let l = panel(&msgs, &Field::default(), 0.0);
        let t = texts(&l.list);
        assert!(t.contains(&"Qual a capital do Peru?".to_string()) && t.contains(&"Pensando…".to_string()));
        assert!(l.hits.iter().any(|(_, h)| *h == Hit::Stop) && !l.hits.iter().any(|(_, h)| *h == Hit::Send));
        assert!(!l.hits.iter().any(|(_, h)| matches!(h, Hit::Copy(_))), "sem copiar enquanto responde");
    }

    #[test]
    fn failed_reply_without_text_is_just_the_note() {
        let a = Wrapped { lines: vec![String::new()], widths: vec![0.0] };
        let msgs = [MessageView { role: Role::Claude, state: State::Failed, wrapped: &a, note: Some("Entre no Claude Code."), images: &[], generating: false }];
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
        let msgs = [MessageView { role: Role::Claude, state: State::Stopped, wrapped: &a, note: Some("Resposta interrompida."), images: &[], generating: false }];
        let l = panel(&msgs, &Field::default(), 0.0);
        assert!(texts(&l.list).contains(&"Resposta interrompida.".to_string()));
        assert!(l.hits.iter().any(|(_, h)| *h == Hit::Copy(0)));
    }

    #[test]
    fn long_conversation_scrolls_and_culls() {
        let text = (0..60).map(|i| format!("linha {i}")).collect::<Vec<_>>().join("\n");
        let a = wrapped(&text);
        let msgs = [MessageView { role: Role::Claude, state: State::Done, wrapped: &a, note: None, images: &[], generating: false }];
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
