//! O `grok agent stdio` do Grok Build: ACP (Agent Client Protocol), JSON-RPC 2.0 por linhas no
//! stdin e no stdout (ADR-0027). O que o Zefiro pede e como as respostas e as notificações viram os
//! mesmos [`Event`]s do Claude e do Codex.

use crate::agent::ModelOption;
use crate::command::SYSTEM_PROMPT;
use crate::protocol::Event;
use serde_json::{Value, json};

fn request(id: u64, method: &str, params: Value) -> String {
    json!({ "jsonrpc": "2.0", "id": id, "method": method, "params": params }).to_string()
}

/// Sem `fs` nem `terminal`: o Zefiro não lê nem escreve arquivos para o agente.
pub fn initialize(id: u64, version: &str) -> String {
    request(id, "initialize", json!({ "protocolVersion": 1, "clientCapabilities": {}, "clientInfo": { "name": "zefiro", "title": "Zefiro", "version": version } }))
}

/// Só o login que a CLI já guardou. O método `grok.com` abre o navegador e nunca é pedido.
pub fn authenticate(id: u64) -> String {
    request(id, "authenticate", json!({ "methodId": "cached_token" }))
}

/// Conversa nova na pasta do chat, sem MCP, com o prompt do chat. Sem `yoloMode`.
pub fn session_new(id: u64, cwd: &str) -> String {
    request(id, "session/new", json!({ "cwd": cwd, "mcpServers": [], "_meta": { "rules": SYSTEM_PROMPT } }))
}

/// Retoma uma conversa gravada (o Grok reenvia o histórico como `session/update` antes da resposta).
pub fn session_load(id: u64, session: &str, cwd: &str) -> String {
    request(id, "session/load", json!({ "sessionId": session, "cwd": cwd, "mcpServers": [] }))
}

/// Uma pergunta, com as imagens PNG (base64) antes do texto. Sem texto, só as imagens.
pub fn prompt(id: u64, session: &str, text: &str, images: &[String]) -> String {
    let mut blocks: Vec<Value> = images.iter().map(|data| json!({ "type": "image", "mimeType": "image/png", "data": data })).collect();
    if !text.trim().is_empty() || blocks.is_empty() {
        blocks.push(json!({ "type": "text", "text": text }));
    }
    request(id, "session/prompt", json!({ "sessionId": session, "prompt": blocks }))
}

/// Para a resposta em andamento (notificação, sem resposta); o `session/prompt` responde
/// `cancelled`.
pub fn cancel(session: &str) -> String {
    json!({ "jsonrpc": "2.0", "method": "session/cancel", "params": { "sessionId": session } }).to_string()
}

/// Troca o modelo da conversa sem reiniciar o processo.
pub fn set_model(id: u64, session: &str, model: &str) -> String {
    request(id, "session/set_config_option", json!({ "sessionId": session, "configId": "model", "value": model }))
}

/// Cota da assinatura (extensão do Grok, com o sublinhado do ACP). Não chama o modelo.
pub fn billing(id: u64) -> String {
    request(id, "_x.ai/billing", json!({}))
}

/// Quem está logado (extensão do Grok): o `principalId` junta as origens da mesma conta.
pub fn auth_info(id: u64) -> String {
    request(id, "_x.ai/auth/info", json!({}))
}

/// Uma linha do agente.
#[derive(Debug, Clone, PartialEq)]
pub enum Incoming {
    Response { id: u64, result: Value },
    Failure { id: u64, message: String },
    Event(Event),
    /// A lista de modelos mudou (`_x.ai/models/update`), com o atual.
    Models { list: Vec<ModelOption>, current: Option<String> },
    /// O modelo chamou uma ferramenta: só o nome, para o log.
    Tool(String),
    /// Pedido do agente: a linha que o responde (permissão negada; o resto, recusado).
    ServerRequest(String),
    Other,
}

