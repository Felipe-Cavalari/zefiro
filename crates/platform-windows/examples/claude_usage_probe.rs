//! Sonda: confere o endpoint de uso do Claude. Imprime só o status HTTP e os NOMES dos campos da
//! resposta; o token nunca é impresso.
use std::time::Duration;

fn names(v: &serde_json::Value, prefix: &str) {
    match v {
        serde_json::Value::Object(map) => {
            for (k, child) in map {
                let kind = match child { serde_json::Value::Object(_) => "objeto", serde_json::Value::Array(_) => "lista", serde_json::Value::Number(_) => "número", serde_json::Value::String(_) => "texto", serde_json::Value::Bool(_) => "booleano", serde_json::Value::Null => "nulo" };
                println!("{prefix}{k}: {kind}");
                names(child, &format!("{prefix}{k}."));
            }
        }
        serde_json::Value::Array(items) => {
            if let Some(first) = items.first() {
                names(first, &format!("{prefix}[0]."));
            }
        }
        _ => {}
    }
}

fn main() {
    let path = std::path::PathBuf::from(std::env::var_os("USERPROFILE").unwrap_or_default()).join(".claude").join(".credentials.json");
    let text = std::fs::read_to_string(&path).expect("credenciais do Claude Code");
    let json: serde_json::Value = serde_json::from_str(&text).expect("JSON das credenciais");
    let token = json["claudeAiOauth"]["accessToken"].as_str().expect("claudeAiOauth.accessToken").to_string();
    let beta = std::env::args().nth(1).unwrap_or_else(|| "oauth-2025-04-20".into());
    let auth = format!("Bearer {token}");
    let (status, body) = zefiro_platform_windows::http::get_with("https://api.anthropic.com/api/oauth/usage", &[("Authorization", &auth), ("anthropic-beta", &beta)], Duration::from_secs(15)).expect("requisição");
    println!("status: {status}");
    match serde_json::from_str::<serde_json::Value>(&body) {
        Ok(v) => names(&v, ""),
        Err(_) => println!("corpo não é JSON ({} bytes)", body.len()),
    }
}
