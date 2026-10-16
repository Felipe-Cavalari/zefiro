//! Notificações do Windows (`UserNotificationListener`), como a ilha C++ (ADR-0006: funciona sem
//! pacote). Sem identidade de pacote não há evento de mudança: a thread consulta a lista uma vez
//! por segundo, fora da thread de UI, e só avisa quando chega uma notificação nova.

use std::collections::HashSet;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use windows::Foundation::Size;
use windows::UI::Notifications::Management::{UserNotificationListener, UserNotificationListenerAccessStatus};
use windows::UI::Notifications::{KnownNotificationBindings, NotificationKinds, UserNotification};
use windows::Win32::Foundation::FILETIME;
use windows::Win32::System::SystemInformation::GetSystemTimeAsFileTime;
use windows::Win32::System::Threading::{GetCurrentProcess, GetProcessTimes};
use windows::Win32::System::WinRT::{RO_INIT_MULTITHREADED, RoInitialize, RoUninitialize};
use windows::core::{Interface, Result};
use zefiro_platform::image::Image;

#[derive(Debug, Clone)]
pub struct Notification { pub app: String, pub text: String, pub icon: Option<Image> }

pub struct NotificationWatcher {
    stop: Arc<AtomicBool>,
    queue: Arc<Mutex<Vec<Notification>>>,
    thread: Option<std::thread::JoinHandle<()>>,
}

impl NotificationWatcher {
    pub fn start(notify: Arc<dyn Fn() + Send + Sync>) -> std::io::Result<NotificationWatcher> {
        let stop = Arc::new(AtomicBool::new(false));
        let queue = Arc::new(Mutex::new(Vec::new()));
        let (flag, shared) = (stop.clone(), queue.clone());
        let thread = std::thread::Builder::new().name("zefiro-notifications".into()).spawn(move || {
            // SAFETY: WinRT nesta thread, desfeito no fim dela.
            let inited = unsafe { RoInitialize(RO_INIT_MULTITHREADED) }.is_ok();
            run(&flag, &shared, notify.as_ref());
            if inited {
                // SAFETY: par do RoInitialize.
                unsafe { RoUninitialize() };
            }
        })?;
        Ok(NotificationWatcher { stop, queue, thread: Some(thread) })
    }

    /// Notificações novas desde a última chamada.
    pub fn take(&self) -> Vec<Notification> {
        self.queue.lock().map(|mut q| std::mem::take(&mut *q)).unwrap_or_default()
    }
}

impl Drop for NotificationWatcher {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        if let Some(t) = self.thread.take() {
            let _ = t.join();
        }
    }
}

/// Espera até `d`, acordando cedo se pedirem para parar. `false` = parar.
fn sleep(stop: &AtomicBool, d: Duration) -> bool {
    let until = Instant::now() + d;
    while Instant::now() < until {
        if stop.load(Ordering::Relaxed) {
            return false;
        }
        std::thread::sleep(Duration::from_millis(100));
    }
    !stop.load(Ordering::Relaxed)
}

/// Tempo desde que o processo começou (a ilha C++ espera o subsistema UWP subir no login).
fn process_uptime() -> Duration {
    let (mut created, mut exited, mut kernel, mut user) = (FILETIME::default(), FILETIME::default(), FILETIME::default(), FILETIME::default());
    // SAFETY: estruturas locais preenchidas pelo Windows.
    unsafe {
        if GetProcessTimes(GetCurrentProcess(), &mut created, &mut exited, &mut kernel, &mut user).is_err() {
            return Duration::MAX;
        }
        let now = GetSystemTimeAsFileTime();
        let ticks = |f: FILETIME| ((f.dwHighDateTime as u64) << 32) | f.dwLowDateTime as u64;
        Duration::from_nanos(ticks(now).saturating_sub(ticks(created)) * 100)
    }
}

