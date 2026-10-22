//! O `codex app-server`: JSON-RPC por linhas no stdin e no stdout. O que o Zefiro pede e como as
//! respostas e notificações viram os mesmos [`Event`]s do Claude.

use crate::agent::ModelOption;
use crate::command::SYSTEM_PROMPT;
use crate::protocol::Event;
use serde_json::{Value, json};

fn request(id: u64, method: &str, params: Value) -> String {
    json!({ "id": id, "method": method, "params": params }).to_string()
}

pub fn initialize(id: u64, version: &str) -> String {
    request(id, "initialize", json!({ "clientInfo": { "name": "zefiro", "title": "Zefiro", "version": version }, "capabilities": null }))
}

pub fn initialized() -> String {
    json!({ "method": "initialized" }).to_string()
}

pub fn model_list(id: u64) -> String {
    request(id, "model/list", json!({}))
}

/// Conversa nova: só leitura, nada de pedir aprovação e o prompt do chat.
pub fn thread_start(id: u64) -> String {
    request(id, "thread/start", json!({ "sandbox": "read-only", "approvalPolicy": "never", "developerInstructions": SYSTEM_PROMPT }))
}

pub fn thread_resume(id: u64, thread: &str) -> String {
    request(id, "thread/resume", json!({ "threadId": thread, "sandbox": "read-only", "approvalPolicy": "never", "developerInstructions": SYSTEM_PROMPT }))
}

/// Uma pergunta, com as imagens PNG (base64) antes do texto como `data:` URLs (valem no Windows e
/// no WSL, sem traduzir caminhos). `model` vazio deixa o Codex decidir.
pub fn turn_start(id: u64, thread: &str, text: &str, images: &[String], model: &str) -> String {
    let mut input: Vec<Value> = images.iter().map(|data| json!({ "type": "image", "url": format!("data:image/png;base64,{data}") })).collect();
    if !text.trim().is_empty() || input.is_empty() {
        input.push(json!({ "type": "text", "text": text, "text_elements": [] }));
    }
    let mut params = json!({ "threadId": thread, "input": input });
    if !model.is_empty() {
        params["model"] = json!(model);
    }
    request(id, "turn/start", params)
}

pub fn turn_interrupt(id: u64, thread: &str, turn: &str) -> String {
    request(id, "turn/interrupt", json!({ "threadId": thread, "turnId": turn }))
}

/// Resposta de erro a um pedido do servidor (aprovações), que o chat não atende.
pub fn reject(id: &Value) -> String {
    json!({ "id": id, "error": { "code": -32601, "message": "o chat do Zefiro não aceita este pedido" } }).to_string()
}

/// Uma linha do servidor.
#[derive(Debug, Clone, PartialEq)]
pub enum Incoming {
    Response { id: u64, result: Value },
    Failure { id: u64, message: String },
    Event(Event),
    /// Pedido do servidor (tem `id` e `method`): precisa de resposta.
    ServerRequest { id: Value },
    Other,
}

/// O texto de um erro do Codex, que às vezes é um JSON dentro do `message`.
pub fn error_text(message: &str) -> String {
    serde_json::from_str::<Value>(message)
        .ok()
        .and_then(|v| v["error"]["message"].as_str().or(v["message"].as_str()).map(str::to_string))
        .unwrap_or_else(|| message.to_string())
}

fn item_id(v: &Value) -> Option<String> {
    v.as_str().filter(|s| !s.is_empty()).map(str::to_string)
}

/// O `ImageGenerationItem` terminado: o PNG vem em base64 no `result` (vale no Windows e no WSL,
/// sem traduzir o `savedPath`).
fn generated_image(item: &Value) -> Event {
    match item["result"].as_str().filter(|r| !r.is_empty()) {
        Some(data) if item["failure"].is_null() && item["status"] != "failed" => Event::Image { item: item["id"].as_str().unwrap_or_default().to_string(), data: data.to_string() },
        _ => Event::ImageFailed,
    }
}

