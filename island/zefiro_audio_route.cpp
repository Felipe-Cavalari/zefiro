// Saída de som por fonte de mídia (ver zefiro_audio_route.h).
//
// Caminho de uma troca:
//   1. AUMID da fonte (SMTC) -> processos do app. Apps da Store ("Pacote_hash!App")
//      casam pelo nome da família do pacote; os outros ("Brave", "Spotify.exe",
//      "MSEdge") pelo nome do executável. Navegadores tocam num processo filho
//      (o serviço de áudio), mas com o mesmo executável, então o nome basta.
//      Primeiro olhamos só os processos que têm sessão de áudio em alguma saída
//      (poucos e baratos); se nenhum casar (app pausado há tempo), varremos todos.
//   2. IAudioPolicyConfig::SetPersistedDefaultAudioEndpoint(pid, eRender, papel,
//      dispositivo) para eMultimedia e eConsole. O Windows grava a escolha por
//      executável e move os fluxos abertos na hora.
#include "wh_shim.h"

#include "zefiro_audio_route.h"

#include <appmodel.h>
#include <audiopolicy.h>
#include <inspectable.h>
#include <mmdeviceapi.h>
#include <roapi.h>
#include <tlhelp32.h>
#include <winstring.h>
#include <wrl/client.h>

#include <algorithm>
#include <cwctype>
#include <optional>
#include <vector>

using Microsoft::WRL::ComPtr;

