//! Desenho do painel do Spotify (estado em `spotify.rs`): campo de busca no topo, trilha ou
//! filtros, as linhas (cartão da música atual, entradas, itens), o menu de ações por cima e o rodapé
//! com os atalhos. Devolve as áreas de clique e as capas que ainda faltam.

use zefiro_integration_spotify::api::Kind;
use zefiro_integration_spotify::auth::REDIRECT_URI;
use zefiro_integration_spotify::model::Item;
use zefiro_platform::color::Rgba;
use zefiro_platform::geometry::{Point, Rect, Size};
use zefiro_platform::image::Image;
use zefiro_platform::text::{Align, Family, FontWeight, TextMeasurer, TextStyle};
use zefiro_resources::icons;
use zefiro_resources::tokens::Tokens;
use zefiro_ui::paint::{DisplayList, DrawOp};

use crate::chips::ChipContent;
use crate::devices::with_svg_icon;
use crate::spotify::{Auth, Entry, Filter, MENU_VISIBLE, MenuTarget, Row, Screen, SpotifyPanel, SubMenu, VISIBLE};

pub const WIDTH: f32 = 400.0;
/// Verde do Spotify (#1ED760), a cor do logo oficial.
pub const SPOTIFY_GREEN: Rgba = Rgba { r: 30.0 / 255.0, g: 215.0 / 255.0, b: 96.0 / 255.0, a: 1.0 };
/// Onde se cria o app (passo 1 da configuração).
pub const DASHBOARD_URL: &str = "https://developer.spotify.com/dashboard";
const ROW_H: f32 = 44.0;
const CARD_H: f32 = 68.0;
const LABEL_H: f32 = 24.0;
const COVER: f32 = 36.0;
const FIELD_Y: f32 = 12.0;
const FIELD_H: f32 = 36.0;
const FOOTER_H: f32 = 30.0;
const MENU_W: f32 = 256.0;
const MENU_ROW: f32 = 30.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CardButton { Previous, PlayPause, Next, Like }

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Hit { Row(usize), Filter(Filter), Back, MenuItem(usize), Card(CardButton), ClientField, CopyRedirect, SaveClientId, Login, OpenDashboard }

/// O desenho, as áreas de clique (o menu primeiro: ele fica por cima) e as URLs de capa sem imagem.
pub struct Drawn { pub list: DisplayList, pub hits: Vec<(Rect, Hit)>, pub missing: Vec<String> }

/// O chip da barra: o logo oficial do Spotify, no verde da marca, no lugar do glifo.
pub fn spotify_chip(chip: ChipContent) -> ChipContent {
    let mut chip = with_svg_icon(chip, icons::SPOTIFY);
    if let Some(DrawOp::Svg { color, .. }) = chip.list.ops.first_mut() {
        *color = SPOTIFY_GREEN;
    }
    chip
}

/// `216000` → `3:36`; com hora, `1:02:03`.
pub fn mmss(ms: u64) -> String {
    let s = ms / 1000;
    if s >= 3600 { format!("{}:{:02}:{:02}", s / 3600, s / 60 % 60, s % 60) } else { format!("{}:{:02}", s / 60, s % 60) }
}

struct Ctx<'a> {
    scale: f32,
    t: &'a Tokens,
    measure: &'a dyn TextMeasurer,
    covers: &'a dyn Fn(&str) -> Option<Image>,
    list: DisplayList,
    hits: Vec<(Rect, Hit)>,
    menu_hits: Vec<(Rect, Hit)>,
    missing: Vec<String>,
}

