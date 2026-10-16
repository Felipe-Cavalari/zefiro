//! Dispositivos Bluetooth conectando e desconectando (`DeviceWatcher` nos seletores "conectado" do
//! Bluetooth clássico e do LE, como a ilha C++): aparecer = conectou, sumir = desconectou. Os que já
//! estavam conectados quando o observador começou só entram no cache, sem aviso.

use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use windows::Devices::Bluetooth::{BluetoothConnectionStatus, BluetoothDevice, BluetoothLEDevice};
use windows::Devices::Enumeration::{DeviceInformation, DeviceInformationUpdate, DeviceWatcher};
use windows::Foundation::TypedEventHandler;
use windows::core::{HSTRING, IInspectable, Result};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BluetoothEvent { pub name: String, pub connected: bool }

pub struct BluetoothWatcher {
    watchers: Vec<DeviceWatcher>,
    queue: Arc<Mutex<Vec<BluetoothEvent>>>,
}

impl BluetoothWatcher {
    pub fn start(notify: Arc<dyn Fn() + Send + Sync>) -> Result<BluetoothWatcher> {
        let queue = Arc::new(Mutex::new(Vec::new()));
        let names: Arc<Mutex<HashMap<String, String>>> = Arc::default();
        let mut watchers = Vec::new();
        let selectors = [
            BluetoothDevice::GetDeviceSelectorFromConnectionStatus(BluetoothConnectionStatus::Connected)?,
            BluetoothLEDevice::GetDeviceSelectorFromConnectionStatus(BluetoothConnectionStatus::Connected)?,
        ];
        for selector in selectors {
            match watch(&selector, &queue, &names, &notify) {
                Ok(w) => watchers.push(w),
                Err(e) => tracing::warn!("observador Bluetooth falhou: {e}"),
            }
        }
        Ok(BluetoothWatcher { watchers, queue })
    }

    pub fn take(&self) -> Vec<BluetoothEvent> {
        self.queue.lock().map(|mut q| std::mem::take(&mut *q)).unwrap_or_default()
    }
}

impl Drop for BluetoothWatcher {
    fn drop(&mut self) {
        for w in &self.watchers {
            let _ = w.Stop();
        }
    }
}

fn watch(selector: &HSTRING, queue: &Arc<Mutex<Vec<BluetoothEvent>>>, names: &Arc<Mutex<HashMap<String, String>>>, notify: &Arc<dyn Fn() + Send + Sync>) -> Result<DeviceWatcher> {
    let watcher = DeviceInformation::CreateWatcherAqsFilter(selector)?;
    let enumerated = Arc::new(AtomicBool::new(false));
    let done = enumerated.clone();
    watcher.EnumerationCompleted(&TypedEventHandler::<DeviceWatcher, IInspectable>::new(move |_, _| {
        done.store(true, Ordering::SeqCst);
        Ok(())
    }))?;
    let (q, n, f, e) = (queue.clone(), names.clone(), notify.clone(), enumerated.clone());
    watcher.Added(&TypedEventHandler::<DeviceWatcher, DeviceInformation>::new(move |_, info| {
        let Some(info) = info.as_ref() else { return Ok(()) };
        let name = info.Name()?.to_string();
        if name.is_empty() {
            return Ok(());
        }
        if let Ok(mut map) = n.lock() {
            map.insert(info.Id()?.to_string(), name.clone());
        }
        if e.load(Ordering::SeqCst) {
            if let Ok(mut queue) = q.lock() {
                queue.push(BluetoothEvent { name, connected: true });
            }
            f();
        }
        Ok(())
    }))?;
    let (q, n, f) = (queue.clone(), names.clone(), notify.clone());
    watcher.Removed(&TypedEventHandler::<DeviceWatcher, DeviceInformationUpdate>::new(move |_, update| {
        let Some(update) = update.as_ref() else { return Ok(()) };
        let name = n.lock().ok().and_then(|mut map| map.remove(&update.Id().map(|s| s.to_string()).unwrap_or_default()));
        if let Some(name) = name {
            if let Ok(mut queue) = q.lock() {
                queue.push(BluetoothEvent { name, connected: false });
            }
            f();
        }
        Ok(())
    }))?;
    // Atualizações de propriedade não interessam, mas o Windows exige o handler para entregar as outras.
    watcher.Updated(&TypedEventHandler::<DeviceWatcher, DeviceInformationUpdate>::new(|_, _| Ok(())))?;
    watcher.Start()?;
    Ok(watcher)
}
