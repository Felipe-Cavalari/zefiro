//! Busca por intenção ("como apago o cache") e a lista de "Mais usados".

use crate::model::{Command, Library, Tool};

/// Palavras que não ajudam a achar nada ("como apago o cache").
const STOPWORDS: &[&str] = &["como", "eu", "o", "a", "os", "as", "de", "do", "da", "dos", "das", "para", "um", "uma", "com", "que", "no", "na", "em", "e"];

/// Termos com 5 letras ou mais casam também pelo começo de uma palavra (4 letras): "apago" acha "apagar".
const PREFIX_FROM: usize = 5;
const PREFIX_LEN: usize = 4;

/// Um comando achado, com a ferramenta a que pertence.
#[derive(Debug, Clone, Copy)]
pub struct Hit<'a> { pub tool: &'a Tool, pub command: &'a Command }

/// Minúsculas e sem acento, para a busca.
pub fn fold(s: &str) -> String {
    s.chars()
        .flat_map(char::to_lowercase)
        .map(|c| match c {
            'á' | 'à' | 'â' | 'ã' | 'ä' => 'a',
            'é' | 'è' | 'ê' | 'ë' => 'e',
            'í' | 'ì' | 'î' | 'ï' => 'i',
            'ó' | 'ò' | 'ô' | 'õ' | 'ö' => 'o',
            'ú' | 'ù' | 'û' | 'ü' => 'u',
            'ç' => 'c',
            'ñ' => 'n',
            c => c,
        })
        .collect()
}

/// As palavras que contam na busca: dobradas e sem as palavras vazias.
pub fn terms(query: &str) -> Vec<String> {
    fold(query).split_whitespace().filter(|w| !STOPWORDS.contains(w)).map(str::to_string).collect()
}

/// O termo aparece no campo (já dobrado), por substring ou, com 5+ letras, pelo começo de uma palavra?
fn term_in(term: &str, field: &str) -> bool {
    if field.contains(term) {
        return true;
    }
    if term.chars().count() < PREFIX_FROM {
        return false;
    }
    let prefix: String = term.chars().take(PREFIX_LEN).collect();
    field.split(|c: char| !c.is_alphanumeric()).any(|word| word.starts_with(&prefix))
}

/// Posição do resultado: 0 se `keywords` sozinho casa todos os termos, 1 `cmd`, 2 `what`, 3 só os
/// campos somados; `None` se não casa.
fn rank(command: &Command, terms: &[String]) -> Option<u8> {
    let fields = [fold(&command.keywords.join(" ")), fold(&command.cmd), fold(&command.what)];
    if let Some(i) = fields.iter().position(|f| terms.iter().all(|t| term_in(t, f))) {
        return Some(i as u8);
    }
    terms.iter().all(|t| fields.iter().any(|f| term_in(t, f))).then_some(3)
}

impl Library {
    pub fn tool(&self, id: &str) -> Option<&Tool> {
        self.tools.iter().find(|t| t.id == id)
    }

    /// As ferramentas em foco: uma pelo id (nenhuma se não existe) ou todas.
    fn scope(&self, tool: Option<&str>) -> Vec<&Tool> {
        match tool {
            Some(id) => self.tool(id).into_iter().collect(),
            None => self.tools.iter().collect(),
        }
    }

    /// Comandos que casam com `query`, melhores primeiro (empate: ordem da biblioteca). Sem termos
    /// (busca vazia ou só palavras vazias) devolve os "Mais usados".
    pub fn search(&self, tool: Option<&str>, query: &str) -> Vec<Hit<'_>> {
        let terms = terms(query);
        if terms.is_empty() {
            return self.popular(tool);
        }
        let mut ranked: Vec<(u8, Hit<'_>)> = self
            .scope(tool)
            .into_iter()
            .flat_map(|tool| tool.commands.iter().map(move |command| Hit { tool, command }))
            .filter_map(|hit| rank(hit.command, &terms).map(|r| (r, hit)))
            .collect();
        ranked.sort_by_key(|(r, _)| *r);
        ranked.into_iter().map(|(_, hit)| hit).collect()
    }

