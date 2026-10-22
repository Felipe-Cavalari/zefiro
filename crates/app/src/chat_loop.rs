//! Chat com o Claude e o Codex no laço de eventos: o painel do chip `chat`, o campo (teclas e
//! `WM_CHAR`), a rolagem, os seletores de agente, origem e modelo, as conversas antigas e os sinais
//! do processo (spec 2026-10-21-chat-claude, ADR-0020). O texto da conversa nunca vai para o log.

use std::collections::{HashMap, HashSet};
use std::sync::{Arc, Mutex};
use std::time::Instant;
use windows::core::Result;
use zefiro_bar::chat::{ChatLayout, ChatView, Edit, Field, Hit, HistoryRow, Key, Line, MessageView, Page, Wrapped, chat_panel, field_text_width, history_max_scroll, plain_text, text_style, text_width, wrap, wrap_text};
use zefiro_bar::controller::PanelKind;
use zefiro_core::config::set_value;
use zefiro_integration_chat::agent::{Agent, ModelOption, claude_models, effective_model, model_label};
use zefiro_integration_chat::conversation::{Conversation, Role};
use zefiro_integration_chat::history::{self, Summary};
use zefiro_integration_chat::image::{MAX_IMAGES, MAX_SIDE, base64, decode_base64, is_png};
use zefiro_integration_chat::origin::{Choice, Prefer, parse_choice};
use zefiro_integration_chat::protocol::{Event, Failure, classify};
use zefiro_platform::geometry::Point;
use zefiro_platform::image::Image;
use zefiro_platform_windows::chat::{ChatService, Signal, Start, conversations_dir, images_dir, new_id, windows_dir};
use zefiro_platform_windows::{imaging, system, system_events, window};
use zefiro_ui::paint::DisplayList;

use crate::event_loop::{Loop, TIMER_CHAT_IDLE, USER_CHAT, USER_CHAT_IMAGE};

/// Sem uso por este tempo, o processo sai (a sessão continua gravada).
const IDLE_MS: u32 = 10 * 60 * 1000;
/// Linhas por passo da roda.
const WHEEL_LINES: f32 = 3.0;
/// Lado das miniaturas decodificadas (px; cabem em 72 DIP a 175 %).
const THUMB_PX: u32 = 128;
/// Lado das imagens geradas na tela (px; a largura do balão a 200 %).
const REPLY_THUMB_PX: u32 = 800;
/// Imagem gerada que não vem em PNG é convertida até este lado (px).
const GENERATED_MAX_SIDE: u32 = 4096;

/// Imagem colada esperando o envio. `thumb` vazio enquanto converte.
struct Attachment { id: String, thumb: Option<Image> }

/// O que as threads de imagem devolvem.
enum ImageDone {
    /// Imagem colada convertida e gravada (ou o erro).
    Pasted { id: String, thumb: std::result::Result<Image, String> },
    /// Miniatura de uma imagem de conversa (`None` se o arquivo sumiu).
    Thumb { id: String, image: Option<Image> },
}

/// Linhas quebradas de uma mensagem, refeitas só quando o texto ou a escala mudam.
struct CachedWrap { len: usize, scale: f32, wrapped: Wrapped }

/// O que a área do painel mostra.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
enum ChatPage {
    #[default]
    Chat,
    Origins,
    Models,
    History,
}

/// O processo vivo: agente, origem (e a preferência, no automático) e (no Claude, que recebe o
/// modelo na partida) o modelo.
#[derive(Debug, Clone, PartialEq)]
struct RunningKey { agent: Agent, choice: Choice, prefer: Option<Prefer>, model: String }

#[derive(Default)]
pub struct ChatRuntime {
    service: Option<ChatService>,
    running: Option<RunningKey>,
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
    /// Imagens coladas no campo.
    attachments: Vec<Attachment>,
    /// Miniaturas das imagens das perguntas, por id, e as que estão sendo lidas.
    thumbs: HashMap<String, Image>,
    loading: HashSet<String>,
    /// Imagens cujo arquivo não abriu (não tenta ler de novo a cada quadro).
    missing: HashSet<String>,
    images_done: Arc<Mutex<Vec<ImageDone>>>,
    /// "iniciando…" até o processo subir; "indisponível" se não subiu.
    status: Option<String>,
    /// Origem real do processo (no automático, aparece no seletor).
    live_origin: Option<String>,
    page: ChatPage,
    /// Origens encontradas, por agente (Claude, Codex).
    origins: [Option<Vec<Choice>>; 2],
    /// Modelos que o Codex desta origem aceita.
    codex_models: Option<Vec<ModelOption>>,
    history: Vec<Summary>,
    history_scroll: usize,
    /// Chegou resposta com o painel fechado.
    unread: bool,
    asked_at: Option<Instant>,
    first_text: bool,
    /// Já tentou de novo nesta pergunta (sessão sumida ou em uso).
    retried: bool,
}

