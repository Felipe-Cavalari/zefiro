//! Argumentos do `claude`, do `codex` e do `grok` do chat e como rodá-los no Windows e no WSL.

use crate::agent::Agent;
use std::path::{Path, PathBuf};

/// Acrescentado ao prompt do Claude Code: o painel é pequeno e não desenha Markdown. Sem aspas
/// duplas nem barras invertidas, porque no WSL ele passa pela linha de comando do `wsl.exe`.
pub const SYSTEM_PROMPT: &str = "Você está num chat rápido num painel pequeno da barra do Windows (Zefiro). Responda em português do Brasil, curto e direto, em texto simples: sem tabelas, sem cabeçalhos e sem negrito. Listas curtas com hífen podem. Código só quando pedirem, sem cercas de três crases.";

/// Pasta das sessões do chat no WSL (relativa à home).
pub const WSL_DIR: &str = ".cache/zefiro-chat";

/// Primeira partida da conversa (`--session-id`) ou continuação (`--resume`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Session<'a> { New(&'a str), Resume(&'a str) }

/// Nome de modelo aceito em `[chat] model`: letras, números e `.-_[]`. Outro texto é ignorado.
pub fn valid_model(model: &str) -> bool {
    !model.is_empty() && model.len() <= 80 && model.chars().all(|c| c.is_ascii_alphanumeric() || ".-_[]".contains(c))
}

pub fn claude_args(session: Session, model: &str) -> Vec<String> {
    let mut args: Vec<String> = [
        "-p",
        "--input-format",
        "stream-json",
        "--output-format",
        "stream-json",
        "--verbose",
        "--include-partial-messages",
        "--tools",
        "",
        "--strict-mcp-config",
        "--disable-slash-commands",
        "--setting-sources",
        "",
        "--append-system-prompt",
        SYSTEM_PROMPT,
    ]
    .iter()
    .map(|s| s.to_string())
    .collect();
    match session {
        Session::New(id) => args.extend(["--session-id".into(), id.into()]),
        Session::Resume(id) => args.extend(["--resume".into(), id.into()]),
    }
    if valid_model(model) {
        args.extend(["--model".into(), model.into()]);
    }
    args
}

/// Argumentos do Codex: o servidor JSON-RPC por stdio.
pub fn codex_args() -> Vec<String> {
    vec!["app-server".into()]
}

/// Arquivo do perfil de agente do Grok, na pasta do chat (`%LOCALAPPDATA%\zefiro\chat` ou
/// `~/.cache/zefiro-chat`).
pub const GROK_PROFILE: &str = "grok-perfil.md";

/// Perfil de agente do chat (ADR-0027): só o `todo_write`, que não lê nem escreve nada fora da
/// sessão, e nenhum servidor MCP herdado. O `--deny` e o `--disallowed-tools` do `grok` não valem no
/// `agent stdio` (sonda de 2026-10-28), e o `always-approve` do usuário roda as ferramentas sem
/// pedir. Uma linha por item, sem aspas duplas nem barras invertidas (no WSL, cada linha vira um
/// `echo` na linha de comando do `wsl.exe`).
pub const GROK_PROFILE_LINES: [&str; 7] = [
    "---",
    "name: zefiro-chat",
    "description: Chat rápido do Zefiro, só conversa",
    "tools: todo_write",
    "mcpInheritance: none",
    "---",
    "Você é o chat rápido do Zefiro. Só converse: não leia arquivos, não rode comandos e não busque na web.",
];

/// Ambiente do `grok` do chat e do chip: sem atualização em segundo plano, sem memória entre
/// sessões, sem workflows e sem subagentes.
pub const GROK_ENV: [(&str, &str); 4] = [("GROK_DISABLE_AUTOUPDATER", "1"), ("GROK_MEMORY", "0"), ("GROK_WORKFLOWS", "0"), ("GROK_SUBAGENTS", "0")];

/// O perfil inteiro, para gravar no Windows.
pub fn grok_profile() -> String {
    let mut text = GROK_PROFILE_LINES.join("\n");
    text.push('\n');
    text
}

/// Argumentos do Grok: o servidor ACP por stdio com o perfil do chat (`profile`: o caminho do
/// arquivo; no WSL, relativo à pasta do chat).
pub fn grok_args(profile: &str) -> Vec<String> {
    vec!["agent".into(), "--agent-profile".into(), profile.into(), "stdio".into()]
}

/// Argumentos do Grok no chip de uso: o servidor ACP sem perfil (nenhuma sessão é aberta).
pub fn grok_usage_args() -> Vec<String> {
    vec!["agent".into(), "stdio".into()]
}

