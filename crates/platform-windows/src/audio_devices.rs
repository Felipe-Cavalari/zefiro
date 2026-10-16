//! Dispositivos de áudio: listar saídas e entradas, trocar o padrão do sistema, mudo do microfone e a
//! saída de som por app (porte de `island/zefiro_audio_route.cpp`).
//!
//! Trocar o padrão usa `IPolicyConfig` e a saída por app usa `IAudioPolicyConfig`: as duas são
//! interfaces internas do Windows usadas há anos pelo EarTrumpet. Se faltarem, as funções devolvem
//! erro e o menu esconde a opção (PLANNING §13, "Trocar dispositivo de áudio").

use std::ffi::c_void;
use windows::Win32::Devices::FunctionDiscovery::PKEY_Device_FriendlyName;
use windows::Win32::Foundation::{CloseHandle, HANDLE};
use windows::Win32::Media::Audio::Endpoints::IAudioEndpointVolume;
use windows::Win32::Media::Audio::{
    DEVICE_STATE_ACTIVE, EDataFlow, ERole, IAudioSessionControl2, IAudioSessionManager2, IMMDevice, IMMDeviceEnumerator, MMDeviceEnumerator, eCapture, eCommunications,
    eConsole, eMultimedia, eRender,
};
use windows::Win32::Storage::Packaging::Appx::GetPackageFamilyName;
use windows::Win32::System::Com::{CLSCTX_ALL, CoCreateInstance, CoTaskMemFree, STGM_READ};
use windows::Win32::System::Diagnostics::ToolHelp::{CreateToolhelp32Snapshot, PROCESSENTRY32W, Process32FirstW, Process32NextW, TH32CS_SNAPPROCESS};
use windows::Win32::System::Threading::{OpenProcess, PROCESS_NAME_WIN32, PROCESS_QUERY_LIMITED_INFORMATION, QueryFullProcessImageNameW};
use windows::Win32::System::WinRT::RoGetActivationFactory;
use windows::core::{GUID, HRESULT, HSTRING, IUnknown, IUnknown_Vtbl, Interface, PCWSTR, PWSTR, Result, interface};
use zefiro_island::audio_route::{SourceMatcher, device_id_from_interface, device_interface_id};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Flow { Output, Input }

