//! Uso do Codex a partir dos `.jsonl` de sessão em `~/.codex/sessions` (sem abrir credenciais).

use serde_json::Value;
use std::io::{Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};
use std::time::SystemTime;

use crate::format::parse_iso_utc;
use crate::origins::{Account, Discover, Origin, OriginKind, is_stopped, sort_accounts};

/// Só o fim de cada arquivo é lido: o último `rate_limits` está perto do fim.
const TAIL_BYTES: u64 = 256 * 1024;
/// Sessão recém-aberta ainda sem `rate_limits`: tenta as anteriores, até este limite.
const MAX_FILES: usize = 5;

/// Uma janela de uso (`primary` ou `secondary`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Window { pub used_percent: f32, pub window_minutes: u32, pub resets_at: i64 }

#[derive(Debug, Clone, Default, PartialEq)]
pub struct CodexUsage {
    pub primary: Option<Window>,
    pub secondary: Option<Window>,
    pub plan: Option<String>,
    /// `timestamp` do evento (ISO 8601, UTC).
    pub event_at: Option<String>,
}

fn window(v: &Value) -> Option<Window> {
    Some(Window {
        used_percent: v.get("used_percent")?.as_f64()? as f32,
        window_minutes: u32::try_from(v.get("window_minutes")?.as_u64()?).ok()?,
        resets_at: v.get("resets_at")?.as_i64()?,
    })
}

/// O último evento com `payload.rate_limits`, lendo de trás para frente. Linhas que não são JSON
/// (inclusive a primeira, cortada pela leitura do fim) são ignoradas.
pub fn parse_codex_tail(text: &str) -> Option<CodexUsage> {
    text.lines().rev().find_map(|line| {
        let v: Value = serde_json::from_str(line.trim()).ok()?;
        let limits = v.get("payload")?.get("rate_limits")?;
        let usage = CodexUsage {
            primary: limits.get("primary").and_then(window),
            secondary: limits.get("secondary").and_then(window),
            plan: limits.get("plan_type").and_then(Value::as_str).map(str::to_string),
            event_at: v.get("timestamp").and_then(Value::as_str).map(str::to_string),
        };
        (usage.primary.is_some() || usage.secondary.is_some()).then_some(usage)
    })
}

fn collect(dir: &Path, out: &mut Vec<(SystemTime, PathBuf)>) {
    let Ok(entries) = std::fs::read_dir(dir) else { return };
    for entry in entries.flatten() {
        let path = entry.path();
        let Ok(meta) = entry.metadata() else { continue };
        if meta.is_dir() {
            collect(&path, out);
        } else if path.extension().is_some_and(|e| e == "jsonl")
            && let Ok(modified) = meta.modified()
        {
            out.push((modified, path));
        }
    }
}

/// Os `limit` `.jsonl` mais modificados, do mais novo para o mais velho. A árvore é percorrida
/// inteira: `codex resume` continua gravando no arquivo da pasta do dia em que a sessão começou.
pub fn recent_sessions(sessions_dir: &Path, limit: usize) -> Vec<(SystemTime, PathBuf)> {
    let mut files = Vec::new();
    collect(sessions_dir, &mut files);
    files.sort_by_key(|f| std::cmp::Reverse(f.0));
    files.truncate(limit);
    files
}

/// A conta da sessão: `payload.creator_account_id` do `session_meta` (1ª linha).
pub fn session_account(first_line: &str) -> Option<String> {
    let v: Value = serde_json::from_str(first_line.trim()).ok()?;
    v.get("payload")?.get("creator_account_id")?.as_str().filter(|s| !s.is_empty()).map(str::to_string)
}

fn first_line(path: &Path) -> Option<String> {
    use std::io::BufRead;
    let mut line = String::new();
    std::io::BufReader::new(std::fs::File::open(path).ok()?).read_line(&mut line).ok()?;
    Some(line)
}

/// O uso lido de uma sessão e a conta dela.
#[derive(Debug, Clone, PartialEq)]
pub struct CodexRead { pub account: Option<String>, pub modified: SystemTime, pub usage: CodexUsage }