namespace {

// Interface interna de AudioSes.dll, como declarada no EarTrumpet
// (Interop/MMDeviceAPI/IAudioPolicyConfigFactoryVariantFor21H2.cs): 19 métodos
// que não usamos depois do IInspectable. O GUID mudou no Windows 10 21H2
// (build 21390); a tabela de métodos não.
struct IAudioPolicyConfig : IInspectable {
    virtual HRESULT STDMETHODCALLTYPE Unused01() = 0;  // add_CtxVolumeChange
    virtual HRESULT STDMETHODCALLTYPE Unused02() = 0;  // remove_CtxVolumeChanged
    virtual HRESULT STDMETHODCALLTYPE Unused03() = 0;  // add_RingerVibrateStateChanged
    virtual HRESULT STDMETHODCALLTYPE Unused04() = 0;  // remove_RingerVibrateStateChange
    virtual HRESULT STDMETHODCALLTYPE Unused05() = 0;  // SetVolumeGroupGainForId
    virtual HRESULT STDMETHODCALLTYPE Unused06() = 0;  // GetVolumeGroupGainForId
    virtual HRESULT STDMETHODCALLTYPE Unused07() = 0;  // GetActiveVolumeGroupForEndpointId
    virtual HRESULT STDMETHODCALLTYPE Unused08() = 0;  // GetVolumeGroupsForEndpoint
    virtual HRESULT STDMETHODCALLTYPE Unused09() = 0;  // GetCurrentVolumeContext
    virtual HRESULT STDMETHODCALLTYPE Unused10() = 0;  // SetVolumeGroupMuteForId
    virtual HRESULT STDMETHODCALLTYPE Unused11() = 0;  // GetVolumeGroupMuteForId
    virtual HRESULT STDMETHODCALLTYPE Unused12() = 0;  // SetRingerVibrateState
    virtual HRESULT STDMETHODCALLTYPE Unused13() = 0;  // GetRingerVibrateState
    virtual HRESULT STDMETHODCALLTYPE Unused14() = 0;  // SetPreferredChatApplication
    virtual HRESULT STDMETHODCALLTYPE Unused15() = 0;  // ResetPreferredChatApplication
    virtual HRESULT STDMETHODCALLTYPE Unused16() = 0;  // GetPreferredChatApplication
    virtual HRESULT STDMETHODCALLTYPE Unused17() = 0;  // GetCurrentChatApplications
    virtual HRESULT STDMETHODCALLTYPE Unused18() = 0;  // add_ChatContextChanged
    virtual HRESULT STDMETHODCALLTYPE Unused19() = 0;  // remove_ChatContextChanged
    virtual HRESULT STDMETHODCALLTYPE SetPersistedDefaultAudioEndpoint(UINT32 processId, EDataFlow flow, ERole role,
                                                                       HSTRING deviceId) = 0;
    virtual HRESULT STDMETHODCALLTYPE GetPersistedDefaultAudioEndpoint(UINT32 processId, EDataFlow flow, ERole role,
                                                                       HSTRING* deviceId) = 0;
    virtual HRESULT STDMETHODCALLTYPE ClearAllPersistedApplicationDefaultEndpoints() = 0;
};

constexpr GUID kIidPolicyConfig21H2 = {0xab3d4648, 0xe242, 0x459f, {0xb0, 0x2f, 0x54, 0x1c, 0x70, 0x30, 0x63, 0x24}};
constexpr GUID kIidPolicyConfigDownlevel = {0x2a59116d, 0x6c4f, 0x45e0, {0xa7, 0x4f, 0x70, 0x7e, 0x3f, 0xef, 0x92, 0x58}};
constexpr DWORD kBuild21H2 = 21390;

// PKEY_Device_FriendlyName, definido aqui para não depender de INITGUID.
constexpr PROPERTYKEY kDeviceFriendlyName = {
    {0xa45c254e, 0xdf1c, 0x4efd, {0x80, 0x20, 0x67, 0xd1, 0x46, 0xa8, 0x50, 0xe0}}, 14};

// A política guarda o id no formato de interface de dispositivo (como o EarTrumpet).
constexpr wchar_t kMmdevapiPrefix[] = L"\\\\?\\SWD#MMDEVAPI#";
constexpr wchar_t kRenderInterfaceSuffix[] = L"#{e6327cad-dcec-4949-ae8a-991e976a79d2}";

struct Device {
    std::wstring id;  // id do IMMDevice ({0.0.0.00000000}.{guid})
    std::wstring name;
};

// Processos do app com o mesmo executável. A escolha é gravada por executável,
// mas o Windows só aceita o pid de um processo que já abriu áudio (os outros dão
// E_INVALIDARG), então guardamos todos e tentamos em ordem, começando pelos que
// têm sessão de áudio.
struct AppExe {
    std::wstring path;
    std::vector<DWORD> pids;
};

// O que o último menu mostrou; usado quando o comando volta do TrackPopupMenu.
struct MenuState {
    std::wstring sourceName;
    std::vector<AppExe> exes;
    std::vector<Device> devices;
};
MenuState g_menu;

// Inicializa COM na thread se ainda não estiver (a thread da janela já costuma ter).
class ComScope {
public:
    ComScope() : hr_(CoInitializeEx(nullptr, COINIT_APARTMENTTHREADED)) {}
    ~ComScope() {
        if (SUCCEEDED(hr_)) CoUninitialize();
    }
    ComScope(const ComScope&) = delete;
    ComScope& operator=(const ComScope&) = delete;

private:
    HRESULT hr_;
};

std::wstring Lower(std::wstring s) {
    std::transform(s.begin(), s.end(), s.begin(), [](wchar_t c) { return static_cast<wchar_t>(std::towlower(c)); });
    return s;
}

std::wstring StripExe(std::wstring s) {
    if (s.size() > 4 && s.compare(s.size() - 4, 4, L".exe") == 0) s.resize(s.size() - 4);
    return s;
}

std::wstring BaseName(const std::wstring& path) {
    const size_t slash = path.find_last_of(L"\\/");
    return slash == std::wstring::npos ? path : path.substr(slash + 1);
}

DWORD WindowsBuild() {
    using RtlGetVersionFn = LONG(WINAPI*)(OSVERSIONINFOW*);
    OSVERSIONINFOW info{sizeof(info)};
    if (auto fn = reinterpret_cast<RtlGetVersionFn>(GetProcAddress(GetModuleHandleW(L"ntdll.dll"), "RtlGetVersion"))) {
        fn(&info);
    }
    return info.dwBuildNumber;
}

// Fábrica da política, criada uma vez. nullptr se a interface não existir
// (registrado no log uma vez só).
IAudioPolicyConfig* PolicyConfig() {
    static bool tried = false;
    static IAudioPolicyConfig* config = nullptr;
    if (tried) return config;
    tried = true;

    const DWORD build = WindowsBuild();
    const GUID* order[2] = {&kIidPolicyConfig21H2, &kIidPolicyConfigDownlevel};
    if (build < kBuild21H2) std::swap(order[0], order[1]);

    HSTRING_HEADER header;
    HSTRING className = nullptr;
    constexpr wchar_t kClass[] = L"Windows.Media.Internal.AudioPolicyConfig";
    HRESULT hr = WindowsCreateStringReference(kClass, ARRAYSIZE(kClass) - 1, &header, &className);
    for (const GUID* iid : order) {
        if (FAILED(hr)) break;
        hr = RoGetActivationFactory(className, *iid, reinterpret_cast<void**>(&config));
        if (SUCCEEDED(hr)) {
            Wh_Log(L"Saída de som: IAudioPolicyConfig disponível (build %lu, %s).", build,
                   iid == &kIidPolicyConfig21H2 ? L"21H2+" : L"anterior ao 21H2");
            return config;
        }
        config = nullptr;
    }
    Wh_Log(L"Saída de som: IAudioPolicyConfig indisponível (build %lu, 0x%08lX); submenu escondido.", build,
           static_cast<unsigned long>(hr));
    return nullptr;
}

// Como casar um processo com a fonte de mídia.
struct SourceMatcher {
    std::wstring packageFamily;      // apps da Store: o que vem antes do '!'
    std::vector<std::wstring> keys;  // outros: nomes de executável aceitos (minúsculos, sem .exe)

