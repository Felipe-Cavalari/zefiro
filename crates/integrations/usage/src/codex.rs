//! Uso do Codex a partir dos `.jsonl` de sessão em `~/.codex/sessions` (sem abrir credenciais).

use serde_json::Value;
use std::io::{Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};
use std::time::SystemTime;

use crate::format::parse_iso_utc;
use crate::origins::{Account, OriginKind, sort_accounts};

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

/// Percurso das sessões (`sessions/AAAA/MM/DD`): as pastas de data, da mais nova para a mais velha,
/// até juntar `limit` arquivos e mais uma pasta de dia (a sessão que passou da meia-noite fica na
/// pasta do dia em que começou). Pastas com outros nomes são sempre percorridas.
struct Walk { files: Vec<(SystemTime, PathBuf)>, limit: usize, extra_day_done: bool }

impl Walk {
    fn done(&self) -> bool {
        self.files.len() >= self.limit && self.extra_day_done
    }

    fn visit(&mut self, dir: &Path, dated: bool) {
        let Ok(entries) = std::fs::read_dir(dir) else { return };
        let before = self.files.len();
        let (mut dates, mut others) = (Vec::new(), Vec::new());
        for entry in entries.flatten() {
            let path = entry.path();
            let Ok(meta) = entry.metadata() else { continue };
            if meta.is_dir() {
                let numeric = path.file_name().and_then(|n| n.to_str()).is_some_and(|n| !n.is_empty() && n.bytes().all(|b| b.is_ascii_digit()));
                if numeric { dates.push(path) } else { others.push(path) }
            } else if path.extension().is_some_and(|e| e == "jsonl")
                && let Ok(modified) = meta.modified()
            {
                self.files.push((modified, path));
            }
        }
        for other in others {
            self.visit(&other, false);
        }
        dates.sort_by(|a, b| b.cmp(a));
        let day = dated && dates.is_empty();
        for date in dates {
            if self.done() {
                break;
            }
            self.visit(&date, true);
        }
        if day && before >= self.limit {
            self.extra_day_done = true;
        }
    }
}

/// Os `.jsonl` das pastas de data mais novas (§6.4 da spec), do mais modificado para o mais velho.
pub fn recent_sessions(sessions_dir: &Path, limit: usize) -> Vec<(SystemTime, PathBuf)> {
    let mut walk = Walk { files: Vec::new(), limit, extra_day_done: false };
    walk.visit(sessions_dir, false);
    walk.files.sort_by_key(|f| std::cmp::Reverse(f.0));
    walk.files
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

/// Ritmo da leitura: só um `stat` do arquivo mais novo; relê quando ele muda.
const INTERVAL: std::time::Duration = std::time::Duration::from_secs(120);

enum Wake { Refresh, Stop }

type Shared = std::sync::Arc<std::sync::Mutex<Option<(SystemTime, CodexUsage)>>>;

/// Thread `zefiro-codex`: lê ao iniciar, a cada 120 s e quando pedem (`refresh`), e avisa
/// `notify` (em outra thread) quando o uso muda.
pub struct CodexService {
    tx: std::sync::mpsc::Sender<Wake>,
    state: Shared,
    thread: Option<std::thread::JoinHandle<()>>,
}

impl CodexService {
    pub fn start(sessions_dir: PathBuf, notify: std::sync::Arc<dyn Fn() + Send + Sync>) -> std::io::Result<CodexService> {
        let (tx, rx) = std::sync::mpsc::channel();
        let state: Shared = Default::default();
        let shared = state.clone();
        let thread = std::thread::Builder::new().name("zefiro-codex".into()).spawn(move || {
            let mut seen: Option<(SystemTime, PathBuf)> = None;
            loop {
                let newest = newest_session(&sessions_dir);
                if newest != seen {
                    seen = newest;
                    let usage = read_codex(&sessions_dir).map(|(_, modified, usage)| (modified, usage));
                    let changed = shared.lock().map(|mut s| std::mem::replace(&mut *s, usage.clone()) != usage).unwrap_or(false);
                    if changed {
                        notify();
                    }
                }
                match rx.recv_timeout(INTERVAL) {
                    Ok(Wake::Refresh) | Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {}
                    Ok(Wake::Stop) | Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => break,
                }
            }
        })?;
        Ok(CodexService { tx, state, thread: Some(thread) })
    }

    /// O último uso lido e quando o arquivo dele foi modificado.
    pub fn latest(&self) -> Option<(SystemTime, CodexUsage)> {
        self.state.lock().ok().and_then(|s| s.clone())
    }

    /// Confere o arquivo agora (ao abrir o painel).
    pub fn refresh(&self) {
        let _ = self.tx.send(Wake::Refresh);
    }
}

impl Drop for CodexService {
    fn drop(&mut self) {
        let _ = self.tx.send(Wake::Stop);
        if let Some(t) = self.thread.take() {
            let _ = t.join();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::origins::OriginKind;

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
    fn recent_sessions_walks_newest_days_first() {
        let dir = temp("days");
        let a = put(&dir, "2026/10/20/a.jsonl", "", 10);
        let b = put(&dir, "2026/10/21/b.jsonl", "", 20);
        let old = put(&dir, "2025/01/01/velho.jsonl", "", 30);
        let found: Vec<PathBuf> = recent_sessions(&dir, 1).into_iter().map(|(_, p)| p).collect();
        assert!(found.contains(&a) && found.contains(&b), "{found:?}");
        assert!(!found.contains(&old), "{found:?}");
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

    #[test]
    fn service_reads_on_start_and_rereads_on_refresh() {
        let dir = std::env::temp_dir().join(format!("zefiro-codex-svc-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("a.jsonl"), SESSION).unwrap();
        let (tx, rx) = std::sync::mpsc::channel();
        let svc = CodexService::start(dir.clone(), std::sync::Arc::new(move || { let _ = tx.send(()); })).unwrap();
        rx.recv_timeout(std::time::Duration::from_secs(3)).expect("aviso inicial");
        assert_eq!(svc.latest().unwrap().1.primary.unwrap().used_percent, 80.0);
        std::thread::sleep(std::time::Duration::from_millis(50));
        let newer = SESSION.replace("\"used_percent\": 80.0", "\"used_percent\": 42.0");
        std::fs::write(dir.join("b.jsonl"), newer).unwrap();
        svc.refresh();
        rx.recv_timeout(std::time::Duration::from_secs(3)).expect("aviso depois do refresh");
        assert_eq!(svc.latest().unwrap().1.primary.unwrap().used_percent, 42.0);
        drop(svc);
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
