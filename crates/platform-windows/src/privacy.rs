//! Câmera e microfone em uso (pontos de privacidade da ilha, §9.1 das notas de porte).
//!
//! O Windows registra o uso em `...\CapabilityAccessManager\ConsentStore\{microphone|webcam}`
//! (HKCU e HKLM, inclusive `NonPackaged`): em uso quando `LastUsedTimeStart ≠ 0` e
//! `LastUsedTimeStop == 0`. A ilha C++ consultava a cada 2 s; aqui uma thread espera
//! `RegNotifyChangeKeyValue` nas quatro chaves e só relê quando algo muda.

use std::sync::{Arc, Mutex};
use windows::Win32::Foundation::{CloseHandle, HANDLE, WAIT_OBJECT_0};
use windows::Win32::System::Registry::{
    HKEY, HKEY_CURRENT_USER, HKEY_LOCAL_MACHINE, KEY_NOTIFY, KEY_READ, REG_NOTIFY_CHANGE_LAST_SET, REG_NOTIFY_CHANGE_NAME, RRF_RT_REG_QWORD, RegCloseKey, RegEnumKeyExW,
    RegGetValueW, RegNotifyChangeKeyValue, RegOpenKeyExW,
};
use windows::Win32::System::Threading::{CreateEventW, INFINITE, SetEvent, WaitForMultipleObjects};
use windows::core::{HSTRING, PWSTR};

const STORE: &str = r"SOFTWARE\Microsoft\Windows\CurrentVersion\CapabilityAccessManager\ConsentStore";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct PrivacyState { pub mic: bool, pub cam: bool }

pub struct PrivacyWatcher {
    stop: HANDLE,
    state: Arc<Mutex<PrivacyState>>,
    thread: Option<std::thread::JoinHandle<()>>,
}

// SAFETY: o handle do evento de parada é só sinalizado de outra thread (SetEvent é seguro entre threads).
unsafe impl Send for PrivacyWatcher {}

fn qword(root: HKEY, path: &str, value: &str) -> Option<u64> {
    let mut data = 0u64;
    let mut size = 8u32;
    // SAFETY: buffer de 8 bytes para um REG_QWORD.
    let r = unsafe { RegGetValueW(root, &HSTRING::from(path), &HSTRING::from(value), RRF_RT_REG_QWORD, None, Some(&mut data as *mut u64 as *mut _), Some(&mut size)) };
    r.is_ok().then_some(data)
}

fn subkeys(root: HKEY, path: &str) -> Vec<String> {
    let mut out = Vec::new();
    // SAFETY: chave aberta só para leitura e fechada no fim; buffer local para cada nome.
    unsafe {
        let mut key = HKEY::default();
        if RegOpenKeyExW(root, &HSTRING::from(path), None, KEY_READ, &mut key).is_err() {
            return out;
        }
        let mut i = 0;
        loop {
            let mut name = [0u16; 512];
            let mut len = name.len() as u32;
            if RegEnumKeyExW(key, i, Some(PWSTR(name.as_mut_ptr())), &mut len, None, None, None, None).is_err() {
                break;
            }
            out.push(String::from_utf16_lossy(&name[..len as usize]));
            i += 1;
        }
        let _ = RegCloseKey(key);
    }
    out
}

fn in_use(root: HKEY, path: &str) -> bool {
    let started = qword(root, path, "LastUsedTimeStart").unwrap_or(0);
    let stopped = qword(root, path, "LastUsedTimeStop").unwrap_or(1);
    started != 0 && stopped == 0
}

/// Algum app (empacotado ou não) usando a capacidade agora.
fn capability_in_use(capability: &str) -> bool {
    let base = format!(r"{STORE}\{capability}");
    for root in [HKEY_CURRENT_USER, HKEY_LOCAL_MACHINE] {
        for app in subkeys(root, &base) {
            if app.eq_ignore_ascii_case("NonPackaged") {
                let np = format!(r"{base}\NonPackaged");
                if subkeys(root, &np).iter().any(|exe| in_use(root, &format!(r"{np}\{exe}"))) {
                    return true;
                }
            } else if in_use(root, &format!(r"{base}\{app}")) {
                return true;
            }
        }
    }
    false
}