impl Ctx<'_> {
    fn px(&self, v: f32) -> f32 {
        (v * self.scale).round()
    }

    fn style(&self, size: f32, weight: FontWeight) -> TextStyle {
        TextStyle { size: size * self.scale, weight, family: Family::Text }
    }

    fn icon_style(&self, size: f32) -> TextStyle {
        TextStyle { size: size * self.scale, weight: FontWeight::Regular, family: Family::Icons }
    }

    fn text(&mut self, rect: Rect, text: impl Into<String>, size: f32, weight: FontWeight, color: Rgba, align: Align) {
        let style = self.style(size, weight);
        self.list.push(DrawOp::Text { rect, text: text.into(), style, color, align });
    }

    fn glyph(&mut self, rect: Rect, glyph: &str, size: f32, color: Rgba) {
        let style = self.icon_style(size);
        self.list.push(DrawOp::Text { rect, text: glyph.into(), style, color, align: Align::Center });
    }

    fn rect(&mut self, rect: Rect, radius: f32, color: Rgba) {
        self.list.push(DrawOp::RoundRect { rect, radius, color });
    }

    /// Capa (ou quadrado do tema enquanto ela não chega); artistas são redondos.
    fn cover(&mut self, rect: Rect, url: Option<&str>, round: bool) {
        let radius = if round { rect.w / 2.0 } else { self.px(6.0) };
        match url {
            Some(url) => match (self.covers)(url) {
                Some(image) => self.list.push(DrawOp::Image { rect, image, radius, opacity: 1.0 }),
                None => {
                    if !self.missing.iter().any(|m| m == url) {
                        self.missing.push(url.to_string());
                    }
                    self.rect(rect, radius, self.t.raised_strong);
                }
            },
            None => self.rect(rect, radius, self.t.raised_strong),
        }
    }

    /// Botão com texto centrado; devolve o retângulo.
    fn button(&mut self, rect: Rect, label: &str, hit: Hit, strong: bool) {
        let (bg, fg) = if strong { (self.t.accent, Rgba::rgb(255, 255, 255)) } else { (self.t.raised_strong, self.t.text) };
        self.rect(rect, rect.h / 2.0, bg);
        self.text(rect, label, 12.0, FontWeight::SemiBold, fg, Align::Center);
        self.hits.push((rect, hit));
    }
}

fn entry_glyph(e: Entry) -> &'static str {
    match e {
        Entry::Library => "\u{E8F1}",
        Entry::Queue => "\u{E8FD}",
        Entry::Devices => "\u{E7F4}",
        Entry::Liked => "\u{EB52}",
        Entry::Playlists => "\u{E90B}",
        Entry::Albums => "\u{E93C}",
        Entry::Artists => "\u{E77B}",
        Entry::Shows => "\u{E720}",
    }
}

fn play_label(kind: Option<Kind>) -> &'static str {
    match kind {
        Some(Kind::Album) => "Tocar o álbum",
        Some(Kind::Artist) => "Tocar o artista",
        Some(Kind::Show) => "Tocar o podcast",
        _ => "Tocar a playlist",
    }
}

fn group_label(kind: Kind) -> &'static str {
    match kind {
        Kind::Track => "Músicas",
        Kind::Artist => "Artistas",
        Kind::Album => "Álbuns",
        Kind::Playlist => "Playlists",
        Kind::Show => "Podcasts",
        Kind::Episode => "Episódios",
        Kind::Device => "Aparelhos",
    }
}

fn footer_hint(p: &SpotifyPanel) -> &'static str {
    if p.menu.is_some() {
        return "↑↓ escolher · ↵ confirmar · Esc fechar menu";
    }
    match p.stack.last().map(|l| &l.screen) {
        Some(Screen::Search) => "↵ tocar · Ctrl+↵ fila · Tab ações · Ctrl+Tab filtro",
        Some(Screen::Library) => "↵ abrir · Esc voltar",
        Some(Screen::List { .. }) => "↵ tocar · Ctrl+↵ fila · Tab ações · Esc voltar",
        _ => "↵ abrir · Tab ações · Espaço tocar/pausar · Esc fechar",
    }
}

