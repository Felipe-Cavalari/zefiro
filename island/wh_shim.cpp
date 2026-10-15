// Implementação da camada de compatibilidade com o Windhawk. Ver wh_shim.h.
#include "wh_shim.h"

#include <shellscalingapi.h>
#include <shlobj.h>

#include <algorithm>
#include <cmath>
#include <cstdarg>
#include <cstdio>
#include <cwchar>
#include <cwctype>
#include <map>
#include <mutex>
#include <string>
#include <string_view>
#include <vector>

namespace {

struct SettingDefault {
    const wchar_t* group;
    const wchar_t* key;
    const wchar_t* value;
    const wchar_t* label;
    const wchar_t* options;
};

// Gerado pelo build a partir do bloco // ==WindhawkModSettings== do mod.
constexpr SettingDefault kModDefaults[] = {
#include "settings_defaults.inc"
};

// Configurações próprias do Zefiro (seção [Zefiro]).
constexpr SettingDefault kZefiroDefaults[] = {
    {L"Zefiro", L"Anchor", L"bar",
     L"Ancoragem: bar = centro do topo do monitor, centrada na barra do Zefiro; mod = posicionamento original do mod (Position/TargetMonitor/Offset*)",
     L"bar | mod"},
    {L"Zefiro", L"BarHeight", L"32", L"Altura da barra do Zefiro, em DIP (multiplicada pela escala do monitor)", L""},
    {L"Zefiro", L"Renderer", L"software",
     L"Desenho do Direct2D: software (sem dispositivo na GPU, ~50 MB a menos) ou hardware (o padrão do mod). Exige reiniciar",
     L"software | hardware"},
    {L"Zefiro", L"PillOffsetY", L"0",
     L"Ajuste vertical fino da pílula dentro da barra, em DIP (positivo = para baixo)", L""},
    {L"Zefiro", L"PillHeight", L"0",
     L"Altura da pílula recolhida em DIP (0 = a do mod). Gravada pela barra do Zefiro conforme a altura e o formato", L""},
};

// Opções do mod cujo padrão no Zefiro é diferente do upstream.
struct ModDefaultOverride {
    const wchar_t* name;  // "Grupo.Chave"
    const wchar_t* value;
};
constexpr ModDefaultOverride kZefiroModOverrides[] = {
    {L"Behavior.ExpandOnHover", L"false"},  // abre só com clique; tirar o mouse fecha
};

const wchar_t* ModDefaultValue(const SettingDefault& d) {
    const std::wstring name = std::wstring(d.group) + L"." + d.key;
    for (const auto& o : kZefiroModOverrides) {
        if (_wcsicmp(o.name, name.c_str()) == 0) return o.value;
    }
    return d.value;
}

std::wstring Lower(std::wstring_view s) {
    std::wstring out(s);
    for (wchar_t& c : out) c = static_cast<wchar_t>(towlower(c));
    return out;
}

std::wstring Trim(std::wstring_view s) {
    size_t b = 0, e = s.size();
    while (b < e && iswspace(s[b])) ++b;
    while (e > b && iswspace(s[e - 1])) --e;
    return std::wstring(s.substr(b, e - b));
}

std::wstring Unquote(std::wstring s) {
    if (s.size() >= 2 && ((s.front() == L'"' && s.back() == L'"') || (s.front() == L'\'' && s.back() == L'\''))) {
        return s.substr(1, s.size() - 2);
    }
    return s;
}

std::wstring KnownFolder(REFKNOWNFOLDERID id) {
    PWSTR path = nullptr;
    std::wstring out;
    if (SUCCEEDED(SHGetKnownFolderPath(id, KF_FLAG_CREATE, nullptr, &path)) && path) {
        out = path;
    }
    CoTaskMemFree(path);
    return out;
}

std::string ToUtf8(std::wstring_view s) {
    if (s.empty()) return {};
    int n = WideCharToMultiByte(CP_UTF8, 0, s.data(), static_cast<int>(s.size()), nullptr, 0, nullptr, nullptr);
    std::string out(n, '\0');
    WideCharToMultiByte(CP_UTF8, 0, s.data(), static_cast<int>(s.size()), out.data(), n, nullptr, nullptr);
    return out;
}

// Lê um arquivo de texto (UTF-8 com ou sem BOM, ou UTF-16 LE com BOM).
bool ReadTextFile(const std::wstring& path, std::wstring* out) {
    HANDLE f = CreateFileW(path.c_str(), GENERIC_READ, FILE_SHARE_READ | FILE_SHARE_WRITE | FILE_SHARE_DELETE,
                           nullptr, OPEN_EXISTING, FILE_ATTRIBUTE_NORMAL, nullptr);
    if (f == INVALID_HANDLE_VALUE) return false;
    LARGE_INTEGER size{};
    GetFileSizeEx(f, &size);
    std::string bytes(static_cast<size_t>(size.QuadPart), '\0');
    DWORD read = 0;
    BOOL ok = bytes.empty() || ReadFile(f, bytes.data(), static_cast<DWORD>(bytes.size()), &read, nullptr);
    CloseHandle(f);
    if (!ok) return false;
    bytes.resize(read);

    if (bytes.size() >= 2 && static_cast<unsigned char>(bytes[0]) == 0xFF && static_cast<unsigned char>(bytes[1]) == 0xFE) {
        out->assign(reinterpret_cast<const wchar_t*>(bytes.data() + 2), (bytes.size() - 2) / sizeof(wchar_t));
        return true;
    }
    size_t skip = (bytes.size() >= 3 && bytes.compare(0, 3, "\xEF\xBB\xBF") == 0) ? 3 : 0;
    int n = MultiByteToWideChar(CP_UTF8, 0, bytes.data() + skip, static_cast<int>(bytes.size() - skip), nullptr, 0);
    out->assign(n, L'\0');
    MultiByteToWideChar(CP_UTF8, 0, bytes.data() + skip, static_cast<int>(bytes.size() - skip), out->data(), n);
    return true;
}

// Grava de forma atômica (arquivo temporário + rename) em UTF-8.
bool WriteTextFile(const std::wstring& path, std::wstring_view text) {
    const std::wstring tmp = path + L".tmp";
    HANDLE f = CreateFileW(tmp.c_str(), GENERIC_WRITE, 0, nullptr, CREATE_ALWAYS, FILE_ATTRIBUTE_NORMAL, nullptr);
    if (f == INVALID_HANDLE_VALUE) return false;
    const std::string bytes = ToUtf8(text);
    DWORD written = 0;
    BOOL ok = WriteFile(f, bytes.data(), static_cast<DWORD>(bytes.size()), &written, nullptr);
    CloseHandle(f);
    if (!ok || written != bytes.size()) {
        DeleteFileW(tmp.c_str());
        return false;
    }
    return MoveFileExW(tmp.c_str(), path.c_str(), MOVEFILE_REPLACE_EXISTING) != FALSE;
}

// Interpreta um .ini simples: [Seção], chave=valor, comentários com ; ou #.
// Chaves devolvidas como "seção.chave" em minúsculas.
std::map<std::wstring, std::wstring> ParseIni(const std::wstring& text) {
    std::map<std::wstring, std::wstring> out;
    std::wstring section;
    size_t pos = 0;
    while (pos <= text.size()) {
        size_t nl = text.find(L'\n', pos);
        if (nl == std::wstring::npos) nl = text.size();
        std::wstring line = Trim(std::wstring_view(text).substr(pos, nl - pos));
        pos = nl + 1;
        if (line.empty() || line[0] == L';' || line[0] == L'#') continue;
        if (line.front() == L'[' && line.back() == L']') {
            section = Lower(Trim(std::wstring_view(line).substr(1, line.size() - 2)));
            continue;
        }
        size_t eq = line.find(L'=');
        if (eq == std::wstring::npos) continue;
        std::wstring key = Lower(Trim(std::wstring_view(line).substr(0, eq)));
        std::wstring value = Unquote(Trim(std::wstring_view(line).substr(eq + 1)));
        out[section.empty() ? key : section + L"." + key] = value;
    }
    return out;
}

// ── Estado ──────────────────────────────────────────────────────────────────

std::mutex g_settingsMutex;
std::map<std::wstring, std::wstring> g_settings;  // "grupo.chave" (minúsculas) -> valor

std::mutex g_valuesMutex;
bool g_valuesLoaded = false;
FILETIME g_valuesWriteTime{};  // para reler se outro processo gravar (ex.: ProgressPercent)
ULONGLONG g_valuesNextCheck = 0;  // próxima checagem do arquivo; 0 força reler
std::map<std::wstring, std::wstring> g_values;  // nome original -> valor

std::mutex g_logMutex;
HANDLE g_logFile = INVALID_HANDLE_VALUE;
int g_logDay = -1;

const std::wstring& LogDir() {
    static const std::wstring dir = [] {
        std::wstring base = KnownFolder(FOLDERID_LocalAppData);
        std::wstring d = base + L"\\zefiro";
        CreateDirectoryW(d.c_str(), nullptr);
        d += L"\\logs";
        CreateDirectoryW(d.c_str(), nullptr);
        return d;
    }();
    return dir;
}

const std::wstring& StatePath() {
    static const std::wstring p = zefiro::ConfigDir() + L"\\island-state.ini";
    return p;
}

// Apaga logs com mais de 7 dias.
void PruneOldLogs() {
    WIN32_FIND_DATAW fd;
    HANDLE h = FindFirstFileW((LogDir() + L"\\island-*.log").c_str(), &fd);
    if (h == INVALID_HANDLE_VALUE) return;
    FILETIME nowFt;
    GetSystemTimeAsFileTime(&nowFt);
    const ULONGLONG now = (static_cast<ULONGLONG>(nowFt.dwHighDateTime) << 32) | nowFt.dwLowDateTime;
    constexpr ULONGLONG kWeek = 7ull * 24 * 3600 * 10'000'000ull;
    do {
        const ULONGLONG t = (static_cast<ULONGLONG>(fd.ftLastWriteTime.dwHighDateTime) << 32) | fd.ftLastWriteTime.dwLowDateTime;
        if (now > t && now - t > kWeek) {
            DeleteFileW((LogDir() + L"\\" + fd.cFileName).c_str());
        }
    } while (FindNextFileW(h, &fd));
    FindClose(h);
}

void EnsureLogFile(const SYSTEMTIME& st) {
    if (g_logFile != INVALID_HANDLE_VALUE && g_logDay == st.wDay) return;
    if (g_logFile != INVALID_HANDLE_VALUE) CloseHandle(g_logFile);
    if (g_logDay == -1) PruneOldLogs();
    wchar_t name[64];
    swprintf_s(name, L"\\island-%04u%02u%02u.log", st.wYear, st.wMonth, st.wDay);
    g_logFile = CreateFileW((LogDir() + name).c_str(), FILE_APPEND_DATA, FILE_SHARE_READ | FILE_SHARE_WRITE | FILE_SHARE_DELETE,
                            nullptr, OPEN_ALWAYS, FILE_ATTRIBUTE_NORMAL, nullptr);
    g_logDay = st.wDay;
}

// O mod lê alguns valores em laço (ProgressPercent a cada 0,25 s), escritos por
// outro programa; no Windhawk cada leitura vai ao armazenamento. Aqui relemos o
// arquivo só quando a data de gravação muda.
void LoadValuesLocked() {
    // O laço de desenho lê valores a cada passada (60x/s parado); checar o
    // arquivo a cada 250 ms basta para o ProgressPercent (lido a cada 250 ms).
    const ULONGLONG nowTick = GetTickCount64();
    if (g_valuesLoaded && nowTick < g_valuesNextCheck) return;
    g_valuesNextCheck = nowTick + 250;
    WIN32_FILE_ATTRIBUTE_DATA attr{};
    const bool exists = GetFileAttributesExW(StatePath().c_str(), GetFileExInfoStandard, &attr) != FALSE;
    if (g_valuesLoaded && (!exists || CompareFileTime(&attr.ftLastWriteTime, &g_valuesWriteTime) == 0)) return;
    g_valuesLoaded = true;
    if (!exists) return;
    g_valuesWriteTime = attr.ftLastWriteTime;
    g_values.clear();
    std::wstring text;
    if (!ReadTextFile(StatePath(), &text)) return;
    // Os nomes dos valores diferenciam maiúsculas; ParseIni não, então
    // reconstruímos com o nome original lendo as linhas de novo.
    size_t pos = 0;
    while (pos <= text.size()) {
        size_t nl = text.find(L'\n', pos);
        if (nl == std::wstring::npos) nl = text.size();
        std::wstring line = Trim(std::wstring_view(text).substr(pos, nl - pos));
        pos = nl + 1;
        if (line.empty() || line[0] == L';' || line[0] == L'[') continue;
        size_t eq = line.find(L'=');
        if (eq == std::wstring::npos) continue;
        g_values[Trim(std::wstring_view(line).substr(0, eq))] = Trim(std::wstring_view(line).substr(eq + 1));
    }
}

std::wstring FormatName(PCWSTR format, va_list args) {
    wchar_t buf[512];
    if (_vsnwprintf_s(buf, _TRUNCATE, format, args) < 0) {
        return format;
    }
    return buf;
}

int ParseInt(const std::wstring& v) {
    const std::wstring l = Lower(v);
    if (l == L"true") return 1;
    if (l == L"false") return 0;
    return static_cast<int>(wcstol(v.c_str(), nullptr, 10));
}

std::wstring DefaultIniText() {
    std::wstring t;
    t += L"; Zefiro — configuração da ilha de mídia (zefiro-island.exe)\r\n";
    t += L"; Valores padrão do Dynamic Island for Windows (MIT, devcode90), mais a seção [Zefiro].\r\n";
    t += L"; Booleanos: true/false. Alterações são aplicadas na hora, sem reiniciar.\r\n";
    std::wstring group;
    auto emit = [&](const SettingDefault& d) {
        if (group != d.group) {
            group = d.group;
            t += L"\r\n[" + group + L"]\r\n";
        }
        t += L"; ";
        t += d.label;
        if (*d.options) {
            t += L" — opções: ";
            t += d.options;
        }
        t += L"\r\n";
        t += d.key;
        t += L"=";
        t += ModDefaultValue(d);
        t += L"\r\n";
    };
    for (const auto& d : kZefiroDefaults) emit(d);
    for (const auto& d : kModDefaults) emit(d);
    return t;
}

}  // namespace