    SourceMatcher(const std::wstring& aumid, const std::wstring& sourceName) {
        const size_t bang = aumid.find(L'!');
        if (bang != std::wstring::npos) {
            packageFamily = Lower(aumid.substr(0, bang));
            return;
        }
        // "Spotify.exe", "C:\...\app.exe", "Brave", "MSEdge", "Brave.XXXXXXXX" (perfil).
        const std::wstring full = StripExe(Lower(BaseName(aumid)));
        AddKey(full);
        AddKey(full.substr(0, full.find(L'.')));
        // Fontes com AUMID opaco (o Firefox usa um hash) ainda casam pelo nome mostrado.
        std::wstring name = Lower(sourceName);
        name.erase(std::remove(name.begin(), name.end(), L' '), name.end());
        if (name != L"media") AddKey(name);
    }

    void AddKey(const std::wstring& key) {
        if (!key.empty() && std::find(keys.begin(), keys.end(), key) == keys.end()) keys.push_back(key);
    }

    // exeName vem do Toolhelp ou do caminho da imagem; o processo só é aberto
    // para apps da Store.
    bool Matches(DWORD pid, const std::wstring& exeName) const {
        if (packageFamily.empty()) {
            const std::wstring exe = StripExe(Lower(exeName));
            return std::find(keys.begin(), keys.end(), exe) != keys.end();
        }
        HANDLE process = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, FALSE, pid);
        if (!process) return false;
        wchar_t family[PACKAGE_FAMILY_NAME_MAX_LENGTH + 1] = {};
        UINT32 length = ARRAYSIZE(family);
        const bool ok = GetPackageFamilyName(process, &length, family) == ERROR_SUCCESS;
        CloseHandle(process);
        return ok && Lower(family) == packageFamily;
    }
};

std::wstring ImagePath(DWORD pid) {
    std::wstring path;
    if (HANDLE process = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, FALSE, pid)) {
        wchar_t buffer[MAX_PATH * 2];
        DWORD size = ARRAYSIZE(buffer);
        if (QueryFullProcessImageNameW(process, 0, buffer, &size)) path.assign(buffer, size);
        CloseHandle(process);
    }
    return path;
}

std::wstring DeviceId(IMMDevice* device) {
    std::wstring id;
    LPWSTR raw = nullptr;
    if (SUCCEEDED(device->GetId(&raw)) && raw) {
        id = raw;
        CoTaskMemFree(raw);
    }
    return id;
}

std::wstring FriendlyName(IMMDevice* device) {
    std::wstring name;
    ComPtr<IPropertyStore> props;
    if (SUCCEEDED(device->OpenPropertyStore(STGM_READ, &props))) {
        PROPVARIANT value;
        PropVariantInit(&value);
        if (SUCCEEDED(props->GetValue(kDeviceFriendlyName, &value)) && value.vt == VT_LPWSTR) {
            name = value.pwszVal;
        }
        PropVariantClear(&value);
    }
    return name;
}

// Saídas ativas, com nome amigável, na ordem do Windows.
std::vector<Device> ActiveOutputs(IMMDeviceEnumerator* enumerator, std::vector<ComPtr<IMMDevice>>* handles) {
    std::vector<Device> devices;
    ComPtr<IMMDeviceCollection> collection;
    UINT count = 0;
    if (FAILED(enumerator->EnumAudioEndpoints(eRender, DEVICE_STATE_ACTIVE, &collection)) ||
        FAILED(collection->GetCount(&count))) {
        return devices;
    }
    for (UINT i = 0; i < count; ++i) {
        ComPtr<IMMDevice> device;
        if (FAILED(collection->Item(i, &device))) continue;
        Device d{DeviceId(device.Get()), FriendlyName(device.Get())};
        if (d.id.empty()) continue;
        if (d.name.empty()) d.name = d.id;
        devices.push_back(std::move(d));
        if (handles) handles->push_back(device);
    }
    return devices;
}

