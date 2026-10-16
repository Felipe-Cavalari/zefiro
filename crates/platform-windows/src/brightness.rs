//! Brilho dos monitores externos por DDC/CI (`dxva2`: VCP 0x10). As chamadas levam dezenas de ms,
//! então tudo roda numa thread própria: a barra pede um valor (a interface já mostra ele), e a thread
//! grava só o último pedido de cada monitor, uma escrita por vez (`Coalescer`).

use std::collections::HashMap;
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use windows::Win32::Devices::Display::{DestroyPhysicalMonitors, GetNumberOfPhysicalMonitorsFromHMONITOR, GetPhysicalMonitorsFromHMONITOR, GetVCPFeatureAndVCPFeatureReply, PHYSICAL_MONITOR, SetVCPFeature};
use windows::Win32::Foundation::{LPARAM, RECT};
use windows::Win32::Graphics::Gdi::{EnumDisplayMonitors, GetMonitorInfoW, HDC, HMONITOR, MONITORINFOEXW};
use windows::core::BOOL;
use zefiro_integration_devices::brightness::{Coalescer, MonitorBrightness};
use zefiro_integration_devices::level::{from_percent, to_percent};

const VCP_BRIGHTNESS: u8 = 0x10;
/// Folga entre escritas no mesmo monitor (alguns monitores perdem comandos seguidos).
const MIN_INTERVAL: Duration = Duration::from_millis(50);

enum Job { Refresh, Set(String, u32), Stop }

struct Physical { device: String, name: String, handle: PHYSICAL_MONITOR, max: u32, coalescer: Coalescer }

pub struct BrightnessService {
    tx: Sender<Job>,
    state: Arc<Mutex<Vec<MonitorBrightness>>>,
    thread: Option<std::thread::JoinHandle<()>>,
}

impl BrightnessService {
    /// `notify` é chamado (em outra thread) quando a leitura dos monitores muda.
    pub fn start(notify: Arc<dyn Fn() + Send + Sync>) -> std::io::Result<BrightnessService> {
        let (tx, rx) = mpsc::channel();
        let state = Arc::new(Mutex::new(Vec::new()));
        let shared = state.clone();
        let thread = std::thread::Builder::new().name("zefiro-brightness".into()).spawn(move || run(&rx, &shared, notify.as_ref()))?;
        let _ = tx.send(Job::Refresh);
        Ok(BrightnessService { tx, state, thread: Some(thread) })
    }

    pub fn monitors(&self) -> Vec<MonitorBrightness> {
        self.state.lock().map(|s| s.clone()).unwrap_or_default()
    }

    /// Pede um brilho (0–100 %). A leitura publicada já muda; a escrita sai quando o monitor deixar.
    pub fn set(&self, device: &str, percent: u32) {
        if let Ok(mut s) = self.state.lock()
            && let Some(m) = s.iter_mut().find(|m| m.device == device && m.percent.is_some())
        {
            m.percent = Some(percent.min(100));
        }
        let _ = self.tx.send(Job::Set(device.to_string(), percent.min(100)));
    }

    /// Monitores mudaram (conectar, desconectar, voltar da suspensão): reabre os handles e relê.
    pub fn refresh(&self) {
        let _ = self.tx.send(Job::Refresh);
    }
}

impl Drop for BrightnessService {
    fn drop(&mut self) {
        let _ = self.tx.send(Job::Stop);
        if let Some(t) = self.thread.take() {
            let _ = t.join();
        }
    }
}

extern "system" fn collect(monitor: HMONITOR, _: HDC, _: *mut RECT, lp: LPARAM) -> BOOL {
    // SAFETY: `lp` aponta para o Vec vivo durante o EnumDisplayMonitors abaixo.
    let out = unsafe { &mut *(lp.0 as *mut Vec<HMONITOR>) };
    out.push(monitor);
    BOOL(1)
}

fn decode(text: &[u16]) -> String {
    String::from_utf16_lossy(&text[..text.iter().position(|c| *c == 0).unwrap_or(text.len())])
}

