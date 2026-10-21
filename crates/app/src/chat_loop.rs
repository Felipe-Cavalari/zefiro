//! Chat com o Claude no laço de eventos: o painel do chip `chat`, o campo (teclas e `WM_CHAR`), a
//! rolagem, os botões e os sinais do processo `claude` (spec 2026-10-21-chat-claude). O texto da
//! conversa nunca vai para o log.

use std::time::Instant;
use windows::core::Result;
use zefiro_bar::chat::{ChatLayout, ChatView, Edit, Field, Hit, Key, Line, MessageView, Wrapped, chat_panel, field_text_width, plain_text, text_style, text_width, wrap, wrap_text};
use zefiro_bar::controller::PanelKind;
use zefiro_core::config::ChatConfig;
use zefiro_integration_chat::conversation::{Conversation, Role};
use zefiro_integration_chat::origin::parse_choice;
use zefiro_integration_chat::protocol::{Event, Failure, classify, user_line};
use zefiro_platform::geometry::Point;
use zefiro_platform_windows::chat::{ChatService, Signal, Start, new_session_id, windows_dir};
use zefiro_platform_windows::{system_events, window};
use zefiro_ui::paint::DisplayList;

use crate::event_loop::{Loop, TIMER_CHAT_IDLE, USER_CHAT};

/// Sem uso por este tempo, o processo do `claude` sai (a sessão continua gravada).
const IDLE_MS: u32 = 10 * 60 * 1000;
/// Linhas por passo da roda.
const WHEEL_LINES: f32 = 3.0;

/// Linhas quebradas de uma mensagem, refeitas só quando o texto ou a escala mudam.
struct CachedWrap { len: usize, scale: f32, wrapped: Wrapped }

#[derive(Default)]
pub struct ChatRuntime {
    service: Option<ChatService>,
    /// Configuração com que o serviço foi criado: mudou, o processo recomeça.
    config: Option<ChatConfig>,
    /// Lida do disco na primeira vez que o painel abre.
    conversation: Option<Conversation>,
    field: Field,
    field_lines: Vec<Line>,
    scale: f32,
    wraps: Vec<CachedWrap>,
    /// Pixels rolados a partir do fim.
    scroll: f32,
    layout: Option<ChatLayout>,
    hover: Option<Hit>,
    copied: Option<usize>,
    /// Ao lado do título: a origem ou o que está acontecendo.
    subtitle: Option<String>,
    /// Chegou resposta com o painel fechado.
    unread: bool,
    asked_at: Option<Instant>,
    first_text: bool,
    /// Já tentou de novo nesta pergunta (sessão sumida ou em uso).
    retried: bool,
}

fn file() -> std::path::PathBuf {
    windows_dir().join("conversa.json")
}

impl Loop {
    fn chat_open(&self) -> bool {
        self.controller.open().is_some_and(|o| o.kind == PanelKind::Chat)
    }

    fn chat_conversation(&mut self) -> &mut Conversation {
        self.chat.conversation.get_or_insert_with(|| std::fs::read_to_string(file()).ok().and_then(|t| Conversation::from_json(&t)).unwrap_or_else(|| Conversation::new(new_session_id())))
    }

    fn chat_start_spec(&mut self) -> Start {
        let config = self.ctx.config.chat.clone();
        let c = self.chat_conversation();
        Start { choice: parse_choice(&config.origin), model: config.model, session: c.session.clone(), resume: c.created, windows_dir: windows_dir() }
    }

    /// Cria o serviço (a thread) e pede o processo, se ainda não está rodando.
    fn chat_start(&mut self) {
        if self.chat.service.is_none() {
            match ChatService::new(USER_CHAT) {
                Ok(s) => {
                    self.chat.service = Some(s);
                    self.chat.config = Some(self.ctx.config.chat.clone());
                }
                Err(e) => {
                    tracing::warn!("chat: não consegui criar a thread do chat: {e}");
                    return;
                }
            }
        }
        let spec = self.chat_start_spec();
        if let Some(s) = &self.chat.service {
            s.start(spec);
        }
        window::set_timer(TIMER_CHAT_IDLE, IDLE_MS);
    }