fn now_unix() -> i64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_secs() as i64)
}

fn slot(agent: Agent) -> usize {
    usize::from(agent == Agent::Codex)
}

impl Loop {
    fn chat_open(&self) -> bool {
        self.controller.open().is_some_and(|o| o.kind == PanelKind::Chat)
    }

    fn chat_agent(&self) -> Agent {
        Agent::parse(&self.ctx.config.chat.agent)
    }

    fn chat_choice(&self, agent: Agent) -> Choice {
        parse_choice(&self.ctx.config.chat.of(agent == Agent::Codex).origin)
    }

    fn chat_prefer(&self, agent: Agent) -> Prefer {
        Prefer::parse(&self.ctx.config.chat.of(agent == Agent::Codex).prefer)
    }

    fn chat_models(&self, agent: Agent) -> Vec<ModelOption> {
        match agent {
            Agent::Claude => claude_models(),
            Agent::Codex => self.chat.codex_models.clone().unwrap_or_default(),
        }
    }

    /// O modelo que vai para o agente (no Codex, sempre um da lista dele).
    fn chat_model(&self, agent: Agent) -> String {
        effective_model(&self.chat_models(agent), &self.ctx.config.chat.of(agent == Agent::Codex).model)
    }

    fn chat_conversation(&mut self) -> &mut Conversation {
        if self.chat.conversation.is_none() {
            history::migrate(&windows_dir().join("conversa.json"), &conversations_dir());
            let agent = self.chat_agent();
            let latest = history::list(&conversations_dir()).into_iter().find(|s| s.agent == agent).and_then(|s| history::load(&conversations_dir(), &s.id));
            self.chat.conversation = Some(latest.unwrap_or_else(|| Conversation::new(new_id(), agent)));
        }
        self.chat.conversation.get_or_insert_with(|| Conversation::new(new_id(), Agent::Claude))
    }

    /// Grava uma opção do `[chat]` (o painel não fecha: ver `Config::same_except_panels`).
    fn chat_write(&self, keys: &[&str], value: &str) {
        if let Err(e) = set_value(&self.ctx.config_path, keys, value) {
            tracing::warn!("chat: não consegui gravar a escolha no config: {e}");
        }
    }

    /// Sobe o processo da conversa atual (se ainda não está rodando com estas escolhas).
    fn chat_start(&mut self) {
        if self.chat.service.is_none() {
            match ChatService::new(USER_CHAT) {
                Ok(s) => self.chat.service = Some(s),
                Err(e) => {
                    tracing::warn!("chat: não consegui criar a thread do chat: {e}");
                    return;
                }
            }
        }
        let agent = self.chat_conversation().agent;
        let choice = self.chat_choice(agent);
        let model = self.chat_model(agent);
        let prefer = self.chat_prefer(agent);
        let key = RunningKey { agent, choice: choice.clone(), prefer: (choice == Choice::Auto).then_some(prefer), model: if agent == Agent::Claude { model.clone() } else { String::new() } };
        if self.chat.running.as_ref().is_some_and(|r| *r != key)
            && let Some(s) = &self.chat.service
        {
            s.stop();
            self.chat.live_origin = None;
        }
        if self.chat.running.as_ref() != Some(&key) {
            self.chat.status = Some("iniciando…".into());
        }
        let c = self.chat_conversation();
        let spec = Start { agent, choice, prefer, model, session: c.session.clone(), resume: c.created, windows_dir: windows_dir() };
        if let Some(s) = &self.chat.service {
            s.start(spec);
        }
        self.chat.running = Some(key);
        window::set_timer(TIMER_CHAT_IDLE, IDLE_MS);
    }

    /// Encerra o processo (a próxima pergunta sobe outro).
    fn chat_stop_process(&mut self) {
        if let Some(s) = &self.chat.service {
            s.stop();
        }
        self.chat.running = None;
        self.chat.live_origin = None;
    }

