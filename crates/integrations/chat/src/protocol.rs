//! Linhas do `claude -p --input-format stream-json --output-format stream-json`: o que o Zefiro
//! escreve no stdin e o que interessa no stdout.

use serde_json::{Value, json};

/// O que uma linha do stdout diz ao chat. O resto (status, hooks, limites, resumos) é `Other`.
#[derive(Debug, Clone, PartialEq)]
pub enum Event {
    /// `system/init`: o turno começou nesta sessão.
    Started { session: String },
    /// O modelo está pensando (bloco ou delta de raciocínio), ainda sem texto.
    Thinking,
    /// Pedaço de texto da resposta. `item`: no Codex, a mensagem a que o pedaço pertence (um turno
    /// pode ter várias, como o aviso antes de uma ferramenta e a resposta final).
    Delta { text: String, item: Option<String> },
    /// Mensagem completa do assistente (vem depois dos pedaços; serve quando eles não vêm).
    Assistant { text: String, item: Option<String> },
    /// Codex: começou a gerar uma imagem.
    ImageStarted,
    /// Codex: imagem gerada (PNG em base64). `item`: o id do item no Codex.
    Image { item: String, data: String },
    /// Codex: a geração da imagem falhou (limite de uso, recusa).
    ImageFailed,
    /// Fim do turno. `error` é o texto do erro (ou o subtipo, sem texto).
    Finished { text: Option<String>, error: Option<String> },
    /// Codex: o turno começou com este id (para interromper).
    Turn(String),
    /// Codex: os modelos que esta versão aceita.
    Models(Vec<crate::agent::ModelOption>),
    Other,
}

/// Mensagem do usuário, numa linha.
pub fn user_line(text: &str) -> String {
    user_message(text, &[])
}

/// Mensagem do usuário com imagens PNG (em base64) antes do texto. Sem texto, só as imagens.
pub fn user_message(text: &str, images: &[String]) -> String {
    if images.is_empty() {
        return json!({ "type": "user", "message": { "role": "user", "content": text } }).to_string();
    }
    let mut content: Vec<Value> = images.iter().map(|data| json!({ "type": "image", "source": { "type": "base64", "media_type": "image/png", "data": data } })).collect();
    if !text.trim().is_empty() {
        content.push(json!({ "type": "text", "text": text }));
    }
    json!({ "type": "user", "message": { "role": "user", "content": content } }).to_string()
}

/// Pede para parar a resposta em andamento; o processo continua vivo para o próximo turno.
pub fn interrupt_line(request_id: &str) -> String {
    json!({ "type": "control_request", "request_id": request_id, "request": { "subtype": "interrupt" } }).to_string()
}

fn text_blocks(content: &Value) -> String {
    content.as_array().into_iter().flatten().filter(|b| b["type"] == "text").filter_map(|b| b["text"].as_str()).collect()
}

pub fn parse_line(line: &str) -> Event {
    let Ok(v) = serde_json::from_str::<Value>(line.trim()) else { return Event::Other };
    match v["type"].as_str() {
        Some("system") if v["subtype"] == "init" => Event::Started { session: v["session_id"].as_str().unwrap_or_default().to_string() },
        Some("stream_event") => {
            let e = &v["event"];
            match e["type"].as_str() {
                Some("content_block_delta") => match e["delta"]["type"].as_str() {
                    Some("text_delta") => e["delta"]["text"].as_str().map_or(Event::Other, |t| Event::Delta { text: t.to_string(), item: None }),
                    Some("thinking_delta") => Event::Thinking,
                    _ => Event::Other,
                },
                Some("content_block_start") if e["content_block"]["type"] == "thinking" => Event::Thinking,
                _ => Event::Other,
            }
        }
        Some("assistant") => {
            let text = text_blocks(&v["message"]["content"]);
            if text.is_empty() { Event::Other } else { Event::Assistant { text, item: None } }
        }
        Some("result") => {
            let text = v["result"].as_str().filter(|t| !t.is_empty()).map(str::to_string);
            if v["is_error"].as_bool() == Some(true) {
                let errors: Vec<&str> = v["errors"].as_array().into_iter().flatten().filter_map(Value::as_str).collect();
                let error = if !errors.is_empty() { errors.join("; ") } else { text.clone().unwrap_or_else(|| v["subtype"].as_str().unwrap_or("erro").to_string()) };
                Event::Finished { text: None, error: Some(error) }
            } else {
                Event::Finished { text, error: None }
            }
        }
        _ => Event::Other,
    }
}

