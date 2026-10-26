//! Origens do uso: o Windows e as distros do WSL que estão rodando. Uma conta junta as origens que
//! são da mesma pessoa no serviço (Claude ou Codex).

use std::path::{Path, PathBuf};

/// De onde veio o dado. A ordem (Windows antes, distros pelo nome) é a ordem do painel.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum OriginKind { Windows, Wsl(String) }

/// Uma origem e a pasta pessoal nela (`%USERPROFILE%` ou `\\wsl.localhost\<distro>\home\<usuário>`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Origin { pub kind: OriginKind, pub home: PathBuf }

/// O que a plataforma descobriu num ciclo: as origens, as distros rodando (nome do registro) e se
/// há distro instalada parada.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Discovery { pub origins: Vec<Origin>, pub running: Vec<String>, pub wsl_stopped: bool }

/// Chamado pelos serviços a cada ciclo, na thread deles.
pub type Discover = std::sync::Arc<dyn Fn() -> Discovery + Send + Sync>;

/// Uma conta: `key` identifica a pessoa no serviço (nunca vai para o log); `stopped` é a conta
/// lembrada de uma distro que parou de rodar.
#[derive(Debug, Clone, PartialEq)]
pub struct Account<T> { pub key: String, pub origins: Vec<OriginKind>, pub stopped: bool, pub data: T }

/// Distros do Docker Desktop não têm o Claude nem o Codex do usuário.
pub fn is_ignored_distro(name: &str) -> bool {
    name.to_lowercase().starts_with("docker-desktop")
}

/// As distros rodando (o `wsl.exe` as devolve em minúsculas) com o nome do registro, sem as
/// ignoradas, em ordem alfabética.
pub fn proper_names(running_lower: &[String], installed: &[String]) -> Vec<String> {
    let mut names: Vec<String> = running_lower
        .iter()
        .filter(|r| !is_ignored_distro(r))
        .map(|r| installed.iter().find(|i| i.eq_ignore_ascii_case(r)).cloned().unwrap_or_else(|| r.clone()))
        .collect();
    names.sort_by_key(|n| n.to_lowercase());
    names
}

fn subdirs(dir: &Path) -> Vec<PathBuf> {
    let Ok(entries) = std::fs::read_dir(dir) else { return Vec::new() };
    let mut dirs: Vec<PathBuf> = entries.flatten().filter(|e| e.file_type().is_ok_and(|t| t.is_dir())).map(|e| e.path()).collect();
    dirs.sort();
    dirs
}

/// Uma origem por pasta em `/home` de cada distro. Pasta que não dá para listar não vira origem.
pub fn wsl_origins(distros: &[String], root: &dyn Fn(&str) -> PathBuf) -> Vec<Origin> {
    distros.iter().flat_map(|d| subdirs(&root(d).join("home")).into_iter().map(|home| Origin { kind: OriginKind::Wsl(d.clone()), home })).collect()
}

fn kind_label(kind: &OriginKind, many_distros: bool) -> String {
    match kind {
        OriginKind::Windows => "Windows".into(),
        OriginKind::Wsl(d) if many_distros => format!("WSL · {d}"),
        OriginKind::Wsl(_) => "WSL".into(),
    }
}

/// "Windows", "WSL", "WSL · Ubuntu-24.04" (com contas em mais de uma distro) ou "Windows · WSL".
pub fn origins_label(origins: &[OriginKind], many_distros: bool) -> String {
    let mut labels: Vec<String> = Vec::new();
    for label in origins.iter().map(|k| kind_label(k, many_distros)) {
        if !labels.contains(&label) {
            labels.push(label);
        }
    }
    labels.join(" · ")
}

/// Há contas em mais de uma distro (o rótulo passa a dizer qual).
pub fn many_distros<T>(accounts: &[Account<T>]) -> bool {
    let mut distros: Vec<&str> = accounts.iter().flat_map(|a| &a.origins).filter_map(|k| if let OriginKind::Wsl(d) = k { Some(d.as_str()) } else { None }).collect();
    distros.sort_unstable();
    distros.dedup();
    distros.len() > 1
}

/// Windows primeiro, depois as distros pelo nome.
pub fn sort_accounts<T>(accounts: &mut [Account<T>]) {
    accounts.sort_by(|a, b| a.origins.iter().min().cmp(&b.origins.iter().min()));
}

/// Origem de uma distro que não está rodando (o Windows nunca está parado).
pub fn is_stopped(kind: &OriginKind, running: &[String]) -> bool {
    matches!(kind, OriginKind::Wsl(d) if !running.iter().any(|r| r.eq_ignore_ascii_case(d)))
}

