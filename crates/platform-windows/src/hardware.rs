//! Monitor de hardware da ilha: CPU (`GetSystemTimes`), RAM, disco C:, GPU e rede (PDH). Só roda
//! enquanto a aba está na tela: enumerar os motores da GPU de todos os processos é caro.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;
use windows::Win32::Foundation::FILETIME;
use windows::Win32::Storage::FileSystem::GetDiskFreeSpaceExW;
use windows::Win32::System::Performance::{
    PDH_FMT_COUNTERVALUE_ITEM_W, PDH_FMT_DOUBLE, PDH_HCOUNTER, PDH_HQUERY, PdhAddEnglishCounterW, PdhCloseQuery, PdhCollectQueryData, PdhGetFormattedCounterArrayW, PdhOpenQueryW,
};
use windows::Win32::System::SystemInformation::{GlobalMemoryStatusEx, MEMORYSTATUSEX};
use windows::Win32::System::Threading::GetSystemTimes;
use windows::core::{HSTRING, PCWSTR, w};
use zefiro_island::tabs::Hardware;

const PDH_MORE_DATA: u32 = 0x8000_07D2;

struct Pdh { query: PDH_HQUERY, gpu: Option<PDH_HCOUNTER>, down: Option<PDH_HCOUNTER>, up: Option<PDH_HCOUNTER> }

impl Pdh {
    fn open() -> Option<Pdh> {
        let mut query = PDH_HQUERY::default();
        // SAFETY: consulta PDH local, fechada no Drop.
        if unsafe { PdhOpenQueryW(PCWSTR::null(), 0, &mut query) } != 0 {
            return None;
        }
        let add = |path: &str| {
            let mut c = PDH_HCOUNTER::default();
            // SAFETY: contador adicionado à consulta aberta acima.
            (unsafe { PdhAddEnglishCounterW(query, &HSTRING::from(path), 0, &mut c) } == 0).then_some(c)
        };
        let pdh = Pdh {
            query,
            gpu: add(r"\GPU Engine(*engtype_3D)\Utilization Percentage"),
            down: add(r"\Network Interface(*)\Bytes Received/sec"),
            up: add(r"\Network Interface(*)\Bytes Sent/sec"),
        };
        // A primeira coleta só prepara as taxas.
        // SAFETY: consulta válida.
        unsafe { PdhCollectQueryData(query) };
        Some(pdh)
    }

    /// Soma das instâncias de um contador com curinga.
    fn sum(&self, counter: Option<PDH_HCOUNTER>) -> Option<f64> {
        let counter = counter?;
        let (mut size, mut count) = (0u32, 0u32);
        // SAFETY: primeiro pede o tamanho, depois lê num buffer alinhado de itens do tamanho pedido.
        unsafe {
            if PdhGetFormattedCounterArrayW(counter, PDH_FMT_DOUBLE, &mut size, &mut count, None) != PDH_MORE_DATA {
                return None;
            }
            let items = (size as usize).div_ceil(size_of::<PDH_FMT_COUNTERVALUE_ITEM_W>()).max(1);
            let mut buf = vec![PDH_FMT_COUNTERVALUE_ITEM_W::default(); items];
            if PdhGetFormattedCounterArrayW(counter, PDH_FMT_DOUBLE, &mut size, &mut count, Some(buf.as_mut_ptr())) != 0 {
                return None;
            }
            Some(buf[..count as usize].iter().map(|i| i.FmtValue.Anonymous.doubleValue).filter(|v| v.is_finite()).sum())
        }
    }
}

impl Drop for Pdh {
    fn drop(&mut self) {
        // SAFETY: fecha a consulta aberta em `open`.
        unsafe { PdhCloseQuery(self.query) };
    }
}

fn ticks(f: FILETIME) -> u64 {
    ((f.dwHighDateTime as u64) << 32) | f.dwLowDateTime as u64
}