    fn chat_save(&self) {
        let Some(c) = &self.chat.conversation else { return };
        let path = file();
        let result = if c.messages.is_empty() {
            match std::fs::remove_file(&path) {
                Err(e) if e.kind() != std::io::ErrorKind::NotFound => Err(e),
                _ => Ok(()),
            }
        } else {
            std::fs::create_dir_all(windows_dir()).and_then(|_| std::fs::write(&path, c.to_json()))
        };
        if let Err(e) = result {
            tracing::warn!("chat: não consegui gravar a conversa: {e}");
        }
    }

    /// O ponto no chip: respondendo ou resposta não lida.
    fn chat_update_badge(&mut self) -> Result<()> {
        let busy = self.chat.conversation.as_ref().is_some_and(Conversation::busy);
        let badge = busy || self.chat.unread;
        if badge != self.ctx.system.chat_badge {
            self.ctx.system.chat_badge = badge;
            self.apply_bars(true)?;
        }
        Ok(())
    }

    /// Ao abrir o painel: lê a conversa e já sobe o processo (a primeira resposta sai mais rápido).
    pub fn chat_prepare(&mut self, kind: PanelKind) -> Result<()> {
        if kind != PanelKind::Chat {
            return Ok(());
        }
        self.chat_conversation();
        let c = &mut self.chat;
        c.unread = false;
        c.hover = None;
        c.copied = None;
        c.scroll = 0.0;
        if c.subtitle.is_none() {
            c.subtitle = Some("iniciando…".into());
        }
        self.chat_start();
        self.chat_update_badge()
    }

    pub fn chat_panel_list(&mut self, scale: f32) -> DisplayList {
        self.chat_conversation();
        let measure = &self.ctx.text;
        let c = &mut self.chat;
        let Some(conv) = c.conversation.as_ref() else { return DisplayList::default() };
        c.wraps.truncate(conv.messages.len());
        for (i, m) in conv.messages.iter().enumerate() {
            if c.wraps.get(i).is_some_and(|w| w.len == m.text.len() && w.scale == scale) {
                continue;
            }
            let text = if m.role == Role::Claude { plain_text(&m.text) } else { m.text.clone() };
            let entry = CachedWrap { len: m.text.len(), scale, wrapped: wrap_text(&text, text_width(m.role, scale), &text_style(scale), measure) };
            if i < c.wraps.len() {
                c.wraps[i] = entry;
            } else {
                c.wraps.push(entry);
            }
        }
        c.field_lines = wrap(&c.field.text, field_text_width(scale), &text_style(scale), measure);
        c.scale = scale;
        let views: Vec<MessageView> = conv.messages.iter().zip(&c.wraps).map(|(m, w)| MessageView { role: m.role, state: m.state, wrapped: &w.wrapped, note: m.note.as_deref() }).collect();
        let tokens = &self.ctx.theme.tokens;
        let mut view = ChatView { messages: &views, field: &c.field, field_lines: &c.field_lines, subtitle: c.subtitle.as_deref(), scroll: c.scroll, hover: c.hover, copied: c.copied };
        let mut layout = chat_panel(&view, scale, tokens, measure);
        // Rolado para cima enquanto a resposta cresce embaixo: o que está na tela fica parado.
        let before = c.layout.as_ref().map_or(layout.max_scroll, |l| l.max_scroll);
        if c.scroll > 0.0 && layout.max_scroll > before {
            c.scroll += layout.max_scroll - before;
            view.scroll = c.scroll;
            layout = chat_panel(&view, scale, tokens, measure);
        }
        c.scroll = c.scroll.min(layout.max_scroll);
        let list = layout.list.clone();
        c.layout = Some(layout);
        list
    }

    fn chat_copy(&self, text: &str) {
        if let Some(open) = self.controller.open()
            && !system_events::set_clipboard_text(self.panels[open.monitor].hwnd(), text)
        {
            tracing::warn!("chat: não consegui copiar para a área de transferência");
        }
    }

