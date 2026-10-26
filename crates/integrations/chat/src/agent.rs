//! Com quem o chat conversa: o Claude (`claude -p`) ou o Codex (`codex app-server`), e os modelos.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Agent {
    #[default]
    Claude,
    Codex,
}

impl Agent {
    pub const ALL: [Agent; 2] = [Agent::Claude, Agent::Codex];

    /// `[chat] agent`: "codex" (sem diferenciar maiúsculas) ou o Claude.
    pub fn parse(value: &str) -> Agent {
        if value.trim().eq_ignore_ascii_case("codex") { Agent::Codex } else { Agent::Claude }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Agent::Claude => "claude",
            Agent::Codex => "codex",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Agent::Claude => "Claude",
            Agent::Codex => "Codex",
        }
    }

    /// Nome do executável (`claude`, `codex`), no Windows com `.exe`.
    pub fn binary(self) -> &'static str {
        self.as_str()
    }
}

/// Um modelo no seletor. `id` vazio = o padrão do Claude Code.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModelOption { pub id: String, pub name: String, pub default: bool }

/// Modelos do Claude: os aliases do `--model`.
pub fn claude_models() -> Vec<ModelOption> {
    [("", "Padrão", true), ("opus", "Opus", false), ("sonnet", "Sonnet", false), ("haiku", "Haiku", false)]
        .iter()
        .map(|(id, name, default)| ModelOption { id: id.to_string(), name: name.to_string(), default: *default })
        .collect()
}

/// O modelo que vale: o escolhido, se está na lista; senão o padrão da lista. Sem lista (o Codex
/// ainda não respondeu), o escolhido.
pub fn effective_model(models: &[ModelOption], chosen: &str) -> String {
    if models.is_empty() || models.iter().any(|m| m.id == chosen) {
        return chosen.to_string();
    }
    models.iter().find(|m| m.default).map(|m| m.id.clone()).unwrap_or_default()
}

/// Nome do modelo no seletor.
pub fn model_label(models: &[ModelOption], chosen: &str) -> String {
    let id = effective_model(models, chosen);
    models.iter().find(|m| m.id == id).map(|m| m.name.clone()).unwrap_or_else(|| if id.is_empty() { "Padrão".into() } else { id })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_and_names() {
        assert_eq!(Agent::parse("Codex"), Agent::Codex);
        assert_eq!(Agent::parse(""), Agent::Claude);
        assert_eq!(Agent::parse("outro"), Agent::Claude);
        assert_eq!((Agent::Codex.as_str(), Agent::Codex.label(), Agent::Claude.binary()), ("codex", "Codex", "claude"));
    }

    #[test]
    fn claude_models_start_with_the_default() {
        let m = claude_models();
        assert_eq!(m.iter().map(|m| m.name.as_str()).collect::<Vec<_>>(), ["Padrão", "Opus", "Sonnet", "Haiku"]);
        assert!(m[0].default && m[0].id.is_empty());
    }

    #[test]
    fn model_falls_back_to_the_list_default() {
        let codex = vec![
            ModelOption { id: "gpt-5.6-sol".into(), name: "GPT-5.6-Sol".into(), default: true },
            ModelOption { id: "gpt-5.6-luna".into(), name: "GPT-5.6-Luna".into(), default: false },
        ];
        assert_eq!(effective_model(&codex, "gpt-5.6-luna"), "gpt-5.6-luna");
        assert_eq!(effective_model(&codex, "gpt-6.1-sol"), "gpt-5.6-sol", "fora da lista desta versão");
        assert_eq!(effective_model(&codex, ""), "gpt-5.6-sol");
        assert_eq!(effective_model(&[], "x"), "x", "sem lista ainda");
        assert_eq!(model_label(&codex, ""), "GPT-5.6-Sol");
        assert_eq!(model_label(&claude_models(), "sonnet"), "Sonnet");
        assert_eq!(model_label(&[], ""), "Padrão");
    }
}
