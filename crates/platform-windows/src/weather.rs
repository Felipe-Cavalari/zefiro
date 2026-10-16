//! Clima em segundo plano: consulta o wttr.in a cada 15 min; em falha tenta de novo em 1, 2, 5 e
//! 15 min (o serviço é comunitário e limita o uso), como a ilha C++.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use windows::Win32::System::WinRT::{RO_INIT_MULTITHREADED, RoInitialize, RoUninitialize};
use zefiro_island::weather::{Weather, parse, url};

pub struct WeatherService {
    stop: Arc<AtomicBool>,
    refresh: Arc<AtomicBool>,
    state: Arc<Mutex<Option<Weather>>>,
    thread: Option<std::thread::JoinHandle<()>>,
}

const OK_WAIT: Duration = Duration::from_secs(15 * 60);
const BACKOFF: [Duration; 4] = [Duration::from_secs(60), Duration::from_secs(120), Duration::from_secs(300), Duration::from_secs(900)];

impl WeatherService {
    pub fn start(city: String, notify: Arc<dyn Fn() + Send + Sync>) -> std::io::Result<WeatherService> {
        let stop = Arc::new(AtomicBool::new(false));
        let refresh = Arc::new(AtomicBool::new(false));
        let state = Arc::new(Mutex::new(None));
        let (s, r, shared) = (stop.clone(), refresh.clone(), state.clone());
        let thread = std::thread::Builder::new().name("zefiro-weather".into()).spawn(move || {
            // SAFETY: WinRT nesta thread (HttpClient), desfeito no fim.
            let inited = unsafe { RoInitialize(RO_INIT_MULTITHREADED) }.is_ok();
            let mut failures = 0usize;
            // Espera 3 s antes da primeira consulta, como a ilha C++ (a rede pode estar subindo).
            let mut wait = Duration::from_secs(3);
            loop {
                let until = Instant::now() + wait;
                while Instant::now() < until && !s.load(Ordering::Relaxed) && !r.swap(false, Ordering::Relaxed) {
                    std::thread::sleep(Duration::from_millis(250));
                }
                if s.load(Ordering::Relaxed) {
                    break;
                }
                let reading = crate::http::get_string(&url(&city)).map_err(|e| e.to_string()).and_then(|json| parse(&json, &city).ok_or_else(|| "resposta sem leitura".to_string()));
                match reading {
                    Ok(w) => {
                        failures = 0;
                        wait = OK_WAIT;
                        let changed = shared.lock().map(|mut st| st.replace(w.clone()) != Some(w)).unwrap_or(false);
                        if changed {
                            notify();
                        }
                    }
                    Err(e) => {
                        tracing::debug!("clima indisponível: {e}");
                        wait = BACKOFF[failures.min(BACKOFF.len() - 1)];
                        failures += 1;
                    }
                }
            }
            if inited {
                // SAFETY: par do RoInitialize.
                unsafe { RoUninitialize() };
            }
        })?;
        Ok(WeatherService { stop, refresh, state, thread: Some(thread) })
    }

    pub fn weather(&self) -> Option<Weather> {
        self.state.lock().ok().and_then(|s| s.clone())
    }

    /// Consulta já (volta da suspensão, cidade mudou).
    pub fn refresh(&self) {
        self.refresh.store(true, Ordering::Relaxed);
    }
}

impl Drop for WeatherService {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        if let Some(t) = self.thread.take() {
            let _ = t.join();
        }
    }
}