/// Script que grava o perfil do Grok na pasta do chat do WSL (roda antes do `exec`).
pub fn grok_profile_script() -> String {
    let lines: Vec<String> = GROK_PROFILE_LINES.iter().map(|l| format!("echo {}", bash_quote(l))).collect();
    format!("{{ {}; }} > {GROK_PROFILE}", lines.join("; "))
}

/// Aspas simples do bash: `it's` → `'it'\''s'`.
pub fn bash_quote(s: &str) -> String {
    format!("'{}'", s.replace('\'', r"'\''"))
}

/// Argumentos do `wsl.exe`: um shell de login (para achar o `claude` do usuário) na pasta do chat.
/// `distro` vazio = a distro padrão.
pub fn wsl_args(distro: Option<&str>, binary: &str, args: &[String]) -> Vec<String> {
    wsl_args_with(distro, "", &[], binary, args)
}

/// Como [`wsl_args`], com um script antes do `exec` (`setup`, vazio = nenhum) e variáveis de
/// ambiente para o executável.
pub fn wsl_args_with(distro: Option<&str>, setup: &str, env: &[(&str, &str)], binary: &str, args: &[String]) -> Vec<String> {
    let quoted: Vec<String> = args.iter().map(|a| bash_quote(a)).collect();
    let setup = if setup.is_empty() { String::new() } else { format!("{setup} && ") };
    let env = if env.is_empty() { String::new() } else { format!("env {} ", env.iter().map(|(k, v)| format!("{k}={}", bash_quote(v))).collect::<Vec<_>>().join(" ")) };
    let script = format!("mkdir -p ~/{WSL_DIR} && cd ~/{WSL_DIR} && {setup}exec {env}{binary} {}", quoted.join(" "));
    let mut out = Vec::new();
    if let Some(d) = distro.filter(|d| !d.is_empty()) {
        out.extend(["-d".to_string(), d.to_string()]);
    }
    out.extend(["--".into(), "bash".into(), "-lc".into(), script]);
    out
}

/// Script que diz se a distro tem o executável (sai com 0 se tem).
pub fn wsl_probe(binary: &str) -> String {
    format!("command -v {binary} >/dev/null")
}

/// Pastas do usuário onde os instaladores põem os executáveis.
pub struct UserDirs<'a> { pub home: &'a Path, pub appdata: &'a Path, pub local_appdata: &'a Path }

