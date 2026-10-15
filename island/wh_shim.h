// Camada de compatibilidade do Zefiro: substitui a API do Windhawk
// (windhawk_api.h) para que o Dynamic Island for Windows rode como um
// executável independente (zefiro-island.exe).
//
// - Wh_Log                     -> %LOCALAPPDATA%\zefiro\logs\island-AAAAMMDD.log
// - Wh_Get*Setting             -> %APPDATA%\zefiro\island.ini (defaults do mod)
// - Wh_GetIntValue/SetIntValue -> %APPDATA%\zefiro\island-state.ini
// - Wh_SetFunctionHook         -> desativado (registra no log e falha)
//
// Precisa vir antes de qualquer outro include: o mod define
// WIN32_LEAN_AND_MEAN e NOMINMAX antes de <windows.h>, e aqui fazemos o mesmo.
#pragma once

#ifndef WIN32_LEAN_AND_MEAN
#define WIN32_LEAN_AND_MEAN
#endif
#ifndef NOMINMAX
#define NOMINMAX
#endif
#ifndef UNICODE
#define UNICODE
#endif
#ifndef _UNICODE
#define _UNICODE
#endif

#include <windows.h>

#include <string>

// O Windhawk compila os mods com o clang do MinGW, que tem estes nomes.
#ifndef WINBOOL
typedef BOOL WINBOOL;
#endif

// O Windhawk define o id do mod (do cabeçalho // @id) na linha de comando.
#ifndef WH_MOD_ID
#define WH_MOD_ID L"dynamic-island-for-windows"
#endif

// Configurações (island.ini). Como no Windhawk, o nome é "Grupo.Chave" e
// aceita argumentos no estilo printf. Booleanos valem 1/0.
int Wh_GetIntSetting(PCWSTR valueName, ...);
PCWSTR Wh_GetStringSetting(PCWSTR valueName, ...);
void Wh_FreeStringSetting(PCWSTR string);

// Valores persistentes do mod (island-state.ini).
int Wh_GetIntValue(PCWSTR valueName, int defaultValue);
BOOL Wh_SetIntValue(PCWSTR valueName, int value);

// Hooks de funções não existem fora do Windhawk.
BOOL Wh_SetFunctionHook(void* targetFunction, void* hookFunction, void** originalFunction);

// Log. Mesmo formato do Windhawk: printf com strings largas (%s = wchar_t*).
void ZefiroLog(const char* function, int line, PCWSTR format, ...);
#define Wh_Log(format, ...) ZefiroLog(__FUNCTION__, __LINE__, format __VA_OPT__(, ) __VA_ARGS__)

// ── Usado pelo wWinMain do Zefiro ───────────────────────────────────────────

namespace zefiro {

// Diretório de configuração (%APPDATA%\zefiro) e de logs.
const std::wstring& ConfigDir();
const std::wstring& SettingsPath();

// Lê island.ini (cria com os defaults se não existir). Devolve false se o
// arquivo existe mas não pôde ser lido.
bool LoadSettingsFile();

// Configurações próprias do Zefiro, seção [Zefiro] do island.ini.
int GetZefiroInt(PCWSTR key, int defaultValue);
std::wstring GetZefiroString(PCWSTR key, PCWSTR defaultValue);

void CloseLog();

// Ancoragem na barra ([Zefiro] Anchor=bar). Devolve false se desligada; senão
// preenche *rect com o retângulo que o mod deve usar no lugar da área de
// trabalho do monitor: largura do monitor inteiro (a barra ocupa tudo) e um
// topo calculado para que a pílula recolhida fique centrada na barra.
//   sizeScale  - g_settings.sizeScale do mod (já inclui o DPI, se AutoDpiScale)
//   flushTop   - estilos que penduram a pílula na borda (notch / border-merged)
//   renderPadY - kRenderPadY do mod (margem transparente acima da pílula)
bool BarAnchorRect(HMONITOR monitor, const MONITORINFO& info, float sizeScale, bool flushTop, float renderPadY,
                   RECT* rect);

// [Zefiro] Renderer=software (padrão) ou hardware. Lido quando o desenhador é
// criado; mudar exige reiniciar a ilha.
bool UseSoftwareRenderer();

// Escala da pílula recolhida. Com [Zefiro] PillHeight > 0, a pílula de
// `designHeight` px de desenho (44 na mídia, 36 na ociosa) passa a ter
// PillHeight DIP; senão devolve `sizeScale` (comportamento do mod).
float CollapsedScale(float designHeight, float sizeScale);

// Chamado pelo mod a cada quadro apresentado (marcado com // ZEFIRO:). No
// primeiro, registra no log o tempo desde a criação do processo.
void NoteFramePresented();

}  // namespace zefiro