// Processos com sessão de áudio (de qualquer estado) em alguma saída ativa.
std::vector<DWORD> AudioSessionPids(const std::vector<ComPtr<IMMDevice>>& devices) {
    std::vector<DWORD> pids;
    for (const ComPtr<IMMDevice>& device : devices) {
        ComPtr<IAudioSessionManager2> manager;
        ComPtr<IAudioSessionEnumerator> sessions;
        int count = 0;
        if (FAILED(device->Activate(__uuidof(IAudioSessionManager2), CLSCTX_ALL, nullptr, &manager)) ||
            FAILED(manager->GetSessionEnumerator(&sessions)) || FAILED(sessions->GetCount(&count))) {
            continue;
        }
        for (int i = 0; i < count; ++i) {
            ComPtr<IAudioSessionControl> control;
            ComPtr<IAudioSessionControl2> control2;
            DWORD pid = 0;
            if (FAILED(sessions->GetSession(i, &control)) || FAILED(control.As(&control2)) ||
                FAILED(control2->GetProcessId(&pid)) || pid == 0) {
                continue;
            }
            if (std::find(pids.begin(), pids.end(), pid) == pids.end()) pids.push_back(pid);
        }
    }
    return pids;
}

void AddProcess(std::vector<AppExe>* exes, DWORD pid) {
    std::wstring path = Lower(ImagePath(pid));
    if (path.empty()) path = L"pid:" + std::to_wstring(pid);
    for (AppExe& exe : *exes) {
        if (exe.path != path) continue;
        if (std::find(exe.pids.begin(), exe.pids.end(), pid) == exe.pids.end()) exe.pids.push_back(pid);
        return;
    }
    exes->push_back(AppExe{std::move(path), {pid}});
}

std::vector<AppExe> FindSourceProcesses(const SourceMatcher& matcher, const std::vector<DWORD>& sessionPids) {
    std::vector<AppExe> exes;
    for (DWORD pid : sessionPids) {
        if (matcher.Matches(pid, BaseName(ImagePath(pid)))) AddProcess(&exes, pid);
    }
    if (!exes.empty()) return exes;

    HANDLE snapshot = CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0);
    if (snapshot == INVALID_HANDLE_VALUE) return exes;
    PROCESSENTRY32W entry{sizeof(entry)};
    for (BOOL ok = Process32FirstW(snapshot, &entry); ok; ok = Process32NextW(snapshot, &entry)) {
        if (entry.th32ProcessID != 0 && matcher.Matches(entry.th32ProcessID, entry.szExeFile)) {
            AddProcess(&exes, entry.th32ProcessID);
        }
    }
    CloseHandle(snapshot);
    return exes;
}

// Saída gravada para o app: id do IMMDevice, vazio = padrão do sistema, ou
// nullopt se o Windows não aceitou nenhum pid (o app ainda não abriu áudio).
std::optional<std::wstring> PersistedOutput(IAudioPolicyConfig* config, const std::vector<AppExe>& exes) {
    for (const AppExe& exe : exes) {
        for (DWORD pid : exe.pids) {
            HSTRING value = nullptr;
            if (FAILED(config->GetPersistedDefaultAudioEndpoint(pid, eRender, eMultimedia, &value))) continue;
            UINT32 length = 0;
            const wchar_t* raw = WindowsGetStringRawBuffer(value, &length);
            std::wstring id(raw, length);
            WindowsDeleteString(value);

            const std::wstring prefix = kMmdevapiPrefix;
            const std::wstring suffix = kRenderInterfaceSuffix;
            if (Lower(id).rfind(Lower(prefix), 0) == 0) id.erase(0, prefix.size());
            if (id.size() >= suffix.size() && Lower(id.substr(id.size() - suffix.size())) == suffix) {
                id.resize(id.size() - suffix.size());
            }
            return id;
        }
    }
    return std::nullopt;
}

bool SameId(const std::wstring& a, const std::wstring& b) {
    return CompareStringOrdinal(a.c_str(), -1, b.c_str(), -1, TRUE) == CSTR_EQUAL;
}

}  // namespace