/// O texto de um erro do JSON-RPC. Sessão que não existe nesta origem (`FS_NOT_FOUND` no
/// `session/load`) vira "session not found", que o chat trata como sessão sumida.
pub fn error_text(error: &Value) -> String {
    if error["data"]["code"] == "FS_NOT_FOUND" {
        return "session not found".into();
    }
    let message = error["message"].as_str().unwrap_or("erro").to_string();
    match error["data"]["detail"].as_str().or(error["data"].as_str()) {
        Some(detail) if !detail.is_empty() => format!("{message}: {detail}"),
        _ => message,
    }
}

/// Resposta a um pedido do agente. Permissão: a opção de recusar uma vez (o chat não roda
/// ferramentas). Outros pedidos (`fs/*`, `terminal/*`): o cliente não oferece.
fn answer(v: &Value) -> String {
    let id = &v["id"];
    if v["method"] == "session/request_permission" {
        let options = v["params"]["options"].as_array().cloned().unwrap_or_default();
        let reject = ["reject_once", "reject_always"].iter().find_map(|kind| options.iter().find(|o| o["kind"] == *kind)).and_then(|o| o["optionId"].as_str());
        let outcome = match reject {
            Some(option) => json!({ "outcome": "selected", "optionId": option }),
            None => json!({ "outcome": "cancelled" }),
        };
        return json!({ "jsonrpc": "2.0", "id": id, "result": { "outcome": outcome } }).to_string();
    }
    json!({ "jsonrpc": "2.0", "id": id, "error": { "code": -32601, "message": "o chat do Zefiro não aceita este pedido" } }).to_string()
}

fn update(u: &Value) -> Incoming {
    match u["sessionUpdate"].as_str() {
        Some("agent_message_chunk") => match u["content"]["text"].as_str() {
            Some(text) if !text.is_empty() => Incoming::Event(Event::Delta { text: text.to_string(), item: None }),
            _ => Incoming::Other,
        },
        Some("agent_thought_chunk") => Incoming::Event(Event::Thinking),
        Some("tool_call") => {
            let name = u["_meta"]["x.ai/tool"]["name"].as_str().or(u["title"].as_str()).unwrap_or("?");
            Incoming::Tool(name.to_string())
        }
        _ => Incoming::Other,
    }
}

pub fn parse(line: &str) -> Incoming {
    let Ok(v) = serde_json::from_str::<Value>(line.trim()) else { return Incoming::Other };
    if let Some(method) = v["method"].as_str() {
        if !v["id"].is_null() {
            return Incoming::ServerRequest(answer(&v));
        }
        return match method {
            "session/update" => update(&v["params"]["update"]),
            "_x.ai/models/update" => {
                let (list, current) = models(&v["params"]);
                Incoming::Models { list, current }
            }
            _ => Incoming::Other,
        };
    }
    // Ids em texto (`skills-reload`) são pedidos internos do próprio Grok.
    let Some(id) = v["id"].as_u64() else { return Incoming::Other };
    match v.get("error") {
        Some(e) => Incoming::Failure { id, message: error_text(e) },
        None => Incoming::Response { id, result: v["result"].clone() },
    }
}

/// O fim do turno, pela resposta do `session/prompt`.
pub fn stop(result: &Value) -> Event {
    match result["stopReason"].as_str() {
        Some("cancelled") => Event::Finished { text: None, error: Some("interrupted".into()) },
        Some("refusal") => Event::Finished { text: None, error: Some("O Grok recusou responder.".into()) },
        _ => Event::Finished { text: None, error: None },
    }
}

/// O id da conversa na resposta do `session/new`.
pub fn session_id(result: &Value) -> Option<String> {
    result["sessionId"].as_str().filter(|s| !s.is_empty()).map(str::to_string)
}

/// A pasta de trabalho do processo (`initialize`): é o `cwd` das sessões, no Windows e no WSL.
pub fn workdir(result: &Value) -> Option<String> {
    result["_meta"]["currentWorkingDirectory"].as_str().filter(|s| !s.is_empty()).map(str::to_string)
}

