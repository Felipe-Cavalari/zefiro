//! Modelo, leitura do TOML (pacote e pasta do usuário) e mesclagem.

use std::path::Path;
use std::sync::OnceLock;

#[derive(Debug, Clone, PartialEq)]
pub struct Command { pub cmd: String, pub what: String, pub keywords: Vec<String>, pub popular: bool }

#[derive(Debug, Clone, PartialEq)]
pub struct Tool { pub id: String, pub name: String, pub icon: Option<char>, pub commands: Vec<Command> }

#[derive(Debug, Clone, PartialEq, Default)]
pub struct Library { pub tools: Vec<Tool> }

/// Arquivos do usuário lidos e o tamanho máximo de cada um.
const MAX_FILES: usize = 64;
const MAX_FILE_BYTES: u64 = 256 * 1024;

#[derive(serde::Deserialize)]
struct RawFile {
    tool: Option<RawTool>,
    #[serde(default)]
    command: Vec<RawCommand>,
}

#[derive(serde::Deserialize)]
struct RawTool { id: Option<String>, name: Option<String>, icon: Option<String> }

#[derive(serde::Deserialize)]
struct RawCommand {
    cmd: Option<String>,
    what: Option<String>,
    #[serde(default)]
    keywords: Vec<String>,
    #[serde(default)]
    popular: bool,
}

/// Lê um arquivo de ferramenta. Devolve a ferramenta e os avisos das entradas descartadas
/// (`cmd` ou `what` vazios, `cmd` com quebra de linha); `Err` se o TOML é inválido ou falta
/// `[tool] id` ou `name`.
pub fn parse_tool(text: &str) -> Result<(Tool, Vec<String>), String> {
    let raw: RawFile = toml::from_str(text).map_err(|e| e.message().to_string())?;
    let header = raw.tool.ok_or("falta a seção [tool]")?;
    let id = header.id.map(|s| s.trim().to_lowercase()).filter(|s| !s.is_empty()).ok_or("falta `id` em [tool]")?;
    let name = header.name.map(|s| s.trim().to_string()).filter(|s| !s.is_empty()).ok_or("falta `name` em [tool]")?;
    let icon = header.icon.and_then(|s| {
        let mut chars = s.chars();
        chars.next().filter(|_| chars.next().is_none())
    });
    let mut warnings = Vec::new();
    let mut commands = Vec::new();
    for (index, c) in raw.command.into_iter().enumerate() {
        let n = index + 1;
        let cmd = c.cmd.as_deref().map(str::trim).unwrap_or_default();
        let what = c.what.as_deref().map(str::trim).unwrap_or_default();
        if cmd.is_empty() {
            warnings.push(format!("comando {n}: `cmd` vazio"));
        } else if cmd.contains(['\n', '\r']) {
            warnings.push(format!("comando {n}: `cmd` com quebra de linha"));
        } else if what.is_empty() {
            warnings.push(format!("comando {n}: `what` vazio"));
        } else {
            commands.push(Command { cmd: cmd.to_string(), what: what.to_string(), keywords: c.keywords, popular: c.popular });
        }
    }
    Ok((Tool { id, name, icon, commands }, warnings))
}

/// O pacote embutido, na ordem do painel. Um arquivo inválido some do pacote; o teste
/// `builtin_is_valid` garante que isso não acontece.
pub fn builtin() -> &'static Library {
    static PACK: OnceLock<Library> = OnceLock::new();
    PACK.get_or_init(|| {
        let files = [
            include_str!("../data/docker.toml"),
            include_str!("../data/linux.toml"),
            include_str!("../data/powershell.toml"),
            include_str!("../data/python.toml"),
            include_str!("../data/git.toml"),
        ];
        Library { tools: files.iter().filter_map(|text| parse_tool(text).ok().map(|(tool, _)| tool)).collect() }
    })
}

/// Lê os `*.toml` de `dir` (até 64, em ordem de nome, cada um com até 256 KB). Devolve as
/// ferramentas e os avisos (um por arquivo ou entrada descartada, começando pelo nome do arquivo).
/// Pasta inexistente não é erro.
pub fn read_user_dir(dir: &Path) -> (Vec<Tool>, Vec<String>) {
    let Ok(entries) = std::fs::read_dir(dir) else { return (Vec::new(), Vec::new()) };
    let mut files: Vec<_> = entries
        .filter_map(Result::ok)
        .filter(|e| e.path().extension().is_some_and(|x| x.eq_ignore_ascii_case("toml")))
        .collect();
    files.sort_by_key(std::fs::DirEntry::file_name);
    let mut warnings = Vec::new();
    if files.len() > MAX_FILES {
        warnings.push(format!("só os {MAX_FILES} primeiros arquivos são lidos"));
        files.truncate(MAX_FILES);
    }
    let mut tools = Vec::new();
    for entry in files {
        let name = entry.file_name().to_string_lossy().into_owned();
        if entry.metadata().map_or(true, |m| m.len() > MAX_FILE_BYTES) {
            warnings.push(format!("{name}: maior que 256 KB (ou ilegível), ignorado"));
            continue;
        }
        let text = match std::fs::read_to_string(entry.path()) {
            Ok(text) => text,
            Err(e) => {
                warnings.push(format!("{name}: {e}"));
                continue;
            }
        };
        match parse_tool(&text) {
            Ok((tool, entry_warnings)) => {
                warnings.extend(entry_warnings.into_iter().map(|w| format!("{name}: {w}")));
                tools.push(tool);
            }
            Err(e) => warnings.push(format!("{name}: {e}")),
        }
    }
    (tools, warnings)
}

