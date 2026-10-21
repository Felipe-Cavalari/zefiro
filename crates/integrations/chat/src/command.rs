//! Argumentos do `claude` do chat e como rodá-lo no Windows e no WSL.

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

/// Aspas simples do bash: `it's` → `'it'\''s'`.
pub fn bash_quote(s: &str) -> String {
    format!("'{}'", s.replace('\'', r"'\''"))
}

/// Argumentos do `wsl.exe`: um shell de login (para achar o `claude` do usuário) na pasta do chat.
/// `distro` vazio = a distro padrão.
pub fn wsl_args(distro: Option<&str>, args: &[String]) -> Vec<String> {
    let quoted: Vec<String> = args.iter().map(|a| bash_quote(a)).collect();
    let script = format!("mkdir -p ~/{WSL_DIR} && cd ~/{WSL_DIR} && exec claude {}", quoted.join(" "));
    let mut out = Vec::new();
    if let Some(d) = distro.filter(|d| !d.is_empty()) {
        out.extend(["-d".to_string(), d.to_string()]);
    }
    out.extend(["--".into(), "bash".into(), "-lc".into(), script]);
    out
}

/// Script que diz se a distro tem `claude` (sai com 0 se tem).
pub const WSL_PROBE: &str = "command -v claude >/dev/null";

/// Onde procurar o `claude.exe` no Windows, na ordem: instalador nativo, npm e cada pasta do `PATH`.
pub fn windows_candidates(home: &Path, appdata: &Path, path_dirs: &[PathBuf]) -> Vec<PathBuf> {
    let mut out = vec![home.join(r".local\bin\claude.exe"), appdata.join(r"npm\node_modules\@anthropic-ai\claude-code\bin\claude.exe")];
    out.extend(path_dirs.iter().map(|d| d.join("claude.exe")));
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
        let w = wsl_args(Some("Ubuntu-24.04"), &args);
        assert_eq!(&w[..5], ["-d", "Ubuntu-24.04", "--", "bash", "-lc"]);
        assert_eq!(w[5], "mkdir -p ~/.cache/zefiro-chat && cd ~/.cache/zefiro-chat && exec claude '-p' '' 'a b'");
        let default = wsl_args(None, &args);
        assert_eq!(&default[..3], ["--", "bash", "-lc"]);
        assert_eq!(wsl_args(Some(""), &args)[0], "--");
    }

    #[test]
    fn windows_candidates_in_order() {
        let (home, appdata, bin) = (Path::new(r"C:\Users\x"), Path::new(r"C:\Users\x\AppData\Roaming"), PathBuf::from(r"C:\bin"));
        let c = windows_candidates(home, appdata, std::slice::from_ref(&bin));
        assert_eq!(c, [home.join(r".local\bin\claude.exe"), appdata.join(r"npm\node_modules\@anthropic-ai\claude-code\bin\claude.exe"), bin.join("claude.exe")]);
    }
}