pub fn parse(line: &str) -> Incoming {
    let Ok(v) = serde_json::from_str::<Value>(line.trim()) else { return Incoming::Other };
    let method = v["method"].as_str();
    if let Some(method) = method {
        if !v["id"].is_null() {
            return Incoming::ServerRequest { id: v["id"].clone() };
        }
        let p = &v["params"];
        let event = match method {
            "item/agentMessage/delta" => p["delta"].as_str().map(|d| Event::Delta { text: d.to_string(), item: item_id(&p["itemId"]) }),
            "item/reasoning/summaryTextDelta" | "item/reasoning/textDelta" | "item/reasoning/summaryPartAdded" => Some(Event::Thinking),
            "item/started" if p["item"]["type"] == "reasoning" => Some(Event::Thinking),
            "item/started" if p["item"]["type"] == "imageGeneration" => Some(Event::ImageStarted),
            "item/completed" if p["item"]["type"] == "agentMessage" => {
                p["item"]["text"].as_str().filter(|t| !t.is_empty()).map(|t| Event::Assistant { text: t.to_string(), item: item_id(&p["item"]["id"]) })
            }
            "item/completed" if p["item"]["type"] == "imageGeneration" => Some(generated_image(&p["item"])),
            "turn/started" => p["turn"]["id"].as_str().map(|id| Event::Turn(id.to_string())),
            "turn/completed" => {
                let turn = &p["turn"];
                Some(match turn["status"].as_str() {
                    Some("completed") => Event::Finished { text: None, error: None },
                    Some("interrupted") => Event::Finished { text: None, error: Some("interrupted".into()) },
                    _ => Event::Finished { text: None, error: Some(turn["error"]["message"].as_str().map(error_text).unwrap_or_else(|| "turn failed".into())) },
                })
            }
            _ => None,
        };
        return event.map_or(Incoming::Other, Incoming::Event);
    }
    let Some(id) = v["id"].as_u64() else { return Incoming::Other };
    if let Some(e) = v.get("error") {
        return Incoming::Failure { id, message: e["message"].as_str().map(error_text).unwrap_or_else(|| e.to_string()) };
    }
    Incoming::Response { id, result: v["result"].clone() }
}

/// O id da conversa na resposta do `thread/start` ou do `thread/resume`.
pub fn thread_id(result: &Value) -> Option<String> {
    result["thread"]["id"].as_str().filter(|s| !s.is_empty()).map(str::to_string)
}

