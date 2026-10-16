//! Volume da saída padrão, por evento (`IAudioEndpointVolumeCallback`), sem consulta periódica.
//! Troca da saída padrão (`IMMNotificationClient`) religa o aviso no dispositivo novo.

use std::sync::mpsc::{self, Sender};
use std::sync::{Arc, Mutex};
use windows::Win32::Media::Audio::Endpoints::{IAudioEndpointVolume, IAudioEndpointVolumeCallback, IAudioEndpointVolumeCallback_Impl};
use windows::Win32::Media::Audio::{
    AUDIO_VOLUME_NOTIFICATION_DATA, DEVICE_STATE, EDataFlow, ERole, IMMDeviceEnumerator, IMMNotificationClient, IMMNotificationClient_Impl, MMDeviceEnumerator, eConsole,
    eRender,
};
use windows::Win32::System::Com::{CLSCTX_ALL, COINIT_MULTITHREADED, CoCreateInstance, CoInitializeEx, CoUninitialize};
use windows::Win32::Foundation::PROPERTYKEY;
use windows::core::{PCWSTR, Result, implement};

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct VolumeState { pub level: f32, pub muted: bool }

enum Msg { Changed(VolumeState), DefaultChanged, Stop }

#[implement(IAudioEndpointVolumeCallback)]
struct VolumeCallback { tx: Sender<Msg> }

impl IAudioEndpointVolumeCallback_Impl for VolumeCallback_Impl {
    fn OnNotify(&self, data: *mut AUDIO_VOLUME_NOTIFICATION_DATA) -> Result<()> {
        if !data.is_null() {
            // SAFETY: o Windows passa uma estrutura válida durante a chamada.
            let d = unsafe { &*data };
            let _ = self.tx.send(Msg::Changed(VolumeState { level: d.fMasterVolume, muted: d.bMuted.as_bool() }));
        }
        Ok(())
    }
}

#[implement(IMMNotificationClient)]
struct DeviceCallback { tx: Sender<Msg> }

impl IMMNotificationClient_Impl for DeviceCallback_Impl {
    fn OnDeviceStateChanged(&self, _: &PCWSTR, _: DEVICE_STATE) -> Result<()> {
        Ok(())
    }
    fn OnDeviceAdded(&self, _: &PCWSTR) -> Result<()> {
        Ok(())
    }
    fn OnDeviceRemoved(&self, _: &PCWSTR) -> Result<()> {
        Ok(())
    }
    fn OnDefaultDeviceChanged(&self, flow: EDataFlow, role: ERole, _: &PCWSTR) -> Result<()> {
        if flow == eRender && role == eConsole {
            let _ = self.tx.send(Msg::DefaultChanged);
        }
        Ok(())
    }
    fn OnPropertyValueChanged(&self, _: &PCWSTR, _: &PROPERTYKEY) -> Result<()> {
        Ok(())
    }
}

pub struct VolumeWatcher {
    tx: Sender<Msg>,
    state: Arc<Mutex<Option<VolumeState>>>,
    thread: Option<std::thread::JoinHandle<()>>,
}

impl VolumeWatcher {
    /// `notify` é chamado (em outra thread) a cada mudança de volume ou mudo feita por qualquer app.
    pub fn start(notify: Arc<dyn Fn() + Send + Sync>) -> std::io::Result<VolumeWatcher> {
        let (tx, rx) = mpsc::channel();
        let state = Arc::new(Mutex::new(None));
        let (worker_tx, worker_state) = (tx.clone(), state.clone());
        let thread = std::thread::Builder::new().name("zefiro-volume".into()).spawn(move || {
            // SAFETY: COM nesta thread, liberado no fim.
            let inited = unsafe { CoInitializeEx(None, COINIT_MULTITHREADED) }.is_ok();
            if let Err(e) = run(&rx, &worker_tx, &worker_state, notify.as_ref()) {
                tracing::warn!("volume do Windows indisponível: {e}");
            }
            if inited {
                // SAFETY: par do CoInitializeEx.
                unsafe { CoUninitialize() };
            }
        })?;
        Ok(VolumeWatcher { tx, state, thread: Some(thread) })
    }

    pub fn state(&self) -> Option<VolumeState> {
        self.state.lock().ok().and_then(|s| *s)
    }
}

impl Drop for VolumeWatcher {
    fn drop(&mut self) {
        let _ = self.tx.send(Msg::Stop);
        if let Some(t) = self.thread.take() {
            let _ = t.join();
        }
    }
}

struct Bound { volume: IAudioEndpointVolume, callback: IAudioEndpointVolumeCallback }

fn bind(enumerator: &IMMDeviceEnumerator, tx: &Sender<Msg>) -> Result<(Bound, VolumeState)> {
    // SAFETY: interfaces COM válidas nesta thread; o callback vive em `Bound` até ser removido.
    unsafe {
        let device = enumerator.GetDefaultAudioEndpoint(eRender, eConsole)?;
        let volume: IAudioEndpointVolume = device.Activate(CLSCTX_ALL, None)?;
        let callback: IAudioEndpointVolumeCallback = VolumeCallback { tx: tx.clone() }.into();
        volume.RegisterControlChangeNotify(&callback)?;
        let state = VolumeState { level: volume.GetMasterVolumeLevelScalar()?, muted: volume.GetMute()?.as_bool() };
        Ok((Bound { volume, callback }, state))
    }
}

fn unbind(b: &Bound) {
    // SAFETY: remove o mesmo callback registrado em `bind`.
    let _ = unsafe { b.volume.UnregisterControlChangeNotify(&b.callback) };
}

fn run(rx: &mpsc::Receiver<Msg>, tx: &Sender<Msg>, state: &Mutex<Option<VolumeState>>, notify: &(dyn Fn() + Send + Sync)) -> Result<()> {
    // SAFETY: enumerador COM criado nesta thread; o cliente de notificação é removido antes de sair.
    let enumerator: IMMDeviceEnumerator = unsafe { CoCreateInstance(&MMDeviceEnumerator, None, CLSCTX_ALL)? };
    let devices: IMMNotificationClient = DeviceCallback { tx: tx.clone() }.into();
    // SAFETY: idem.
    unsafe { enumerator.RegisterEndpointNotificationCallback(&devices)? };
    let mut bound = None;
    let rebind = |bound: &mut Option<Bound>| {
        if let Some(b) = bound.take() {
            unbind(&b);
        }
        match bind(&enumerator, tx) {
            Ok((b, s)) => {
                *bound = Some(b);
                // O valor inicial (ou do dispositivo novo) não é uma mudança feita pelo usuário.
                if let Ok(mut st) = state.lock() {
                    *st = Some(s);
                }
            }
            Err(e) => tracing::debug!("sem saída de áudio padrão: {e}"),
        }
    };
    rebind(&mut bound);
    while let Ok(msg) = rx.recv() {
        match msg {
            Msg::Changed(s) => {
                let changed = state.lock().map(|mut st| st.replace(s) != Some(s)).unwrap_or(false);
                if changed {
                    notify();
                }
            }
            Msg::DefaultChanged => rebind(&mut bound),
            Msg::Stop => break,
        }
    }
    if let Some(b) = bound {
        unbind(&b);
    }
    // SAFETY: remove o cliente registrado acima.
    let _ = unsafe { enumerator.UnregisterEndpointNotificationCallback(&devices) };
    Ok(())
}