fn open_all() -> Vec<Physical> {
    let mut monitors: Vec<HMONITOR> = Vec::new();
    let mut out = Vec::new();
    // SAFETY: enumeração com callback local; cada HMONITOR é só consultado; os handles físicos são
    // destruídos em `close_all`.
    unsafe {
        let _ = EnumDisplayMonitors(None, None, Some(collect), LPARAM(&mut monitors as *mut _ as isize));
        for hm in monitors {
            let mut info = MONITORINFOEXW::default();
            info.monitorInfo.cbSize = size_of::<MONITORINFOEXW>() as u32;
            if !GetMonitorInfoW(hm, &mut info as *mut MONITORINFOEXW as *mut _).as_bool() {
                continue;
            }
            let device = decode(&info.szDevice);
            let mut count = 0u32;
            if GetNumberOfPhysicalMonitorsFromHMONITOR(hm, &mut count).is_err() || count == 0 {
                continue;
            }
            let mut physical = vec![PHYSICAL_MONITOR::default(); count as usize];
            if GetPhysicalMonitorsFromHMONITOR(hm, &mut physical).is_err() {
                continue;
            }
            // Um monitor lógico com vários físicos (espelhado): o primeiro manda no brilho.
            let first = physical[0];
            if physical.len() > 1 {
                let _ = DestroyPhysicalMonitors(&physical[1..]);
            }
            // Estrutura compactada: copia o campo antes de ler.
            let description = first.szPhysicalMonitorDescription;
            out.push(Physical { device, name: decode(&description), handle: first, max: 0, coalescer: Coalescer::new(MIN_INTERVAL) });
        }
    }
    out
}

fn close_all(list: &mut Vec<Physical>) {
    for p in list.drain(..) {
        // SAFETY: handle aberto em `open_all`, destruído uma vez.
        let _ = unsafe { DestroyPhysicalMonitors(&[p.handle]) };
    }
}

/// Lê o brilho (e o máximo) de um monitor. `None` = não respondeu ao DDC/CI.
fn read(p: &mut Physical) -> Option<u32> {
    let (mut cur, mut max) = (0u32, 0u32);
    // SAFETY: handle físico aberto; saídas locais.
    let ok = unsafe { GetVCPFeatureAndVCPFeatureReply(p.handle.hPhysicalMonitor, VCP_BRIGHTNESS, None, &mut cur, Some(&mut max)) } != 0;
    (ok && max > 0).then(|| {
        p.max = max;
        to_percent(cur, max)
    })
}

fn publish(list: &[Physical], readings: &HashMap<String, Option<u32>>, state: &Mutex<Vec<MonitorBrightness>>, notify: &(dyn Fn() + Send + Sync)) {
    let snapshot: Vec<MonitorBrightness> = list.iter().map(|p| MonitorBrightness { device: p.device.clone(), name: p.name.clone(), percent: readings.get(&p.device).copied().flatten() }).collect();
    let changed = state.lock().map(|mut s| std::mem::replace(&mut *s, snapshot.clone()) != snapshot).unwrap_or(false);
    if changed {
        notify();
    }
}

fn run(rx: &Receiver<Job>, state: &Mutex<Vec<MonitorBrightness>>, notify: &(dyn Fn() + Send + Sync)) {
    let mut list: Vec<Physical> = Vec::new();
    let mut readings: HashMap<String, Option<u32>> = HashMap::new();
    let mut wait = None;
    loop {
        let job = match wait {
            Some(d) => rx.recv_timeout(d),
            None => rx.recv().map_err(|_| RecvTimeoutError::Disconnected),
        };
        match job {
            Ok(Job::Stop) | Err(RecvTimeoutError::Disconnected) => break,
            Ok(Job::Refresh) => {
                close_all(&mut list);
                list = open_all();
                readings = list.iter_mut().map(|p| (p.device.clone(), read(p))).collect();
                publish(&list, &readings, state, notify);
            }
            Ok(Job::Set(device, percent)) => {
                if let Some(p) = list.iter_mut().find(|p| p.device == device) {
                    p.coalescer.request(percent);
                    readings.insert(device, Some(percent));
                }
            }
            Err(RecvTimeoutError::Timeout) => {}
        }
        // Escreve o que estiver pendente e livre; calcula quanto esperar pelo resto.
        let now = Instant::now();
        wait = None;
        for p in &mut list {
            if let Some(percent) = p.coalescer.next_write(now) {
                let value = from_percent(percent, if p.max == 0 { 100 } else { p.max });
                // SAFETY: handle físico aberto.
                let ok = unsafe { SetVCPFeature(p.handle.hPhysicalMonitor, VCP_BRIGHTNESS, value) } != 0;
                if !ok {
                    tracing::debug!(monitor = %p.name, "o monitor recusou o brilho por DDC/CI");
                }
                p.coalescer.complete(Instant::now());
            }
            if let Some(d) = p.coalescer.wait(Instant::now()) {
                wait = Some(wait.map_or(d, |w: Duration| w.min(d)));
            }
        }
    }
    close_all(&mut list);
}

/// Leitura direta, sem thread (sondas e diagnóstico): nome, porcentagem e o tempo da leitura.
pub fn probe() -> Vec<(String, String, Option<u32>, Duration)> {
    let mut list = open_all();
    let out = list
        .iter_mut()
        .map(|p| {
            let t = Instant::now();
            let v = read(p);
            (p.device.clone(), p.name.clone(), v, t.elapsed())
        })
        .collect();
    close_all(&mut list);
    out
}