/// Junta os comandos do usuário ao pacote. `cmd` igual na mesma ferramenta troca a entrada do pacote;
/// o nome da ferramenta embutida vale; as ferramentas só do usuário vêm depois, em ordem de nome.
pub fn merge(builtin: &Library, user: Vec<Tool>) -> Library {
    let mut tools = builtin.tools.clone();
    let mut extra: Vec<Tool> = Vec::new();
    for incoming in user {
        let slot = match tools.iter_mut().chain(extra.iter_mut()).find(|t| t.id == incoming.id) {
            Some(existing) => existing,
            None => {
                extra.push(Tool { commands: Vec::new(), ..incoming.clone() });
                extra.last_mut().expect("acabou de entrar")
            }
        };
        for command in incoming.commands {
            match slot.commands.iter_mut().find(|c| c.cmd == command.cmd) {
                Some(existing) => *existing = command,
                None => slot.commands.push(command),
            }
        }
    }
    extra.sort_by_key(|t| t.name.to_lowercase());
    tools.extend(extra);
    Library { tools }
}

#[cfg(test)]
mod tests {
    use super::*;

    const GOOD: &str = r#"
[tool]
id = " Docker "
name = "Docker"
icon = "D"

[[command]]
cmd = "docker builder prune"
what = "Apaga o cache de build."
keywords = ["limpar cache", "liberar espaço"]
popular = true

[[command]]
cmd = "docker ps"
what = "Lista os contêineres."
"#;

    fn tool(id: &str, name: &str, cmds: &[(&str, &str)]) -> Tool {
        Tool { id: id.into(), name: name.into(), icon: None, commands: cmds.iter().map(|(c, w)| Command { cmd: (*c).into(), what: (*w).into(), keywords: Vec::new(), popular: false }).collect() }
    }

    #[test]
    fn parses_a_tool_file() {
        let (t, warnings) = parse_tool(GOOD).unwrap();
        assert!(warnings.is_empty());
        assert_eq!((t.id.as_str(), t.name.as_str(), t.icon), ("docker", "Docker", Some('D')));
        assert_eq!(t.commands.len(), 2);
        assert_eq!(t.commands[0].keywords, vec!["limpar cache", "liberar espaço"]);
        assert!(t.commands[0].popular && !t.commands[1].popular);
    }

    #[test]
    fn drops_bad_entries_but_keeps_the_rest() {
        let text = format!("{GOOD}\n[[command]]\ncmd = \"\"\nwhat = \"x\"\n\n[[command]]\ncmd = \"a\\nb\"\nwhat = \"x\"\n\n[[command]]\ncmd = \"ls\"\nwhat = \"\"\n");
        let (t, warnings) = parse_tool(&text).unwrap();
        assert_eq!(t.commands.len(), 2, "só as duas boas ficam");
        assert_eq!(warnings.len(), 3);
        assert!(warnings.iter().all(|w| w.starts_with("comando ")), "{warnings:?}");
    }

    #[test]
    fn bad_toml_or_missing_id_is_an_error() {
        assert!(parse_tool("[tool\n").is_err());
        assert!(parse_tool("[tool]\nname = \"X\"\n").is_err());
        assert!(parse_tool("[tool]\nid = \"x\"\n").is_err(), "sem name");
    }

    #[test]
    fn cmd_is_trimmed_and_never_ends_with_a_newline() {
        let (t, _) = parse_tool("[tool]\nid = \"x\"\nname = \"X\"\n[[command]]\ncmd = \"ls -la  \\n\"\nwhat = \"w\"\n").unwrap();
        assert_eq!(t.commands[0].cmd, "ls -la");
    }