/// O estado dos modelos numa resposta: `models` (`session/new`, `session/load`) ou
/// `_meta.modelState` (`initialize`).
pub fn model_state(result: &Value) -> Option<&Value> {
    [&result["models"], &result["_meta"]["modelState"]].into_iter().find(|s| s["availableModels"].is_array())
}

/// Os modelos (`availableModels`) e o atual (`currentModelId`). `default` fica falso: quem chama
/// marca o padrão (o primeiro atual que o processo anunciou).
pub fn models(state: &Value) -> (Vec<ModelOption>, Option<String>) {
    let list = state["availableModels"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|m| {
            let id = m["modelId"].as_str().filter(|s| !s.is_empty())?.to_string();
            let mut name = m["name"].as_str().filter(|s| !s.is_empty()).map_or_else(|| id.clone(), str::to_string);
            if m["description"].as_str().is_some_and(|d| d.to_lowercase().contains("2x the price")) {
                name.push_str(" (2× o preço)");
            }
            Some(ModelOption { id, name, default: false })
        })
        .collect();
    (list, state["currentModelId"].as_str().filter(|s| !s.is_empty()).map(str::to_string))
}

/// A lista com o padrão marcado.
pub fn with_default(mut list: Vec<ModelOption>, default: Option<&str>) -> Vec<ModelOption> {
    for m in &mut list {
        m.default = Some(m.id.as_str()) == default;
    }
    list
}

#[cfg(test)]
mod tests {
    use super::*;

    fn v(s: &str) -> Value {
        serde_json::from_str(s).unwrap()
    }

    // Linhas do Grok Build 1.0.70 (sonda de 2026-10-28), encurtadas nos campos que não importam.
    const INIT: &str = r#"{"jsonrpc":"2.0","id":1,"result":{"protocolVersion":1,"agentCapabilities":{"loadSession":true,"promptCapabilities":{"image":true}},"authMethods":[{"id":"cached_token"},{"id":"grok.com"}],"_meta":{"currentWorkingDirectory":"/home/x/.cache/zefiro-chat","agentVersion":"1.0.70","modelState":{"currentModelId":"grok-4.7","availableModels":[{"modelId":"grok-4.7","name":"Grok 4.7","description":"SpaceXAI's latest frontier model"},{"modelId":"grok-4.7-build-fast","name":"Grok 4.7 Fast","description":"Fast variant. 2x the price."},{"modelId":"grok-4.5","name":"Grok 4.5"}]}}}}"#;
    const CHUNK: &str = r#"{"jsonrpc":"2.0","method":"session/update","params":{"sessionId":"01a1","update":{"sessionUpdate":"agent_message_chunk","content":{"type":"text","text":"Ol"}},"_meta":{"totalTokens":1347}}}"#;
    const THOUGHT: &str = r#"{"jsonrpc":"2.0","method":"session/update","params":{"sessionId":"01a1","update":{"sessionUpdate":"agent_thought_chunk","content":{"type":"text","text":"Vou ler"}}}}"#;
    const TOOL: &str = r#"{"jsonrpc":"2.0","method":"session/update","params":{"sessionId":"01a1","update":{"sessionUpdate":"tool_call","toolCallId":"call-1","title":"read_file","rawInput":{"target_file":"/etc/hostname"},"_meta":{"x.ai/tool":{"name":"read_file","kind":"read","read_only":true}}}}}"#;
    const DONE: &str = r#"{"jsonrpc":"2.0","id":5,"result":{"stopReason":"end_turn","_meta":{"sessionId":"01a1","totalTokens":11814,"modelId":"grok-4.7"}}}"#;
    const CANCELLED: &str = r#"{"jsonrpc":"2.0","id":4,"result":{"stopReason":"cancelled","_meta":{"cancellationCategory":"MidTurnAbort"}}}"#;
    const MISSING: &str = r#"{"jsonrpc":"2.0","id":3,"error":{"code":-32603,"message":"Path not found.","data":{"detail":"No such file or directory (os error 2)","code":"FS_NOT_FOUND"}}}"#;
    const BAD_PARAMS: &str = r#"{"jsonrpc":"2.0","id":5,"error":{"code":-32602,"message":"Invalid params","data":"data did not match any variant"}}"#;
    const MODELS: &str = r#"{"jsonrpc":"2.0","method":"_x.ai/models/update","params":{"currentModelId":"grok-4.5","availableModels":[{"modelId":"grok-4.7","name":"Grok 4.7"},{"modelId":"grok-4.5","name":"Grok 4.5"}]}}"#;