// ── API do Windhawk ─────────────────────────────────────────────────────────

int Wh_GetIntSetting(PCWSTR valueName, ...) {
    va_list args;
    va_start(args, valueName);
    const std::wstring name = Lower(FormatName(valueName, args));
    va_end(args);
    std::lock_guard lock(g_settingsMutex);
    auto it = g_settings.find(name);
    return it == g_settings.end() ? 0 : ParseInt(it->second);
}

PCWSTR Wh_GetStringSetting(PCWSTR valueName, ...) {
    va_list args;
    va_start(args, valueName);
    const std::wstring name = Lower(FormatName(valueName, args));
    va_end(args);
    std::wstring value;
    {
        std::lock_guard lock(g_settingsMutex);
        auto it = g_settings.find(name);
        if (it != g_settings.end()) value = it->second;
    }
    auto* copy = new wchar_t[value.size() + 1];
    wmemcpy(copy, value.c_str(), value.size() + 1);
    return copy;
}

void Wh_FreeStringSetting(PCWSTR string) {
    delete[] string;
}

int Wh_GetIntValue(PCWSTR valueName, int defaultValue) {
    std::lock_guard lock(g_valuesMutex);
    LoadValuesLocked();
    auto it = g_values.find(valueName);
    return it == g_values.end() ? defaultValue : ParseInt(it->second);
}