    fn chat_send(&mut self) -> Result<()> {
        let text = self.chat.field.text.clone();
        let c = self.chat_conversation();
        if !c.ask(&text) {
            return Ok(());
        }
        let question = c.messages[c.messages.len() - 2].text.clone();
        let chat = &mut self.chat;
        chat.field.clear();
        chat.scroll = 0.0;
        chat.copied = None;
        chat.retried = false;
        chat.first_text = false;
        chat.asked_at = Some(Instant::now());
        tracing::info!(caracteres = question.chars().count(), "chat: pergunta enviada");
        self.chat_start();
        if let Some(s) = &self.chat.service {
            s.send(user_line(&question));
        }
        self.chat_save();
        self.chat_update_badge()?;
        self.redraw_open_panel()
    }

    fn chat_stop(&mut self) -> Result<()> {
        if let Some(c) = &mut self.chat.conversation
            && c.busy()
        {
            c.stop_requested();
            if let Some(s) = &self.chat.service {
                s.interrupt();
            }
            tracing::info!("chat: parando a resposta");
        }
        Ok(())
    }

    fn chat_new(&mut self) -> Result<()> {
        if let Some(s) = &self.chat.service {
            s.stop();
        }
        let c = &mut self.chat;
        c.conversation = Some(Conversation::new(new_session_id()));
        c.wraps.clear();
        c.layout = None;
        c.scroll = 0.0;
        c.copied = None;
        c.hover = None;
        c.unread = false;
        tracing::info!("chat: conversa nova");
        self.chat_save();
        if self.chat_open() {
            self.chat_start();
        }
        self.chat_update_badge()?;
        self.redraw_open_panel()
    }

    /// Teclas com o painel do chat aberto. `true` se tratou (o Esc segue e fecha o painel).
    pub fn chat_key(&mut self, vk: u32) -> Result<bool> {
        if !self.chat_open() {
            return Ok(false);
        }
        let ctrl = system_events::ctrl_down();
        let page = self.chat.layout.as_ref().map_or(0.0, |l| l.line * 10.0);
        let max = self.chat.layout.as_ref().map_or(0.0, |l| l.max_scroll);
        let f = &mut self.chat.field;
        let changed = match vk {
            0x0D if system_events::shift_down() => f.key(Key::NewLine),
            0x0D => return self.chat_send().map(|_| true),
            0x25 => f.key(if ctrl { Key::WordLeft } else { Key::Left }),
            0x27 => f.key(if ctrl { Key::WordRight } else { Key::Right }),
            0x24 => f.key(Key::Home),
            0x23 => f.key(Key::End),
            0x2E => f.key(Key::Delete),
            0x26 | 0x28 => {
                let style = text_style(self.chat.scale.max(1.0));
                self.chat.field.vertical(&self.chat.field_lines, vk == 0x26, &style, &self.ctx.text)
            }
            // PageUp / PageDown rolam a conversa.
            0x21 | 0x22 => {
                let c = &mut self.chat;
                c.scroll = if vk == 0x21 { (c.scroll + page).min(max) } else { (c.scroll - page).max(0.0) };
                true
            }
            0x09 => false,
            _ => return Ok(false),
        };
        if changed {
            self.redraw_open_panel()?;
        }
        Ok(true)
    }

    /// `WM_CHAR` com o painel do chat aberto. `true` se tratou.
    pub fn chat_char(&mut self, code: u32) -> Result<bool> {
        if !self.chat_open() {
            return Ok(false);
        }
        let paste = if code == 22 { system_events::clipboard_text_full() } else { None };
        match self.chat.field.char(code, paste.as_deref()) {
            Edit::Changed => self.redraw_open_panel()?,
            Edit::Copy(text) => self.chat_copy(&text),
            Edit::Cut(text) => {
                self.chat_copy(&text);
                self.redraw_open_panel()?;
            }
            // Ctrl+C sem seleção no campo: copia a última resposta.
            Edit::Nothing if code == 3 => {
                let last = self.chat.conversation.as_ref().and_then(|c| c.messages.iter().rposition(|m| m.role == Role::Claude && !m.text.is_empty()));
                if let Some(i) = last {
                    let text = self.chat.conversation.as_ref().map(|c| c.messages[i].text.clone()).unwrap_or_default();
                    self.chat_copy(&text);
                    self.chat.copied = Some(i);
                    self.redraw_open_panel()?;
                }
            }
            Edit::Nothing => {}
        }
        Ok(true)
    }