/// O uso mais recente de cada conta entre os `MAX_FILES` arquivos mais novos. Sessão sem conta (Codex
/// antigo) entra na conta da sessão mais nova que tem uma.
pub fn read_codex_accounts(sessions_dir: &Path) -> Vec<CodexRead> {
    let files: Vec<(SystemTime, PathBuf, Option<String>)> = recent_sessions(sessions_dir, MAX_FILES)
        .into_iter()
        .take(MAX_FILES)
        .map(|(modified, path)| {
            let account = first_line(&path).as_deref().and_then(session_account);
            (modified, path, account)
        })
        .collect();
    let known = files.iter().find_map(|f| f.2.clone());
    let mut reads: Vec<CodexRead> = Vec::new();
    for (modified, path, account) in files {
        let account = account.or_else(|| known.clone());
        if reads.iter().any(|r| r.account == account) {
            continue;
        }
        if let Some(usage) = tail(&path).as_deref().and_then(parse_codex_tail) {
            reads.push(CodexRead { account, modified, usage });
        }
    }
    reads
}

/// Uma conta do Codex: o uso mais recente e quando o arquivo dele mudou.
#[derive(Debug, Clone, PartialEq)]
pub struct CodexEntry { pub modified: SystemTime, pub usage: CodexUsage }

/// O que o serviço publica.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct CodexSnapshot { pub accounts: Vec<Account<CodexEntry>>, pub wsl_stopped: bool }

fn event_time(entry: &CodexEntry) -> (Option<i64>, SystemTime) {
    (entry.usage.event_at.as_deref().and_then(parse_iso_utc), entry.modified)
}

/// Junta as origens da mesma conta, ficando com o evento mais recente entre elas.
pub fn merge_codex(per_origin: Vec<(OriginKind, Vec<CodexRead>)>) -> Vec<Account<CodexEntry>> {
    let mut accounts: Vec<Account<CodexEntry>> = Vec::new();
    for (kind, reads) in per_origin {
        for read in reads {
            let key = read.account.unwrap_or_else(|| format!("origem:{kind:?}"));
            let entry = CodexEntry { modified: read.modified, usage: read.usage };
            match accounts.iter_mut().find(|a| a.key == key) {
                Some(a) => {
                    if !a.origins.contains(&kind) {
                        a.origins.push(kind.clone());
                        a.origins.sort();
                    }
                    if event_time(&entry) > event_time(&a.data) {
                        a.data = entry;
                    }
                }
                None => accounts.push(Account { key, origins: vec![kind.clone()], stopped: false, data: entry }),
            }
        }
    }
    sort_accounts(&mut accounts);
    accounts
}

/// % do chip com várias contas: a maior janela primária (0 % se já reiniciou).
pub fn codex_chip_percent(accounts: &[Account<CodexEntry>], now_unix: i64) -> Option<f32> {
    accounts.iter().filter_map(|a| a.data.usage.primary.map(|w| effective_percent(&w, now_unix))).reduce(f32::max)
}

/// O próximo reinício de janela entre as contas: a UI refaz o chip nessa hora.
pub fn next_reset(accounts: &[Account<CodexEntry>], now_unix: i64) -> Option<i64> {
    accounts.iter().flat_map(|a| [a.data.usage.primary, a.data.usage.secondary]).flatten().map(|w| w.resets_at).filter(|t| *t > now_unix).min()
}

/// Os últimos `TAIL_BYTES` do arquivo.
fn tail(path: &Path) -> Option<String> {
    let mut file = std::fs::File::open(path).ok()?;
    let len = file.metadata().ok()?.len();
    file.seek(SeekFrom::Start(len.saturating_sub(TAIL_BYTES))).ok()?;
    let mut bytes = Vec::new();
    file.read_to_end(&mut bytes).ok()?;
    Some(String::from_utf8_lossy(&bytes).into_owned())
}

/// O `.jsonl` mais recente (recursivo) e quando foi modificado.
pub fn newest_session(sessions_dir: &Path) -> Option<(SystemTime, PathBuf)> {
    recent_sessions(sessions_dir, 1).into_iter().next()
}

/// O uso mais recente: do arquivo mais novo para o mais velho, até `MAX_FILES`.
pub fn read_codex(sessions_dir: &Path) -> Option<(PathBuf, SystemTime, CodexUsage)> {
    recent_sessions(sessions_dir, MAX_FILES).into_iter().take(MAX_FILES).find_map(|(modified, path)| {
        let usage = parse_codex_tail(&tail(&path)?)?;
        Some((path, modified, usage))
    })
}