    fn chat_save(&mut self) {
        if let Some(c) = &self.chat.conversation
            && let Err(e) = history::save(&conversations_dir(), c)
        {
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
        c.page = ChatPage::Chat;
        self.chat_start();
        self.chat_update_badge()
    }

    /// Começa outra conversa com o agente escolhido (a atual fica nas conversas antigas).
    fn chat_new_conversation(&mut self) {
        self.chat_save();
        let agent = self.chat_agent();
        let c = &mut self.chat;
        c.conversation = Some(Conversation::new(new_id(), agent));
        c.wraps.clear();
        c.layout = None;
        c.scroll = 0.0;
        c.copied = None;
        c.unread = false;
        c.page = ChatPage::Chat;
    }

    fn chat_history_rows(&self) -> Vec<HistoryRow> {
        let offset = zefiro_integration_clock::windows::utc_offset_seconds();
        let current = self.chat.conversation.as_ref().map(|c| c.id.as_str());
        self.chat
            .history
            .iter()
            .map(|s| {
                let origin = if s.origin.is_empty() { String::new() } else { format!(" · {}", s.origin) };
                HistoryRow { title: s.title.clone(), meta: format!("{}{origin} · {}", s.agent.label(), zefiro_integration_usage::format::format_short(s.updated, offset)), current: current == Some(s.id.as_str()) }
            })
            .collect()
    }

    /// As opções do seletor de origem: as encontradas e, se não estiver entre elas, a escolhida.
    fn chat_origin_options(&self, agent: Agent) -> Vec<Choice> {
        let mut options = self.chat.origins[slot(agent)].clone().unwrap_or_else(|| vec![Choice::Auto]);
        let chosen = self.chat_choice(agent);
        if !options.contains(&chosen) {
            options.push(chosen);
        }
        options
    }

    pub fn chat_panel_list(&mut self, scale: f32) -> DisplayList {
        let agent = self.chat_conversation().agent;
        let choice = self.chat_choice(agent);
        let origin_label = match (&choice, &self.chat.live_origin) {
            (Choice::Auto, Some(live)) => format!("Automático · {live}"),
            _ => choice.label(),
        };
        let models = self.chat_models(agent);
        let chosen_model = self.ctx.config.chat.of(agent == Agent::Codex).model.clone();
        let model_text = if agent == Agent::Codex && self.chat.codex_models.is_none() && chosen_model.is_empty() { "Modelo".to_string() } else { model_label(&models, &chosen_model) };
        let origin_options = self.chat_origin_options(agent);
        let prefer = self.chat_prefer(agent);
        let history_rows = self.chat_history_rows();
        let wanted: Vec<(String, u32)> = self
            .chat
            .conversation
            .as_ref()
            .map(|c| {
                c.messages
                    .iter()
                    .flat_map(|m| {
                        let side = if m.role == Role::Claude { REPLY_THUMB_PX } else { THUMB_PX };
                        m.images.iter().map(move |id| (id.clone(), side))
                    })
                    .collect()
            })
            .unwrap_or_default();
        self.chat_load_thumbs(wanted);
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
        let origin_names: Vec<String> = origin_options.iter().map(Choice::label).collect();
        let model_names: Vec<String> = models.iter().map(|m| m.name.clone()).collect();
        let origin_title = format!("Origem do {}", agent.label());
        let model_title = format!("Modelo do {}", agent.label());
        let page = match c.page {
            ChatPage::Chat => Page::Chat,
            ChatPage::Origins => Page::Picker {
                title: &origin_title,
                options: &origin_names,
                selected: origin_options.iter().position(|o| *o == choice),
                note: c.origins[slot(agent)].is_none().then_some("Procurando o Windows e as distros do WSL…"),
                prefer: Some(prefer),
            },
            ChatPage::Models => Page::Picker {
                title: &model_title,
                options: &model_names,
                selected: models.iter().position(|m| m.id == effective_model(&models, &chosen_model)),
                note: models.is_empty().then_some("Carregando os modelos do Codex…"),
                prefer: None,
            },
            ChatPage::History => Page::History { rows: &history_rows, scroll: c.history_scroll },
        };
        let pictures: Vec<Vec<Option<Image>>> = conv.messages.iter().map(|m| m.images.iter().map(|id| c.thumbs.get(id).cloned()).collect()).collect();
        let attachments: Vec<Option<Image>> = c.attachments.iter().map(|a| a.thumb.clone()).collect();
        let last = conv.messages.len().saturating_sub(1);
        let generating = conv.generating();
        let views: Vec<MessageView> = conv
            .messages
            .iter()
            .zip(&c.wraps)
            .zip(&pictures)
            .enumerate()
            .map(|(i, ((m, w), p))| MessageView { role: m.role, state: m.state, wrapped: &w.wrapped, note: m.note.as_deref(), images: p, generating: generating && i == last })
            .collect();
        let tokens = &self.ctx.theme.tokens;
        let mut view = ChatView { messages: &views, field: &c.field, field_lines: &c.field_lines, attachments: &attachments, agent, origin: &origin_label, model: &model_text, status: c.status.as_deref(), page, scroll: c.scroll, hover: c.hover, copied: c.copied };
        let mut layout = chat_panel(&view, scale, tokens, measure);
        // Rolado para cima enquanto a resposta cresce embaixo: o que está na tela fica parado.
        if c.page == ChatPage::Chat {
            let before = c.layout.as_ref().map_or(layout.max_scroll, |l| l.max_scroll);
            if c.scroll > 0.0 && layout.max_scroll > before {
                c.scroll += layout.max_scroll - before;
                view.scroll = c.scroll;
                layout = chat_panel(&view, scale, tokens, measure);
            }
            c.scroll = c.scroll.min(layout.max_scroll);
        }
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

    /// As imagens (PNG em base64) de uma pergunta, lidas da pasta das imagens.
    fn chat_image_data(ids: &[String]) -> Vec<String> {
        ids.iter()
            .filter_map(|id| history::image_file(&images_dir(), id))
            .filter_map(|path| match std::fs::read(&path) {
                Ok(bytes) => Some(base64(&bytes)),
                Err(e) => {
                    tracing::warn!("chat: não consegui ler uma imagem colada: {e}");
                    None
                }
            })
            .collect()
    }

    fn chat_send(&mut self) -> Result<()> {
        if self.chat.attachments.iter().any(|a| a.thumb.is_none()) {
            return Ok(());
        }
        let text = self.chat.field.text.clone();
        let ids: Vec<String> = self.chat.attachments.iter().map(|a| a.id.clone()).collect();
        let c = self.chat_conversation();
        if !c.ask_with(&text, ids.clone()) {
            return Ok(());
        }
        c.updated = now_unix();
        let agent = c.agent;
        let question = c.messages[c.messages.len() - 2].text.clone();
        let chat = &mut self.chat;
        for a in chat.attachments.drain(..) {
            if let Some(thumb) = a.thumb {
                chat.thumbs.insert(a.id, thumb);
            }
        }
        chat.field.clear();
        chat.scroll = 0.0;
        chat.copied = None;
        chat.retried = false;
        chat.first_text = false;
        chat.asked_at = Some(Instant::now());
        tracing::info!(agente = agent.label(), caracteres = question.chars().count(), imagens = ids.len(), "chat: pergunta enviada");
        self.chat_start();
        let model = self.chat_model(agent);
        if let Some(c) = &mut self.chat.conversation {
            c.model.clone_from(&self.ctx.config.chat.of(agent == Agent::Codex).model);
        }
        let images = Self::chat_image_data(&ids);
        if let Some(s) = &self.chat.service {
            s.send(question, images, model);
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
        self.chat_stop_process();
        self.chat_new_conversation();
        tracing::info!("chat: conversa nova");
        if self.chat_open() {
            self.chat_start();
        }
        self.chat_update_badge()?;
        self.redraw_open_panel()
    }

    /// Claude | Codex. Com a conversa vazia, ela só troca de agente; senão, começa outra.
    fn chat_set_agent(&mut self, agent: Agent) -> Result<()> {
        if agent == self.chat_conversation().agent {
            return Ok(());
        }
        self.ctx.config.chat.agent = agent.as_str().into();
        self.chat_write(&["chat", "agent"], agent.as_str());
        self.chat_stop_process();
        self.chat_new_conversation();
        tracing::info!(agente = agent.label(), "chat: agente trocado");
        self.chat_start();
        self.chat_update_badge()?;
        self.redraw_open_panel()
    }

    /// Outra origem: a sessão só existe onde nasceu, então começa outra conversa (se havia uma).
    fn chat_set_origin(&mut self, choice: Choice) -> Result<()> {
        let agent = self.chat_conversation().agent;
        self.chat.page = ChatPage::Chat;
        if choice != self.chat_choice(agent) {
            self.ctx.config.chat.of_mut(agent == Agent::Codex).origin = choice.value();
            self.chat_write(&["chat", agent.as_str(), "origin"], &choice.value());
            self.chat_stop_process();
            self.chat.codex_models = None;
            if !self.chat_conversation().messages.is_empty() {
                self.chat_new_conversation();
            }
            tracing::info!(agente = agent.label(), origem = %choice.label(), "chat: origem trocada");
            self.chat_start();
        }
        self.redraw_open_panel()
    }

    /// O que o automático tenta primeiro. Com a origem no automático, o processo pode mudar de
    /// lugar: aí começa outra conversa (se havia uma), como ao trocar de origem.
    fn chat_set_prefer(&mut self, prefer: Prefer) -> Result<()> {
        let agent = self.chat_conversation().agent;
        if prefer == self.chat_prefer(agent) {
            return Ok(());
        }
        self.ctx.config.chat.of_mut(agent == Agent::Codex).prefer = prefer.as_str().into();
        self.chat_write(&["chat", agent.as_str(), "prefer"], prefer.as_str());
        tracing::info!(agente = agent.label(), primeiro = prefer.label(), "chat: preferência do automático trocada");
        if self.chat_choice(agent) == Choice::Auto {
            self.chat_stop_process();
            self.chat.codex_models = None;
            if !self.chat_conversation().messages.is_empty() {
                self.chat_new_conversation();
                self.chat.page = ChatPage::Origins;
            }
            self.chat_start();
        }
        self.redraw_open_panel()
    }

    /// Outro modelo: a conversa continua (o Claude reinicia com `--resume --model`; o Codex usa o
    /// modelo novo na próxima pergunta).
    fn chat_set_model(&mut self, id: String) -> Result<()> {
        let agent = self.chat_conversation().agent;
        self.chat.page = ChatPage::Chat;
        let cfg = self.ctx.config.chat.of_mut(agent == Agent::Codex);
        if cfg.model != id {
            cfg.model.clone_from(&id);
            self.chat_write(&["chat", agent.as_str(), "model"], &id);
            tracing::info!(agente = agent.label(), modelo = %id, "chat: modelo trocado");
            if agent == Agent::Claude && !self.chat_conversation().busy() {
                self.chat_start();
            }
        }
        self.redraw_open_panel()
    }

    /// Reabre uma conversa antiga com o agente, a origem e o modelo dela.
    fn chat_open_conversation(&mut self, index: usize) -> Result<()> {
        let Some(summary) = self.chat.history.get(index).cloned() else { return Ok(()) };
        if self.chat.conversation.as_ref().is_some_and(|c| c.busy()) {
            return Ok(());
        }
        let Some(conv) = history::load(&conversations_dir(), &summary.id) else {
            tracing::warn!("chat: não consegui ler a conversa antiga");
            return Ok(());
        };
        self.chat_save();
        let agent = conv.agent;
        let codex = agent == Agent::Codex;
        let choice = Choice::from_label(&conv.origin);
        if self.chat_agent() != agent {
            self.ctx.config.chat.agent = agent.as_str().into();
            self.chat_write(&["chat", "agent"], agent.as_str());
        }
        if self.chat_choice(agent) != choice {
            self.ctx.config.chat.of_mut(codex).origin = choice.value();
            self.chat_write(&["chat", agent.as_str(), "origin"], &choice.value());
            self.chat.codex_models = None;
        }
        if self.ctx.config.chat.of(codex).model != conv.model {
            self.ctx.config.chat.of_mut(codex).model.clone_from(&conv.model);
            self.chat_write(&["chat", agent.as_str(), "model"], &conv.model);
        }
        self.chat_stop_process();
        let c = &mut self.chat;
        c.conversation = Some(conv);
        c.wraps.clear();
        c.layout = None;
        c.scroll = 0.0;
        c.copied = None;
        c.page = ChatPage::Chat;
        tracing::info!(agente = agent.label(), "chat: conversa antiga reaberta");
        self.chat_start();
        self.redraw_open_panel()
    }

    fn chat_delete_conversation(&mut self, index: usize) -> Result<()> {
        let Some(summary) = self.chat.history.get(index).cloned() else { return Ok(()) };
        if let Err(e) = history::delete(&conversations_dir(), &images_dir(), &summary.id) {
            tracing::warn!("chat: não consegui apagar a conversa: {e}");
        }
        if self.chat.conversation.as_ref().is_some_and(|c| c.id == summary.id) {
            self.chat_stop_process();
            let agent = self.chat_agent();
            self.chat.conversation = Some(Conversation::new(new_id(), agent));
            self.chat.wraps.clear();
        }
        self.chat.history = history::list(&conversations_dir());
        self.chat.history_scroll = self.chat.history_scroll.min(history_max_scroll(self.chat.history.len()));
        self.chat.hover = None;
        self.redraw_open_panel()
    }

    fn chat_show_page(&mut self, page: ChatPage) -> Result<()> {
        let agent = self.chat_conversation().agent;
        self.chat.page = if self.chat.page == page { ChatPage::Chat } else { page };
        self.chat.hover = None;
        match self.chat.page {
            ChatPage::History => {
                self.chat_save();
                self.chat.history = history::list(&conversations_dir());
                self.chat.history_scroll = 0;
            }
            ChatPage::Origins => {
                self.chat_start();
                if let Some(s) = &self.chat.service {
                    s.discover(agent);
                }
            }
            // O Codex manda os modelos assim que sobe.
            ChatPage::Models => self.chat_start(),
            ChatPage::Chat => {}
        }
        self.redraw_open_panel()
    }

    /// Teclas com o painel do chat aberto. `true` se tratou (o Esc no chat segue e fecha o painel).
    pub fn chat_key(&mut self, vk: u32) -> Result<bool> {
        if !self.chat_open() {
            return Ok(false);
        }
        if self.chat.page != ChatPage::Chat {
            if vk == 0x1B {
                self.chat.page = ChatPage::Chat;
                self.redraw_open_panel()?;
            }
            return Ok(true);
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
        if self.chat.page != ChatPage::Chat {
            return Ok(true);
        }
        let paste = if code == 22 { system_events::clipboard_text_full() } else { None };
        // Ctrl+V sem texto: uma imagem (print, imagem copiada, arquivo do Explorer). Com texto, o
        // texto ganha (o Excel e o Word copiam os dois).
        if code == 22
            && paste.is_none()
            && let Some(bytes) = system_events::clipboard_image()
        {
            self.chat_attach(bytes)?;
            return Ok(true);
        }
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
        if self.chat.page == ChatPage::History {
            let max = history_max_scroll(self.chat.history.len());
            let c = &mut self.chat;
            c.history_scroll = if delta > 0 { c.history_scroll.saturating_sub(1) } else { (c.history_scroll + 1).min(max) };
            self.redraw_open_panel()?;
            return Ok(true);
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

    /// Clique no painel. `true` se o painel é o do chat.
    pub fn chat_click(&mut self, p: Point) -> Result<bool> {
        if !self.chat_open() {
            return Ok(false);
        }
        let agent = self.chat_conversation().agent;
        match self.chat_hit(p) {
            Some(Hit::Send) => self.chat_send()?,
            Some(Hit::Stop) => self.chat_stop()?,
            Some(Hit::NewChat) => self.chat_new()?,
            Some(Hit::History) => self.chat_show_page(ChatPage::History)?,
            Some(Hit::Origins) => self.chat_show_page(ChatPage::Origins)?,
            Some(Hit::Models) => self.chat_show_page(ChatPage::Models)?,
            Some(Hit::Back) => {
                self.chat.page = ChatPage::Chat;
                self.redraw_open_panel()?;
            }
            Some(Hit::Agent(a)) => self.chat_set_agent(a)?,
            Some(Hit::Pick(i)) => match self.chat.page {
                ChatPage::Origins => {
                    if let Some(choice) = self.chat_origin_options(agent).get(i).cloned() {
                        self.chat_set_origin(choice)?;
                    }
                }
                ChatPage::Models => {
                    if let Some(m) = self.chat_models(agent).get(i) {
                        self.chat_set_model(m.id.clone())?;
                    }
                }
                _ => {}
            },
            Some(Hit::Prefer(p)) => self.chat_set_prefer(p)?,
            Some(Hit::Open(i)) => self.chat_open_conversation(i)?,
            Some(Hit::Delete(i)) => self.chat_delete_conversation(i)?,
            Some(Hit::Copy(i)) => {
                if let Some(text) = self.chat.conversation.as_ref().and_then(|c| c.messages.get(i)).map(|m| m.text.clone()) {
                    self.chat_copy(&text);
                    self.chat.copied = Some(i);
                    self.redraw_open_panel()?;
                }
            }
            Some(Hit::Unattach(i)) => self.chat_unattach(i)?,
            Some(Hit::Picture(m, i)) => self.chat_open_picture(m, i)?,
            Some(Hit::Field) | None => {}
        }
        Ok(true)
    }

    pub fn chat_panel_closed(&mut self) {
        let c = &mut self.chat;
        c.layout = None;
        c.hover = None;
        c.copied = None;
        c.page = ChatPage::Chat;
    }

    /// A sessão sumiu (outra origem) ou já existe: começa de novo do jeito certo e manda a pergunta
    /// outra vez. Só uma vez por pergunta, e só se a resposta nem começou.
    fn chat_retry(&mut self, failure: &Failure) -> bool {
        if self.chat.retried {
            return false;
        }
        let Some(c) = self.chat.conversation.as_mut() else { return false };
        let Some((question, ids)) = c.unanswered().map(|m| (m.text.clone(), m.images.clone())) else { return false };
        match failure {
            Failure::SessionMissing => {
                c.session = if c.agent == Agent::Claude { new_id() } else { String::new() };
                c.created = false;
            }
            Failure::SessionInUse => c.created = true,
            _ => return false,
        }
        let agent = c.agent;
        tracing::info!(?failure, "chat: tentando de novo noutra sessão");
        self.chat.retried = true;
        self.chat_stop_process();
        self.chat_start();
        let model = self.chat_model(agent);
        let images = Self::chat_image_data(&ids);
        if let Some(s) = &self.chat.service {
            s.send(question, images, model);
        }
        true
    }

    /// Ctrl+V com imagem (e sem texto) na área de transferência: converte numa thread para PNG
    /// reduzido, grava na pasta das imagens e faz a miniatura.
    fn chat_attach(&mut self, bytes: Vec<u8>) -> Result<()> {
        if self.chat.attachments.len() >= MAX_IMAGES {
            tracing::info!("chat: já há {MAX_IMAGES} imagens na pergunta; a nova foi ignorada");
            return Ok(());
        }
        let id = new_id();
        self.chat.attachments.push(Attachment { id: id.clone(), thumb: None });
        let done = self.chat.images_done.clone();
        let spawned = std::thread::Builder::new().name("zefiro-chat-image".into()).spawn(move || {
            let result = imaging::to_png(&bytes, MAX_SIDE).map_err(|e| e.to_string()).and_then(|png| {
                let path = history::image_file(&images_dir(), &id).ok_or("id inválido")?;
                std::fs::create_dir_all(images_dir()).and_then(|_| std::fs::write(&path, &png)).map_err(|e| e.to_string())?;
                imaging::decode_bytes(&png, THUMB_PX).ok_or_else(|| "miniatura".to_string())
            });
            if let Ok(mut d) = done.lock() {
                d.push(ImageDone::Pasted { id, thumb: result });
            }
            window::post_user(USER_CHAT_IMAGE);
        });
        if let Err(e) = spawned {
            tracing::warn!("chat: não consegui converter a imagem: {e}");
            self.chat.attachments.pop();
        }
        self.redraw_open_panel()
    }

    /// Tira uma imagem colada antes de enviar (e o arquivo dela).
    fn chat_unattach(&mut self, index: usize) -> Result<()> {
        if index < self.chat.attachments.len() {
            let a = self.chat.attachments.remove(index);
            if let Some(path) = history::image_file(&images_dir(), &a.id) {
                let _ = std::fs::remove_file(path);
            }
            self.chat.hover = None;
            self.redraw_open_panel()?;
        }
        Ok(())
    }

    /// Abre uma imagem da conversa no visualizador do Windows (e fecha o painel, que ficaria por
    /// cima dele).
    fn chat_open_picture(&mut self, message: usize, index: usize) -> Result<()> {
        let id = self.chat.conversation.as_ref().and_then(|c| c.messages.get(message)).and_then(|m| m.images.get(index)).cloned();
        let Some(path) = id.and_then(|id| history::image_file(&images_dir(), &id)).filter(|p| p.is_file()) else {
            tracing::warn!("chat: a imagem não está mais na pasta");
            return Ok(());
        };
        tracing::info!("chat: abrindo a imagem no visualizador");
        system::open_folder(&path);
        self.close_open_panel()
    }

    /// O Codex gerou uma imagem: ela entra na resposta já com o id, e uma thread grava o PNG na
    /// pasta das imagens e faz a versão da tela.
    fn chat_generated_image(&mut self, data: String) -> bool {
        let id = new_id();
        let Some(c) = self.chat.conversation.as_mut() else { return false };
        if !c.add_reply_image(id.clone()) {
            return false;
        }
        self.chat.loading.insert(id.clone());
        let done = self.chat.images_done.clone();
        let spawned = std::thread::Builder::new().name("zefiro-chat-generated".into()).spawn(move || {
            let result = decode_base64(&data)
                .ok_or_else(|| "base64 inválido".to_string())
                .and_then(|bytes| if is_png(&bytes) { Ok(bytes) } else { imaging::to_png(&bytes, GENERATED_MAX_SIDE).map_err(|e| e.to_string()) })
                .and_then(|png| {
                    let path = history::image_file(&images_dir(), &id).ok_or("id inválido")?;
                    std::fs::create_dir_all(images_dir()).and_then(|_| std::fs::write(&path, &png)).map_err(|e| e.to_string())?;
                    Ok(png)
                });
            let image = match result {
                Ok(png) => {
                    tracing::info!(bytes = png.len(), "chat: imagem gerada gravada");
                    imaging::decode_bytes(&png, REPLY_THUMB_PX)
                }
                Err(e) => {
                    tracing::warn!("chat: não consegui gravar a imagem gerada: {e}");
                    None
                }
            };
            if let Ok(mut d) = done.lock() {
                d.push(ImageDone::Thumb { id, image });
            }
            window::post_user(USER_CHAT_IMAGE);
        });
        if let Err(e) = spawned {
            tracing::warn!("chat: não consegui gravar a imagem gerada: {e}");
        }
        self.chat_save();
        true
    }

    /// Lê numa thread as imagens da conversa que ainda não estão na memória (`side`: lado em px).
    fn chat_load_thumbs(&mut self, ids: Vec<(String, u32)>) {
        let c = &self.chat;
        let missing: Vec<(String, u32)> = ids.into_iter().filter(|(id, _)| !c.thumbs.contains_key(id) && !c.loading.contains(id) && !c.missing.contains(id)).collect();
        if missing.is_empty() {
            return;
        }
        self.chat.loading.extend(missing.iter().map(|(id, _)| id.clone()));
        let done = self.chat.images_done.clone();
        let spawned = std::thread::Builder::new().name("zefiro-chat-thumbs".into()).spawn(move || {
            for (id, side) in missing {
                let image = history::image_file(&images_dir(), &id).and_then(|p| std::fs::read(p).ok()).and_then(|b| imaging::decode_bytes(&b, side));
                if let Ok(mut d) = done.lock() {
                    d.push(ImageDone::Thumb { id, image });
                }
            }
            window::post_user(USER_CHAT_IMAGE);
        });
        if let Err(e) = spawned {
            tracing::warn!("chat: não consegui ler as miniaturas: {e}");
        }
    }

    /// As threads de imagem terminaram (`USER_CHAT_IMAGE`).
    pub fn chat_images(&mut self) -> Result<()> {
        let done = self.chat.images_done.lock().map(|mut d| std::mem::take(&mut *d)).unwrap_or_default();
        for d in done {
            match d {
                ImageDone::Pasted { id, thumb } => match thumb {
                    Ok(thumb) => {
                        if let Some(a) = self.chat.attachments.iter_mut().find(|a| a.id == id) {
                            a.thumb = Some(thumb);
                        }
                    }
                    Err(e) => {
                        tracing::warn!("chat: não consegui converter a imagem colada: {e}");
                        self.chat.attachments.retain(|a| a.id != id);
                    }
                },
                ImageDone::Thumb { id, image } => {
                    self.chat.loading.remove(&id);
                    match image {
                        Some(image) => {
                            self.chat.thumbs.insert(id, image);
                        }
                        None => {
                            self.chat.missing.insert(id);
                        }
                    }
                }
            }
        }
        if self.chat_open() {
            self.redraw_open_panel()?;
        }
        Ok(())
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
        if let Some(c) = &mut self.chat.conversation {
            c.updated = now_unix();
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
                    self.chat.live_origin = Some(origin);
                    self.chat.status = None;
                    changed = true;
                }
                Signal::Origins { agent, choices } => {
                    self.chat.origins[slot(agent)] = Some(choices);
                    changed = true;
                }
                Signal::Failed(text) => {
                    self.chat.status = Some("indisponível".into());
                    self.chat.running = None;
                    if busy && let Some(c) = &mut self.chat.conversation {
                        c.fail(&Failure::Other(text));
                        self.chat_turn_done();
                    }
                    changed = true;
                }
                Signal::Event(Event::Models(models)) => {
                    self.chat.codex_models = Some(models);
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
                Signal::Event(Event::Image { data, .. }) => {
                    if busy {
                        changed |= self.chat_generated_image(data);
                    }
                }
                Signal::Event(e) => {
                    if matches!(e, Event::Delta { .. }) && !self.chat.first_text {
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
                    self.chat.running = None;
                    self.chat.live_origin = None;
                    if busy {
                        let last = stderr.lines().rev().map(str::trim).find(|l| !l.is_empty()).unwrap_or_default().to_string();
                        tracing::warn!(stderr = %last, "chat: o processo saiu no meio da resposta");
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
                    }
                    changed = true;
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
        if self.chat.running.is_some() {
            self.chat_stop_process();
            tracing::info!("chat: processo encerrado por falta de uso");
        }
    }

    /// O `[chat]` mudou por fora (ou pelo próprio painel): a próxima partida usa as escolhas novas.
    pub fn chat_config_changed(&mut self) -> Result<()> {
        if self.chat_open() {
            self.redraw_open_panel()?;
        }
        Ok(())
    }

    /// O chat segue o chip: sem ele na barra, o processo sai.
    pub fn chat_services(&mut self) {
        if self.chat.service.is_some() && !self.ctx.config.bar.uses("chat") {
            self.chat.service = None;
            self.chat.running = None;
            self.chat.live_origin = None;
            self.chat.status = None;
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