namespace zefiro::audio_route {

void AppendOutputSubmenu(HMENU menu, const std::wstring& sourceAumid, const std::wstring& sourceName) {
    g_menu = MenuState{};
    if (sourceAumid.empty()) return;

    ComScope com;
    IAudioPolicyConfig* config = PolicyConfig();
    if (!config) return;

    ComPtr<IMMDeviceEnumerator> enumerator;
    if (FAILED(CoCreateInstance(__uuidof(MMDeviceEnumerator), nullptr, CLSCTX_ALL, IID_PPV_ARGS(&enumerator)))) {
        Wh_Log(L"Saída de som: MMDeviceEnumerator indisponível.");
        return;
    }
    std::vector<ComPtr<IMMDevice>> handles;
    std::vector<Device> devices = ActiveOutputs(enumerator.Get(), &handles);
    if (devices.empty()) return;

    std::wstring defaultName;
    {
        ComPtr<IMMDevice> device;
        if (SUCCEEDED(enumerator->GetDefaultAudioEndpoint(eRender, eMultimedia, &device))) {
            defaultName = FriendlyName(device.Get());
        }
    }

    const SourceMatcher matcher(sourceAumid, sourceName);
    std::vector<AppExe> exes = FindSourceProcesses(matcher, AudioSessionPids(handles));
    const std::optional<std::wstring> persisted = PersistedOutput(config, exes);

    HMENU sub = CreatePopupMenu();
    const std::wstring title = sourceName.empty() ? sourceAumid : sourceName;
    if (!persisted) {
        Wh_Log(L"Saída de som: %s sem processo com áudio aberto (%zu executáveis).", sourceAumid.c_str(),
               exes.size());
        const std::wstring label = title + (exes.empty() ? L" não está aberto" : L" ainda não abriu o áudio");
        AppendMenuW(sub, MF_STRING | MF_GRAYED | MF_DISABLED, 0, label.c_str());
        AppendMenuW(menu, MF_POPUP, reinterpret_cast<UINT_PTR>(sub), L"Saída de som");
        return;
    }

    const std::wstring& current = *persisted;
    AppendMenuW(sub, MF_STRING | MF_GRAYED | MF_DISABLED, 0, title.c_str());
    AppendMenuW(sub, MF_SEPARATOR, 0, nullptr);
    const std::wstring defaultLabel =
        defaultName.empty() ? std::wstring(L"Padrão do sistema") : L"Padrão do sistema (" + defaultName + L")";
    AppendMenuW(sub, MF_STRING | (current.empty() ? MF_CHECKED : 0), kMenuIdBase, defaultLabel.c_str());
    AppendMenuW(sub, MF_SEPARATOR, 0, nullptr);
    const size_t shown = std::min<size_t>(devices.size(), kMenuIdLast - kMenuIdBase);
    for (size_t i = 0; i < shown; ++i) {
        const bool checked = !current.empty() && SameId(current, devices[i].id);
        AppendMenuW(sub, MF_STRING | (checked ? MF_CHECKED : 0), kMenuIdBase + 1 + static_cast<UINT>(i),
                    devices[i].name.c_str());
    }
    AppendMenuW(menu, MF_POPUP, reinterpret_cast<UINT_PTR>(sub), L"Saída de som");

    g_menu.sourceName = title;
    g_menu.exes = std::move(exes);
    devices.resize(shown);
    g_menu.devices = std::move(devices);
}

bool HandleMenuCommand(UINT cmd) {
    if (cmd < kMenuIdBase || cmd > kMenuIdLast) return false;
    MenuState menu = std::move(g_menu);
    g_menu = MenuState{};
    if (menu.exes.empty()) return true;

    std::wstring deviceId;  // vazio = padrão do sistema
    std::wstring deviceName = L"padrão do sistema";
    if (cmd > kMenuIdBase) {
        const size_t index = cmd - kMenuIdBase - 1;
        if (index >= menu.devices.size()) return true;
        deviceId = menu.devices[index].id;
        deviceName = menu.devices[index].name;
    }

    ComScope com;
    IAudioPolicyConfig* config = PolicyConfig();
    if (!config) return true;

    HSTRING value = nullptr;
    if (!deviceId.empty()) {
        const std::wstring full = kMmdevapiPrefix + deviceId + kRenderInterfaceSuffix;
        if (FAILED(WindowsCreateString(full.c_str(), static_cast<UINT32>(full.size()), &value))) return true;
    }
    for (const AppExe& exe : menu.exes) {
        HRESULT hr = E_INVALIDARG;
        for (DWORD pid : exe.pids) {
            hr = config->SetPersistedDefaultAudioEndpoint(pid, eRender, eMultimedia, value);
            if (FAILED(hr)) continue;
            const HRESULT hrConsole = config->SetPersistedDefaultAudioEndpoint(pid, eRender, eConsole, value);
            Wh_Log(L"Saída de som: %s (pid %lu) -> %s (console 0x%08lX).", menu.sourceName.c_str(), pid,
                   deviceName.c_str(), static_cast<unsigned long>(hrConsole));
            break;
        }
        if (FAILED(hr)) {
            Wh_Log(L"Saída de som: nenhum pid de %s aceitou a troca (0x%08lX).", exe.path.c_str(),
                   static_cast<unsigned long>(hr));
        }
    }
    WindowsDeleteString(value);
    return true;
}

}  // namespace zefiro::audio_route