pub fn spotify_panel(p: &SpotifyPanel, covers: &dyn Fn(&str) -> Option<Image>, measure: &dyn TextMeasurer, scale: f32, tokens: &Tokens) -> Drawn {
    let mut c = Ctx { scale, t: tokens, measure, covers, list: DisplayList::new(Size { w: (WIDTH * scale).round(), h: 0.0 }), hits: Vec::new(), menu_hits: Vec::new(), missing: Vec::new() };
    let bottom = if p.auth == Auth::Ready { ready(&mut c, p) } else { setup(&mut c, p) };
    let footer_y = bottom + c.px(4.0);
    let w = c.list.size.w;
    let (note, color) = match (&p.message, &p.toast) {
        (Some(m), _) => (m.clone(), c.t.text),
        (None, Some(t)) => (t.clone(), c.t.text_secondary),
        (None, None) => (if p.auth == Auth::Ready { footer_hint(p).to_string() } else { String::new() }, c.t.text_tertiary),
    };
    c.rect(Rect::new(c.px(14.0), footer_y, w - c.px(28.0), c.px(1.0)), 0.0, c.t.hairline);
    let fr = Rect::new(c.px(16.0), footer_y + c.px(4.0), w - c.px(32.0), c.px(FOOTER_H) - c.px(8.0));
    c.text(fr, note, 11.0, FontWeight::Regular, color, Align::Center);
    let mut h = footer_y + c.px(FOOTER_H);
    if let Some(menu_bottom) = c.menu_hits.iter().map(|(r, _)| r.bottom()).reduce(f32::max) {
        h = h.max(menu_bottom + c.px(8.0));
    }
    c.list.size.h = h;
    let mut hits = std::mem::take(&mut c.menu_hits);
    hits.append(&mut c.hits);
    Drawn { list: c.list, hits, missing: c.missing }
}

/// Telas de configuração e de login. Devolve o fim do conteúdo.
fn setup(c: &mut Ctx, p: &SpotifyPanel) -> f32 {
    let w = c.list.size.w;
    let x = c.px(20.0);
    let inner = w - c.px(40.0);
    let logo = Rect::new(x, c.px(15.0), c.px(20.0), c.px(20.0));
    c.list.push(DrawOp::Svg { rect: logo, svg: icons::SPOTIFY, color: SPOTIFY_GREEN });
    c.text(Rect::new(logo.right() + c.px(8.0), c.px(14.0), inner - c.px(28.0), c.px(22.0)), "Spotify", 13.0, FontWeight::SemiBold, c.t.text_secondary, Align::Leading);
    let mut y = c.px(44.0);
    let line = |c: &mut Ctx, y: &mut f32, text: &str| {
        let r = Rect::new(x, *y, inner, c.px(18.0));
        c.text(r, text, 12.0, FontWeight::Regular, c.t.text, Align::Leading);
        *y += c.px(22.0);
    };
    match p.auth {
        Auth::NeedsClientId => {
            // Passo 1 com o botão que abre o painel do desenvolvedor no navegador.
            c.text(Rect::new(x, y, inner - c.px(176.0), c.px(18.0)), "1. Crie um app com a Web API em", 12.0, FontWeight::Regular, c.t.text, Align::Leading);
            let open = Rect::new(x + inner - c.px(168.0), y - c.px(5.0), c.px(168.0), c.px(28.0));
            c.button(open, "developer.spotify.com ↗", Hit::OpenDashboard, false);
            y += c.px(30.0);
            line(c, &mut y, "2. Cadastre este Redirect URI no app:");
            let boxr = Rect::new(x, y, inner, c.px(32.0));
            c.rect(boxr, c.px(10.0), c.t.raised);
            c.text(Rect::new(boxr.x + c.px(10.0), boxr.y, boxr.w - c.px(90.0), boxr.h), REDIRECT_URI, 12.0, FontWeight::Regular, c.t.text, Align::Leading);
            let copy = Rect::new(boxr.right() - c.px(74.0), boxr.y + c.px(4.0), c.px(68.0), boxr.h - c.px(8.0));
            c.button(copy, "Copiar", Hit::CopyRedirect, false);
            y += c.px(42.0);
            line(c, &mut y, "3. Cole o Client ID do app e salve.");
            let field = Rect::new(x, y, inner - c.px(86.0), c.px(32.0));
            c.rect(field, c.px(10.0), c.t.raised);
            let tr = Rect::new(field.x + c.px(10.0), field.y, field.w - c.px(20.0), field.h);
            if p.client_field.is_empty() {
                c.text(tr, "Client ID", 12.0, FontWeight::Regular, c.t.text_tertiary, Align::Leading);
            } else {
                c.text(tr, p.client_field.clone(), 12.0, FontWeight::Regular, c.t.text, Align::Leading);
            }
            let caret_x = tr.x + if p.client_field.is_empty() { 0.0 } else { c.measure.measure(&p.client_field, &c.style(12.0, FontWeight::Regular)).w.min(tr.w) };
            c.rect(Rect::new(caret_x, field.y + c.px(8.0), c.px(1.5), field.h - c.px(16.0)), c.px(0.75), c.t.accent);
            c.hits.push((field, Hit::ClientField));
            let save = Rect::new(field.right() + c.px(8.0), field.y, c.px(78.0), field.h);
            c.button(save, "Salvar", Hit::SaveClientId, true);
            y + c.px(44.0)
        }
        Auth::Waiting => {
            line(c, &mut y, "Termine o login no navegador…");
            y + c.px(6.0)
        }
        _ => {
            if p.auth == Auth::PortBusy {
                line(c, &mut y, "Não consegui abrir uma porta local para o login");
            }
            line(c, &mut y, "Entre na sua conta do Spotify (Premium) para");
            line(c, &mut y, "buscar e tocar direto daqui.");
            let b = Rect::new(x, y + c.px(4.0), c.px(180.0), c.px(32.0));
            c.button(b, "Entrar com o Spotify", Hit::Login, true);
            y + c.px(46.0)
        }
    }
}