    #[test]
    fn user_dir_skips_broken_files_and_limits() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("bom.toml"), GOOD).unwrap();
        std::fs::write(dir.path().join("quebrado.toml"), "[tool\n").unwrap();
        std::fs::write(dir.path().join("nota.txt"), "nada").unwrap();
        std::fs::write(dir.path().join("grande.toml"), "#".repeat(300 * 1024)).unwrap();
        let (tools, warnings) = read_user_dir(dir.path());
        assert_eq!(tools.len(), 1);
        assert_eq!(tools[0].id, "docker");
        assert_eq!(warnings.len(), 2, "{warnings:?}");
        assert!(warnings.iter().any(|w| w.starts_with("quebrado.toml")) && warnings.iter().any(|w| w.starts_with("grande.toml")));
    }

    #[test]
    fn missing_user_dir_is_silent() {
        let dir = tempfile::tempdir().unwrap();
        assert_eq!(read_user_dir(&dir.path().join("nao-existe")), (Vec::new(), Vec::new()));
    }

    #[test]
    fn user_dir_reads_at_most_64_files_in_name_order() {
        let dir = tempfile::tempdir().unwrap();
        for i in 0..70 {
            std::fs::write(dir.path().join(format!("t{i:02}.toml")), format!("[tool]\nid = \"t{i:02}\"\nname = \"T\"\n")).unwrap();
        }
        let (tools, _) = read_user_dir(dir.path());
        assert_eq!(tools.len(), 64);
        assert_eq!(tools[0].id, "t00");
        assert_eq!(tools[63].id, "t63");
    }

    #[test]
    fn merge_same_cmd_in_same_tool_overrides() {
        let base = Library { tools: vec![tool("docker", "Docker", &[("docker ps", "velho"), ("docker images", "imagens")]), tool("git", "Git", &[("docker ps", "outra")])] };
        let user = vec![tool("docker", "Docker", &[("docker ps", "novo")])];
        let merged = merge(&base, user);
        assert_eq!(merged.tools[0].commands[0].what, "novo", "troca na mesma posição");
        assert_eq!(merged.tools[0].commands.len(), 2);
        assert_eq!(merged.tools[1].commands[0].what, "outra", "outra ferramenta não muda");
    }

    #[test]
    fn merge_keeps_builtin_name_and_orders_user_tools_last() {
        let base = Library { tools: vec![tool("docker", "Docker", &[("docker ps", "a")])] };
        let user = vec![tool("zeta", "Zeta", &[("z", "z")]), tool("docker", "Meu Docker", &[("docker logs", "logs")]), tool("alfa", "Alfa", &[("a", "a")])];
        let merged = merge(&base, user);
        let names: Vec<_> = merged.tools.iter().map(|t| t.name.as_str()).collect();
        assert_eq!(names, ["Docker", "Alfa", "Zeta"]);
        assert_eq!(merged.tools[0].commands.len(), 2, "o comando novo entra no fim");
        assert_eq!(merged.tools[0].commands[1].cmd, "docker logs");
    }

    #[test]
    fn pack_has_enough_commands() {
        for t in &builtin().tools {
            assert!(t.commands.len() >= 35, "{} tem só {} comandos", t.id, t.commands.len());
            assert!(t.commands.iter().filter(|c| c.popular).count() >= 8, "{} tem poucos populares", t.id);
        }
    }

    #[test]
    fn spec_examples_are_findable() {
        let lib = builtin();
        let has = |hits: Vec<crate::Hit<'_>>, tool: &str, cmd: &str| hits.iter().any(|h| h.tool.id == tool && h.command.cmd == cmd);
        assert!(has(lib.search(Some("docker"), "limpar cache"), "docker", "docker builder prune"));
        assert!(has(lib.search(Some("docker"), "como apago o cache"), "docker", "docker builder prune"));
        assert!(has(lib.search(None, "subir fastapi uvicorn"), "python", "uvicorn main:app --reload"));
    }

    #[test]
    fn destructive_commands_carry_a_warning() {
        const DANGEROUS: &[&str] = &[
            "prune", "rm -rf", "rm -fr", "reset --hard", "clean -f", "push --force", "push -f", "remove-item", " rmi", "docker rm", "volume rm", "branch -d", "kill -9", "stash drop", "mkfs", "dd if=",
            "git checkout -- ", "git restore ", " > ", "stop-process",
        ];
        for t in &builtin().tools {
            for c in &t.commands {
                let lower = c.cmd.to_lowercase();
                let dangerous = DANGEROUS.iter().any(|p| lower.contains(p)) && !lower.contains("--staged");
                assert!(!dangerous || c.what.starts_with("⚠ "), "{}: `{}` apaga dados e não avisa", t.id, c.cmd);
            }
        }
    }

    #[test]
    fn no_duplicate_commands_per_tool() {
        for t in &builtin().tools {
            let mut seen = std::collections::HashSet::new();
            for c in &t.commands {
                assert!(seen.insert(c.cmd.as_str()), "{}: `{}` repetido", t.id, c.cmd);
            }
        }
    }

    #[test]
    fn builtin_is_valid() {
        let lib = builtin();
        let ids: Vec<_> = lib.tools.iter().map(|t| t.id.as_str()).collect();
        assert_eq!(ids, ["docker", "linux", "powershell", "python", "git"]);
        for t in &lib.tools {
            assert!(!t.commands.is_empty(), "{} sem comandos", t.id);
            for c in &t.commands {
                assert!(!c.cmd.is_empty() && !c.what.is_empty(), "{}: entrada vazia", t.id);
                assert!(!c.cmd.contains('\n') && !c.cmd.contains('\r'), "{}: quebra de linha em {:?}", t.id, c.cmd);
            }
        }
    }
}
