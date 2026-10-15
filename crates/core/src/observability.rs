//! Logs em arquivo com rotação diária e relatório de crash.

use std::ffi::OsString;
use std::path::PathBuf;
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

/// Mantém o escritor de logs vivo; ao ser descartado, grava o que falta.
pub struct LogGuard {
    _guard: tracing_appender::non_blocking::WorkerGuard,
}

/// Pasta de logs a partir das variáveis de ambiente (função pura, testável).
pub fn log_dir_from(local_app_data: Option<OsString>, xdg_state_home: Option<OsString>, home: Option<OsString>) -> PathBuf {
    if let Some(local) = local_app_data {
        return PathBuf::from(local).join("zefiro").join("logs");
    }
    if let Some(state) = xdg_state_home {
        return PathBuf::from(state).join("zefiro").join("logs");
    }
    let home = home.map(PathBuf::from).unwrap_or_else(|| PathBuf::from("."));
    home.join(".local").join("state").join("zefiro").join("logs")
}

/// `%LOCALAPPDATA%\zefiro\logs` no Windows.
pub fn default_log_dir() -> PathBuf {
    log_dir_from(std::env::var_os("LOCALAPPDATA"), std::env::var_os("XDG_STATE_HOME"), std::env::var_os("HOME"))
}

/// Arquivo diário `zefiro.AAAA-MM-DD.log`, mantendo 7 arquivos.
pub fn init_logging(dir: &Path, filter: &str) -> std::io::Result<LogGuard> {
    std::fs::create_dir_all(dir)?;
    let appender = tracing_appender::rolling::Builder::new()
        .rotation(tracing_appender::rolling::Rotation::DAILY)
        .filename_prefix("zefiro")
        .filename_suffix("log")
        .max_log_files(7)
        .build(dir)
        .map_err(std::io::Error::other)?;
    let (writer, guard) = tracing_appender::non_blocking(appender);
    tracing_subscriber::fmt()
        .with_writer(writer)
        .with_ansi(false)
        .with_env_filter(tracing_subscriber::EnvFilter::new(filter))
        .try_init()
        .map_err(std::io::Error::other)?;
    Ok(LogGuard { _guard: guard })
}

/// Em panic, grava `crash-<segundos>-<ms>-<pid>.txt` com versão, mensagem e backtrace.
pub fn install_panic_hook(dir: PathBuf, version: &'static str) {
    let previous = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        let backtrace = std::backtrace::Backtrace::force_capture();
        let now = SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default();
        let name = format!("crash-{}-{:03}-{}.txt", now.as_secs(), now.subsec_millis(), std::process::id());
        let body = format!("zefiro {version}\n{info}\n\nbacktrace:\n{backtrace}\n");
        let _ = std::fs::create_dir_all(&dir);
        let _ = std::fs::write(dir.join(name), body);
        tracing::error!(panic = %info, "panic");
        previous(info);
    }));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn windows_uses_local_app_data() {
        let p = log_dir_from(Some(OsString::from(r"C:\Users\x\AppData\Local")), None, None);
        assert_eq!(p, PathBuf::from(r"C:\Users\x\AppData\Local").join("zefiro").join("logs"));
    }

    #[test]
    fn linux_prefers_xdg_state_then_home() {
        assert_eq!(log_dir_from(None, Some("/s".into()), Some("/h".into())), PathBuf::from("/s/zefiro/logs"));
        assert_eq!(log_dir_from(None, None, Some("/h".into())), PathBuf::from("/h/.local/state/zefiro/logs"));
        assert_eq!(log_dir_from(None, None, None), PathBuf::from("./.local/state/zefiro/logs"));
    }
}
