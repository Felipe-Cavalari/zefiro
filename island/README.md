# zefiro-island

A ilha de mídia do Zefiro, por enquanto um **fork do [Dynamic Island for Windows](https://github.com/devcode90/Dynamic-Island-for-Windows)** (MIT, © 2026 devcode90; licença em [`LICENSE`](LICENSE)) compilado como executável independente, sem o Windhawk. Por que um fork em C++ e não Rust: [ADR-0006](../docs/adr/0006-ilha-de-midia-fork.md).

## Arquivos

| Arquivo | O que é |
|---|---|
| `dynamic-island-for-windows.wh.cpp` | O mod do upstream (v1.3.1, commit `e6cfedb`). Mudanças do Zefiro só dentro de `#ifdef ZEFIRO_ISLAND` e marcadas com `// ZEFIRO:`. Sem `ZEFIRO_ISLAND` o arquivo continua sendo um mod válido do Windhawk. |
| `wh_shim.h/.cpp` | Substitui a API do Windhawk (log, configurações, valores, hooks). |
| `zefiro_island_main.cpp` | `wWinMain`: inclui o shim e o mod numa unidade só e chama `WhTool_ModInit` / `WhTool_ModSettingsChanged` / `WhTool_ModUninit`. |
| `gen-settings.ps1` | Gera os padrões do `island.ini` a partir do bloco `// ==WindhawkModSettings==` do mod. |
| `build.ps1` | Compila com o MSVC (VS 2022 Build Tools). |
| `first-frame.ps1` | Mede o tempo até o primeiro quadro em N aberturas. |

## Compilar e rodar

```powershell
./island/build.ps1                 # gera island/build/zefiro-island.exe (~20 s)
./island/build/zefiro-island.exe   # abre a ilha (uma instância só)
./island/build/zefiro-island.exe --stop   # fecha a instância em execução
```

Requisitos: VS 2022 Build Tools com "Desenvolvimento para desktop com C++" e o Windows SDK 10.0.26100 (que traz os cabeçalhos C++/WinRT).

## Configuração

- **`%APPDATA%\zefiro\island.ini`**: criado na primeira execução, com todas as opções do mod (mesmos padrões, mesmos nomes de grupo e chave do Windhawk) e a seção `[Zefiro]`. Mudanças são aplicadas na hora (o arquivo é observado), exceto `Renderer`, que exige reiniciar.
- **`%APPDATA%\zefiro\island-state.ini`**: o que o mod guardaria com `Wh_SetIntValue` (tema escolhido no menu, letra fixada etc.). Outro programa pode escrever `ProgressPercent=0..100` aqui para acender o anel de progresso; o valor é relido quando o arquivo muda.
- **`%LOCALAPPDATA%\zefiro\logs\island-AAAAMMDD.log`**: log (`Wh_Log`), um arquivo por dia, apagados depois de 7 dias.
- O cache de letras e a bandeja de arquivos continuam em `%LOCALAPPDATA%\DynamicIslandForWindows\`, como no mod.

Padrões do Zefiro diferentes do mod: `Behavior.ExpandOnHover=false` (a ilha abre só com clique; tirar o mouse fecha). A lista fica em `kZefiroModOverrides`, no `wh_shim.cpp`.

Seção `[Zefiro]`:

| Chave | Padrão | O que faz |
|---|---|---|
| `Anchor` | `bar` | `bar`: centro do topo do monitor (de `TargetMonitor`), com a pílula recolhida centrada na faixa da barra; usa o monitor inteiro, não a área de trabalho, para não descer quando a AppBar do Zefiro reservar o topo. `mod`: posicionamento original (`Position`, `OffsetX/Y`…). |
| `BarHeight` | `32` | Altura da barra em DIP. |
| `PillOffsetY` | `0` | Ajuste fino vertical em DIP. |
| `Renderer` | `software` | `software`: Direct2D sem dispositivo na GPU (≈ 40 MB de memória privada a menos). `hardware`: o padrão do mod. |

## Acompanhar o upstream

```powershell
git diff <commit-da-importação> -- island/dynamic-island-for-windows.wh.cpp   # só as mudanças do Zefiro
```

Para atualizar: substituir o arquivo pelo novo do upstream, reaplicar os blocos `// ZEFIRO:` (são 3: primeiro quadro, desenho por software, ancoragem em `GetAnchorWorkRect`), rodar `build.ps1` e atualizar a versão/commit aqui, no `zefiro_island_main.cpp` e no `THIRD_PARTY_NOTICES.md`.