/// Janela já reiniciada conta como 0 %: o arquivo só muda quando o Codex roda.
pub fn effective_percent(w: &Window, now_unix: i64) -> f32 {
    if w.resets_at <= now_unix { 0.0 } else { w.used_percent }
}

/// Nome da janela pelos minutos: 300 → "5 horas", 10080 → "7 dias".
pub fn window_name(minutes: u32) -> String {
    if minutes >= 1440 && minutes.is_multiple_of(1440) {
        let d = minutes / 1440;
        format!("{d} {}", if d == 1 { "dia" } else { "dias" })
    } else {
        let h = (minutes as f32 / 60.0).round().max(1.0) as u32;
        format!("{h} {}", if h == 1 { "hora" } else { "horas" })
    }
}

/// Ritmo da leitura: só um `stat` do arquivo mais novo de cada origem; relê quando algum muda.
const INTERVAL: std::time::Duration = std::time::Duration::from_secs(120);

enum Wake { Refresh, Stop }

type Shared = std::sync::Arc<std::sync::Mutex<Option<CodexSnapshot>>>;
/// O que a última leitura viu: origens, arquivo mais novo de cada uma e WSL parado.
type Seen = (Vec<OriginKind>, Vec<Option<(SystemTime, PathBuf)>>, bool);

fn now_unix() -> i64 {
    SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_secs() as i64)
}

/// Thread `zefiro-codex`: a cada 120 s, quando pedem (`refresh`) e logo depois do reinício de uma
/// janela, descobre as origens (`discover`), relê as que mudaram e avisa `notify` (em outra thread)
/// quando o uso muda ou uma janela reinicia.
pub struct CodexService {
    tx: std::sync::mpsc::Sender<Wake>,
    state: Shared,
}

impl CodexService {
    pub fn start(discover: Discover, notify: std::sync::Arc<dyn Fn() + Send + Sync>) -> std::io::Result<CodexService> {
        let (tx, rx) = std::sync::mpsc::channel();
        let state: Shared = Default::default();
        let shared = state.clone();
        std::thread::Builder::new().name("zefiro-codex".into()).spawn(move || {
            let mut seen: Option<Seen> = None;
            let mut reset_due: Option<i64> = None;
            // Leituras do ciclo anterior: as de uma distro que parou continuam valendo.
            let mut last_reads: Vec<(OriginKind, Vec<CodexRead>)> = Vec::new();
            loop {
                let found = discover();
                let origins: Vec<&Origin> = found.origins.iter().filter(|o| o.home.join(".codex").join("sessions").is_dir()).collect();
                let newest: Vec<Option<(SystemTime, PathBuf)>> = origins.iter().map(|o| newest_session(&o.home.join(".codex").join("sessions"))).collect();
                let key = (origins.iter().map(|o| o.kind.clone()).collect::<Vec<_>>(), newest, found.wsl_stopped);
                let reset_passed = reset_due.is_some_and(|t| t <= now_unix());
                if seen.as_ref() != Some(&key) {
                    seen = Some(key);
                    let mut per_origin: Vec<(OriginKind, Vec<CodexRead>)> = origins.iter().map(|o| (o.kind.clone(), read_codex_accounts(&o.home.join(".codex").join("sessions")))).collect();
                    for (kind, reads) in std::mem::take(&mut last_reads) {
                        if is_stopped(&kind, &found.running) && !per_origin.iter().any(|(k, _)| *k == kind) {
                            per_origin.push((kind, reads));
                        }
                    }
                    last_reads = per_origin.clone();
                    let mut accounts = merge_codex(per_origin);
                    for account in &mut accounts {
                        account.stopped = account.origins.iter().all(|k| is_stopped(k, &found.running));
                    }
                    let snapshot = CodexSnapshot { accounts, wsl_stopped: found.wsl_stopped };
                    reset_due = next_reset(&snapshot.accounts, now_unix());
                    let changed = shared.lock().map(|mut s| s.replace(snapshot.clone()).as_ref() != Some(&snapshot)).unwrap_or(false);
                    if changed || reset_passed {
                        notify();
                    }
                } else if reset_passed {
                    reset_due = shared.lock().ok().and_then(|s| s.as_ref().and_then(|s| next_reset(&s.accounts, now_unix())));
                    notify();
                }
                let wait = reset_due.map_or(INTERVAL, |t| INTERVAL.min(std::time::Duration::from_secs((t - now_unix()).max(0) as u64 + 1)));
                match rx.recv_timeout(wait) {
                    Ok(Wake::Refresh) | Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {}
                    Ok(Wake::Stop) | Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => break,
                }
            }
        })?;
        Ok(CodexService { tx, state })
    }