BOOL Wh_SetIntValue(PCWSTR valueName, int value) {
    std::lock_guard lock(g_valuesMutex);
    LoadValuesLocked();
    g_values[valueName] = std::to_wstring(value);
    std::wstring text = L"; Estado salvo pela ilha (equivale aos valores do mod no Windhawk). Pode apagar.\r\n[Values]\r\n";
    for (const auto& [k, v] : g_values) text += k + L"=" + v + L"\r\n";
    if (!WriteTextFile(StatePath(), text)) {
        Wh_Log(L"Falha ao gravar %s (erro %lu)", StatePath().c_str(), GetLastError());
        return FALSE;
    }
    WIN32_FILE_ATTRIBUTE_DATA attr{};
    if (GetFileAttributesExW(StatePath().c_str(), GetFileExInfoStandard, &attr)) {
        g_valuesWriteTime = attr.ftLastWriteTime;
    }
    return TRUE;
}

BOOL Wh_SetFunctionHook(void* targetFunction, void*, void**) {
    Wh_Log(L"Wh_SetFunctionHook(%p) ignorado: o zefiro-island roda fora do Windhawk e não instala hooks.",
           targetFunction);
    return FALSE;
}

void ZefiroLog(const char* function, int line, PCWSTR format, ...) {
    wchar_t msg[2048];
    va_list args;
    va_start(args, format);
    if (_vsnwprintf_s(msg, _TRUNCATE, format, args) < 0) {
        wcscpy_s(msg, L"(mensagem truncada)");
    }
    va_end(args);

    SYSTEMTIME st;
    GetLocalTime(&st);
    wchar_t lineBuf[2304];
    swprintf_s(lineBuf, L"%04u-%02u-%02u %02u:%02u:%02u.%03u [%5lu] %S:%d: %s\r\n", st.wYear, st.wMonth, st.wDay,
               st.wHour, st.wMinute, st.wSecond, st.wMilliseconds, GetCurrentThreadId(), function, line, msg);
    OutputDebugStringW(lineBuf);

    const std::string bytes = ToUtf8(lineBuf);
    std::lock_guard lock(g_logMutex);
    EnsureLogFile(st);
    if (g_logFile != INVALID_HANDLE_VALUE) {
        DWORD written = 0;
        WriteFile(g_logFile, bytes.data(), static_cast<DWORD>(bytes.size()), &written, nullptr);
    }
}