/// Os modelos da resposta do `model/list`, sem os escondidos.
pub fn models(result: &Value) -> Vec<ModelOption> {
    result["data"]
        .as_array()
        .into_iter()
        .flatten()
        .filter(|m| m["hidden"].as_bool() != Some(true))
        .filter_map(|m| {
            let id = m["model"].as_str().or(m["id"].as_str())?.to_string();
            let name = m["displayName"].as_str().filter(|s| !s.is_empty()).map_or_else(|| id.clone(), str::to_string);
            Some(ModelOption { id, name, default: m["isDefault"].as_bool() == Some(true) })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn v(s: &str) -> Value {
        serde_json::from_str(s).unwrap()
    }

    #[test]
    fn requests_are_json_rpc_lines() {
        let r = v(&initialize(1, "1.2.0"));
        assert_eq!((r["id"].as_u64(), r["method"].as_str(), r["params"]["clientInfo"]["name"].as_str()), (Some(1), Some("initialize"), Some("zefiro")));
        assert_eq!(v(&initialized()), json!({ "method": "initialized" }));
        let s = v(&thread_start(3));
        assert_eq!((s["params"]["sandbox"].as_str(), s["params"]["approvalPolicy"].as_str()), (Some("read-only"), Some("never")));
        assert_eq!(s["params"]["developerInstructions"], SYSTEM_PROMPT);
        assert_eq!(v(&thread_resume(4, "t1"))["params"]["threadId"], "t1");
        let t = v(&turn_start(5, "t1", "oi\n\"x\"", &[], "gpt-5.6-luna"));
        assert_eq!(t["params"]["input"][0], json!({ "type": "text", "text": "oi\n\"x\"", "text_elements": [] }));
        assert_eq!(t["params"]["model"], "gpt-5.6-luna");
        assert!(v(&turn_start(5, "t1", "oi", &[], ""))["params"].get("model").is_none());
        let img = v(&turn_start(6, "t1", "e isto?", &["QUJD".into()], ""));
        assert_eq!(img["params"]["input"][0], json!({ "type": "image", "url": "data:image/png;base64,QUJD" }));
        assert_eq!(img["params"]["input"][1]["text"], "e isto?");
        assert_eq!(v(&turn_start(6, "t1", "", &["QUJD".into()], ""))["params"]["input"].as_array().unwrap().len(), 1, "sem texto, só a imagem");
        assert_eq!(v(&turn_interrupt(6, "t1", "u1"))["params"], json!({ "threadId": "t1", "turnId": "u1" }));
        assert!(!turn_start(1, "t", "a\nb", &[], "").contains('\n'));
    }

    #[test]
    fn responses_and_failures() {
        let r = parse(r#"{"id":3,"result":{"thread":{"id":"01a1","preview":""},"model":"gpt-5.6-sol"}}"#);
        let Incoming::Response { id: 3, result } = r else { panic!("{r:?}") };
        assert_eq!(thread_id(&result).as_deref(), Some("01a1"));
        assert_eq!(parse(r#"{"id":4,"error":{"code":-32600,"message":"no rollout found for thread id x"}}"#), Incoming::Failure { id: 4, message: "no rollout found for thread id x".into() });
    }

    #[test]
    fn notifications_become_events() {
        assert_eq!(parse(r#"{"method":"item/agentMessage/delta","params":{"threadId":"t","turnId":"u","itemId":"i","delta":"Ol"}}"#), Incoming::Event(Event::Delta { text: "Ol".into(), item: Some("i".into()) }));
        assert_eq!(parse(r#"{"method":"turn/started","params":{"threadId":"t","turn":{"id":"u1","items":[],"status":"inProgress"}}}"#), Incoming::Event(Event::Turn("u1".into())));
        assert_eq!(parse(r#"{"method":"item/started","params":{"item":{"type":"reasoning","id":"r","summary":[],"content":[]}}}"#), Incoming::Event(Event::Thinking));
        assert_eq!(parse(r#"{"method":"item/completed","params":{"item":{"type":"agentMessage","id":"i","text":"Olá"}}}"#), Incoming::Event(Event::Assistant { text: "Olá".into(), item: Some("i".into()) }));
        assert_eq!(parse(r#"{"method":"turn/completed","params":{"turn":{"id":"u","status":"completed","error":null}}}"#), Incoming::Event(Event::Finished { text: None, error: None }));
        assert_eq!(parse(r#"{"method":"turn/completed","params":{"turn":{"id":"u","status":"interrupted"}}}"#), Incoming::Event(Event::Finished { text: None, error: Some("interrupted".into()) }));
        let failed = r#"{"method":"turn/completed","params":{"turn":{"status":"failed","error":{"message":"{\"type\":\"error\",\"status\":400,\"error\":{\"type\":\"invalid_request_error\",\"message\":\"The 'gpt-6.1-sol' model is not supported when using Codex with a ChatGPT account.\"}}"}}}}"#;
        assert_eq!(parse(failed), Incoming::Event(Event::Finished { text: None, error: Some("The 'gpt-6.1-sol' model is not supported when using Codex with a ChatGPT account.".into()) }));
        for other in [r#"{"method":"thread/status/changed","params":{}}"#, r#"{"method":"mcpServer/startupStatus/updated","params":{}}"#, "lixo"] {
            assert_eq!(parse(other), Incoming::Other, "{other}");
        }
    }

    #[test]
    fn generated_images_come_as_base64() {
        // Formato do `ImageGenerationItem` (`codex app-server generate-ts`), com o PNG encurtado.
        let done = r#"{"method":"item/completed","params":{"threadId":"t","turnId":"u","item":{"type":"imageGeneration","id":"exec-5b77","status":"completed","revisedPrompt":"Ilustração","result":"iVBORw0KGgo=","transparentBackground":false,"failure":null,"savedPath":"C:\\Users\\x\\.codex\\generated_images\\a.png"}}}"#;
        assert_eq!(parse(done), Incoming::Event(Event::Image { item: "exec-5b77".into(), data: "iVBORw0KGgo=".into() }));
        assert_eq!(parse(r#"{"method":"item/started","params":{"item":{"type":"imageGeneration","id":"exec-5b77","status":"inProgress","revisedPrompt":null,"result":"","failure":null}}}"#), Incoming::Event(Event::ImageStarted));
        let limit = r#"{"method":"item/completed","params":{"item":{"type":"imageGeneration","id":"x","status":"failed","revisedPrompt":null,"result":"","failure":{"type":"usageLimitExceeded","limitId":"l","resetsAt":null}}}}"#;
        assert_eq!(parse(limit), Incoming::Event(Event::ImageFailed));
    }

    #[test]
    fn server_requests_need_an_answer() {
        assert_eq!(parse(r#"{"id":"s1","method":"item/commandExecution/requestApproval","params":{}}"#), Incoming::ServerRequest { id: json!("s1") });
        assert_eq!(v(&reject(&json!("s1")))["id"], "s1");
    }

    #[test]
    fn models_skip_hidden_and_keep_the_default() {
        let r = v(r#"{"data":[{"model":"gpt-5.6-sol","displayName":"GPT-5.6-Sol","hidden":false,"isDefault":true},{"model":"x","displayName":"X","hidden":true,"isDefault":false},{"model":"gpt-5.6-luna","displayName":"","hidden":false,"isDefault":false}]}"#);
        assert_eq!(
            models(&r),
            [ModelOption { id: "gpt-5.6-sol".into(), name: "GPT-5.6-Sol".into(), default: true }, ModelOption { id: "gpt-5.6-luna".into(), name: "gpt-5.6-luna".into(), default: false }]
        );
    }
}