    /// Roda sobre o painel. `true` se tratou.
    pub fn chat_wheel(&mut self, delta: i32) -> Result<bool> {
        if !self.chat_open() {
            return Ok(false);
        }
        let (step, max) = self.chat.layout.as_ref().map_or((0.0, 0.0), |l| (l.line * WHEEL_LINES, l.max_scroll));
        let notches = delta as f32 / 120.0;
        let next = (self.chat.scroll + notches * step).clamp(0.0, max);
        if (next - self.chat.scroll).abs() > 0.5 {
            self.chat.scroll = next;
            self.redraw_open_panel()?;
        }
        Ok(true)
    }

    fn chat_hit(&self, p: Point) -> Option<Hit> {
        self.chat.layout.as_ref().and_then(|l| l.hit(p)).filter(|h| *h != Hit::Field)
    }

    pub fn chat_move(&mut self, p: Point) -> Result<()> {
        if !self.chat_open() {
            return Ok(());
        }
        let hover = self.chat_hit(p);
        if hover != self.chat.hover {
            self.chat.hover = hover;
            self.redraw_open_panel()?;
        }
        Ok(())
    }

    pub fn chat_leave(&mut self) -> Result<()> {
        if self.chat_open() && self.chat.hover.take().is_some() {
            self.redraw_open_panel()?;
        }
        Ok(())
    }

    /// Clique no painel: enviar, parar, conversa nova ou copiar. `true` se o painel é o do chat.
    pub fn chat_click(&mut self, p: Point) -> Result<bool> {
        if !self.chat_open() {
            return Ok(false);
        }
        match self.chat_hit(p) {
            Some(Hit::Send) => self.chat_send()?,
            Some(Hit::Stop) => self.chat_stop()?,
            Some(Hit::NewChat) => self.chat_new()?,
            Some(Hit::Copy(i)) => {
                if let Some(text) = self.chat.conversation.as_ref().and_then(|c| c.messages.get(i)).map(|m| m.text.clone()) {
                    self.chat_copy(&text);
                    self.chat.copied = Some(i);
                    self.redraw_open_panel()?;
                }
            }
            Some(Hit::Field) | None => {}
        }
        Ok(true)
    }

    pub fn chat_panel_closed(&mut self) {
        let c = &mut self.chat;
        c.layout = None;
        c.hover = None;
        c.copied = None;
    }

    /// A sessão sumiu (outra origem) ou já existe: começa de novo do jeito certo e manda a pergunta
    /// outra vez. Só uma vez por pergunta, e só se a resposta nem começou.
    fn chat_retry(&mut self, failure: &Failure) -> bool {
        if self.chat.retried {
            return false;
        }
        let Some(c) = self.chat.conversation.as_mut() else { return false };
        let Some(question) = c.unanswered().map(str::to_string) else { return false };
        match failure {
            Failure::SessionMissing => {
                c.session = new_session_id();
                c.created = false;
            }
            Failure::SessionInUse => c.created = true,
            _ => return false,
        }
        tracing::info!(?failure, "chat: tentando de novo noutra sessão");
        self.chat.retried = true;
        if let Some(s) = &self.chat.service {
            s.stop();
        }
        self.chat_start();
        if let Some(s) = &self.chat.service {
            s.send(user_line(&question));
        }
        true
    }

    /// O turno acabou (bem ou mal).
    fn chat_turn_done(&mut self) {
        if let Some(at) = self.chat.asked_at.take() {
            let state = self.chat.conversation.as_ref().and_then(|c| c.messages.last()).map(|m| m.state);
            tracing::info!(segundos = format!("{:.1}", at.elapsed().as_secs_f32()), ?state, "chat: resposta terminou");
        }
        if !self.chat_open() {
            self.chat.unread = true;
        }
        self.chat_save();
        window::set_timer(TIMER_CHAT_IDLE, IDLE_MS);
    }

