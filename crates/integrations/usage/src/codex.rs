//! Uso do Codex a partir dos `.jsonl` de sessão em `~/.codex/sessions` (sem abrir credenciais).

use serde_json::Value;
use std::io::{Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};
use std::time::SystemTime;

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
    let mut files = Vec::new();
    collect(sessions_dir, &mut files);
    files.into_iter().max_by(|a, b| a.0.cmp(&b.0))
}

/// O uso mais recente: do arquivo mais novo para o mais velho, até `MAX_FILES`.
pub fn read_codex(sessions_dir: &Path) -> Option<(PathBuf, SystemTime, CodexUsage)> {
    let mut files = Vec::new();
    collect(sessions_dir, &mut files);
    files.sort_by_key(|f| std::cmp::Reverse(f.0));
    files.into_iter().take(MAX_FILES).find_map(|(modified, path)| {
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
