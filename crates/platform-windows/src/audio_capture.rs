//! Captura do som do sistema (WASAPI em loopback) para o visualizador da ilha. Fica ligada só
//! enquanto a ilha mostra o visualizador e há mídia tocando; reinicia sozinha quando a saída padrão
//! muda ou o dispositivo some.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};
use windows::Win32::Media::Audio::*;
use windows::Win32::System::Com::*;
use windows::core::Result;

/// Recebe áudio mono em float e a taxa de amostragem.
pub type Sink = Box<dyn FnMut(&[f32], f32) + Send>;

pub struct AudioCapture {
    stop: Arc<AtomicBool>,
    handle: Option<JoinHandle<()>>,
}

impl AudioCapture {
    pub fn start(mut sink: Sink) -> std::io::Result<AudioCapture> {
        let stop = Arc::new(AtomicBool::new(false));
        let flag = stop.clone();
        let handle = std::thread::Builder::new().name("zefiro-audio".into()).spawn(move || {
            // SAFETY: COM inicializado nesta thread e liberado no fim dela.
            let inited = unsafe { CoInitializeEx(None, COINIT_MULTITHREADED) }.is_ok();
            let mut failures = 0u32;
            while !flag.load(Ordering::Relaxed) {
                match run_once(&flag, &mut sink) {
                    Ok(()) => failures = 0,
                    Err(e) => {
                        failures += 1;
                        if failures == 1 {
                            tracing::warn!("captura de áudio falhou: {e}; tentando de novo");
                        }
                        // Espera crescente, interrompível pelo pedido de parar.
                        let wait = Duration::from_millis((250 * failures as u64).min(5_000));
                        let until = Instant::now() + wait;
                        while Instant::now() < until && !flag.load(Ordering::Relaxed) {
                            std::thread::sleep(Duration::from_millis(50));
                        }
                    }
                }
            }
            if inited {
                // SAFETY: par do CoInitializeEx.
                unsafe { CoUninitialize() };
            }
        })?;
        Ok(AudioCapture { stop, handle: Some(handle) })
    }
}

impl Drop for AudioCapture {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        if let Some(h) = self.handle.take() {
            let _ = h.join();
        }
    }
}

fn default_device_id(e: &IMMDeviceEnumerator) -> Result<String> {
    // SAFETY: COM válido; a string é liberada com CoTaskMemFree.
    unsafe {
        let id = e.GetDefaultAudioEndpoint(eRender, eConsole)?.GetId()?;
        let s = id.to_string().unwrap_or_default();
        CoTaskMemFree(Some(id.0 as _));
        Ok(s)
    }
}

/// Captura até a saída padrão mudar, o dispositivo ficar inválido ou pedirem para parar.
fn run_once(stop: &AtomicBool, sink: &mut Sink) -> Result<()> {
    // SAFETY: sequência padrão de captura em loopback; cada GetBuffer tem seu ReleaseBuffer, e o
    // formato devolvido por GetMixFormat é liberado com CoTaskMemFree.
    unsafe {
        let enumerator: IMMDeviceEnumerator = CoCreateInstance(&MMDeviceEnumerator, None, CLSCTX_ALL)?;
        let device_id = default_device_id(&enumerator)?;
        let device = enumerator.GetDefaultAudioEndpoint(eRender, eConsole)?;
        let client: IAudioClient = device.Activate(CLSCTX_ALL, None)?;
        let fmt = client.GetMixFormat()?;
        let channels = (*fmt).nChannels.max(1) as usize;
        let rate = (*fmt).nSamplesPerSec as f32;
        let bits = (*fmt).wBitsPerSample;
        // O formato compartilhado do Windows é float de 32 bits; outro formato fica sem visualizador.
        if bits != 32 {
            CoTaskMemFree(Some(fmt as _));
            return Err(windows::core::Error::new(windows::Win32::Foundation::E_NOTIMPL, "formato de mixagem diferente de float32"));
        }
        let init = client.Initialize(AUDCLNT_SHAREMODE_SHARED, AUDCLNT_STREAMFLAGS_LOOPBACK, 200_000, 0, fmt, None);
        CoTaskMemFree(Some(fmt as _));
        init?;
        let capture: IAudioCaptureClient = client.GetService()?;
        client.Start()?;
        let mut mono = Vec::with_capacity(4096);
        let mut last_check = Instant::now();
        let result = loop {
            if stop.load(Ordering::Relaxed) {
                break Ok(());
            }
            std::thread::sleep(Duration::from_millis(10));
            if last_check.elapsed() > Duration::from_secs(2) {
                last_check = Instant::now();
                if default_device_id(&enumerator).ok().as_deref() != Some(device_id.as_str()) {
                    break Ok(());
                }
            }
            let mut packet = match capture.GetNextPacketSize() {
                Ok(p) => p,
                Err(e) => break Err(e),
            };
            while packet > 0 {
                let mut data = std::ptr::null_mut();
                let mut frames = 0u32;
                let mut flags = 0u32;
                capture.GetBuffer(&mut data, &mut frames, &mut flags, None, None)?;
                mono.clear();
                if flags & AUDCLNT_BUFFERFLAGS_SILENT.0 as u32 != 0 || data.is_null() {
                    mono.resize(frames as usize, 0.0);
                } else {
                    let samples = std::slice::from_raw_parts(data as *const f32, frames as usize * channels);
                    mono.extend(samples.chunks(channels).map(|c| c.iter().sum::<f32>() / channels as f32));
                }
                capture.ReleaseBuffer(frames)?;
                sink(&mono, rate);
                packet = capture.GetNextPacketSize()?;
            }
        };
        let _ = client.Stop();
        result
    }
}
