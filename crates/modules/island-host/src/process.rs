//! Processo do zefiro-island.exe (std::process; sem a crate windows).

use std::path::{Path, PathBuf};
use std::process::{Child, Command};
use std::time::{Duration, Instant};

pub fn resolve_exe(configured: &str, beside: &Path) -> Option<PathBuf> {
    let candidate = if configured.is_empty() { beside.join("zefiro-island.exe") } else { PathBuf::from(configured) };
    candidate.is_file().then_some(candidate)
}

pub struct IslandProcess { exe: PathBuf, child: Option<Child> }

impl IslandProcess {
    /// Para uma instância anterior (a ilha só aceita uma) e inicia outra.
    pub fn start(exe: &Path) -> std::io::Result<IslandProcess> {
        let _ = Command::new(exe).arg("--stop").status();
        std::thread::sleep(Duration::from_millis(300));
        let child = Command::new(exe).spawn()?;
        tracing::info!(pid = child.id(), exe = %exe.display(), "ilha iniciada");
        Ok(IslandProcess { exe: exe.to_path_buf(), child: Some(child) })
    }

    pub fn has_exited(&mut self) -> bool {
        match self.child.as_mut() {
            None => true,
            Some(c) => !matches!(c.try_wait(), Ok(None)),
        }
    }

    /// Pede para fechar (`--stop`), espera até `timeout` e, se não fechar, encerra o processo.
    pub fn stop(&mut self, timeout: Duration) {
        let Some(mut child) = self.child.take() else { return };
        let _ = Command::new(&self.exe).arg("--stop").status();
        let deadline = Instant::now() + timeout;
        while Instant::now() < deadline {
            if !matches!(child.try_wait(), Ok(None)) {
                return;
            }
            std::thread::sleep(Duration::from_millis(50));
        }
        tracing::warn!("a ilha não fechou em {timeout:?}; encerrando");
        let _ = child.kill();
        let _ = child.wait();
    }
}