fn run(stop: &AtomicBool, queue: &Mutex<Vec<Notification>>, notify: &(dyn Fn() + Send + Sync)) {
    let warmup = Duration::from_secs(30).saturating_sub(process_uptime());
    if !warmup.is_zero() && !sleep(stop, warmup) {
        return;
    }
    let mut logged = false;
    let mut seen: Option<HashSet<u32>> = None;
    while !stop.load(Ordering::Relaxed) {
        let listener = match UserNotificationListener::Current() {
            Ok(l) => l,
            Err(e) => {
                if !logged {
                    tracing::warn!("notificações indisponíveis: {e}");
                    logged = true;
                }
                if !sleep(stop, Duration::from_secs(3)) {
                    return;
                }
                continue;
            }
        };
        match listener.RequestAccessAsync().and_then(|op| op.get()) {
            Ok(UserNotificationListenerAccessStatus::Allowed) => {}
            other => {
                if !logged {
                    tracing::warn!(?other, "sem permissão para ler notificações (Configurações › Privacidade › Notificações)");
                    logged = true;
                }
                if !sleep(stop, Duration::from_secs(3)) {
                    return;
                }
                continue;
            }
        }
        while !stop.load(Ordering::Relaxed) {
            match poll(&listener, &mut seen) {
                Ok(fresh) if !fresh.is_empty() => {
                    if let Ok(mut q) = queue.lock() {
                        q.extend(fresh);
                    }
                    notify();
                }
                Ok(_) => {}
                Err(e) => {
                    tracing::debug!("leitura de notificações falhou: {e}; reconectando");
                    break;
                }
            }
            if !sleep(stop, Duration::from_secs(1)) {
                return;
            }
        }
    }
}

fn poll(listener: &UserNotificationListener, seen: &mut Option<HashSet<u32>>) -> Result<Vec<Notification>> {
    let list = listener.GetNotificationsAsync(NotificationKinds::Toast)?.get()?;
    let mut current = HashSet::new();
    let mut fresh = Vec::new();
    for n in &list {
        let id = n.Id()?;
        current.insert(id);
        // A primeira leitura só registra o que já estava na central.
        if seen.as_ref().is_some_and(|s| !s.contains(&id))
            && let Ok(parsed) = parse(&n)
        {
            fresh.push(parsed);
        }
    }
    *seen = Some(current);
    Ok(fresh)
}

fn parse(n: &UserNotification) -> Result<Notification> {
    let display = n.AppInfo()?.DisplayInfo()?;
    let app = display.DisplayName()?.to_string();
    let icon = display.GetLogo(Size { Width: 32.0, Height: 32.0 }).ok().and_then(|logo| logo.cast().ok()).and_then(|logo| crate::imaging::decode(&logo, 32, false).ok().flatten());
    let mut title = String::new();
    let mut body = String::new();
    if let Ok(binding) = n.Notification()?.Visual()?.GetBinding(&KnownNotificationBindings::ToastGeneric()?) {
        let texts = binding.GetTextElements()?;
        if texts.Size()? > 0 {
            title = texts.GetAt(0)?.Text()?.to_string();
        }
        if texts.Size()? > 1 {
            body = texts.GetAt(1)?.Text()?.to_string();
        }
    }
    Ok(Notification { text: summary(&app, &title, &body), app, icon })
}

/// "Título - corpo", até 120 caracteres (como a ilha C++).
pub fn summary(app: &str, title: &str, body: &str) -> String {
    let mut text = if title.is_empty() { if app.is_empty() { "Nova notificação".to_string() } else { app.to_string() } } else { title.to_string() };
    if !body.is_empty() {
        text = format!("{text} - {body}");
    }
    if text.chars().count() > 120 {
        text = text.chars().take(120).collect::<String>() + "…";
    }
    text
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn summary_joins_title_and_body_and_truncates() {
        assert_eq!(summary("WhatsApp", "Vida", "oi"), "Vida - oi");
        assert_eq!(summary("WhatsApp", "", ""), "WhatsApp");
        assert_eq!(summary("", "", ""), "Nova notificação");
        let long = summary("A", &"x".repeat(200), "");
        assert_eq!(long.chars().count(), 121);
        assert!(long.ends_with('…'));
    }
}
