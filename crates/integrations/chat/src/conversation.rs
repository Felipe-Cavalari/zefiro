//! A conversa do painel: as mensagens, em que pé está a resposta e a gravação em JSON.

use crate::agent::Agent;
use crate::protocol::{Event, Failure, failure_text};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Role { User, Claude }

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum State {
    Done,
    /// Pergunta enviada, nada chegou ainda.
    Waiting,
    /// O modelo está pensando, ainda sem texto.
    Thinking,
    Streaming,
    /// Parada pelo usuário (ou pelo Zefiro fechando no meio).
    Stopped,
    Failed,
}

impl State {
    pub fn busy(self) -> bool {
        matches!(self, State::Waiting | State::Thinking | State::Streaming)
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Message {
    pub role: Role,
    pub text: String,
    pub state: State,
    /// Aviso abaixo da resposta ("Resposta interrompida.", o erro).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
    /// Imagens coladas na pergunta ou geradas na resposta (ids dos PNGs na pasta das imagens do
    /// chat).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub images: Vec<String>,
}

/// Limite do campo (caracteres).
pub const MAX_PROMPT: usize = 8000;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct Conversation {
    /// Id do Zefiro (nome do arquivo). Nas conversas da primeira versão, o da sessão.
    #[serde(default)]
    pub id: String,
    #[serde(default)]
    pub agent: Agent,
    /// Id da sessão do `claude` ou do `thread` do Codex (vazio até o Codex criar).
    pub session: String,
    /// O `claude` já criou a sessão (então a próxima partida é `--resume`).
    #[serde(default)]
    pub created: bool,
    /// Rótulo da origem em que a sessão existe ("Windows", nome da distro).
    #[serde(default)]
    pub origin: String,
    /// Modelo escolhido (vazio = o padrão).
    #[serde(default)]
    pub model: String,
    /// Última mudança (segundos Unix), para ordenar as conversas antigas.
    #[serde(default)]
    pub updated: i64,
    pub messages: Vec<Message>,
    #[serde(skip)]
    stopping: bool,
    /// Codex: a mensagem do agente que está chegando (um turno pode ter várias).
    #[serde(skip)]
    item: Option<String>,
    /// Acabou de entrar a linha em branco entre duas mensagens.
    #[serde(skip)]
    boundary: bool,
    /// Imagens sendo geradas neste turno.
    #[serde(skip)]
    generating: usize,
}

impl Conversation {
    /// Conversa nova. No Claude o id do Zefiro é também o da sessão (o Zefiro escolhe); no Codex,
    /// o `thread` só existe depois do `thread/start`.
    pub fn new(id: String, agent: Agent) -> Self {
        let session = if agent == Agent::Claude { id.clone() } else { String::new() };
        Conversation { id, agent, session, ..Default::default() }
    }

    /// Título na lista de conversas: a primeira linha da primeira pergunta.
    pub fn title(&self) -> String {
        let first = self.messages.iter().find(|m| m.role == Role::User);
        let fallback = if first.is_some_and(|m| !m.images.is_empty()) { "Imagem" } else { "Conversa sem perguntas" };
        let line = first.map_or("", |m| m.text.as_str()).lines().map(str::trim).find(|l| !l.is_empty()).unwrap_or(fallback);
        let mut t: String = line.chars().take(80).collect();
        if line.chars().count() > 80 {
            t.push('…');
        }
        t
    }

    fn reply_mut(&mut self) -> Option<&mut Message> {
        self.messages.last_mut().filter(|m| m.role == Role::Claude && m.state.busy())
    }

    pub fn busy(&self) -> bool {
        self.messages.last().is_some_and(|m| m.role == Role::Claude && m.state.busy())
    }

    /// Manda uma pergunta: ela e uma resposta vazia esperando. `false` com resposta em andamento ou
    /// texto em branco.
    pub fn ask(&mut self, text: &str) -> bool {
        self.ask_with(text, Vec::new())
    }

    /// Pergunta com imagens; só imagens, sem texto, também vale.
    pub fn ask_with(&mut self, text: &str, images: Vec<String>) -> bool {
        let text = text.trim_end();
        if self.busy() || (text.trim().is_empty() && images.is_empty()) {
            return false;
        }
        self.stopping = false;
        self.item = None;
        self.boundary = false;
        self.generating = 0;
        self.messages.push(Message { role: Role::User, text: text.chars().take(MAX_PROMPT).collect(), state: State::Done, note: None, images });
        self.messages.push(Message { role: Role::Claude, text: String::new(), state: State::Waiting, note: None, images: Vec::new() });
        true
    }

    /// A pergunta da resposta que ainda nem começou (para mandar de novo noutra partida).
    pub fn unanswered(&self) -> Option<&Message> {
        match self.messages.as_slice() {
            [.., question, reply] if question.role == Role::User && reply.state == State::Waiting && reply.text.is_empty() => Some(question),
            _ => None,
        }
    }

    /// Há imagem sendo gerada na resposta em andamento.
    pub fn generating(&self) -> bool {
        self.generating > 0 && self.busy()
    }

    /// Imagem gerada (já gravada com este id) entra na resposta em andamento. `false` sem resposta.
    pub fn add_reply_image(&mut self, id: String) -> bool {
        let Some(m) = self.reply_mut() else { return false };
        m.images.push(id);
        if m.state == State::Waiting || m.state == State::Thinking {
            m.state = State::Streaming;
        }
        self.generating = self.generating.saturating_sub(1);
        true
    }

    /// Outra mensagem do agente no mesmo turno: uma linha em branco antes dela.
    fn next_item(&mut self, item: Option<&String>) {
        let Some(item) = item else { return };
        if self.item.as_ref() == Some(item) {
            return;
        }
        self.item = Some(item.clone());
        if let Some(m) = self.reply_mut()
            && !m.text.is_empty()
        {
            // As quebras do fim da mensagem anterior não somam com a linha em branco.
            m.text.truncate(m.text.trim_end_matches(['\n', '\r']).len());
            m.text.push_str("\n\n");
            self.boundary = true;
        }
    }

    /// Acrescenta texto à resposta em andamento. Logo depois da linha em branco entre duas
    /// mensagens, as quebras do começo da próxima também não somam.
    fn push_text(&mut self, text: &str) {
        let text = if self.boundary { text.trim_start_matches(['\n', '\r']) } else { text };
        if !text.is_empty() {
            self.boundary = false;
        }
        if let Some(m) = self.reply_mut() {
            m.text.push_str(text);
            m.state = State::Streaming;
        }
    }

    /// Todas as imagens da conversa (para apagar junto).
    pub fn images(&self) -> impl Iterator<Item = &str> {
        self.messages.iter().flat_map(|m| m.images.iter().map(String::as_str))
    }

    /// Um evento do processo. `true` se algo na tela mudou. Os erros do fim do turno vão por
    /// [`Conversation::fail`], depois de o laço decidir se tenta de novo.
    pub fn apply(&mut self, event: &Event) -> bool {
        match event {
            Event::Started { session } => {
                self.created = true;
                if !session.is_empty() {
                    self.session.clone_from(session);
                }
                false
            }
            Event::Thinking => match self.reply_mut() {
                Some(m) if m.state == State::Waiting => {
                    m.state = State::Thinking;
                    true
                }
                _ => false,
            },
            Event::Delta { text, item } => {
                if self.reply_mut().is_none() {
                    return false;
                }
                self.next_item(item.as_ref());
                self.push_text(text);
                true
            }
            // Mensagem inteira de um item que não mandou pedaços (no Codex, outra mensagem do turno).
            Event::Assistant { text, item: Some(item) } if self.reply_mut().is_some() && self.item.as_ref() != Some(item) => {
                self.next_item(Some(item));
                self.push_text(text);
                true
            }
            Event::Assistant { text, .. } | Event::Finished { text: Some(text), error: None } if self.reply_mut().is_some_and(|m| m.text.is_empty()) => {
                let finished = matches!(event, Event::Finished { .. });
                if let Some(m) = self.reply_mut() {
                    m.text.clone_from(text);
                    m.state = if finished { State::Done } else { State::Streaming };
                }
                if finished {
                    self.generating = 0;
                }
                true
            }
            Event::Finished { error: None, .. } => {
                self.generating = 0;
                match self.reply_mut() {
                    Some(m) => {
                        m.state = State::Done;
                        true
                    }
                    None => false,
                }
            }
            Event::ImageStarted if self.busy() => {
                self.generating += 1;
                true
            }
            Event::ImageFailed if self.generating > 0 => {
                self.generating -= 1;
                true
            }
            // A imagem gerada entra pela [`Conversation::add_reply_image`], depois de gravada.
            Event::Assistant { .. } | Event::Finished { .. } | Event::ImageStarted | Event::ImageFailed | Event::Image { .. } | Event::Turn(_) | Event::Models(_) | Event::Other => false,
        }
    }

    /// O usuário pediu para parar: o erro que vier depois é a parada, não uma falha.
    pub fn stop_requested(&mut self) {
        self.stopping = self.busy();
    }

    /// O turno acabou com erro (ou o processo caiu no meio).
    pub fn fail(&mut self, failure: &Failure) {
        let stopping = std::mem::take(&mut self.stopping);
        self.generating = 0;
        if let Some(m) = self.reply_mut() {
            if stopping || *failure == Failure::Interrupted {
                m.state = State::Stopped;
                m.note = Some(failure_text(&Failure::Interrupted));
            } else {
                m.state = State::Failed;
                m.note = Some(failure_text(failure));
            }
        }
    }

    /// Texto da última resposta com conteúdo (Ctrl+Shift+C).
    pub fn last_reply(&self) -> Option<&str> {
        self.messages.iter().rev().find(|m| m.role == Role::Claude && !m.text.is_empty()).map(|m| m.text.as_str())
    }

    pub fn to_json(&self) -> String {
        serde_json::to_string_pretty(self).unwrap_or_default()
    }

    /// Lê a conversa gravada. Uma resposta que estava chegando quando o Zefiro fechou vira parada.
    pub fn from_json(text: &str) -> Option<Self> {
        let mut c: Conversation = serde_json::from_str(text).ok()?;
        if c.id.is_empty() {
            // Primeira versão: um arquivo só, sem id; o id era o da sessão do Claude.
            c.id.clone_from(&c.session);
        }
        if c.id.is_empty() {
            return None;
        }
        for m in c.messages.iter_mut().filter(|m| m.state.busy()) {
            m.state = State::Stopped;
            m.note = Some(failure_text(&Failure::Interrupted));
        }
        Some(c)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn delta(text: &str) -> Event {
        Event::Delta { text: text.into(), item: None }
    }

    fn assistant(text: &str) -> Event {
        Event::Assistant { text: text.into(), item: None }
    }

    fn item_delta(item: &str, text: &str) -> Event {
        Event::Delta { text: text.into(), item: Some(item.into()) }
    }

    fn asked() -> Conversation {
        let mut c = Conversation::new("s1".into(), Agent::Claude);
        assert!(c.ask("Qual a capital do Peru?  \n"));
        c
    }

    #[test]
    fn asking_adds_the_question_and_a_waiting_reply() {
        let c = asked();
        assert_eq!(c.messages.len(), 2);
        assert_eq!((c.messages[0].role, c.messages[0].text.as_str()), (Role::User, "Qual a capital do Peru?"));
        assert_eq!((c.messages[1].role, c.messages[1].state), (Role::Claude, State::Waiting));
        assert!(c.busy());
        assert_eq!(c.unanswered().map(|m| m.text.as_str()), Some("Qual a capital do Peru?"));
    }

    #[test]
    fn questions_can_carry_images_or_be_only_images() {
        let mut c = Conversation::new("s".into(), Agent::Codex);
        assert!(!c.ask_with("  ", Vec::new()));
        assert!(c.ask_with("", vec!["img-1".into()]));
        assert_eq!(c.messages[0].images, ["img-1"]);
        assert_eq!(c.unanswered().map(|m| m.images.clone()), Some(vec!["img-1".to_string()]));
        assert_eq!(c.title(), "Imagem");
        c.apply(&Event::Finished { text: Some("ok".into()), error: None });
        assert!(c.ask_with("e esta?", vec!["img-2".into()]));
        assert_eq!(c.images().collect::<Vec<_>>(), ["img-1", "img-2"]);
        let back = Conversation::from_json(&c.to_json()).unwrap();
        assert_eq!(back.messages[2].images, ["img-2"]);
        assert!(!c.to_json().contains("\"images\": []"), "sem a chave quando não há imagem");
    }

    #[test]
    fn cannot_ask_while_busy_or_blank() {
        let mut c = asked();
        assert!(!c.ask("outra"), "com resposta em andamento");
        let mut fresh = Conversation::new("s".into(), Agent::Claude);
        assert!(!fresh.ask("  \n "));
        assert!(fresh.messages.is_empty());
    }

    #[test]
    fn streaming_builds_the_reply() {
        let mut c = asked();
        assert!(!c.apply(&Event::Started { session: "s1".into() }));
        assert!(c.created);
        assert!(c.apply(&Event::Thinking));
        assert_eq!(c.messages[1].state, State::Thinking);
        assert!(c.apply(&delta("Li")));
        assert!(c.apply(&delta("ma")));
        assert_eq!(c.unanswered(), None, "já começou");
        assert!(!c.apply(&Event::Thinking), "pensando depois do texto não muda nada");
        assert!(!c.apply(&assistant("Lima")), "o texto completo repete os pedaços");
        assert!(c.apply(&Event::Finished { text: Some("Lima".into()), error: None }));
        assert_eq!((c.messages[1].text.as_str(), c.messages[1].state), ("Lima", State::Done));
        assert!(!c.busy());
        assert!(!c.apply(&delta("x")), "nada chega fora de um turno");
    }

    #[test]
    fn codex_messages_in_one_turn_get_a_blank_line_between_them() {
        let mut c = asked();
        assert!(c.apply(&item_delta("m1", "Vou gerar a ")));
        assert!(c.apply(&item_delta("m1", "imagem.")));
        assert!(!c.apply(&Event::Assistant { text: "Vou gerar a imagem.".into(), item: Some("m1".into()) }), "o texto completo repete os pedaços");
        assert!(c.apply(&item_delta("m2", "Pronto!")));
        assert_eq!(c.messages[1].text, "Vou gerar a imagem.\n\nPronto!");
        // Uma mensagem que chega inteira, sem pedaços, também vai separada.
        assert!(c.apply(&Event::Assistant { text: "Mais uma.".into(), item: Some("m3".into()) }));
        assert_eq!(c.messages[1].text, "Vou gerar a imagem.\n\nPronto!\n\nMais uma.");
        c.apply(&Event::Finished { text: None, error: None });
        assert!(c.ask("outra"));
        assert!(c.apply(&item_delta("m3", "Oi")), "o item da resposta anterior não conta");
        assert_eq!(c.messages[3].text, "Oi");
        // Quebras de linha nas pontas das mensagens não somam: fica uma linha em branco só.
        c.apply(&item_delta("m3", ".\n"));
        c.apply(&item_delta("m4", "\n"));
        c.apply(&item_delta("m4", "\nPronto!"));
        assert_eq!(c.messages[3].text, "Oi.\n\nPronto!");
        c.apply(&item_delta("m4", "\n\nfim"));
        assert_eq!(c.messages[3].text, "Oi.\n\nPronto!\n\nfim", "dentro da mesma mensagem, o texto fica como veio");
        c.apply(&Event::Finished { text: None, error: None });
        assert!(c.ask("mais"));
        c.apply(&item_delta("m5", "\nOi"));
        assert_eq!(c.messages[5].text, "\nOi", "a primeira mensagem da resposta fica como veio");
    }

    #[test]
    fn generated_images_join_the_reply() {
        let mut c = Conversation::new("s".into(), Agent::Codex);
        c.ask("um bonsai");
        assert!(!c.generating());
        assert!(c.apply(&Event::ImageStarted));
        assert!(c.generating());
        assert!(c.add_reply_image("gen-1".into()));
        assert!(!c.generating(), "a imagem chegou");
        c.apply(&item_delta("m1", "Pronto!"));
        c.apply(&Event::Finished { text: None, error: None });
        assert_eq!(c.messages[1].images, ["gen-1"]);
        assert_eq!(c.images().collect::<Vec<_>>(), ["gen-1"], "apagar a conversa apaga a imagem");
        assert!(!c.add_reply_image("gen-2".into()), "fora de um turno não há resposta");
        let back = Conversation::from_json(&c.to_json()).unwrap();
        assert_eq!(back.messages[1].images, ["gen-1"], "volta ao reabrir");
        c.ask("outra");
        c.apply(&Event::ImageStarted);
        assert!(c.apply(&Event::ImageFailed));
        assert!(!c.generating());
        c.apply(&Event::ImageStarted);
        c.apply(&Event::Finished { text: None, error: None });
        assert!(!c.generating(), "o fim do turno encerra a espera");
    }

    #[test]
    fn reply_without_partials_comes_from_assistant_or_result() {
        let mut c = asked();
        assert!(c.apply(&assistant("Lima")));
        assert_eq!(c.messages[1].state, State::Streaming);
        assert!(c.apply(&Event::Finished { text: None, error: None }));
        assert_eq!((c.messages[1].text.as_str(), c.messages[1].state), ("Lima", State::Done));
        let mut c = asked();
        assert!(c.apply(&Event::Finished { text: Some("Lima".into()), error: None }));
        assert_eq!((c.messages[1].text.as_str(), c.messages[1].state), ("Lima", State::Done));
    }

    #[test]
    fn stop_keeps_the_text_and_notes_it() {
        let mut c = asked();
        c.apply(&delta("Li"));
        c.stop_requested();
        c.fail(&Failure::Other("error_during_execution".into()));
        assert_eq!((c.messages[1].text.as_str(), c.messages[1].state), ("Li", State::Stopped));
        assert_eq!(c.messages[1].note.as_deref(), Some("Resposta interrompida."));
        assert!(c.ask("de novo"), "pode perguntar depois de parar");
    }

    #[test]
    fn failure_notes_the_error() {
        let mut c = asked();
        c.fail(&Failure::NotLoggedIn);
        assert_eq!(c.messages[1].state, State::Failed);
        assert!(c.messages[1].note.as_deref().unwrap().contains("login"));
        assert!(!c.busy());
    }

    #[test]
    fn last_reply_skips_empty_ones() {
        let mut c = asked();
        c.apply(&delta("Lima"));
        c.apply(&Event::Finished { text: None, error: None });
        c.ask("e do Chile?");
        c.fail(&Failure::UsageLimit);
        assert_eq!(c.last_reply(), Some("Lima"));
    }

    #[test]
    fn json_round_trip_and_interrupted_replies() {
        let mut c = asked();
        c.origin = "Windows".into();
        c.apply(&Event::Started { session: "s1".into() });
        c.apply(&delta("Li"));
        let back = Conversation::from_json(&c.to_json()).unwrap();
        assert_eq!((back.session.as_str(), back.created, back.origin.as_str()), ("s1", true, "Windows"));
        assert_eq!((back.messages[1].text.as_str(), back.messages[1].state), ("Li", State::Stopped), "chegando quando o Zefiro fechou");
        assert!(!back.busy());
        assert_eq!(Conversation::from_json("{}"), None);
        let old = Conversation::from_json(r#"{"session":"abc","created":true,"origin":"Windows","messages":[]}"#).unwrap();
        assert_eq!((old.id.as_str(), old.agent), ("abc", Agent::Claude), "arquivo da primeira versão");
        assert_eq!(Conversation::from_json("lixo"), None);
    }
}