/// O que deu errado, para a interface e para tentar de novo.
#[derive(Debug, Clone, PartialEq)]
pub enum Failure {
    /// `--resume` de uma sessão que esta origem não tem: começa outra.
    SessionMissing,
    /// `--session-id` de uma sessão que já existe: retoma.
    SessionInUse,
    NotLoggedIn,
    UsageLimit,
    /// Parada pedida pelo usuário.
    Interrupted,
    Other(String),
}

/// Classifica o texto de um erro (do `result` ou do stderr).
pub fn classify(error: &str) -> Failure {
    let lower = error.to_lowercase();
    if lower.contains("no conversation found") || lower.contains("no rollout found") || lower.contains("thread not found") || lower.contains("session not found") {
        Failure::SessionMissing
    } else if lower.contains("already in use") {
        Failure::SessionInUse
    } else if lower.contains("not logged in") || lower.contains("/login") || lower.contains("invalid api key") || lower.contains("oauth token") || lower.contains("unauthenticated") || lower.contains("not authenticated") || lower.contains("authentication required") {
        Failure::NotLoggedIn
    } else if lower.contains("usage limit") || lower.contains("limit reached") || lower.contains("rate limit") || lower.contains("quota") || lower.contains("out of credits") {
        Failure::UsageLimit
    } else if lower == "error_during_execution" || lower.contains("interrupt") || lower.contains("aborted") {
        Failure::Interrupted
    } else {
        Failure::Other(error.trim().to_string())
    }
}

/// Texto do erro para o painel, com o nome do agente onde importa (o login e a cota do Grok são
/// outros; Claude e Codex mantêm os textos de sempre).
pub fn failure_text_for(agent: crate::agent::Agent, f: &Failure) -> String {
    if agent != crate::agent::Agent::Grok {
        return failure_text(f);
    }
    match f {
        Failure::NotLoggedIn => "Entre no Grok Build para usar o chat: rode `grok login` num terminal (no WSL, `grok login --device-auth`).".into(),
        Failure::UsageLimit => "A cota semanal do Grok acabou. O chip do Grok mostra quando ela volta.".into(),
        Failure::Other(text) if text.is_empty() => "O Grok parou de responder.".into(),
        f => failure_text(f),
    }
}

