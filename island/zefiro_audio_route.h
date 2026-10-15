// Saída de som por fonte de mídia: o submenu "Saída de som" do clique direito
// da ilha, que troca o dispositivo de saída do app que está tocando (Spotify,
// Brave...) como a página "Volume e preferências de dispositivo do app" do
// Windows faz.
//
// Usa a interface não documentada IAudioPolicyConfig (AudioSes.dll), a mesma
// do EarTrumpet. A escolha é gravada pelo Windows por executável/pacote e vale
// para todos os processos do app, inclusive depois de reabri-lo. Se a
// interface não existir nesta versão do Windows, o submenu não aparece.
//
// Só é chamado pela thread da janela da ilha (ShowContextMenu), nos ganchos
// marcados com // ZEFIRO: no dynamic-island-for-windows.wh.cpp.
#pragma once

#include <windows.h>

#include <string>

namespace zefiro::audio_route {

// Ids do submenu. Ficam bem acima dos ids do mod (o maior é a lista de
// fontes de mídia, a partir de 100).
constexpr UINT kMenuIdBase = 0x7A00;  // +0 = padrão do sistema, +1.. = dispositivos
constexpr UINT kMenuIdLast = kMenuIdBase + 0xFF;

// Acrescenta o submenu "Saída de som" a `menu` para a fonte de mídia atual
// (`sourceAumid` é o AUMID do SMTC; `sourceName`, o nome que a ilha mostra).
// Não acrescenta nada se não houver fonte ou se a API não existir.
void AppendOutputSubmenu(HMENU menu, const std::wstring& sourceAumid, const std::wstring& sourceName);

// Aplica o comando escolhido no menu. Devolve true se `cmd` era do submenu.
bool HandleMenuCommand(UINT cmd);

}  // namespace zefiro::audio_route