fn cpu_times() -> Option<(u64, u64)> {
    let (mut idle, mut kernel, mut user) = (FILETIME::default(), FILETIME::default(), FILETIME::default());
    // SAFETY: estruturas locais.
    unsafe { GetSystemTimes(Some(&mut idle), Some(&mut kernel), Some(&mut user)) }.ok()?;
    // O tempo de kernel inclui o ocioso.
    Some((ticks(idle), ticks(kernel) + ticks(user)))
}

fn sample(prev: &mut Option<(u64, u64)>, pdh: Option<&Pdh>) -> Hardware {
    let mut h = Hardware::default();
    if let (Some((idle0, total0)), Some((idle1, total1))) = (*prev, cpu_times()) {
        let total = total1.saturating_sub(total0);
        if total > 0 {
            h.cpu = (100.0 * (1.0 - idle1.saturating_sub(idle0) as f64 / total as f64)).clamp(0.0, 100.0) as f32;
        }
    }
    *prev = cpu_times();
    let mut mem = MEMORYSTATUSEX { dwLength: size_of::<MEMORYSTATUSEX>() as u32, ..Default::default() };
    // SAFETY: estrutura com dwLength preenchido.
    if unsafe { GlobalMemoryStatusEx(&mut mem) }.is_ok() {
        h.ram = mem.dwMemoryLoad as f32;
        h.ram_total_gb = mem.ullTotalPhys as f32 / 1_073_741_824.0;
        h.ram_used_gb = (mem.ullTotalPhys - mem.ullAvailPhys) as f32 / 1_073_741_824.0;
    }
    let (mut free, mut total) = (0u64, 0u64);
    // SAFETY: saídas locais.
    if unsafe { GetDiskFreeSpaceExW(w!("C:\\"), Some(&mut free), Some(&mut total), None) }.is_ok() && total > 0 {
        h.disk = 100.0 * (1.0 - free as f32 / total as f32);
        h.disk_free_gb = free as f32 / 1_073_741_824.0;
    }
    if let Some(p) = pdh {
        // SAFETY: consulta viva.
        unsafe { PdhCollectQueryData(p.query) };
        h.gpu = p.sum(p.gpu).map(|v| v.clamp(0.0, 100.0) as f32);
        h.net_down_bps = p.sum(p.down).unwrap_or(0.0);
        h.net_up_bps = p.sum(p.up).unwrap_or(0.0);
    }
    h
}

pub struct HardwareMonitor {
    stop: Arc<AtomicBool>,
    state: Arc<Mutex<Option<Hardware>>>,
    thread: Option<std::thread::JoinHandle<()>>,
}

impl HardwareMonitor {
    /// Amostra a cada segundo até ser descartado.
    pub fn start(notify: Arc<dyn Fn() + Send + Sync>) -> std::io::Result<HardwareMonitor> {
        let stop = Arc::new(AtomicBool::new(false));
        let state = Arc::new(Mutex::new(None));
        let (s, shared) = (stop.clone(), state.clone());
        let thread = std::thread::Builder::new().name("zefiro-hardware".into()).spawn(move || {
            let pdh = Pdh::open();
            let mut prev = cpu_times();
            while !s.load(Ordering::Relaxed) {
                for _ in 0..10 {
                    std::thread::sleep(Duration::from_millis(100));
                    if s.load(Ordering::Relaxed) {
                        return;
                    }
                }
                let h = sample(&mut prev, pdh.as_ref());
                if let Ok(mut st) = shared.lock() {
                    *st = Some(h);
                }
                notify();
            }
        })?;
        Ok(HardwareMonitor { stop, state, thread: Some(thread) })
    }

    pub fn latest(&self) -> Option<Hardware> {
        self.state.lock().ok().and_then(|s| *s)
    }
}

impl Drop for HardwareMonitor {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        if let Some(t) = self.thread.take() {
            let _ = t.join();
        }
    }
}