fn ready(c: &mut Ctx, p: &SpotifyPanel) -> f32 {
    let w = c.list.size.w;
    let Some(layer) = p.stack.last() else { return c.px(FIELD_Y + FIELD_H) };
    // Campo de busca.
    let field = Rect::new(c.px(10.0), c.px(FIELD_Y), w - c.px(20.0), c.px(FIELD_H));
    c.rect(field, c.px(12.0), c.t.raised);
    c.glyph(Rect::new(field.x + c.px(10.0), field.y, c.px(18.0), field.h), "\u{E721}", 13.0, c.t.text_tertiary);
    let tr = Rect::new(field.x + c.px(36.0), field.y, field.w - c.px(46.0), field.h);
    let text = p.field_text();
    let hint = match &layer.screen {
        Screen::Root | Screen::Search => "Buscar no Spotify…".to_string(),
        Screen::Library => "Filtrar Sua biblioteca…".to_string(),
        Screen::List { title, .. } => format!("Filtrar {title}…"),
    };
    if text.is_empty() {
        c.text(tr, hint, 13.5, FontWeight::Regular, c.t.text_tertiary, Align::Leading);
    } else {
        c.text(tr, text, 13.5, FontWeight::Regular, c.t.text, Align::Leading);
    }
    let caret_x = tr.x + if text.is_empty() { 0.0 } else { c.measure.measure(text, &c.style(13.5, FontWeight::Regular)).w.min(tr.w) };
    c.rect(Rect::new(caret_x, field.y + c.px(9.0), c.px(1.5), field.h - c.px(18.0)), c.px(0.75), c.t.accent);
    let mut y = field.bottom() + c.px(8.0);

    // Trilha ou filtros.
    match &layer.screen {
        Screen::Library | Screen::List { .. } => {
            let names: Vec<String> = p.stack.iter().skip(1).filter_map(|l| match &l.screen {
                Screen::Library => Some("Sua biblioteca".to_string()),
                Screen::List { title, .. } => Some(title.clone()),
                Screen::Search => Some(format!("\u{201C}{}\u{201D}", p.query)),
                Screen::Root => None,
            }).collect();
            let r = Rect::new(c.px(14.0), y, w - c.px(28.0), c.px(22.0));
            c.text(Rect::new(r.x, r.y, c.px(18.0), r.h), "‹", 15.0, FontWeight::SemiBold, c.t.text_secondary, Align::Leading);
            c.text(Rect::new(r.x + c.px(18.0), r.y, r.w - c.px(18.0), r.h), names.join(" / "), 11.5, FontWeight::Regular, c.t.text_secondary, Align::Leading);
            c.hits.push((r, Hit::Back));
            y += c.px(26.0);
        }
        Screen::Search => {
            let mut x = c.px(12.0);
            let style = c.style(11.5, FontWeight::SemiBold);
            for f in Filter::ALL {
                let tw = c.measure.measure(f.label(), &style).w + c.px(22.0);
                let r = Rect::new(x, y, tw, c.px(24.0));
                let on = p.filter == f;
                c.rect(r, r.h / 2.0, if on { c.t.text } else { c.t.raised });
                c.text(r, f.label(), 11.5, FontWeight::SemiBold, if on { c.t.fill.with_alpha(1.0) } else { c.t.text_secondary }, Align::Center);
                c.hits.push((r, Hit::Filter(f)));
                x += tw + c.px(6.0);
            }
            y += c.px(32.0);
        }
        Screen::Root => {}
    }

    // Linhas.
    let rows = p.rows();
    let items = p.items();
    let empty_note = if layer.items.is_none() {
        Some("Carregando…".to_string())
    } else if let Some(e) = layer.error.filter(|_| items.is_empty()) {
        Some(crate::spotify::error_message(e))
    } else if rows.is_empty() {
        Some("Nenhum resultado".to_string())
    } else {
        None
    };
    if let Some(note) = empty_note.filter(|_| rows.is_empty()) {
        c.text(Rect::new(c.px(20.0), y + c.px(6.0), w - c.px(40.0), c.px(20.0)), note, 12.5, FontWeight::Regular, c.t.text_secondary, Align::Leading);
        return y + c.px(36.0);
    }
    let mut selected_rect = None;
    let mut last_kind: Option<Kind> = None;
    let mut recent_label = false;
    for (index, row) in rows.iter().enumerate().skip(layer.scroll).take(VISIBLE) {
        // Rótulos de grupo (busca "Tudo") e de "Tocadas há pouco".
        if let Row::Item(i) = row
            && layer.screen == Screen::Search
            && p.filter == Filter::All
            && let Some(item) = items.get(*i)
            && last_kind != Some(item.kind)
        {
            last_kind = Some(item.kind);
            c.text(Rect::new(c.px(16.0), y + c.px(4.0), w - c.px(32.0), c.px(16.0)), group_label(item.kind).to_uppercase(), 10.5, FontWeight::SemiBold, c.t.text_tertiary, Align::Leading);
            y += c.px(LABEL_H);
        }
        if let Row::Recent(_) = row
            && !recent_label
        {
            recent_label = true;
            c.text(Rect::new(c.px(16.0), y + c.px(4.0), w - c.px(32.0), c.px(16.0)), "Tocadas há pouco", 11.0, FontWeight::SemiBold, c.t.text_tertiary, Align::Leading);
            y += c.px(LABEL_H);
        }
        let h = c.px(if *row == Row::Card { CARD_H } else { ROW_H });
        let r = Rect::new(c.px(8.0), y, w - c.px(16.0), h);
        let lit = index == layer.selected;
        if lit {
            selected_rect = Some(r);
        }
        draw_row(c, p, *row, r, lit);
        c.hits.push((r, Hit::Row(index)));
        y += h + c.px(if *row == Row::Card { 6.0 } else { 0.0 });
    }
    if let Some(menu) = &p.menu {
        // O menu do cartão abre de um atalho também (sem linha escolhida): fica sob o campo.
        let anchor = match menu.target {
            MenuTarget::Card if !matches!(rows.get(layer.selected), Some(Row::Card)) => None,
            _ => selected_rect,
        };
        draw_menu(c, p, anchor.unwrap_or(Rect::new(0.0, field.bottom(), w, 0.0)), y);
    }
    y
}