/// Texto do erro para o painel.
pub fn failure_text(f: &Failure) -> String {
    match f {
        Failure::NotLoggedIn => "Entre no Claude Code para usar o chat: rode `claude` num terminal e faça o login.".into(),
        Failure::UsageLimit => "O limite de uso do Claude foi atingido. Tente de novo mais tarde.".into(),
        Failure::Interrupted => "Resposta interrompida.".into(),
        Failure::SessionMissing | Failure::SessionInUse => "Não consegui retomar a conversa.".into(),
        Failure::Other(text) if text.is_empty() => "O Claude parou de responder.".into(),
        Failure::Other(text) => text.clone(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Linhas reais do Claude Code 2.1.307 (encurtadas nos campos que não importam).
    const INIT: &str = r#"{"type":"system","subtype":"init","cwd":"C:\\x","session_id":"0673b2d0-53b5-47ea-8d84-b078a50bf1f5","tools":[]}"#;
    const DELTA: &str = r#"{"type":"stream_event","event":{"type":"content_block_delta","index":0,"delta":{"type":"text_delta","text":"o"}},"session_id":"s"}"#;
    const THINKING: &str = r#"{"type":"stream_event","event":{"type":"content_block_start","index":0,"content_block":{"type":"thinking","thinking":""}},"session_id":"s"}"#;
    const ASSISTANT: &str = r#"{"type":"assistant","message":{"model":"m","id":"msg","type":"message","role":"assistant","content":[{"type":"thinking","thinking":"x"},{"type":"text","text":"oi"}]}}"#;
    const DONE: &str = r#"{"type":"result","subtype":"success","is_error":false,"result":"oi","session_id":"s","duration_ms":1202}"#;
    const MISSING: &str = r#"{"type":"result","subtype":"error_during_execution","duration_ms":0,"is_error":true,"num_turns":0,"session_id":"1111","errors":["No conversation found with session ID: 1111"]}"#;
    const INTERRUPTED: &str = r#"{"type":"result","subtype":"error_during_execution","is_error":true,"session_id":"s"}"#;
    const STATUS: &str = r#"{"type":"system","subtype":"status","status":"requesting","session_id":"s"}"#;
    const LIMITS: &str = r#"{"type":"rate_limit_event","rate_limit_info":{"status":"allowed"}}"#;

    #[test]
    fn reads_the_events_the_chat_needs() {
        assert_eq!(parse_line(INIT), Event::Started { session: "0673b2d0-53b5-47ea-8d84-b078a50bf1f5".into() });
        assert_eq!(parse_line(DELTA), Event::Delta { text: "o".into(), item: None });
        assert_eq!(parse_line(THINKING), Event::Thinking);
        assert_eq!(parse_line(ASSISTANT), Event::Assistant { text: "oi".into(), item: None }, "só os blocos de texto");
        assert_eq!(parse_line(DONE), Event::Finished { text: Some("oi".into()), error: None });
    }

    #[test]
    fn errors_come_from_the_errors_list_or_the_subtype() {
        assert_eq!(parse_line(MISSING), Event::Finished { text: None, error: Some("No conversation found with session ID: 1111".into()) });
        assert_eq!(parse_line(INTERRUPTED), Event::Finished { text: None, error: Some("error_during_execution".into()) });
    }

    #[test]
    fn everything_else_is_other() {
        for line in [STATUS, LIMITS, "", "não é json", r#"{"type":"stream_event","event":{"type":"message_stop"}}"#] {
            assert_eq!(parse_line(line), Event::Other, "{line}");
        }
    }

    #[test]
    fn input_lines_are_single_line_json() {
        let line = user_line("linha 1\nlinha \"2\" ç");
        assert!(!line.contains('\n'));
        let v: Value = serde_json::from_str(&line).unwrap();
        assert_eq!(v["type"], "user");
        assert_eq!(v["message"]["role"], "user");
        assert_eq!(v["message"]["content"], "linha 1\nlinha \"2\" ç");
        let v: Value = serde_json::from_str(&interrupt_line("z1")).unwrap();
        assert_eq!((v["type"].as_str(), v["request_id"].as_str(), v["request"]["subtype"].as_str()), (Some("control_request"), Some("z1"), Some("interrupt")));
    }

    #[test]
    fn images_go_before_the_text_as_base64_blocks() {
        let v: Value = serde_json::from_str(&user_message("O que é isto?", &["QUJD".into()])).unwrap();
        let content = &v["message"]["content"];
        assert_eq!(content[0], json!({ "type": "image", "source": { "type": "base64", "media_type": "image/png", "data": "QUJD" } }));
        assert_eq!(content[1], json!({ "type": "text", "text": "O que é isto?" }));
        let only: Value = serde_json::from_str(&user_message("  ", &["QUJD".into()])).unwrap();
        assert_eq!(only["message"]["content"].as_array().unwrap().len(), 1, "sem texto, só a imagem");
        assert_eq!(user_message("oi", &[]), user_line("oi"));
    }

    #[test]
    fn failures_are_classified() {
        assert_eq!(classify("No conversation found with session ID: x"), Failure::SessionMissing);
        assert_eq!(classify("no rollout found for thread id 01a1"), Failure::SessionMissing, "Codex");
        assert_eq!(classify("interrupted"), Failure::Interrupted, "Codex");
        assert_eq!(classify("Error: Session ID x is already in use."), Failure::SessionInUse);
        assert_eq!(classify("Not logged in · Please run /login"), Failure::NotLoggedIn);
        assert_eq!(classify("Claude AI usage limit reached|1792624800"), Failure::UsageLimit);
        assert_eq!(classify("error_during_execution"), Failure::Interrupted);
        assert_eq!(classify("  algo estranho "), Failure::Other("algo estranho".into()));
        assert!(failure_text(&Failure::NotLoggedIn).contains("claude"));
        assert_eq!(failure_text(&Failure::Other(String::new())), "O Claude parou de responder.");
    }

    #[test]
    fn grok_failures() {
        use crate::agent::Agent;
        assert_eq!(classify("session not found"), Failure::SessionMissing, "FS_NOT_FOUND no session/load");
        assert_eq!(classify("Unauthenticated: run grok login"), Failure::NotLoggedIn);
        assert_eq!(classify("Weekly quota exceeded"), Failure::UsageLimit);
        assert!(failure_text_for(Agent::Grok, &Failure::NotLoggedIn).contains("`grok login`"));
        assert!(failure_text_for(Agent::Grok, &Failure::UsageLimit).contains("cota semanal do Grok"));
        assert_eq!(failure_text_for(Agent::Grok, &Failure::Other(String::new())), "O Grok parou de responder.");
        assert_eq!(failure_text_for(Agent::Grok, &Failure::Interrupted), "Resposta interrompida.");
        assert_eq!(failure_text_for(Agent::Claude, &Failure::NotLoggedIn), failure_text(&Failure::NotLoggedIn));
    }
}