    /// Os marcados como `popular`. Numa ferramenta específica sem nenhum marcado, todos os dela.
    pub fn popular(&self, tool: Option<&str>) -> Vec<Hit<'_>> {
        let specific = tool.is_some();
        self.scope(tool)
            .into_iter()
            .flat_map(|tool| {
                let any = tool.commands.iter().any(|c| c.popular);
                tool.commands.iter().filter(move |c| c.popular || (specific && !any)).map(move |command| Hit { tool, command })
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cmd(cmd: &str, what: &str, keywords: &[&str], popular: bool) -> Command {
        Command { cmd: cmd.into(), what: what.into(), keywords: keywords.iter().map(|k| (*k).into()).collect(), popular }
    }

    fn tool(id: &str, name: &str, commands: Vec<Command>) -> Tool {
        Tool { id: id.into(), name: name.into(), icon: None, commands }
    }

    fn lib() -> Library {
        Library {
            tools: vec![
                tool(
                    "docker",
                    "Docker",
                    vec![
                        cmd("docker ps", "Lista os contêineres em execução.", &["listar containers"], true),
                        cmd("docker builder prune", "⚠ Apaga o cache de build.", &["apagar cache", "limpar cache"], true),
                        cmd("docker images", "Lista as imagens.", &[], false),
                    ],
                ),
                tool("git", "Git", vec![cmd("git status", "Mostra o estado.", &["ver mudanças"], true), cmd("git log", "Mostra o histórico.", &[], false)]),
                tool("sql", "SQL", vec![cmd("select 1", "Teste.", &[], false), cmd("select 2", "Teste 2.", &[], false)]),
            ],
        }
    }

    fn cmds(hits: &[Hit<'_>]) -> Vec<String> {
        hits.iter().map(|h| h.command.cmd.clone()).collect()
    }

    #[test]
    fn fold_strips_accents_and_case() {
        assert_eq!(fold("AÇÃO ü ñ"), "acao u n");
    }

    #[test]
    fn terms_drop_stopwords() {
        assert_eq!(terms("como apago o cache"), ["apago", "cache"]);
        assert_eq!(terms("  Limpar   CACHE "), ["limpar", "cache"]);
    }

    #[test]
    fn intent_search_finds_docker_cache() {
        let l = lib();
        assert_eq!(cmds(&l.search(Some("docker"), "como apago o cache")), ["docker builder prune"], "apago casa com apagar pelo prefixo");
        assert_eq!(cmds(&l.search(Some("docker"), "limpar cache")), ["docker builder prune"]);
    }

    #[test]
    fn every_term_must_match() {
        assert!(lib().search(Some("docker"), "limpar zzz").is_empty());
    }

    #[test]
    fn short_terms_are_not_prefix_matched() {
        let l = lib();
        assert_eq!(l.search(Some("docker"), "limpo").len(), 1, "5 letras: prefixo limp casa com limpar");
        assert!(l.search(Some("docker"), "limo").is_empty(), "4 letras: só substring");
    }

    #[test]
    fn ranking_prefers_keywords_over_cmd_over_what() {
        let l = Library {
            tools: vec![tool(
                "x",
                "X",
                vec![
                    cmd("a-um", "explica cache", &[], false),
                    cmd("cache-dois", "outra", &[], false),
                    cmd("a-tres", "outra", &["cache"], false),
                    cmd("a-quatro", "outra", &["cache"], false),
                ],
            )],
        };
        assert_eq!(cmds(&l.search(Some("x"), "cache")), ["a-tres", "a-quatro", "cache-dois", "a-um"]);
    }

    #[test]
    fn terms_spread_over_fields_rank_last() {
        let l = Library { tools: vec![tool("x", "X", vec![cmd("alfa", "beta", &[], false), cmd("alfa beta", "w", &[], false)])] };
        assert_eq!(cmds(&l.search(Some("x"), "alfa beta")), ["alfa beta", "alfa"], "um campo sozinho vence os campos somados");
    }

    #[test]
    fn tool_filter_and_all() {
        let l = lib();
        assert!(l.search(Some("git"), "docker").is_empty());
        let all = l.search(None, "status");
        assert_eq!((all.len(), all[0].tool.id.as_str()), (1, "git"));
        assert_eq!(l.search(None, "mostra").len(), 2);
    }

    #[test]
    fn only_stopwords_behaves_like_empty() {
        let l = lib();
        let blank = l.search(Some("docker"), "como o");
        assert!(!blank.is_empty());
        assert_eq!(cmds(&blank), cmds(&l.popular(Some("docker"))));
        assert_eq!(cmds(&l.search(Some("docker"), "")), cmds(&l.popular(Some("docker"))));
    }

    #[test]
    fn popular_falls_back_to_all_for_a_tool_without_flags() {
        assert_eq!(cmds(&lib().popular(Some("sql"))), ["select 1", "select 2"]);
    }

    #[test]
    fn popular_all_tools_only_lists_flagged() {
        assert_eq!(cmds(&lib().popular(None)), ["docker ps", "docker builder prune", "git status"]);
    }

    #[test]
    fn unknown_tool_is_empty() {
        let l = lib();
        assert!(l.search(Some("nada"), "ps").is_empty());
        assert!(l.popular(Some("nada")).is_empty());
        assert!(l.tool("git").is_some() && l.tool("nada").is_none());
    }
}