/// Onde procurar o executável no Windows, na ordem. Claude: instalador nativo, npm. Codex: o
/// instalador da OpenAI. Grok: o `install.ps1` da xAI. Depois, cada pasta do `PATH`.
pub fn windows_candidates(agent: Agent, dirs: &UserDirs, path_dirs: &[PathBuf]) -> Vec<PathBuf> {
    let mut out = match agent {
        Agent::Claude => vec![dirs.home.join(r".local\bin\claude.exe"), dirs.appdata.join(r"npm\node_modules\@anthropic-ai\claude-code\bin\claude.exe")],
        Agent::Codex => vec![dirs.local_appdata.join(r"Programs\OpenAI\Codex\bin\codex.exe")],
        Agent::Grok => vec![dirs.home.join(r".grok\bin\grok.exe")],
    };
    let exe = format!("{}.exe", agent.binary());
    out.extend(path_dirs.iter().map(|d| d.join(&exe)));
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn after<'a>(args: &'a [String], flag: &str) -> Option<&'a str> {
        args.iter().position(|a| a == flag).and_then(|i| args.get(i + 1)).map(String::as_str)
    }

    #[test]
    fn args_stream_json_without_tools_or_user_settings() {
        let a = claude_args(Session::New("id-1"), "");
        assert_eq!(a[0], "-p");
        assert_eq!(after(&a, "--input-format"), Some("stream-json"));
        assert_eq!(after(&a, "--output-format"), Some("stream-json"));
        assert!(a.contains(&"--verbose".into()) && a.contains(&"--include-partial-messages".into()));
        assert_eq!(after(&a, "--tools"), Some(""), "sem ferramentas");
        assert_eq!(after(&a, "--setting-sources"), Some(""), "sem hooks e plugins do usuário");
        assert!(a.contains(&"--strict-mcp-config".into()) && a.contains(&"--disable-slash-commands".into()));
        assert_eq!(after(&a, "--append-system-prompt"), Some(SYSTEM_PROMPT));
        assert_eq!(after(&a, "--session-id"), Some("id-1"));
        assert!(!a.contains(&"--model".into()) && !a.contains(&"--resume".into()));
    }

    #[test]
    fn resume_and_model() {
        let a = claude_args(Session::Resume("id-2"), "sonnet");
        assert_eq!(after(&a, "--resume"), Some("id-2"));
        assert!(!a.contains(&"--session-id".into()));
        assert_eq!(after(&a, "--model"), Some("sonnet"));
        assert!(!claude_args(Session::New("x"), "sonnet; rm -rf ~").contains(&"--model".into()), "modelo estranho é ignorado");
        assert!(valid_model("claude-sonnet-5-5[1m]") && !valid_model("") && !valid_model("a b"));
    }

    #[test]
    fn system_prompt_survives_the_wsl_command_line() {
        assert!(!SYSTEM_PROMPT.contains('"') && !SYSTEM_PROMPT.contains('\\'));
    }

    #[test]
    fn bash_quoting() {
        assert_eq!(bash_quote("abc"), "'abc'");
        assert_eq!(bash_quote(""), "''");
        assert_eq!(bash_quote("it's"), r"'it'\''s'");
    }

    #[test]
    fn wsl_runs_claude_in_a_login_shell_in_the_chat_folder() {
        let args = vec!["-p".to_string(), "".to_string(), "a b".to_string()];
        let w = wsl_args(Some("Ubuntu-24.04"), "claude", &args);
        assert_eq!(&w[..5], ["-d", "Ubuntu-24.04", "--", "bash", "-lc"]);
        assert_eq!(w[5], "mkdir -p ~/.cache/zefiro-chat && cd ~/.cache/zefiro-chat && exec claude '-p' '' 'a b'");
        let default = wsl_args(None, "codex", &codex_args());
        assert_eq!(&default[..3], ["--", "bash", "-lc"]);
        assert!(default[3].ends_with("exec codex 'app-server'"));
        assert_eq!(wsl_args(Some(""), "claude", &args)[0], "--");
        assert_eq!(wsl_probe("codex"), "command -v codex >/dev/null");
    }

    #[test]
    fn windows_candidates_in_order() {
        let (home, appdata, local, bin) = (Path::new(r"C:\Users\x"), Path::new(r"C:\Users\x\AppData\Roaming"), Path::new(r"C:\Users\x\AppData\Local"), PathBuf::from(r"C:\bin"));
        let dirs = UserDirs { home, appdata, local_appdata: local };
        let c = windows_candidates(Agent::Claude, &dirs, std::slice::from_ref(&bin));
        assert_eq!(c, [home.join(r".local\bin\claude.exe"), appdata.join(r"npm\node_modules\@anthropic-ai\claude-code\bin\claude.exe"), bin.join("claude.exe")]);
        let c = windows_candidates(Agent::Codex, &dirs, std::slice::from_ref(&bin));
        assert_eq!(c, [local.join(r"Programs\OpenAI\Codex\bin\codex.exe"), bin.join("codex.exe")]);
        let c = windows_candidates(Agent::Grok, &dirs, std::slice::from_ref(&bin));
        assert_eq!(c, [home.join(r".grok\bin\grok.exe"), bin.join("grok.exe")]);
    }

    #[test]
    fn grok_runs_the_acp_server_with_the_chat_profile() {
        assert_eq!(grok_args(GROK_PROFILE), ["agent", "--agent-profile", "grok-perfil.md", "stdio"]);
        assert_eq!(grok_usage_args(), ["agent", "stdio"], "o chip não abre sessão");
        let profile = grok_profile();
        assert!(profile.starts_with("---\nname: zefiro-chat\n") && profile.ends_with('\n'));
        assert!(profile.contains("\ntools: todo_write\n") && profile.contains("\nmcpInheritance: none\n"));
        assert!(GROK_ENV.contains(&("GROK_DISABLE_AUTOUPDATER", "1")) && GROK_ENV.contains(&("GROK_MEMORY", "0")));
    }

    #[test]
    fn grok_profile_survives_the_wsl_command_line() {
        for line in GROK_PROFILE_LINES {
            assert!(!line.contains('"') && !line.contains('\\') && !line.contains('\''), "{line}");
        }
        let script = grok_profile_script();
        assert!(script.starts_with("{ echo '---'; echo 'name: zefiro-chat'; ") && script.ends_with("; } > grok-perfil.md"), "{script}");
    }

    #[test]
    fn wsl_with_setup_and_env() {
        let w = wsl_args_with(Some("Ubuntu-24.04"), "echo a > f", &[("GROK_MEMORY", "0"), ("X", "a b")], "grok", &grok_args(GROK_PROFILE));
        assert_eq!(&w[..5], ["-d", "Ubuntu-24.04", "--", "bash", "-lc"]);
        assert_eq!(w[5], "mkdir -p ~/.cache/zefiro-chat && cd ~/.cache/zefiro-chat && echo a > f && exec env GROK_MEMORY='0' X='a b' grok 'agent' '--agent-profile' 'grok-perfil.md' 'stdio'");
        assert_eq!(wsl_args_with(None, "", &[], "codex", &codex_args()), wsl_args(None, "codex", &codex_args()));
    }
}
