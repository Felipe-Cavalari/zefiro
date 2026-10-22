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
        let first = self.messages.iter().find(|m| m.role == Role::User).map_or("", |m| m.text.as_str());
        let line = first.lines().map(str::trim).find(|l| !l.is_empty()).unwrap_or("Conversa sem perguntas");
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
        let text = text.trim_end();
        if self.busy() || text.trim().is_empty() {
            return false;
        }
        self.stopping = false;
        self.messages.push(Message { role: Role::User, text: text.chars().take(MAX_PROMPT).collect(), state: State::Done, note: None });
        self.messages.push(Message { role: Role::Claude, text: String::new(), state: State::Waiting, note: None });
        true
    }

    /// A pergunta da resposta que ainda nem começou (para mandar de novo noutra partida).
    pub fn unanswered(&self) -> Option<&str> {
        match self.messages.as_slice() {
            [.., question, reply] if question.role == Role::User && reply.state == State::Waiting && reply.text.is_empty() => Some(&question.text),
            _ => None,
        }
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
            Event::Delta(text) => match self.reply_mut() {
                Some(m) => {
                    m.text.push_str(text);
                    m.state = State::Streaming;
                    true
                }
                None => false,
            },
            Event::Assistant(text) | Event::Finished { text: Some(text), error: None } if self.reply_mut().is_some_and(|m| m.text.is_empty()) => {
                let finished = matches!(event, Event::Finished { .. });
                if let Some(m) = self.reply_mut() {
                    m.text.clone_from(text);
                    m.state = if finished { State::Done } else { State::Streaming };
                }
                true
            }
            Event::Finished { error: None, .. } => match self.reply_mut() {
                Some(m) => {
                    m.state = State::Done;
                    true
                }
                None => false,
            },
            Event::Assistant(_) | Event::Finished { .. } | Event::Turn(_) | Event::Models(_) | Event::Other => false,
        }
    }

    /// O usuário pediu para parar: o erro que vier depois é a parada, não uma falha.
    pub fn stop_requested(&mut self) {
        self.stopping = self.busy();
    }

    /// O turno acabou com erro (ou o processo caiu no meio).
    pub fn fail(&mut self, failure: &Failure) {
        let stopping = std::mem::take(&mut self.stopping);
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
        assert_eq!(c.unanswered(), Some("Qual a capital do Peru?"));
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
        assert!(c.apply(&Event::Delta("Li".into())));
        assert!(c.apply(&Event::Delta("ma".into())));
        assert_eq!(c.unanswered(), None, "já começou");
        assert!(!c.apply(&Event::Thinking), "pensando depois do texto não muda nada");
        assert!(!c.apply(&Event::Assistant("Lima".into())), "o texto completo repete os pedaços");
        assert!(c.apply(&Event::Finished { text: Some("Lima".into()), error: None }));
        assert_eq!((c.messages[1].text.as_str(), c.messages[1].state), ("Lima", State::Done));
        assert!(!c.busy());
        assert!(!c.apply(&Event::Delta("x".into())), "nada chega fora de um turno");
    }

    #[test]
    fn reply_without_partials_comes_from_assistant_or_result() {
        let mut c = asked();
        assert!(c.apply(&Event::Assistant("Lima".into())));
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
        c.apply(&Event::Delta("Li".into()));
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
        c.apply(&Event::Delta("Lima".into()));
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
        c.apply(&Event::Delta("Li".into()));
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
