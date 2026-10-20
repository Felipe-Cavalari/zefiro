//! Atualização pelo GitHub Releases (ADR-0014): uma thread `zefiro-update` consulta a última release
//! 60 s depois de abrir e a cada 24 h, e baixa o instalador quando o usuário pede. Avisa a UI por
//! `notify`; quem decide rodar o instalador é o app.

use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, RecvTimeoutError, Sender};
use std::sync::{Arc, Mutex};
use std::time::Duration;
use windows::Win32::UI::Shell::ShellExecuteW;
use windows::Win32::UI::WindowsAndMessaging::SW_HIDE;
use windows::core::{HSTRING, w};
use zefiro_integration_update::{LATEST_URL, Release, parse_latest, trusted_url, verify};

const FIRST_CHECK: Duration = Duration::from_secs(60);
const EVERY: Duration = Duration::from_secs(24 * 60 * 60);
const API_TIMEOUT: Duration = Duration::from_secs(20);
const DOWNLOAD_TIMEOUT: Duration = Duration::from_secs(300);

#[derive(Debug, Clone, PartialEq)]
pub enum UpdateState {
    /// Nada novo (ou ainda não consultou).
    Idle,
    Available(Release),
    Downloading(Release),
    /// Baixado e conferido: pronto para rodar.
    Ready(Release, PathBuf),
    Failed(Release, String),
}

enum Command { Download, Stop }

pub struct UpdateService { tx: Sender<Command>, state: Arc<Mutex<UpdateState>> }

impl UpdateService {
    /// `current`: a versão deste exe. `notify` roda na thread de atualização a cada mudança de estado.
    pub fn start(current: String, notify: Arc<dyn Fn() + Send + Sync>) -> std::io::Result<UpdateService> {
        let (tx, rx) = mpsc::channel();
        let state = Arc::new(Mutex::new(UpdateState::Idle));
        let shared = state.clone();
        std::thread::Builder::new().name("zefiro-update".into()).spawn(move || {
            let set = |s: UpdateState| {
                if let Ok(mut g) = shared.lock() {
                    *g = s;
                }
                notify();
            };
            let get = || shared.lock().map(|g| g.clone()).unwrap_or(UpdateState::Idle);
            let mut wait = FIRST_CHECK;
            loop {
                match rx.recv_timeout(wait) {
                    Err(RecvTimeoutError::Timeout) => {
                        wait = EVERY;
                        // Não troca um download em andamento ou pronto por uma consulta nova.
                        if matches!(get(), UpdateState::Idle | UpdateState::Available(_) | UpdateState::Failed(..)) {
                            match check(&current) {
                                Ok(Some(r)) => {
                                    tracing::info!(versao = %r.version, "atualização disponível");
                                    set(UpdateState::Available(r));
                                }
                                Ok(None) => tracing::info!("Zefiro {current} é a versão mais nova"),
                                Err(e) => tracing::warn!("não consegui consultar as releases: {e}"),
                            }
                        }
                    }
                    Ok(Command::Download) => {
                        let release = match get() {
                            UpdateState::Available(r) | UpdateState::Failed(r, _) => r,
                            _ => continue,
                        };
                        set(UpdateState::Downloading(release.clone()));
                        match download(&release) {
                            Ok(path) => {
                                tracing::info!(versao = %release.version, "instalador baixado e conferido");
                                set(UpdateState::Ready(release, path));
                            }
                            Err(e) => {
                                tracing::warn!(versao = %release.version, "atualização falhou: {e}");
                                set(UpdateState::Failed(release, e));
                            }
                        }
                    }
                    Ok(Command::Stop) | Err(RecvTimeoutError::Disconnected) => break,
                }
            }
        })?;
        Ok(UpdateService { tx, state })
    }

    pub fn state(&self) -> UpdateState {
        self.state.lock().map(|g| g.clone()).unwrap_or(UpdateState::Idle)
    }

    /// Baixa e confere o instalador da versão disponível (ou tenta de novo depois de uma falha).
    pub fn download(&self) {
        let _ = self.tx.send(Command::Download);
    }
}

impl Drop for UpdateService {
    fn drop(&mut self) {
        let _ = self.tx.send(Command::Stop);
    }
}

fn check(current: &str) -> Result<Option<Release>, String> {
    let (status, body) = crate::http::get_with(LATEST_URL, &[("Accept", "application/vnd.github+json")], API_TIMEOUT).map_err(|e| e.to_string())?;
    match status {
        200 => {}
        404 => return Ok(None), // nenhuma release publicada
        s => return Err(format!("o GitHub respondeu {s}")),
    }
    let release = parse_latest(&body)?;
    let current = zefiro_integration_update::Version::parse(current).ok_or("versão local inválida")?;
    Ok((release.version > current).then_some(release))
}

fn download(release: &Release) -> Result<PathBuf, String> {
    if !trusted_url(&release.asset.url) {
        return Err("download fora do repositório".into());
    }
    let (status, bytes) = crate::http::get_bytes_with(&release.asset.url, DOWNLOAD_TIMEOUT).map_err(|e| e.to_string())?;
    if status != 200 {
        return Err(format!("o GitHub respondeu {status} ao baixar"));
    }
    verify(&bytes, &release.asset)?;
    let dir = std::env::temp_dir().join("zefiro-update");
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let path = dir.join(&release.asset.name);
    std::fs::write(&path, &bytes).map_err(|e| e.to_string())?;
    Ok(path)
}

/// Roda o instalador baixado sem janelas. Ele fecha o Zefiro (se ainda estiver aberto), atualiza e
/// abre de novo. `true` se o Windows aceitou abrir.
pub fn run_installer(path: &Path) -> bool {
    // SAFETY: strings válidas durante a chamada; o processo do instalador não depende de nós.
    let r = unsafe { ShellExecuteW(None, w!("open"), &HSTRING::from(path.as_os_str()), w!("/VERYSILENT /SUPPRESSMSGBOXES /NORESTART"), None, SW_HIDE) };
    r.0 as isize > 32
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[ignore = "usa a rede: cargo test -p zefiro-platform-windows update -- --ignored"]
    fn real_latest_release_is_found_downloaded_and_verified() {
        let release = check("1.0.0").expect("consulta").expect("há versão mais nova que a 1.0.0");
        assert!(release.asset.sha256.is_some(), "o GitHub publica o digest");
        let path = download(&release).expect("baixa e confere");
        assert_eq!(std::fs::metadata(&path).unwrap().len(), release.asset.size);
        let _ = std::fs::remove_file(path);
        assert!(check("999.0.0").unwrap().is_none());
    }
}