/// Repõe, marcadas `stopped`, as contas que só existiam em distros que pararam de rodar: o limite
/// delas continua valendo. Contas do Windows nunca são repostas (o Windows sempre é lido).
pub fn carry_stopped<T: Clone>(previous: &[Account<T>], current: &mut Vec<Account<T>>, running: &[String]) {
    for old in previous {
        if !current.iter().any(|c| c.key == old.key) && !old.origins.is_empty() && old.origins.iter().all(|k| is_stopped(k, running)) {
            current.push(Account { stopped: true, ..old.clone() });
        }
    }
    sort_accounts(current);
}

/// Linha do log: "uso do Claude: 2 contas (Windows, WSL)", sem dado pessoal.
pub fn accounts_summary<T>(service: &str, accounts: &[Account<T>]) -> String {
    let many = many_distros(accounts);
    let labels: Vec<String> = accounts.iter().map(|a| origins_label(&a.origins, many)).collect();
    let count = if accounts.len() == 1 { "1 conta".to_string() } else { format!("{} contas", accounts.len()) };
    format!("uso do {service}: {count} ({})", labels.join(", "))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn account(key: &str, origins: Vec<OriginKind>) -> Account<u8> {
        Account { key: key.into(), origins, stopped: false, data: 0 }
    }

    fn wsl(name: &str) -> OriginKind {
        OriginKind::Wsl(name.into())
    }

    #[test]
    fn ignores_docker_distros() {
        assert!(is_ignored_distro("docker-desktop"));
        assert!(is_ignored_distro("Docker-Desktop-Data"));
        assert!(!is_ignored_distro("Ubuntu-24.04"));
    }

    #[test]
    fn proper_names_use_registry_case() {
        let running = ["ubuntu-24.04", "docker-desktop", "arch"].map(String::from);
        let installed = ["Ubuntu-24.04", "docker-desktop"].map(String::from);
        assert_eq!(proper_names(&running, &installed), vec!["arch".to_string(), "Ubuntu-24.04".to_string()]);
    }

    #[test]
    fn wsl_origins_lists_home_folders() {
        let root = std::env::temp_dir().join(format!("zefiro-origins-{}", std::process::id()));
        std::fs::create_dir_all(root.join("home").join("bia")).unwrap();
        std::fs::create_dir_all(root.join("home").join("ana")).unwrap();
        let found = wsl_origins(&["Ubuntu".to_string()], &|_| root.clone());
        assert_eq!(found, vec![
            Origin { kind: wsl("Ubuntu"), home: root.join("home").join("ana") },
            Origin { kind: wsl("Ubuntu"), home: root.join("home").join("bia") },
        ]);
        assert!(wsl_origins(&["Ubuntu".to_string()], &|_| PathBuf::from("Z:\\nao-existe")).is_empty());
        std::fs::remove_dir_all(&root).unwrap();
    }

    #[test]
    fn labels() {
        assert_eq!(origins_label(&[OriginKind::Windows], false), "Windows");
        assert_eq!(origins_label(&[wsl("Ubuntu-24.04")], false), "WSL");
        assert_eq!(origins_label(&[wsl("Ubuntu-24.04")], true), "WSL · Ubuntu-24.04");
        assert_eq!(origins_label(&[OriginKind::Windows, wsl("U")], false), "Windows · WSL");
    }

    #[test]
    fn many_distros_counts_distinct_wsl_distros() {
        assert!(!many_distros(&[account("a", vec![OriginKind::Windows]), account("b", vec![wsl("U")])]));
        assert!(many_distros(&[account("a", vec![wsl("Arch")]), account("b", vec![wsl("U")])]));
    }

    #[test]
    fn carry_keeps_stopped_wsl_accounts_only() {
        let previous = vec![account("a", vec![OriginKind::Windows]), account("b", vec![wsl("Ubuntu")])];
        let mut current = Vec::new();
        carry_stopped(&previous, &mut current, &[]);
        assert_eq!(current, vec![Account { stopped: true, ..account("b", vec![wsl("Ubuntu")]) }]);
        let mut current = Vec::new();
        carry_stopped(&previous, &mut current, &["ubuntu".to_string()]);
        assert!(current.is_empty());
    }

    #[test]
    fn sort_puts_windows_first() {
        let mut accounts = vec![account("b", vec![wsl("Ubuntu")]), account("c", vec![wsl("Arch")]), account("a", vec![OriginKind::Windows])];
        sort_accounts(&mut accounts);
        assert_eq!(accounts.iter().map(|a| a.key.as_str()).collect::<Vec<_>>(), ["a", "c", "b"]);
    }

    #[test]
    fn summary_has_no_personal_data() {
        let two = [account("uuid-x", vec![OriginKind::Windows]), account("uuid-y", vec![wsl("U")])];
        let text = accounts_summary("Claude", &two);
        assert_eq!(text, "uso do Claude: 2 contas (Windows, WSL)");
        assert!(!text.contains("uuid"));
        assert_eq!(accounts_summary("Codex", &[account("k", vec![OriginKind::Windows, wsl("U")])]), "uso do Codex: 1 conta (Windows · WSL)");
    }
}
