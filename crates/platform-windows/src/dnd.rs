//! "Não perturbe" do Windows por evento (WNF `QUIET_HOURS_ACTIVE_PROFILE_CHANGED`), como a ilha C++.
//! API não documentada do ntdll, carregada em tempo de execução; se faltar, o estado fica "desligado"
//! e nada quebra (PLANNING §13).

use std::ffi::c_void;
use std::sync::atomic::{AtomicBool, AtomicPtr, Ordering};
use std::sync::{Arc, OnceLock};
use windows::Win32::System::LibraryLoader::{GetModuleHandleW, GetProcAddress};
use windows::core::{s, w};

const WNF_QUIET_HOURS_ACTIVE_PROFILE_CHANGED: u64 = 0x0D83_063E_A3BF_1C75;

type Callback = unsafe extern "system" fn(u64, u32, *const c_void, *const c_void, *const c_void, u32) -> i32;
type Subscribe = unsafe extern "system" fn(*mut *mut c_void, u64, u32, Callback, *const c_void, *const c_void, u32, u32) -> i32;
type Unsubscribe = unsafe extern "system" fn(*mut c_void) -> i32;
type Query = unsafe extern "system" fn(*const u64, *const c_void, *const c_void, *mut u32, *mut c_void, *mut u32) -> i32;

static ACTIVE: AtomicBool = AtomicBool::new(false);
static NOTIFY: OnceLock<Arc<dyn Fn() + Send + Sync>> = OnceLock::new();
static SUBSCRIPTION: AtomicPtr<c_void> = AtomicPtr::new(std::ptr::null_mut());

unsafe extern "system" fn on_change(state: u64, _stamp: u32, _type: *const c_void, _ctx: *const c_void, buffer: *const c_void, size: u32) -> i32 {
    if state != WNF_QUIET_HOURS_ACTIVE_PROFILE_CHANGED {
        return 0;
    }
    // SAFETY: o Windows passa `size` bytes válidos em `buffer` durante o callback.
    let value = if !buffer.is_null() && size >= 4 { unsafe { *(buffer as *const i32) } } else { 0 };
    let active = value != 0;
    if ACTIVE.swap(active, Ordering::SeqCst) != active
        && let Some(notify) = NOTIFY.get()
    {
        notify();
    }
    0
}

fn ntdll(name: windows::core::PCSTR) -> Option<unsafe extern "system" fn() -> isize> {
    // SAFETY: o ntdll está sempre carregado; só procura o endereço da função.
    unsafe { GetProcAddress(GetModuleHandleW(w!("ntdll.dll")).ok()?, name) }
}

/// Abre as configurações de notificação do Windows (onde fica o Não perturbe). Alternar direto não
/// foi validado (S5 incompleto; o usuário preferiu não alternar o estado dele na fase 1e).
pub fn open_settings() {
    crate::system::open_uri("ms-settings:notifications");
}

pub fn active() -> bool {
    ACTIVE.load(Ordering::SeqCst)
}

/// Lê o estado atual e passa a avisar `notify` (em outra thread) a cada troca. Uma vez por processo.
pub fn watch(notify: Arc<dyn Fn() + Send + Sync>) {
    if NOTIFY.set(notify).is_err() {
        return;
    }
    if let Some(f) = ntdll(s!("NtQueryWnfStateData")) {
        // SAFETY: assinatura de NtQueryWnfStateData; buffer local de 4 bytes.
        unsafe {
            let query: Query = std::mem::transmute(f);
            let (mut stamp, mut size, mut value) = (0u32, 4u32, 0i32);
            let name = WNF_QUIET_HOURS_ACTIVE_PROFILE_CHANGED;
            if query(&name, std::ptr::null(), std::ptr::null(), &mut stamp, &mut value as *mut i32 as *mut c_void, &mut size) == 0 && size >= 4 {
                ACTIVE.store(value != 0, Ordering::SeqCst);
            }
        }
    }
    if let Some(f) = ntdll(s!("RtlSubscribeWnfStateChangeNotification")) {
        // SAFETY: assinatura de RtlSubscribeWnfStateChangeNotification; o callback é estático.
        unsafe {
            let subscribe: Subscribe = std::mem::transmute(f);
            let mut sub = std::ptr::null_mut();
            // ChangeStamp 0 entrega o valor atual primeiro: igual ao lido acima, não avisa.
            if subscribe(&mut sub, WNF_QUIET_HOURS_ACTIVE_PROFILE_CHANGED, 0, on_change, std::ptr::null(), std::ptr::null(), 0, 0) == 0 {
                SUBSCRIPTION.store(sub, Ordering::SeqCst);
            }
        }
    }
}

pub fn unwatch() {
    let sub = SUBSCRIPTION.swap(std::ptr::null_mut(), Ordering::SeqCst);
    if !sub.is_null()
        && let Some(f) = ntdll(s!("RtlUnsubscribeWnfStateChangeNotification"))
    {
        // SAFETY: assinatura conhecida; a inscrição veio do subscribe acima.
        unsafe {
            let unsubscribe: Unsubscribe = std::mem::transmute(f);
            unsubscribe(sub);
        }
    }
}
