//! Logs em arquivo com rotação diária e relatório de crash.

use std::ffi::OsString;
use std::path::PathBuf;
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

/// Mantém o escritor de logs vivo; ao ser descartado, grava o que falta. `recent` guarda os últimos
/// avisos e erros (o HUD de debug mostra).
pub struct LogGuard {
    _guard: tracing_appender::non_blocking::WorkerGuard,
    pub recent: RecentWarnings,
}

/// Quantos avisos e erros o HUD guarda.
pub const RECENT_CAPACITY: usize = 16;

/// Um aviso (`WARN`) ou erro (`ERROR`) do log.
#[derive(Debug, Clone, PartialEq)]
pub struct Warning { pub at_unix_s: u64, pub error: bool, pub target: String, pub message: String }

/// Os últimos avisos e erros, do mais antigo ao mais novo. Clonar compartilha a mesma lista.
#[derive(Debug, Clone, Default)]
pub struct RecentWarnings(std::sync::Arc<std::sync::Mutex<std::collections::VecDeque<Warning>>>);

impl RecentWarnings {
    pub fn list(&self) -> Vec<Warning> {
        self.0.lock().map(|l| l.iter().cloned().collect()).unwrap_or_default()
    }
}

/// Camada do `tracing` que alimenta `RecentWarnings`.
pub struct RecentLayer { store: RecentWarnings, capacity: usize }

impl RecentLayer {
    pub fn new(store: RecentWarnings, capacity: usize) -> RecentLayer {
        RecentLayer { store, capacity: capacity.max(1) }
    }
}

/// Junta a mensagem e os outros campos ("mensagem k=v").
#[derive(Default)]
struct MessageVisitor { message: String, fields: Vec<String> }

impl tracing::field::Visit for MessageVisitor {
    fn record_str(&mut self, field: &tracing::field::Field, value: &str) {
        if field.name() == "message" { self.message = value.to_string() } else { self.fields.push(format!("{}={value}", field.name())) }
    }

    fn record_debug(&mut self, field: &tracing::field::Field, value: &dyn std::fmt::Debug) {
        if field.name() == "message" { self.message = format!("{value:?}") } else { self.fields.push(format!("{}={value:?}", field.name())) }
    }
}

impl<S: tracing::Subscriber> tracing_subscriber::Layer<S> for RecentLayer {
    fn on_event(&self, event: &tracing::Event<'_>, _ctx: tracing_subscriber::layer::Context<'_, S>) {
        let level = *event.metadata().level();
        if level > tracing::Level::WARN {
            return;
        }
        let mut v = MessageVisitor::default();
        event.record(&mut v);
        let message = std::iter::once(v.message).chain(v.fields).filter(|s| !s.is_empty()).collect::<Vec<_>>().join(" ");
        let at_unix_s = SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |d| d.as_secs());
        let w = Warning { at_unix_s, error: level == tracing::Level::ERROR, target: event.metadata().target().to_string(), message };
        if let Ok(mut list) = self.store.0.lock() {
            list.push_back(w);
            while list.len() > self.capacity {
                list.pop_front();
            }
        }
    }
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
    let recent = RecentWarnings::default();
    use tracing_subscriber::layer::SubscriberExt;
    use tracing_subscriber::util::SubscriberInitExt;
    tracing_subscriber::registry()
        .with(tracing_subscriber::EnvFilter::new(filter))
        .with(tracing_subscriber::fmt::layer().with_writer(writer).with_ansi(false))
        .with(RecentLayer::new(recent.clone(), RECENT_CAPACITY))
        .try_init()
        .map_err(std::io::Error::other)?;
    Ok(LogGuard { _guard: guard, recent })
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
    fn recent_keeps_only_the_last_warnings_and_errors() {
        use tracing_subscriber::layer::SubscriberExt;
        let recent = RecentWarnings::default();
        let subscriber = tracing_subscriber::registry().with(RecentLayer::new(recent.clone(), 2));
        tracing::subscriber::with_default(subscriber, || {
            tracing::info!("ignorado");
            tracing::warn!("primeiro aviso");
            tracing::warn!(codigo = 7, "clima indisponível");
            tracing::error!("falhou");
        });
        let list = recent.list();
        assert_eq!(list.len(), 2, "{list:?}");
        assert_eq!((list[0].error, list[0].message.as_str()), (false, "clima indisponível codigo=7"));
        assert_eq!((list[1].error, list[1].message.as_str()), (true, "falhou"));
        assert!(list[1].target.contains("observability"), "{}", list[1].target);
        assert!(list[1].at_unix_s > 1_700_000_000);
    }

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