fn draw_row(c: &mut Ctx, p: &SpotifyPanel, row: Row, r: Rect, lit: bool) {
    let t = c.t;
    match row {
        Row::Card => {
            c.rect(r, c.px(14.0), if lit { t.raised_strong } else { t.raised });
            let item = p.player.as_ref().and_then(|pl| pl.item.as_ref());
            let Some(item) = item else {
                c.glyph(Rect::new(r.x + c.px(10.0), r.y + c.px(10.0), c.px(48.0), c.px(48.0)), "\u{EC4F}", 18.0, t.text_tertiary);
                c.text(Rect::new(r.x + c.px(68.0), r.y, r.w - c.px(80.0), r.h), "Nada tocando", 12.5, FontWeight::Regular, t.text_secondary, Align::Leading);
                return;
            };
            let player = p.player.as_ref().expect("item vem do player");
            c.cover(Rect::new(r.x + c.px(10.0), r.y + c.px(10.0), c.px(48.0), c.px(48.0)), item.image.as_deref(), false);
            let tx = r.x + c.px(68.0);
            let buttons_w = c.px(124.0);
            let tw = r.w - c.px(76.0) - buttons_w;
            c.text(Rect::new(tx, r.y + c.px(9.0), tw, c.px(18.0)), item.name.clone(), 12.5, FontWeight::SemiBold, t.text, Align::Leading);
            let artist = item.artists.iter().map(|a| a.name.as_str()).collect::<Vec<_>>().join(", ");
            let who = if artist.is_empty() { item.show.as_ref().map(|s| s.name.clone()).unwrap_or_default() } else { artist };
            let sub = match player.device.as_ref() {
                Some(d) => format!("{who} · {}", d.name),
                None => who,
            };
            c.text(Rect::new(tx, r.y + c.px(27.0), tw, c.px(16.0)), sub, 11.0, FontWeight::Regular, t.text_secondary, Align::Leading);
            // Progresso.
            let progress = if player.playing { player.progress_ms + p.now_ms.saturating_sub(p.player_at_ms) } else { player.progress_ms };
            let total = item.duration_ms.unwrap_or(0);
            let progress = if total > 0 { progress.min(total) } else { progress };
            let bar = Rect::new(tx, r.y + c.px(50.0), tw - c.px(78.0), c.px(3.0));
            c.rect(bar, bar.h / 2.0, t.raised_strong);
            if total > 0 {
                c.rect(Rect::new(bar.x, bar.y, bar.w * progress as f32 / total as f32, bar.h), bar.h / 2.0, t.text);
            }
            c.text(Rect::new(bar.right() + c.px(6.0), r.y + c.px(44.0), c.px(34.0), c.px(14.0)), mmss(progress), 10.0, FontWeight::Regular, t.text_tertiary, Align::Leading);
            c.text(Rect::new(bar.right() + c.px(40.0), r.y + c.px(44.0), c.px(36.0), c.px(14.0)), mmss(total), 10.0, FontWeight::Regular, t.text_tertiary, Align::Leading);
            // Botões.
            let liked = matches!(&p.liked, Some((u, true)) if *u == item.uri);
            let glyphs = [(CardButton::Previous, "\u{E892}", t.text), (CardButton::PlayPause, if player.playing { "\u{E769}" } else { "\u{E768}" }, t.text), (CardButton::Next, "\u{E893}", t.text), (CardButton::Like, if liked { "\u{EB52}" } else { "\u{EB51}" }, if liked { t.accent } else { t.text_secondary })];
            let mut bx = r.right() - buttons_w - c.px(4.0);
            for (button, glyph, color) in glyphs {
                if button == CardButton::Like && item.kind != Kind::Track {
                    continue;
                }
                let br = Rect::new(bx, r.y + (r.h - c.px(30.0)) / 2.0, c.px(30.0), c.px(30.0));
                c.glyph(br, glyph, 14.0, color);
                c.hits.push((br, Hit::Card(button)));
                bx += c.px(31.0);
            }
        }
        _ => {
            if lit {
                c.rect(r, c.px(10.0), t.raised);
            }
            let cover = Rect::new(r.x + c.px(8.0), r.y + (r.h - c.px(COVER)) / 2.0, c.px(COVER), c.px(COVER));
            let tx = cover.right() + c.px(10.0);
            let (title, subtitle, trailing) = match row {
                Row::Entry(e) => {
                    c.rect(cover, c.px(8.0), t.raised_strong);
                    c.glyph(cover, entry_glyph(e), 15.0, t.text_secondary);
                    let sub = match e {
                        Entry::Library => "Curtidas, playlists, álbuns, artistas, podcasts".to_string(),
                        Entry::Queue => p.up_next.as_ref().map_or("Nada a seguir".into(), |n| format!("A seguir: {n}")),
                        Entry::Devices => p.player.as_ref().and_then(|pl| pl.device.as_ref()).map_or("Nenhum aparelho tocando".into(), |d| format!("Tocando no {}", d.name)),
                        _ => String::new(),
                    };
                    (e.title().to_string(), sub, "›".to_string())
                }
                Row::Play => {
                    c.rect(cover, c.px(8.0), t.raised_strong);
                    c.glyph(cover, "\u{E768}", 14.0, t.accent);
                    let kind = match p.stack.last().map(|l| &l.screen) {
                        Some(Screen::List { parent: Some(parent), .. }) => Some(parent.kind),
                        _ => None,
                    };
                    (play_label(kind).to_string(), "⇧↵ no aleatório".to_string(), String::new())
                }
                Row::Note => {
                    c.text(Rect::new(r.x + c.px(10.0), r.y, r.w - c.px(20.0), r.h), "O Spotify não mostra as músicas desta playlist", 12.0, FontWeight::Regular, t.text_tertiary, Align::Leading);
                    return;
                }
                Row::Recent(i) => item_parts(c, p.recent.get(i), cover),
                Row::Item(i) => item_parts(c, p.items().get(i), cover),
                Row::Card => unreachable!("tratado acima"),
            };
            let trailing_w = if trailing.is_empty() { 0.0 } else { c.px(52.0) };
            let tw = r.right() - tx - c.px(10.0) - trailing_w;
            if subtitle.is_empty() {
                c.text(Rect::new(tx, r.y, tw, r.h), title, 12.5, FontWeight::SemiBold, t.text, Align::Leading);
            } else {
                c.text(Rect::new(tx, r.y + c.px(5.0), tw, c.px(18.0)), title, 12.5, FontWeight::SemiBold, t.text, Align::Leading);
                c.text(Rect::new(tx, r.y + c.px(23.0), tw, c.px(16.0)), subtitle, 11.0, FontWeight::Regular, t.text_tertiary, Align::Leading);
            }
            if !trailing.is_empty() {
                let size = if trailing == "›" { 15.0 } else { 11.0 };
                c.text(Rect::new(r.right() - c.px(10.0) - trailing_w, r.y, trailing_w, r.h), trailing, size, FontWeight::Regular, t.text_tertiary, Align::Trailing);
            }
        }
    }
}

