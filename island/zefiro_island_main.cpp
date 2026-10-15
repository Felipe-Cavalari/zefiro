// zefiro-island.exe: o Dynamic Island for Windows (mod do Windhawk) rodando
// como executável independente.
//
// O mod é incluído inteiro nesta unidade de compilação, depois do shim, para
// que o wWinMain enxergue o que ele declara no namespace anônimo
// (g_renderThread) sem mexer no arquivo do upstream.
//
// No Windhawk o mod é um "tool mod": Wh_ModInit só lança um windhawk.exe
// dedicado que chama WhTool_ModInit/WhTool_ModSettingsChanged/WhTool_ModUninit.
// Aqui o wWinMain chama esses três diretamente; o código de lançamento do
// Windhawk continua compilado, mas nunca é chamado.
#include "wh_shim.h"
#include "zefiro_audio_route.h"  // submenu "Saída de som", chamado pelos ganchos // ZEFIRO: do mod

// O SDK do MSVC define `small` como `char` (rpcndr.h); o MinGW não, e o mod
// usa `small` como nome de variável. Incluímos o cabeçalho antes e desfazemos
// a macro; o include guard impede que ela volte.
#include <rpc.h>
#include <rpcndr.h>
#undef small

#include "dynamic-island-for-windows.wh.cpp"

namespace {

constexpr wchar_t kInstanceMutex[] = L"Local\\zefiro-island";
constexpr wchar_t kStopEvent[] = L"Local\\zefiro-island-stop";

FILETIME SettingsWriteTime() {
    WIN32_FILE_ATTRIBUTE_DATA data{};
    GetFileAttributesExW(zefiro::SettingsPath().c_str(), GetFileExInfoStandard, &data);
    return data.ftLastWriteTime;
}

bool HasArg(PCWSTR name) {
    int argc = 0;
    LPWSTR* argv = CommandLineToArgvW(GetCommandLineW(), &argc);
    bool found = false;
    for (int i = 1; argv && i < argc; ++i) {
        if (_wcsicmp(argv[i], name) == 0) found = true;
    }
    LocalFree(argv);
    return found;
}

}  // namespace

int WINAPI wWinMain(HINSTANCE, HINSTANCE, PWSTR, int) {
    // --stop: pede para a instância em execução fechar (usado pela barra e pelos scripts).
    if (HasArg(L"--stop")) {
        HANDLE stop = OpenEventW(EVENT_MODIFY_STATE, FALSE, kStopEvent);
        if (!stop) return 1;
        SetEvent(stop);
        CloseHandle(stop);
        return 0;
    }

    HANDLE instance = CreateMutexW(nullptr, TRUE, kInstanceMutex);
    if (!instance || GetLastError() == ERROR_ALREADY_EXISTS) {
        Wh_Log(L"zefiro-island já está rodando; saindo.");
        return 1;
    }
    HANDLE stopEvent = CreateEventW(nullptr, TRUE, FALSE, kStopEvent);

    zefiro::LoadSettingsFile();
    Wh_Log(L"zefiro-island iniciando (Dynamic Island for Windows 1.3.1, upstream e6cfedb). Configuração: %s",
           zefiro::SettingsPath().c_str());

    if (!WhTool_ModInit()) {
        Wh_Log(L"WhTool_ModInit falhou; saindo.");
        StopThreads();
        zefiro::CloseLog();
        return 1;
    }

    // Observa a pasta de configuração: o Windhawk chamaria Wh_ModSettingsChanged
    // quando o usuário salvasse as configurações; aqui é quando o island.ini muda.
    HANDLE change = FindFirstChangeNotificationW(zefiro::ConfigDir().c_str(), FALSE,
                                                 FILE_NOTIFY_CHANGE_LAST_WRITE | FILE_NOTIFY_CHANGE_FILE_NAME);
    FILETIME lastWrite = SettingsWriteTime();

    for (;;) {
        HANDLE handles[3];
        DWORD count = 0;
        handles[count++] = stopEvent;
        handles[count++] = g_renderThread;  // a janela fechou (ex.: encerrada por fora)
        if (change != INVALID_HANDLE_VALUE) handles[count++] = change;

        const DWORD r = MsgWaitForMultipleObjects(count, handles, FALSE, INFINITE, QS_ALLINPUT);
        if (r == WAIT_OBJECT_0) {
            Wh_Log(L"Pedido de parada recebido.");
            break;
        }
        if (r == WAIT_OBJECT_0 + 1) {
            Wh_Log(L"A thread de desenho terminou; saindo.");
            break;
        }
        if (r == WAIT_OBJECT_0 + 2) {
            FindNextChangeNotification(change);
            Sleep(150);  // editores gravam em mais de um passo
            const FILETIME now = SettingsWriteTime();
            if (CompareFileTime(&now, &lastWrite) != 0) {
                lastWrite = now;
                if (zefiro::LoadSettingsFile()) {
                    Wh_Log(L"island.ini mudou; aplicando.");
                    WhTool_ModSettingsChanged();
                }
            }
            continue;
        }
        if (r == WAIT_OBJECT_0 + count) {
            MSG msg;
            while (PeekMessageW(&msg, nullptr, 0, 0, PM_REMOVE)) {
                if (msg.message == WM_QUIT) goto done;
                TranslateMessage(&msg);
                DispatchMessageW(&msg);
            }
            continue;
        }
        Wh_Log(L"MsgWaitForMultipleObjects falhou (%lu); saindo.", GetLastError());
        break;
    }
done:
    if (change != INVALID_HANDLE_VALUE) FindCloseChangeNotification(change);
    WhTool_ModUninit();
    zefiro::CloseLog();
    // Como no Windhawk (Wh_ModUninit), sai sem rodar destrutores estáticos:
    // objetos WinRT globais não podem ser destruídos depois do uninit_apartment.
    ExitProcess(0);
}