    /// As contas lidas e se há WSL parado.
    pub fn latest(&self) -> Option<CodexSnapshot> {
        self.state.lock().ok().and_then(|s| s.clone())
    }

    /// Confere os arquivos agora (ao abrir o painel).
    pub fn refresh(&self) {
        let _ = self.tx.send(Wake::Refresh);
    }
}

/// Não espera a thread: ela pode estar presa num `wsl.exe`, num `\\wsl.localhost` ou num HTTP, e
/// quem solta o serviço é a thread da UI. A thread sai sozinha ao ver o `Stop`.
impl Drop for CodexService {
    fn drop(&mut self) {
        let _ = self.tx.send(Wake::Stop);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::origins::Discovery;

    const SESSION: &str = include_str!("../tests/fixtures/codex_session.jsonl");

    #[test]
    fn parses_rate_limits_from_real_event() {
        let u = parse_codex_tail(SESSION).expect("rate_limits");
        let p = u.primary.unwrap();
        assert_eq!((p.used_percent, p.window_minutes, p.resets_at), (80.0, 300, 1792130364));
        let s = u.secondary.unwrap();
        assert_eq!((s.used_percent, s.window_minutes, s.resets_at), (19.0, 10080, 1792678081));
        assert_eq!(u.plan.as_deref(), Some("plus"));
        assert_eq!(u.event_at.as_deref(), Some("2026-10-16T02:56:30.609Z"));
    }

    #[test]
    fn skips_cut_first_line_and_non_json() {
        // A fixture começa com uma linha cortada e tem uma linha que não é JSON.
        assert!(parse_codex_tail(SESSION).is_some());
        assert!(parse_codex_tail("{\"payload\": {\"rate_limits\": {\"primary\": {\"used_percent\": 5.0, \"window_minutes\": 300, \"resets_at\": 1}}}}\n}}}cortada").is_some());
    }

    #[test]
    fn none_without_rate_limits() {
        assert!(parse_codex_tail("{\"payload\": {\"type\": \"agent_message\"}}\n").is_none());
        assert!(parse_codex_tail("").is_none());
    }

    #[test]
    fn falls_back_to_older_session_without_rate_limits() {
        let dir = std::env::temp_dir().join(format!("zefiro-codex-{}", std::process::id()));
        let day = dir.join("2026").join("10").join("16");
        std::fs::create_dir_all(&day).unwrap();
        let old = day.join("old.jsonl");
        std::fs::write(&old, SESSION).unwrap();
        std::thread::sleep(std::time::Duration::from_millis(50));
        std::fs::write(day.join("new.jsonl"), "{\"payload\": {\"type\": \"session_meta\"}}\n").unwrap();
        let (path, _, usage) = read_codex(&dir).expect("arquivo mais velho");
        assert_eq!(path, old);
        assert_eq!(usage.primary.unwrap().used_percent, 80.0);
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn reset_window_counts_as_zero() {
        let w = Window { used_percent: 80.0, window_minutes: 300, resets_at: 1000 };
        assert_eq!(effective_percent(&w, 1000), 0.0);
        assert_eq!(effective_percent(&w, 999), 80.0);
    }

    #[test]
    fn window_names() {
        assert_eq!(window_name(300), "5 horas");
        assert_eq!(window_name(10080), "7 dias");
        assert_eq!(window_name(2880), "2 dias");
        assert_eq!(window_name(120), "2 horas");
    }
    /// Sessão mínima: `session_meta` (com a conta, se houver) e um evento com `rate_limits`.
    fn session(account: Option<&str>, used: f32, event_at: &str) -> String {
        let meta = match account {
            Some(a) => format!(r#"{{"type": "session_meta", "payload": {{"creator_account_id": "{a}"}}}}"#),
            None => r#"{"type": "session_meta", "payload": {"cwd": "C:\\"}}"#.to_string(),
        };
        format!("{meta}\n{{\"timestamp\": \"{event_at}\", \"payload\": {{\"rate_limits\": {{\"plan_type\": \"plus\", \"primary\": {{\"used_percent\": {used}, \"window_minutes\": 300, \"resets_at\": 4102444800}}}}}}}}\n")
    }

    /// Grava `text` em `dir/rel` com a data de modificação `secs` depois de 2026-01-01.
    fn put(dir: &Path, rel: &str, text: &str, secs: u64) -> PathBuf {
        let path = dir.join(rel);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(&path, text).unwrap();
        let at = std::time::UNIX_EPOCH + std::time::Duration::from_secs(1_767_225_600 + secs);
        std::fs::File::options().write(true).open(&path).unwrap().set_modified(at).unwrap();
        path
    }

    fn temp(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("zefiro-codex-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        dir
    }

    fn read(account: Option<&str>, used: f32, event_at: &str) -> CodexRead {
        CodexRead { account: account.map(String::from), modified: SystemTime::UNIX_EPOCH, usage: parse_codex_tail(&session(None, used, event_at)).unwrap() }
    }

    #[test]
    fn session_account_from_meta() {
        assert_eq!(session_account(r#"{"type": "session_meta", "payload": {"creator_account_id": "c1"}}"#).as_deref(), Some("c1"));
        assert_eq!(session_account(r#"{"type": "session_meta", "payload": {}}"#), None);
        assert_eq!(session_account("cortada"), None);
    }

    #[test]
    fn recent_sessions_are_newest_first() {
        let dir = temp("days");
        let a = put(&dir, "2026/10/20/a.jsonl", "", 10);
        let b = put(&dir, "2026/10/21/b.jsonl", "", 20);
        let old = put(&dir, "2025/01/01/velho.jsonl", "", 30);
        let found: Vec<PathBuf> = recent_sessions(&dir, 3).into_iter().map(|(_, p)| p).collect();
        assert_eq!(found, [old, b.clone(), a]);
        assert_eq!(recent_sessions(&dir, 1).len(), 1);
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn resumed_session_in_old_day_folder_is_found() {
        // `codex resume` continua gravando no arquivo da pasta do dia em que a sessão começou.
        let dir = temp("resumed");
        let resumed = put(&dir, "2026/10/18/retomada.jsonl", "", 100);
        put(&dir, "2026/10/20/a.jsonl", "", 10);
        put(&dir, "2026/10/21/b.jsonl", "", 20);
        assert_eq!(recent_sessions(&dir, 1)[0].1, resumed);
        assert_eq!(newest_session(&dir).map(|(_, p)| p), Some(resumed));
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn session_from_yesterday_modified_today_is_found() {
        let dir = temp("midnight");
        let a = put(&dir, "2026/10/20/a.jsonl", "", 50);
        put(&dir, "2026/10/21/b.jsonl", "", 20);
        assert_eq!(recent_sessions(&dir, 1)[0].1, a);
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn two_accounts_in_one_origin() {
        let dir = temp("two");
        put(&dir, "2026/10/20/velho.jsonl", &session(Some("c1"), 80.0, "2026-10-20T10:00:00Z"), 10);
        put(&dir, "2026/10/21/novo.jsonl", &session(Some("c2"), 42.0, "2026-10-21T10:00:00Z"), 20);
        let mut reads = read_codex_accounts(&dir);
        reads.sort_by(|a, b| a.account.cmp(&b.account));
        assert_eq!(reads.iter().map(|r| (r.account.as_deref(), r.usage.primary.unwrap().used_percent)).collect::<Vec<_>>(), [(Some("c1"), 80.0), (Some("c2"), 42.0)]);
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn file_without_account_joins_newest_known() {
        let dir = temp("anon");
        put(&dir, "2026/10/20/conta.jsonl", &session(Some("c1"), 80.0, "2026-10-20T10:00:00Z"), 10);
        put(&dir, "2026/10/21/sem.jsonl", &session(None, 12.0, "2026-10-21T10:00:00Z"), 20);
        let reads = read_codex_accounts(&dir);
        assert_eq!(reads.len(), 1);
        assert_eq!(reads[0].account.as_deref(), Some("c1"));
        assert_eq!(reads[0].usage.primary.unwrap().used_percent, 12.0);
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn same_account_across_origins_keeps_newest_event() {
        let wsl = OriginKind::Wsl("Ubuntu".into());
        let merged = merge_codex(vec![
            (wsl.clone(), vec![read(Some("c1"), 12.0, "2026-10-21T10:00:00Z")]),
            (OriginKind::Windows, vec![read(Some("c1"), 80.0, "2026-10-16T02:56:30Z")]),
        ]);
        assert_eq!(merged.len(), 1);
        assert_eq!(merged[0].origins, vec![OriginKind::Windows, wsl.clone()]);
        assert_eq!(merged[0].data.usage.primary.unwrap().used_percent, 12.0);
        let two = merge_codex(vec![(OriginKind::Windows, vec![read(Some("c1"), 1.0, "2026-10-21T10:00:00Z")]), (wsl, vec![read(Some("c2"), 2.0, "2026-10-21T10:00:00Z")])]);
        assert_eq!(two.len(), 2);
    }

    #[test]
    fn chip_and_next_reset() {
        let now = 1000;
        let entry = |used: f32, resets_at: i64| Account {
            key: format!("{used}"),
            origins: vec![OriginKind::Windows],
            stopped: false,
            data: CodexEntry { modified: SystemTime::UNIX_EPOCH, usage: CodexUsage { primary: Some(Window { used_percent: used, window_minutes: 300, resets_at }), ..Default::default() } },
        };
        let accounts = [entry(80.0, now + 100), entry(30.0, now + 500)];
        assert_eq!(codex_chip_percent(&accounts, now), Some(80.0));
        assert_eq!(next_reset(&accounts, now), Some(now + 100));
        assert_eq!(codex_chip_percent(&[entry(80.0, now), entry(30.0, now + 500)], now), Some(30.0));
        assert_eq!(codex_chip_percent(&[], now), None);
    }

    /// `Discover` de teste: devolve o que estiver no `Mutex` na hora.
    fn discover(d: Discovery) -> (std::sync::Arc<std::sync::Mutex<Discovery>>, crate::origins::Discover) {
        let shared = std::sync::Arc::new(std::sync::Mutex::new(d));
        let inner = shared.clone();
        (shared, std::sync::Arc::new(move || inner.lock().unwrap().clone()))
    }

    fn start(d: Discovery) -> (CodexService, std::sync::mpsc::Receiver<()>, std::sync::Arc<std::sync::Mutex<Discovery>>) {
        let (tx, rx) = std::sync::mpsc::channel();
        let (shared, discover) = discover(d);
        let svc = CodexService::start(discover, std::sync::Arc::new(move || { let _ = tx.send(()); })).unwrap();
        (svc, rx, shared)
    }

    fn windows_home(name: &str) -> (PathBuf, Discovery) {
        let home = temp(name);
        let d = Discovery { origins: vec![Origin { kind: OriginKind::Windows, home: home.clone() }], ..Default::default() };
        (home, d)
    }

    #[test]
    fn service_reads_on_start_and_rereads_on_refresh() {
        let (home, d) = windows_home("svc");
        put(&home, ".codex/sessions/a.jsonl", SESSION, 10);
        let (svc, rx, _) = start(d);
        rx.recv_timeout(std::time::Duration::from_secs(3)).expect("aviso inicial");
        assert_eq!(svc.latest().unwrap().accounts[0].data.usage.primary.unwrap().used_percent, 80.0);
        let newer = SESSION.replace("\"used_percent\": 80.0", "\"used_percent\": 42.0");
        put(&home, ".codex/sessions/b.jsonl", &newer, 20);
        svc.refresh();
        rx.recv_timeout(std::time::Duration::from_secs(3)).expect("aviso depois do refresh");
        assert_eq!(svc.latest().unwrap().accounts[0].data.usage.primary.unwrap().used_percent, 42.0);
        drop(svc);
        std::fs::remove_dir_all(&home).unwrap();
    }

    #[test]
    fn service_notifies_at_reset_without_new_file() {
        let (home, d) = windows_home("reset");
        let soon = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_secs() + 2;
        put(&home, ".codex/sessions/a.jsonl", &session(Some("c1"), 80.0, "2026-10-21T10:00:00Z").replace("4102444800", &soon.to_string()), 10);
        let (svc, rx, _) = start(d);
        rx.recv_timeout(std::time::Duration::from_secs(3)).expect("aviso inicial");
        rx.recv_timeout(std::time::Duration::from_secs(5)).expect("aviso no reinício da janela");
        drop(svc);
        std::fs::remove_dir_all(&home).unwrap();
    }

    #[test]
    fn service_keeps_wsl_event_of_merged_account_when_distro_stops() {
        let (windows, wsl_home) = (temp("merged-win"), temp("merged-wsl"));
        put(&windows, ".codex/sessions/a.jsonl", &session(Some("c1"), 80.0, "2026-10-16T02:56:30Z"), 10);
        put(&wsl_home, ".codex/sessions/b.jsonl", &session(Some("c1"), 45.0, "2026-10-21T10:00:00Z"), 20);
        let wsl = OriginKind::Wsl("U".into());
        let both = vec![Origin { kind: OriginKind::Windows, home: windows.clone() }, Origin { kind: wsl.clone(), home: wsl_home.clone() }];
        let (svc, rx, shared) = start(Discovery { origins: both.clone(), running: vec!["U".into()], wsl_stopped: false });
        rx.recv_timeout(std::time::Duration::from_secs(3)).expect("aviso inicial");
        assert_eq!(svc.latest().unwrap().accounts[0].data.usage.primary.unwrap().used_percent, 45.0);
        *shared.lock().unwrap() = Discovery { origins: vec![both[0].clone()], running: Vec::new(), wsl_stopped: true };
        svc.refresh();
        // Sem aviso: o uso não mudou (o evento do WSL continua valendo).
        let _ = rx.recv_timeout(std::time::Duration::from_millis(500));
        let snap = svc.latest().unwrap();
        assert_eq!(snap.accounts.len(), 1);
        assert_eq!(snap.accounts[0].data.usage.primary.unwrap().used_percent, 45.0);
        assert_eq!(snap.accounts[0].origins, vec![OriginKind::Windows, wsl]);
        assert!(!snap.accounts[0].stopped, "a conta continua no Windows");
        drop(svc);
        std::fs::remove_dir_all(&windows).unwrap();
        std::fs::remove_dir_all(&wsl_home).unwrap();
    }

    #[test]
    fn dropping_the_service_does_not_wait_for_a_slow_cycle() {
        // `discover` preso (um `wsl.exe` ou um `\\wsl.localhost` lento): soltar o serviço na thread
        // da UI não pode esperar por ele.
        let slow: crate::origins::Discover = std::sync::Arc::new(|| {
            std::thread::sleep(std::time::Duration::from_secs(2));
            Discovery::default()
        });
        let svc = CodexService::start(slow, std::sync::Arc::new(|| {})).unwrap();
        std::thread::sleep(std::time::Duration::from_millis(50));
        let t = std::time::Instant::now();
        drop(svc);
        assert!(t.elapsed() < std::time::Duration::from_millis(500), "{:?}", t.elapsed());
    }

    #[test]
    fn service_keeps_stopped_wsl_account() {
        let home = temp("stopped");
        put(&home, ".codex/sessions/a.jsonl", &session(Some("c1"), 80.0, "2026-10-21T10:00:00Z"), 10);
        let wsl = OriginKind::Wsl("U".into());
        let (svc, rx, shared) = start(Discovery { origins: vec![Origin { kind: wsl.clone(), home: home.clone() }], running: vec!["U".into()], wsl_stopped: false });
        rx.recv_timeout(std::time::Duration::from_secs(3)).expect("aviso inicial");
        *shared.lock().unwrap() = Discovery { origins: Vec::new(), running: Vec::new(), wsl_stopped: true };
        svc.refresh();
        rx.recv_timeout(std::time::Duration::from_secs(3)).expect("aviso com a distro parada");
        let snap = svc.latest().unwrap();
        assert!(snap.wsl_stopped);
        assert_eq!(snap.accounts.len(), 1);
        assert!(snap.accounts[0].stopped);
        assert_eq!(snap.accounts[0].origins, vec![wsl]);
        drop(svc);
        std::fs::remove_dir_all(&home).unwrap();
    }
}
