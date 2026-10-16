//! Ciclo de vida do zefiro-island.exe (spec §7).

use std::path::PathBuf;
use std::sync::{Arc, Mutex, OnceLock, mpsc};
use std::time::{Duration, Instant};
use zefiro_island_host::ini::{IslandSettings, apply_ini, apply_state, settings_for};
use zefiro_island_host::policy::{Decision, RestartPolicy};
use zefiro_island_host::process::{IslandProcess, resolve_exe};
use zefiro_platform_windows::{system, window};

use crate::app::AppCtx;

enum Cmd { Restart, Stop }

/// Reescreve um arquivo .ini da ilha a partir do texto atual.
type IniWriter = fn(&str, &IslandSettings) -> String;

struct Host { tx: mpsc::Sender<Cmd>, thread: Option<std::thread::JoinHandle<()>> }

static HOST: OnceLock<Mutex<Option<Host>>> = OnceLock::new();

fn host() -> &'static Mutex<Option<Host>> { HOST.get_or_init(|| Mutex::new(None)) }

pub fn sync(ctx: &AppCtx) {
    if !ctx.config.island.external() {
        return;
    }
    let dir = ctx.config_path.parent().map(PathBuf::from).unwrap_or_default();
    let s = settings_for(&ctx.theme.theme, ctx.config.bar.height, ctx.config.bar.format);
    let writers: [(&str, IniWriter); 2] = [("island-state.ini", apply_state), ("island.ini", apply_ini)];
    for (name, apply) in writers {
        let path = dir.join(name);
        let old = std::fs::read_to_string(&path).unwrap_or_default();
        let new = apply(&old, &s);
        if new != old && let Err(e) = std::fs::write(&path, new) {
            tracing::warn!(path = %path.display(), "não consegui gravar: {e}");
        }
    }
    window::set_timer(crate::event_loop::TIMER_RAISE, 500);
}

pub fn start(ctx: &AppCtx) {
    if !ctx.config.island.external() {
        return;
    }
    let Some(exe) = resolve_exe(&ctx.config.island.exe, &system::exe_dir()) else {
        tracing::warn!("zefiro-island.exe não encontrado ao lado do zefiro.exe nem em [island] exe; centro vazio");
        return;
    };
    sync(ctx);
    let (tx, rx) = mpsc::channel::<Cmd>();
    let thread = std::thread::Builder::new().name("zefiro-island".into()).spawn(move || {
        let mut policy = RestartPolicy::new(3, Duration::from_secs(60));
        let proc: Arc<Mutex<Option<IslandProcess>>> = Arc::new(Mutex::new(IslandProcess::start(&exe).ok()));
        let mut gave_up = false;
        loop {
            match rx.recv_timeout(Duration::from_secs(1)) {
                Ok(Cmd::Stop) | Err(mpsc::RecvTimeoutError::Disconnected) => {
                    if let Some(mut p) = proc.lock().unwrap().take() {
                        p.stop(Duration::from_secs(3));
                    }
                    return;
                }
                Ok(Cmd::Restart) => {
                    policy.reset();
                    gave_up = false;
                    if let Some(mut p) = proc.lock().unwrap().take() {
                        p.stop(Duration::from_secs(3));
                    }
                    *proc.lock().unwrap() = IslandProcess::start(&exe).ok();
                    window::post_user(crate::event_loop::USER_ISLAND + 100);
                }
                Err(mpsc::RecvTimeoutError::Timeout) => {
                    let exited = proc.lock().unwrap().as_mut().is_some_and(|p| p.has_exited());
                    if exited && !gave_up {
                        match policy.on_exit(Instant::now()) {
                            Decision::Restart => {
                                tracing::warn!("a ilha saiu sozinha; reiniciando");
                                *proc.lock().unwrap() = IslandProcess::start(&exe).ok();
                                window::post_user(crate::event_loop::USER_ISLAND + 100);
                            }
                            Decision::GiveUp => {
                                tracing::error!("a ilha caiu 3 vezes em 60 s; centro vazio até \"Reiniciar a ilha\"");
                                gave_up = true;
                                *proc.lock().unwrap() = None;
                            }
                        }
                    }
                }
            }
        }
    });
    match thread {
        Ok(t) => *host().lock().unwrap() = Some(Host { tx, thread: Some(t) }),
        Err(e) => tracing::error!("não consegui criar a thread da ilha: {e}"),
    }
}

pub fn restart(ctx: &AppCtx) {
    let running = host().lock().unwrap().as_ref().map(|h| h.tx.send(Cmd::Restart).is_ok()).unwrap_or(false);
    if !running {
        start(ctx);
    }
}

pub fn stop() {
    if let Some(mut h) = host().lock().unwrap().take() {
        let _ = h.tx.send(Cmd::Stop);
        if let Some(t) = h.thread.take() {
            let _ = t.join();
        }
    }
}