impl Flow {
    fn data_flow(self) -> EDataFlow {
        match self {
            Flow::Output => eRender,
            Flow::Input => eCapture,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AudioDevice { pub id: String, pub name: String }

/// `IPolicyConfig` (Windows 7+), só o método usado; os anteriores ocupam a posição na tabela.
#[interface("f8679f50-850a-41cf-9c72-430f290290c8")]
unsafe trait IPolicyConfig: IUnknown {
    fn get_mix_format(&self) -> HRESULT;
    fn get_device_format(&self) -> HRESULT;
    fn reset_device_format(&self) -> HRESULT;
    fn set_device_format(&self) -> HRESULT;
    fn get_processing_period(&self) -> HRESULT;
    fn set_processing_period(&self) -> HRESULT;
    fn get_share_mode(&self) -> HRESULT;
    fn set_share_mode(&self) -> HRESULT;
    fn get_property_value(&self) -> HRESULT;
    fn set_property_value(&self) -> HRESULT;
    fn set_default_endpoint(&self, device_id: PCWSTR, role: ERole) -> HRESULT;
}

const CLSID_POLICY_CONFIG_CLIENT: GUID = GUID::from_u128(0x870af99c_171d_4f9e_af0d_e63df40c2bc9);

macro_rules! audio_policy_config {
    ($name:ident, $iid:literal) => {
        /// `IAudioPolicyConfig` (AudioSes.dll), como declarada no EarTrumpet: 19 métodos sem uso depois
        /// do IInspectable (declarado aqui como os 3 primeiros). O GUID mudou no 21H2; a tabela não.
        #[interface($iid)]
        unsafe trait $name: IUnknown {
            // IInspectable: GetIids, GetRuntimeClassName, GetTrustLevel.
            fn get_iids(&self) -> HRESULT;
            fn get_runtime_class_name(&self) -> HRESULT;
            fn get_trust_level(&self) -> HRESULT;
            fn u01(&self) -> HRESULT;
            fn u02(&self) -> HRESULT;
            fn u03(&self) -> HRESULT;
            fn u04(&self) -> HRESULT;
            fn u05(&self) -> HRESULT;
            fn u06(&self) -> HRESULT;
            fn u07(&self) -> HRESULT;
            fn u08(&self) -> HRESULT;
            fn u09(&self) -> HRESULT;
            fn u10(&self) -> HRESULT;
            fn u11(&self) -> HRESULT;
            fn u12(&self) -> HRESULT;
            fn u13(&self) -> HRESULT;
            fn u14(&self) -> HRESULT;
            fn u15(&self) -> HRESULT;
            fn u16(&self) -> HRESULT;
            fn u17(&self) -> HRESULT;
            fn u18(&self) -> HRESULT;
            fn u19(&self) -> HRESULT;
            fn set_persisted(&self, pid: u32, flow: EDataFlow, role: ERole, device: *mut c_void) -> HRESULT;
            fn get_persisted(&self, pid: u32, flow: EDataFlow, role: ERole, device: *mut *mut c_void) -> HRESULT;
        }
    };
}
audio_policy_config!(IAudioPolicyConfig21H2, "ab3d4648-e242-459f-b02f-541c70306324");
audio_policy_config!(IAudioPolicyConfigDownlevel, "2a59116d-6c4f-45e0-a74f-707e3fef9258");

enum PolicyConfig { Current(IAudioPolicyConfig21H2), Downlevel(IAudioPolicyConfigDownlevel) }

impl PolicyConfig {
    fn open() -> Option<PolicyConfig> {
        let class = HSTRING::from("Windows.Media.Internal.AudioPolicyConfig");
        // SAFETY: fábrica WinRT pedida pelo nome e IID conhecidos; devolvida como referência COM.
        unsafe {
            if let Ok(c) = RoGetActivationFactory::<IAudioPolicyConfig21H2>(&class) {
                return Some(PolicyConfig::Current(c));
            }
            RoGetActivationFactory::<IAudioPolicyConfigDownlevel>(&class).ok().map(PolicyConfig::Downlevel)
        }
    }

    fn set(&self, pid: u32, role: ERole, device: &HSTRING) -> Result<()> {
        // SAFETY: o HSTRING continua vivo durante a chamada; a interface só lê o valor.
        let raw: *mut c_void = unsafe { std::mem::transmute_copy(device) };
        // SAFETY: chamada na tabela declarada acima.
        unsafe {
            match self {
                PolicyConfig::Current(c) => c.set_persisted(pid, eRender, role, raw).ok(),
                PolicyConfig::Downlevel(c) => c.set_persisted(pid, eRender, role, raw).ok(),
            }
        }
    }

    fn persisted(&self, pid: u32) -> Result<String> {
        let mut raw: *mut c_void = std::ptr::null_mut();
        // SAFETY: a interface devolve um HSTRING nosso; `HSTRING` libera ao sair do escopo.
        unsafe {
            match self {
                PolicyConfig::Current(c) => c.get_persisted(pid, eRender, eMultimedia, &mut raw).ok()?,
                PolicyConfig::Downlevel(c) => c.get_persisted(pid, eRender, eMultimedia, &mut raw).ok()?,
            }
            let value: HSTRING = std::mem::transmute(raw);
            Ok(value.to_string())
        }
    }
}

fn enumerator() -> Result<IMMDeviceEnumerator> {
    // SAFETY: COM já inicializado na thread de UI (fila de despacho STA).
    unsafe { CoCreateInstance(&MMDeviceEnumerator, None, CLSCTX_ALL) }
}

fn device_id(d: &IMMDevice) -> Result<String> {
    // SAFETY: string do COM liberada com CoTaskMemFree.
    unsafe {
        let id = d.GetId()?;
        let s = id.to_string().unwrap_or_default();
        CoTaskMemFree(Some(id.0 as _));
        Ok(s)
    }
}

fn friendly_name(d: &IMMDevice) -> Option<String> {
    // SAFETY: propriedade lida do repositório do dispositivo; o PROPVARIANT é liberado no drop.
    unsafe {
        let store = d.OpenPropertyStore(STGM_READ).ok()?;
        let value = store.GetValue(&PKEY_Device_FriendlyName).ok()?;
        let s = value.to_string();
        (!s.is_empty()).then_some(s)
    }
}

/// Dispositivos ativos (ordem do Windows) e o id do padrão atual.
pub fn devices(flow: Flow) -> Result<(Vec<AudioDevice>, Option<String>)> {
    let e = enumerator()?;
    let mut out = Vec::new();
    // SAFETY: coleção COM percorrida por índice dentro do tamanho informado.
    unsafe {
        let collection = e.EnumAudioEndpoints(flow.data_flow(), DEVICE_STATE_ACTIVE)?;
        for i in 0..collection.GetCount()? {
            let Ok(d) = collection.Item(i) else { continue };
            let Ok(id) = device_id(&d) else { continue };
            let name = friendly_name(&d).unwrap_or_else(|| id.clone());
            out.push(AudioDevice { id, name });
        }
        let default = e.GetDefaultAudioEndpoint(flow.data_flow(), eConsole).ok().and_then(|d| device_id(&d).ok());
        Ok((out, default))
    }
}

/// Novo padrão do sistema para os três papéis (como o painel de som do Windows).
pub fn set_default(id: &str) -> Result<()> {
    // SAFETY: interface interna criada pelo CLSID conhecido; o id vive até o fim das chamadas.
    unsafe {
        let policy: IPolicyConfig = CoCreateInstance(&CLSID_POLICY_CONFIG_CLIENT, None, CLSCTX_ALL)?;
        let wide = HSTRING::from(id);
        for role in [eConsole, eMultimedia, eCommunications] {
            policy.set_default_endpoint(PCWSTR(wide.as_ptr()), role).ok()?;
        }
    }
    Ok(())
}

fn capture_volume() -> Result<IAudioEndpointVolume> {
    // SAFETY: interfaces COM da entrada padrão.
    unsafe { enumerator()?.GetDefaultAudioEndpoint(eCapture, eConsole)?.Activate(CLSCTX_ALL, None) }
}

pub fn mic_muted() -> Option<bool> {
    // SAFETY: leitura simples do mudo.
    capture_volume().ok().and_then(|v| unsafe { v.GetMute() }.ok()).map(|b| b.as_bool())
}

pub fn set_mic_muted(muted: bool) -> Result<()> {
    // SAFETY: muda só o mudo da entrada padrão.
    unsafe { capture_volume()?.SetMute(muted, std::ptr::null()) }
}

/// Processos de um app agrupados por executável. O Windows grava a escolha por executável, mas só
/// aceita o pid de um processo que já abriu áudio; por isso guarda todos e tenta em ordem.
#[derive(Debug, Clone, Default)]
pub struct AppRoute { exes: Vec<(String, Vec<u32>)> }

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RouteState {
    /// Saída gravada para o app (`None` = padrão do sistema).
    Current(Option<String>),
    /// O app está aberto, mas ainda não abriu áudio.
    NoAudioYet,
    NotRunning,
}

fn image_path(pid: u32) -> Option<String> {
    // SAFETY: handle aberto só para consulta e fechado em seguida; buffer local.
    unsafe {
        let process = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid).ok()?;
        let mut buf = vec![0u16; 1024];
        let mut size = buf.len() as u32;
        let ok = QueryFullProcessImageNameW(process, PROCESS_NAME_WIN32, PWSTR(buf.as_mut_ptr()), &mut size).is_ok();
        let _ = CloseHandle(process);
        ok.then(|| String::from_utf16_lossy(&buf[..size as usize]))
    }
}

fn package_family(pid: u32) -> Option<String> {
    // SAFETY: handle aberto só para consulta e fechado em seguida; buffer local do tamanho máximo.
    unsafe {
        let process: HANDLE = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid).ok()?;
        let mut buf = vec![0u16; 128];
        let mut len = buf.len() as u32;
        let r = GetPackageFamilyName(process, &mut len, Some(PWSTR(buf.as_mut_ptr())));
        let _ = CloseHandle(process);
        (r.0 == 0).then(|| String::from_utf16_lossy(&buf[..len.saturating_sub(1) as usize]))
    }
}

fn matches(m: &SourceMatcher, pid: u32, exe: &str) -> bool {
    match &m.package_family {
        Some(family) => package_family(pid).is_some_and(|f| f.eq_ignore_ascii_case(family)),
        None => m.matches_exe(exe),
    }
}

fn session_pids(e: &IMMDeviceEnumerator) -> Vec<u32> {
    let mut pids = Vec::new();
    // SAFETY: percorre as sessões de áudio de cada saída ativa, por índice dentro do tamanho informado.
    unsafe {
        let Ok(collection) = e.EnumAudioEndpoints(eRender, DEVICE_STATE_ACTIVE) else { return pids };
        for i in 0..collection.GetCount().unwrap_or(0) {
            let Ok(device) = collection.Item(i) else { continue };
            let Ok(manager) = device.Activate::<IAudioSessionManager2>(CLSCTX_ALL, None) else { continue };
            let Ok(sessions) = manager.GetSessionEnumerator() else { continue };
            for j in 0..sessions.GetCount().unwrap_or(0) {
                let pid = sessions.GetSession(j).and_then(|s| s.cast::<IAudioSessionControl2>()).and_then(|s| s.GetProcessId());
                if let Ok(pid) = pid
                    && pid != 0
                    && !pids.contains(&pid)
                {
                    pids.push(pid);
                }
            }
        }
    }
    pids
}

fn add(route: &mut AppRoute, pid: u32) {
    let path = image_path(pid).map(|p| p.to_lowercase()).unwrap_or_else(|| format!("pid:{pid}"));
    match route.exes.iter_mut().find(|(p, _)| *p == path) {
        Some((_, pids)) => {
            if !pids.contains(&pid) {
                pids.push(pid);
            }
        }
        None => route.exes.push((path, vec![pid])),
    }
}

/// Processos da fonte de mídia e a saída gravada para ela. `None` se a interface não existir.
pub fn app_route(aumid: &str, source_name: &str) -> Option<(AppRoute, RouteState)> {
    let policy = PolicyConfig::open()?;
    let m = SourceMatcher::new(aumid, source_name);
    let e = enumerator().ok()?;
    let mut route = AppRoute::default();
    for pid in session_pids(&e) {
        let exe = image_path(pid).unwrap_or_default();
        let base = exe.rsplit(['\\', '/']).next().unwrap_or(&exe).to_string();
        if matches(&m, pid, &base) {
            add(&mut route, pid);
        }
    }
    if route.exes.is_empty() {
        // App pausado há tempo: procura em todos os processos.
        // SAFETY: instantâneo do Toolhelp percorrido e fechado; estrutura com cbSize preenchido.
        unsafe {
            if let Ok(snapshot) = CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0) {
                let mut entry = PROCESSENTRY32W { dwSize: size_of::<PROCESSENTRY32W>() as u32, ..Default::default() };
                let mut ok = Process32FirstW(snapshot, &mut entry).is_ok();
                while ok {
                    let len = entry.szExeFile.iter().position(|c| *c == 0).unwrap_or(entry.szExeFile.len());
                    let exe = String::from_utf16_lossy(&entry.szExeFile[..len]);
                    if entry.th32ProcessID != 0 && matches(&m, entry.th32ProcessID, &exe) {
                        add(&mut route, entry.th32ProcessID);
                    }
                    ok = Process32NextW(snapshot, &mut entry).is_ok();
                }
                let _ = CloseHandle(snapshot);
            }
        }
    }
    if route.exes.is_empty() {
        return Some((route, RouteState::NotRunning));
    }
    for (_, pids) in &route.exes {
        for &pid in pids {
            if let Ok(id) = policy.persisted(pid) {
                let id = device_id_from_interface(&id);
                return Some((route, RouteState::Current((!id.is_empty()).then_some(id))));
            }
        }
    }
    Some((route, RouteState::NoAudioYet))
}

/// Grava a saída do app (`None` = voltar ao padrão do sistema). Tenta os pids até um aceitar.
pub fn set_app_output(route: &AppRoute, device: Option<&str>) -> Result<()> {
    let policy = PolicyConfig::open().ok_or_else(|| windows::core::Error::new(windows::Win32::Foundation::E_NOINTERFACE, "IAudioPolicyConfig indisponível"))?;
    let value = HSTRING::from(device.map(device_interface_id).unwrap_or_default());
    let mut last = Err(windows::core::Error::new(windows::Win32::Foundation::E_INVALIDARG, "nenhum processo do app aceitou"));
    for (_, pids) in &route.exes {
        for &pid in pids {
            let r = policy.set(pid, eMultimedia, &value).and_then(|_| policy.set(pid, eConsole, &value));
            if r.is_ok() {
                return Ok(());
            }
            last = r;
        }
    }
    last
}