// ── Funções do Zefiro ───────────────────────────────────────────────────────

namespace zefiro {

const std::wstring& ConfigDir() {
    static const std::wstring dir = [] {
        std::wstring d = KnownFolder(FOLDERID_RoamingAppData) + L"\\zefiro";
        CreateDirectoryW(d.c_str(), nullptr);
        return d;
    }();
    return dir;
}

const std::wstring& SettingsPath() {
    static const std::wstring p = ConfigDir() + L"\\island.ini";
    return p;
}

bool LoadSettingsFile() {
    std::map<std::wstring, std::wstring> next;
    for (const auto& d : kZefiroDefaults) next[Lower(std::wstring(d.group) + L"." + d.key)] = d.value;
    for (const auto& d : kModDefaults) {
        std::wstring name = *d.group ? std::wstring(d.group) + L"." + d.key : std::wstring(d.key);
        next[Lower(name)] = ModDefaultValue(d);
    }

    std::wstring text;
    bool ok = true;
    if (ReadTextFile(SettingsPath(), &text)) {
        for (auto& [k, v] : ParseIni(text)) {
            if (next.find(k) == next.end()) {
                Wh_Log(L"island.ini: chave desconhecida '%s' (ignorada)", k.c_str());
                continue;
            }
            next[k] = v;
        }
    } else if (GetFileAttributesW(SettingsPath().c_str()) == INVALID_FILE_ATTRIBUTES) {
        if (WriteTextFile(SettingsPath(), DefaultIniText())) {
            Wh_Log(L"Criado %s com os valores padrão", SettingsPath().c_str());
        } else {
            Wh_Log(L"Falha ao criar %s (erro %lu)", SettingsPath().c_str(), GetLastError());
        }
    } else {
        Wh_Log(L"Falha ao ler %s (erro %lu); usando os valores anteriores", SettingsPath().c_str(), GetLastError());
        ok = false;
    }

    if (ok) {
        std::lock_guard lock(g_settingsMutex);
        g_settings = std::move(next);
    }
    {
        // A barra grava o tema no island.ini e no island-state.ini juntos; relê o segundo já.
        std::lock_guard valuesLock(g_valuesMutex);
        g_valuesNextCheck = 0;
    }
    return ok;
}

int GetZefiroInt(PCWSTR key, int defaultValue) {
    std::lock_guard lock(g_settingsMutex);
    auto it = g_settings.find(Lower(std::wstring(L"zefiro.") + key));
    return it == g_settings.end() ? defaultValue : ParseInt(it->second);
}

std::wstring GetZefiroString(PCWSTR key, PCWSTR defaultValue) {
    std::lock_guard lock(g_settingsMutex);
    auto it = g_settings.find(Lower(std::wstring(L"zefiro.") + key));
    return it == g_settings.end() ? std::wstring(defaultValue) : it->second;
}

bool BarAnchorRect(HMONITOR monitor, const MONITORINFO& info, float sizeScale, bool flushTop, float renderPadY,
                   RECT* rect) {
    if (_wcsicmp(GetZefiroString(L"Anchor", L"bar").c_str(), L"bar") != 0) return false;

    // Constantes do mod: ComputeStageLayout põe a janela em work.top + 8 e a
    // pílula renderPadY abaixo disso; a pílula recolhida tem 44 px de conteúdo
    // (kCollapsedPillHeight), multiplicados por sizeScale.
    constexpr float kModTopGap = 8.0f;
    constexpr float kCollapsedPillHeight = 44.0f;

    UINT dpiX = 96, dpiY = 96;
    GetDpiForMonitor(monitor, MDT_EFFECTIVE_DPI, &dpiX, &dpiY);
    const float dpi = static_cast<float>(dpiY) / 96.0f;
    const float bar = static_cast<float>(GetZefiroInt(L"BarHeight", 32)) * dpi;
    const float pill = kCollapsedPillHeight * CollapsedScale(kCollapsedPillHeight, sizeScale);
    float pillTop = std::max(0.0f, (bar - pill) * 0.5f) + static_cast<float>(GetZefiroInt(L"PillOffsetY", 0)) * dpi;

    *rect = info.rcWork;
    rect->left = info.rcMonitor.left;
    rect->right = info.rcMonitor.right;
    const float top = flushTop ? pillTop : pillTop - kModTopGap - renderPadY;
    rect->top = info.rcMonitor.top + static_cast<LONG>(std::lround(top));
    return true;
}

float CollapsedScale(float designHeight, float sizeScale) {
    const int pill = GetZefiroInt(L"PillHeight", 0);
    if (pill <= 0 || designHeight <= 0.0f) return sizeScale;
    UINT dpiX = 96, dpiY = 96;
    GetDpiForMonitor(MonitorFromPoint(POINT{0, 0}, MONITOR_DEFAULTTOPRIMARY), MDT_EFFECTIVE_DPI, &dpiX, &dpiY);
    return static_cast<float>(pill) * (static_cast<float>(dpiY) / 96.0f) / designHeight;
}

bool UseSoftwareRenderer() {
    const bool software = _wcsicmp(GetZefiroString(L"Renderer", L"software").c_str(), L"hardware") != 0;
    Wh_Log(L"Direct2D: desenho por %s", software ? L"software" : L"hardware (GPU)");
    return software;
}

void NoteFramePresented() {
    static std::once_flag once;
    std::call_once(once, [] {
        FILETIME created, exited, kernel, user, now;
        GetProcessTimes(GetCurrentProcess(), &created, &exited, &kernel, &user);
        GetSystemTimeAsFileTime(&now);
        const auto ticks = [](const FILETIME& f) {
            return (static_cast<ULONGLONG>(f.dwHighDateTime) << 32) | f.dwLowDateTime;
        };
        const double ms = static_cast<double>(ticks(now) - ticks(created)) / 10'000.0;
        Wh_Log(L"Primeiro quadro apresentado: %.0f ms desde a criação do processo", ms);
    });
}

void CloseLog() {
    std::lock_guard lock(g_logMutex);
    if (g_logFile != INVALID_HANDLE_VALUE) {
        CloseHandle(g_logFile);
        g_logFile = INVALID_HANDLE_VALUE;
    }
}

}  // namespace zefiro