    #[test]
    fn requests_are_json_rpc_lines() {
        let i = v(&initialize(1, "1.2.0"));
        assert_eq!((i["jsonrpc"].as_str(), i["id"].as_u64(), i["method"].as_str()), (Some("2.0"), Some(1), Some("initialize")));
        assert_eq!(i["params"]["protocolVersion"], 1);
        assert_eq!(i["params"]["clientCapabilities"], json!({}), "sem fs nem terminal");
        assert_eq!(v(&authenticate(2))["params"], json!({ "methodId": "cached_token" }));
        let n = v(&session_new(3, "/home/x/.cache/zefiro-chat"));
        assert_eq!((n["method"].as_str(), n["params"]["cwd"].as_str()), (Some("session/new"), Some("/home/x/.cache/zefiro-chat")));
        assert_eq!(n["params"]["mcpServers"], json!([]));
        assert_eq!(n["params"]["_meta"], json!({ "rules": SYSTEM_PROMPT }), "sem yoloMode");
        assert_eq!(v(&session_load(4, "01a1", "/c"))["params"], json!({ "sessionId": "01a1", "cwd": "/c", "mcpServers": [] }));
        let p = v(&prompt(5, "01a1", "oi\n\"x\"", &[]));
        assert_eq!(p["params"], json!({ "sessionId": "01a1", "prompt": [{ "type": "text", "text": "oi\n\"x\"" }] }));
        let img = v(&prompt(6, "01a1", "e isto?", &["QUJD".into()]));
        assert_eq!(img["params"]["prompt"][0], json!({ "type": "image", "mimeType": "image/png", "data": "QUJD" }));
        assert_eq!(img["params"]["prompt"][1]["text"], "e isto?");
        assert_eq!(v(&prompt(6, "s", " ", &["QUJD".into()]))["params"]["prompt"].as_array().unwrap().len(), 1, "sem texto, só a imagem");
        let c = v(&cancel("01a1"));
        assert_eq!((c["method"].as_str(), c.get("id")), (Some("session/cancel"), None), "notificação");
        assert_eq!(v(&set_model(7, "01a1", "grok-4.5"))["params"], json!({ "sessionId": "01a1", "configId": "model", "value": "grok-4.5" }), "texto puro (com objeto o Grok responde Invalid params)");
        assert_eq!(v(&billing(8))["method"], "_x.ai/billing");
        assert_eq!(v(&auth_info(9))["method"], "_x.ai/auth/info");
        assert!(!prompt(1, "s", "a\nb", &[]).contains('\n'));
    }

    #[test]
    fn updates_become_events() {
        assert_eq!(parse(CHUNK), Incoming::Event(Event::Delta { text: "Ol".into(), item: None }));
        assert_eq!(parse(THOUGHT), Incoming::Event(Event::Thinking));
        assert_eq!(parse(TOOL), Incoming::Tool("read_file".into()));
        for other in [
            r#"{"jsonrpc":"2.0","method":"session/update","params":{"update":{"sessionUpdate":"available_commands_update","availableCommands":[]}}}"#,
            r#"{"jsonrpc":"2.0","method":"session/update","params":{"update":{"sessionUpdate":"user_message_chunk","content":{"type":"text","text":"x"}}}}"#,
            r#"{"jsonrpc":"2.0","method":"_x.ai/session/setup","params":{"phase":"auth"}}"#,
            r#"{"jsonrpc":"2.0","id":"skills-reload","result":{"result":{"reloaded":1}}}"#,
            "lixo",
        ] {
            assert_eq!(parse(other), Incoming::Other, "{other}");
        }
    }

