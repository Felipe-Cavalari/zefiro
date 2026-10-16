//! Leitura e persistência fora da thread de UI, com revisão conferida antes da substituição.

use std::{io::Write, path::{Path, PathBuf}, sync::{atomic::{AtomicU64, Ordering}, mpsc}};

#[derive(Debug)]
pub enum SaveError { Conflict(String), Io(String) }

impl From<std::io::Error> for SaveError { fn from(error: std::io::Error) -> Self { Self::Io(error.to_string()) } }

/// Conferência otimista seguida de substituição atômica. A janela de corrida com escritores
/// externos não cooperantes foi aceita pelo usuário e fica documentada no ADR da fase.
pub fn save_snapshot(path: &Path, expected: &str, output: &str) -> Result<(), SaveError> {
    save_snapshot_before_commit(path, expected, output, || {})
}

fn save_snapshot_before_commit(path: &Path, expected: &str, output: &str, before_commit: impl FnOnce()) -> Result<(), SaveError> {
    zefiro_core::config::parse_config(output).map_err(|e| SaveError::Io(e.message))?;
    let current = std::fs::read_to_string(path)?;
    if current != expected { return Err(SaveError::Conflict(current)); }
    static NEXT: AtomicU64 = AtomicU64::new(0);
    let temporary = path.with_extension(format!("toml.{}-{}.tmp", std::process::id(), NEXT.fetch_add(1, Ordering::Relaxed)));
    let mut file = std::fs::OpenOptions::new().create_new(true).write(true).open(&temporary)?;
    let result = (|| {
        file.write_all(output.as_bytes())?;
        file.sync_all()?;
        drop(file);
        before_commit();
        let latest = std::fs::read_to_string(path)?;
        if latest != expected { return Err(SaveError::Conflict(latest)); }
        std::fs::rename(&temporary, path)?;
        Ok(())
    })();
    if result.is_err() { let _ = std::fs::remove_file(&temporary); }
    result
}

enum Job { Read { generation: u64, path: PathBuf }, Save { generation: u64, path: PathBuf, expected: String, output: String } }

pub struct IoReply { pub generation: u64, pub saved: bool, pub result: Result<String, SaveError> }

pub struct ConfigWorker { tx: Option<mpsc::Sender<Job>>, rx: mpsc::Receiver<IoReply>, thread: Option<std::thread::JoinHandle<()>> }

impl ConfigWorker {
    pub fn new() -> std::io::Result<Self> {
        let (tx, jobs) = mpsc::channel();
        let (replies, rx) = mpsc::channel();
        let thread = std::thread::Builder::new().name("zefiro-config".into()).spawn(move || {
            while let Ok(job) = jobs.recv() {
                let reply = match job {
                    Job::Read { generation, path } => IoReply { generation, saved: false, result: std::fs::read_to_string(path).map_err(SaveError::from) },
                    Job::Save { generation, path, expected, output } => IoReply { generation, saved: true, result: save_snapshot(&path, &expected, &output).map(|()| output) },
                };
                if replies.send(reply).is_err() { break; }
                zefiro_platform_windows::window::post_user(crate::event_loop::USER_EDITOR_IO);
            }
        })?;
        Ok(Self { tx: Some(tx), rx, thread: Some(thread) })
    }

    pub fn read(&self, generation: u64, path: PathBuf) { if let Some(tx) = &self.tx { let _ = tx.send(Job::Read { generation, path }); } }
    pub fn save(&self, generation: u64, path: PathBuf, expected: String, output: String) { if let Some(tx) = &self.tx { let _ = tx.send(Job::Save { generation, path, expected, output }); } }
    pub fn replies(&self) -> Vec<IoReply> { self.rx.try_iter().collect() }
}

impl Drop for ConfigWorker {
    fn drop(&mut self) {
        self.tx.take();
        if let Some(thread) = self.thread.take() { let _ = thread.join(); }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn save_replaces_complete_valid_file_and_detects_external_revision() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.toml");
        let base = "theme = 'obsidian'\n";
        std::fs::write(&path, base).unwrap();
        save_snapshot(&path, base, "theme = 'nord'\n").unwrap();
        assert_eq!(std::fs::read_to_string(&path).unwrap(), "theme = 'nord'\n");
        assert!(matches!(save_snapshot(&path, base, "theme = 'plum'\n"), Err(SaveError::Conflict(_))));
        assert_eq!(std::fs::read_to_string(&path).unwrap(), "theme = 'nord'\n");
        assert_eq!(std::fs::read_dir(dir.path()).unwrap().count(), 1);
    }

    #[test]
    fn invalid_output_and_missing_source_never_create_a_partial_config() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.toml");
        std::fs::write(&path, "theme = 'nord'\n").unwrap();
        assert!(save_snapshot(&path, "theme = 'nord'\n", "[broken").is_err());
        assert_eq!(std::fs::read_to_string(&path).unwrap(), "theme = 'nord'\n");
        assert!(save_snapshot(&dir.path().join("missing.toml"), "", "").is_err());
    }

    #[test]
    fn external_save_during_temporary_preparation_is_preserved_and_temporary_is_removed() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.toml");
        let base = "theme = 'obsidian'\n";
        std::fs::write(&path, base).unwrap();
        let result = save_snapshot_before_commit(&path, base, "theme = 'nord'\n", || {
            std::fs::write(&path, "theme = 'plum' # outro editor\n").unwrap();
        });
        assert!(matches!(result, Err(SaveError::Conflict(_))));
        assert_eq!(std::fs::read_to_string(&path).unwrap(), "theme = 'plum' # outro editor\n");
        assert_eq!(std::fs::read_dir(dir.path()).unwrap().count(), 1);
    }

    #[test]
    fn failed_replacement_keeps_original_file_and_removes_own_temporary() {
        use std::os::windows::fs::OpenOptionsExt;
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.toml");
        let base = "theme = 'obsidian'\n";
        std::fs::write(&path, base).unwrap();
        // Permite ler/escrever, mas não substituir: simula outro processo mantendo o arquivo aberto.
        let held = std::fs::OpenOptions::new().read(true).share_mode(3).open(&path).unwrap();
        assert!(matches!(save_snapshot(&path, base, "theme = 'nord'\n"), Err(SaveError::Io(_))));
        assert_eq!(std::fs::read_to_string(&path).unwrap(), base);
        assert_eq!(std::fs::read_dir(dir.path()).unwrap().count(), 1);
        drop(held);
    }
}