/// Capa, título, subtítulo e o que vai à direita (duração ou `›`) de um item.
fn item_parts(c: &mut Ctx, item: Option<&Item>, cover: Rect) -> (String, String, String) {
    let Some(item) = item else { return Default::default() };
    match item.kind {
        Kind::Device => {
            c.rect(cover, c.px(8.0), c.t.raised_strong);
            c.glyph(cover, if item.subtitle.starts_with("Celular") { "\u{E8EA}" } else { "\u{E7F4}" }, 15.0, if item.active { c.t.accent } else { c.t.text_secondary });
        }
        kind => c.cover(cover, item.image.as_deref(), kind == Kind::Artist),
    }
    let trailing = match item.kind {
        Kind::Track | Kind::Episode => item.duration_ms.map(mmss).unwrap_or_default(),
        Kind::Device => String::new(),
        _ => "›".into(),
    };
    (item.name.clone(), item.subtitle.clone(), trailing)
}

fn draw_menu(c: &mut Ctx, p: &SpotifyPanel, anchor: Rect, content_bottom: f32) {
    let Some(menu) = &p.menu else { return };
    let lines: Vec<(String, String, bool)> = match &menu.sub {
        Some(SubMenu::Volume) => p.volume_steps().iter().enumerate().map(|(i, s)| (match i {
            5 => format!("−{} %", -s),
            6 => format!("+{s} %"),
            _ => format!("{s} %"),
        }, String::new(), true)).collect(),
        Some(SubMenu::Playlists(None)) => vec![("Carregando…".into(), String::new(), false)],
        Some(SubMenu::Playlists(Some(list))) if list.is_empty() => vec![("Nenhuma playlist sua".into(), String::new(), false)],
        Some(SubMenu::Playlists(Some(list))) => list.iter().map(|pl| (pl.name.clone(), String::new(), true)).collect(),
        None => menu.actions.iter().map(|a| {
            let (label, key) = p.action_label(*a, menu.target);
            (label, key.to_string(), true)
        }).collect(),
    };
    let w = c.list.size.w;
    let pad = c.px(6.0);
    let shown = lines.len().saturating_sub(menu.scroll).min(MENU_VISIBLE);
    let h = shown as f32 * c.px(MENU_ROW) + pad * 2.0;
    let x = w - c.px(10.0) - c.px(MENU_W);
    let below = anchor.bottom() + c.px(4.0);
    // Abaixo da linha se couber até o fim das linhas (o painel cresce no máximo um pouco); senão,
    // acima dela, sem cobrir o campo.
    let y = if below + h <= content_bottom + c.px(ROW_H) { below } else { (anchor.y - h - c.px(4.0)).max(c.px(FIELD_Y + FIELD_H + 6.0)) };
    let box_r = Rect::new(x, y, c.px(MENU_W), h);
    c.rect(box_r, c.px(12.0), c.t.fill.with_alpha(1.0));
    c.rect(box_r, c.px(12.0), c.t.raised_strong);
    c.list.push(DrawOp::RoundRectStroke { rect: box_r, radius: c.px(12.0), color: c.t.hairline, width: c.px(1.0) });
    for (row, (i, (label, key, enabled))) in lines.into_iter().enumerate().skip(menu.scroll).take(MENU_VISIBLE).enumerate() {
        let r = Rect::new(x + pad, y + pad + row as f32 * c.px(MENU_ROW), c.px(MENU_W) - pad * 2.0, c.px(MENU_ROW));
        if enabled && i == menu.selected {
            c.rect(r, c.px(8.0), c.t.raised_strong);
        }
        let color = if enabled { c.t.text } else { c.t.text_tertiary };
        c.text(Rect::new(r.x + c.px(10.0), r.y, r.w - c.px(80.0), r.h), label, 12.0, FontWeight::Regular, color, Align::Leading);
        if !key.is_empty() {
            c.text(Rect::new(r.right() - c.px(70.0), r.y, c.px(60.0), r.h), key, 10.5, FontWeight::Regular, c.t.text_tertiary, Align::Trailing);
        }
        if enabled {
            c.menu_hits.push((r, Hit::MenuItem(i)));
        }
    }
}

/// Área de clique sob `p` (o menu primeiro).
pub fn hit_at(hits: &[(Rect, Hit)], p: Point) -> Option<Hit> {
    hits.iter().find(|(r, _)| r.contains(p)).map(|(_, h)| *h)
}