    #[test]
    fn responses_failures_and_the_end_of_the_turn() {
        let Incoming::Response { id: 5, result } = parse(DONE) else { panic!() };
        assert_eq!(stop(&result), Event::Finished { text: None, error: None });
        let Incoming::Response { id: 4, result } = parse(CANCELLED) else { panic!() };
        assert_eq!(stop(&result), Event::Finished { text: None, error: Some("interrupted".into()) });
        assert_eq!(stop(&json!({ "stopReason": "refusal" })), Event::Finished { text: None, error: Some("O Grok recusou responder.".into()) });
        assert_eq!(stop(&json!({ "stopReason": "max_tokens" })), Event::Finished { text: None, error: None });
        assert_eq!(parse(MISSING), Incoming::Failure { id: 3, message: "session not found".into() });
        assert_eq!(parse(BAD_PARAMS), Incoming::Failure { id: 5, message: "Invalid params: data did not match any variant".into() });
        assert_eq!(session_id(&json!({ "sessionId": "01a18382-86d6" })).as_deref(), Some("01a18382-86d6"));
        assert_eq!(session_id(&json!({})), None);
    }

    #[test]
    fn models_come_from_initialize_session_and_updates() {
        let Incoming::Response { result, .. } = parse(INIT) else { panic!() };
        assert_eq!(workdir(&result).as_deref(), Some("/home/x/.cache/zefiro-chat"));
        let (list, current) = models(model_state(&result).unwrap());
        assert_eq!(current.as_deref(), Some("grok-4.7"));
        assert_eq!(list.iter().map(|m| m.name.as_str()).collect::<Vec<_>>(), ["Grok 4.7", "Grok 4.7 Fast (2× o preço)", "Grok 4.5"]);
        let marked = with_default(list, current.as_deref());
        assert_eq!(marked.iter().filter(|m| m.default).map(|m| m.id.as_str()).collect::<Vec<_>>(), ["grok-4.7"]);
        let session = json!({ "sessionId": "s", "models": { "currentModelId": "grok-4.6", "availableModels": [{ "modelId": "grok-4.6", "name": "Grok 4.6" }] } });
        assert_eq!(models(model_state(&session).unwrap()).1.as_deref(), Some("grok-4.6"));
        assert!(model_state(&json!({ "stopReason": "end_turn" })).is_none());
        let Incoming::Models { list, current } = parse(MODELS) else { panic!() };
        assert_eq!((list.len(), current.as_deref()), (2, Some("grok-4.5")));
    }

    #[test]
    fn permission_requests_are_rejected() {
        let ask = r#"{"jsonrpc":"2.0","id":0,"method":"session/request_permission","params":{"sessionId":"s","toolCall":{"toolCallId":"c"},"options":[{"optionId":"allow","name":"Allow once","kind":"allow_once"},{"optionId":"no","name":"Reject once","kind":"reject_once"}]}}"#;
        let Incoming::ServerRequest(reply) = parse(ask) else { panic!() };
        assert_eq!(v(&reply), json!({ "jsonrpc": "2.0", "id": 0, "result": { "outcome": { "outcome": "selected", "optionId": "no" } } }));
        let only_allow = r#"{"jsonrpc":"2.0","id":"p1","method":"session/request_permission","params":{"options":[{"optionId":"allow","kind":"allow_once"}]}}"#;
        let Incoming::ServerRequest(reply) = parse(only_allow) else { panic!() };
        assert_eq!(v(&reply)["result"]["outcome"], json!({ "outcome": "cancelled" }), "nunca permite");
        let Incoming::ServerRequest(reply) = parse(r#"{"jsonrpc":"2.0","id":7,"method":"fs/read_text_file","params":{"path":"/etc/passwd"}}"#) else { panic!() };
        assert_eq!((v(&reply)["id"].as_u64(), v(&reply)["error"]["code"].as_i64()), (Some(7), Some(-32601)));
    }
}