pub fn read() -> PrivacyState {
    PrivacyState { mic: capability_in_use("microphone"), cam: capability_in_use("webcam") }
}

impl PrivacyWatcher {
    /// `notify` é chamado (em outra thread) quando câmera ou microfone passam a ser usados ou param.
    pub fn start(notify: Arc<dyn Fn() + Send + Sync>) -> windows::core::Result<PrivacyWatcher> {
        // SAFETY: evento manual sem nome; fechado no Drop.
        let stop = unsafe { CreateEventW(None, true, false, None)? };
        let state = Arc::new(Mutex::new(read()));
        let shared = state.clone();
        let raw_stop = stop.0 as isize;
        let thread = std::thread::Builder::new()
            .name("zefiro-privacy".into())
            .spawn(move || watch(HANDLE(raw_stop as *mut _), &shared, notify.as_ref()))
            .map_err(|e| windows::core::Error::new(windows::Win32::Foundation::E_FAIL, e.to_string()))?;
        Ok(PrivacyWatcher { stop, state, thread: Some(thread) })
    }

    pub fn state(&self) -> PrivacyState {
        self.state.lock().map(|s| *s).unwrap_or_default()
    }
}

impl Drop for PrivacyWatcher {
    fn drop(&mut self) {
        // SAFETY: sinaliza o evento de parada que a thread espera; fechado depois do join.
        unsafe {
            let _ = SetEvent(self.stop);
        }
        if let Some(t) = self.thread.take() {
            let _ = t.join();
        }
        // SAFETY: handle nosso, ninguém mais o usa depois do join.
        unsafe {
            let _ = CloseHandle(self.stop);
        }
    }
}

fn watch(stop: HANDLE, state: &Mutex<PrivacyState>, notify: &(dyn Fn() + Send + Sync)) {
    let mut keys = Vec::new();
    let mut events = vec![stop];
    for root in [HKEY_CURRENT_USER, HKEY_LOCAL_MACHINE] {
        for capability in ["microphone", "webcam"] {
            // SAFETY: chave aberta só para notificação e evento próprio, ambos fechados no fim.
            unsafe {
                let mut key = HKEY::default();
                if RegOpenKeyExW(root, &HSTRING::from(format!(r"{STORE}\{capability}")), None, KEY_NOTIFY | KEY_READ, &mut key).is_err() {
                    continue;
                }
                let Ok(event) = CreateEventW(None, false, false, None) else {
                    let _ = RegCloseKey(key);
                    continue;
                };
                keys.push((key, event));
                events.push(event);
            }
        }
    }
    let arm = |key: HKEY, event: HANDLE| {
        // SAFETY: pedido assíncrono na chave aberta acima, sinalizado no evento dela.
        unsafe { RegNotifyChangeKeyValue(key, true, REG_NOTIFY_CHANGE_LAST_SET | REG_NOTIFY_CHANGE_NAME, Some(event), true) }
    };
    for (key, event) in &keys {
        let _ = arm(*key, *event);
    }
    loop {
        // SAFETY: espera nos eventos vivos deste laço.
        let r = unsafe { WaitForMultipleObjects(&events, false, INFINITE) };
        let index = r.0.wrapping_sub(WAIT_OBJECT_0.0) as usize;
        if index == 0 || index >= events.len() {
            break;
        }
        let (key, event) = keys[index - 1];
        let _ = arm(key, event);
        let now = read();
        let changed = state.lock().map(|mut s| std::mem::replace(&mut *s, now) != now).unwrap_or(false);
        if changed {
            notify();
        }
    }
    for (key, event) in keys {
        // SAFETY: recursos abertos acima, fechados uma vez.
        unsafe {
            let _ = RegCloseKey(key);
            let _ = CloseHandle(event);
        }
    }
}