    /// Sinais do processo (`USER_CHAT`).
    pub fn chat_signals(&mut self) -> Result<()> {
        let signals = match &self.chat.service {
            Some(s) => s.take(),
            None => return Ok(()),
        };
        let mut changed = false;
        for signal in signals {
            let busy = self.chat.conversation.as_ref().is_some_and(Conversation::busy);
            match signal {
                Signal::Ready { origin } => {
                    if let Some(c) = &mut self.chat.conversation {
                        c.origin.clone_from(&origin);
                    }
                    self.chat.subtitle = Some(origin);
                    changed = true;
                }
                Signal::Failed(text) => {
                    self.chat.subtitle = Some("indisponível".into());
                    if busy && let Some(c) = &mut self.chat.conversation {
                        c.fail(&Failure::Other(text));
                        self.chat_turn_done();
                    }
                    changed = true;
                }
                Signal::Event(Event::Finished { error: Some(e), .. }) => {
                    let failure = classify(&e);
                    if busy && !self.chat_retry(&failure) {
                        if !matches!(failure, Failure::Interrupted) {
                            tracing::warn!(?failure, "chat: a resposta terminou com erro");
                        }
                        if let Some(c) = &mut self.chat.conversation {
                            c.fail(&failure);
                        }
                        self.chat_turn_done();
                    }
                    changed = true;
                }
                Signal::Event(e) => {
                    if matches!(e, Event::Delta(_)) && !self.chat.first_text {
                        self.chat.first_text = true;
                        if let Some(at) = self.chat.asked_at {
                            tracing::info!(ms = at.elapsed().as_millis() as u64, "chat: primeiro texto");
                        }
                    }
                    if let Some(c) = &mut self.chat.conversation {
                        changed |= c.apply(&e);
                    }
                    if busy && !self.chat.conversation.as_ref().is_some_and(Conversation::busy) {
                        self.chat_turn_done();
                    }
                }
                Signal::Exited { stderr } => {
                    if busy {
                        let last = stderr.lines().rev().map(str::trim).find(|l| !l.is_empty()).unwrap_or_default().to_string();
                        tracing::warn!(stderr = %last, "chat: o processo do Claude saiu no meio da resposta");
                        let failure = match classify(&last) {
                            Failure::Interrupted => Failure::Other(String::new()),
                            f => f,
                        };
                        if !self.chat_retry(&failure) {
                            if let Some(c) = &mut self.chat.conversation {
                                c.fail(&failure);
                            }
                            self.chat_turn_done();
                        }
                        changed = true;
                    }
                }
            }
        }
        self.chat_update_badge()?;
        if changed && self.chat_open() {
            self.redraw_open_panel()?;
        }
        Ok(())
    }

    /// Dez minutos sem uso: o processo sai (a próxima pergunta retoma a sessão).
    pub fn chat_idle(&mut self) {
        window::kill_timer(TIMER_CHAT_IDLE);
        if self.chat.conversation.as_ref().is_some_and(Conversation::busy) || self.chat_open() {
            window::set_timer(TIMER_CHAT_IDLE, IDLE_MS);
            return;
        }
        if let Some(s) = &self.chat.service {
            s.stop();
            tracing::info!("chat: processo encerrado por falta de uso");
        }
    }

    /// O chat segue o chip: sem ele na barra (ou com outro `[chat]`), o processo sai.
    pub fn chat_services(&mut self) {
        let used = self.ctx.config.bar.uses("chat");
        let same = self.chat.config.as_ref() == Some(&self.ctx.config.chat);
        if self.chat.service.is_some() && (!used || !same) {
            self.chat.service = None;
            self.chat.config = None;
            self.chat.subtitle = None;
            if let Some(c) = &mut self.chat.conversation
                && c.busy()
            {
                c.fail(&Failure::Interrupted);
            }
            window::kill_timer(TIMER_CHAT_IDLE);
        }
    }

    /// Saindo do Zefiro: grava a conversa. O processo sai sozinho quando o stdin fecha.
    pub fn chat_shutdown(&mut self) {
        if let Some(c) = &mut self.chat.conversation
            && c.busy()
        {
            c.fail(&Failure::Interrupted);
        }
        self.chat_save();
        self.chat.service = None;
    }
}
