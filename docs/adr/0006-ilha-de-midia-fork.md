# ADR-0006: Ilha de mídia como fork do Dynamic Island, em processo separado

- **Status:** aceito
- **Data:** 2026-10-15
- **Branch:** `claude/spike-s6-island-fork-6f5cda` (código em [`island/`](../../island/))

## Contexto

A ilha de mídia é o centro da barra da Fase 1 (PLANNING §3.6). O spike S6 em Rust (`spike/s6-island`) cobre a janela, as molas no compositor, o visualizador e o clique na área transparente, mas está longe do conjunto de recursos que o usuário já usa no [Dynamic Island for Windows](https://github.com/devcode90/Dynamic-Island-for-Windows) (mod do Windhawk, MIT, um arquivo C++ de ~23,7 mil linhas): troca de fonte entre players, letras sincronizadas, notificações, indicadores de câmera e microfone, Bluetooth, volume e brilho, monitor de hardware, calendário, clima, pesquisa rápida e bandeja de arquivos.

Pergunta: como ter essa ilha **agora**, rodando no centro da barra do Zefiro, sem depender do Windhawk, e sem travar o porte para Rust?

## Opções consideradas

1. **Portar para Rust agora**, dentro da barra. Uma base só e o pipeline do ADR-0001 (compositor, WARP, molas nativas). Contra: são ~23,7 mil linhas com uma dúzia de integrações do sistema; leva meses, e a barra em si ainda não existe.
2. **Continuar usando o mod no Windhawk.** Zero trabalho. Contra: depende de um terceiro instalado e rodando; não dá para ancorar na barra, versionar junto nem mudar padrões; configuração presa à UI do Windhawk.
3. **Fork do mod como executável independente (`zefiro-island.exe`)**, em processo separado da barra, com uma camada que substitui a API do Windhawk. A favor: todos os recursos hoje, mudanças mínimas no arquivo do upstream (fácil acompanhar), uma falha na ilha não derruba a barra. Contra: C++ ao lado do Rust, dois processos, e o modelo de desenho do mod (quadro a quadro na CPU) diverge do ADR-0001.

## Números medidos

Ambiente: Windows 11 Pro 26200, RTX 5070, 16 processadores lógicos, `DISPLAY2` primário 1920×1080 a 165 Hz, 2026-10-15. YASB fechado. Discord com o microfone aberto o tempo todo (o ponto laranja da ilha ficou aceso em todas as medições, o que faz a ilha redesenhar a 60 Hz). Ferramentas: `scripts/perf.ps1 -Process zefiro-island -WarmupSeconds 60 -SampleSeconds 180` (CPU em % da máquina inteira) e `island/first-frame.ps1 -Runs 5` (tempo medido pela própria ilha, da criação do processo ao primeiro `UpdateLayeredWindowIndirect`).

| Métrica | Software (padrão) | GPU (padrão do mod) | Meta (ADR-0001 / B0) |
|---|---|---|---|
| Memória privada, mídia tocando (MB) | **18,3** | 63,0 | ≤ 25 |
| Working set, mídia tocando (MB) | 52,1 | 70,6 | acompanhar |
| CPU, mídia tocando (%) | 1,0 | 0,51 | ≤ 0,3 |
| Memória privada, repouso (MB) | **18,6** (16,4 sem o ponto) | — | ≤ 25 |
| Working set, repouso (MB) | 55,4 (53,2 sem o ponto) | — | acompanhar |
| CPU, repouso, com o ponto do microfone (%) | 1,0 | — | ≤ 0,2 |
| CPU, repouso, sem o ponto do microfone (%) | **0,0** | — | ≤ 0,2 |
| Primeiro quadro (ms, 5 aberturas) | **70–112** (mediana 98) | 217–244 (mediana 235) | ≤ 250 |

Repouso = Spotify e Brave pausados, ilha recolhida, sem interação. As medições com mídia tocando rodaram em builds intermediários deste ramo (software no `2058955`; GPU com a releitura do `island-state.ini` já aplicada); as de repouso, no `0847424`. Entre eles só mudaram o padrão de expansão (clique) e a releitura do `island-state.ini`, nada no desenho. Os números batem: 1,0 % de CPU nos dois cenários com o ponto aceso.

**De onde vem a CPU:** sem o ponto do microfone, a ilha parada fica em 0,0 %. Com ele, 1,0 %. Todo o custo é o redesenho a 60 Hz que o ponto pulsante (e, tocando, as barras do áudio) força: no modo software, cada quadro rasteriza a pílula na CPU.

**Depois de usar:** numa medição descartada (o Spotify voltou a tocar no meio, depois de expandir a ilha e trocar de fonte), a memória privada subiu para **~38 MB** e ficou estável (sem crescer em 5 min, 620 handles, 34 threads). São caches do Direct2D/DirectWrite e imagens do painel expandido, que o mod não libera ao recolher. Acima do orçamento de 25 MB: ver Riscos.

"Software" é o `ID2D1DCRenderTarget` com `D2D1_RENDER_TARGET_TYPE_SOFTWARE`; "GPU" é o `DEFAULT` do mod, que cria um dispositivo D3D11 no driver. É o mesmo custo de ~45 MB de memória privada que o ADR-0001 encontrou e resolveu com WARP.

Para comparar: o YASB (B0) usa 105,7 MB privada / 175,5 MB working set / 0,19 % de CPU; o S1 com WARP, 10,7 MB / 35 MB / ~0 %.

## Decisão

**Opção 3: fork em processo separado**, com:
- desenho do Direct2D **por software por padrão** (`[Zefiro] Renderer=software`), que cabe no orçamento de memória e ainda abre mais rápido;
- ancoragem no **centro do topo do monitor principal**, pela faixa da barra de 32 DIP (`[Zefiro] Anchor=bar`), usando o monitor inteiro e não a área de trabalho que a AppBar do Zefiro vai encolher. A pílula recolhida do mod (44 px) é mais alta que a barra (32 DIP); por isso ela encosta no topo da tela (validado com o usuário);
- **expansão só com clique** (`ExpandOnHover=false`), a pedido do usuário.

Por que não Rust agora: a barra (Fase 1) ainda não existe, e o porte completo custaria meses antes de entregar o que o usuário já usa. O fork entrega tudo hoje, isolado num processo, e o porte para Rust passa a ser incremental, guiado por [`docs/island-port-notes.md`](../island-port-notes.md).

**Metas não cumpridas (decisão consciente):**
- **CPU** com conteúdo animado: 1,0 % no software (0,51 % na GPU) contra 0,3 %, tanto tocando quanto parada com o ponto do microfone aceso. A causa é o desenho do mod: a pílula inteira é redesenhada a 60 Hz enquanto houver conteúdo contínuo (barras do áudio, pontos de privacidade pulsando), e no modo software isso é rasterizado na CPU. Sem conteúdo contínuo, 0,0 %. A barra em Rust resolve isso por construção (molas, barras e pulso no compositor, PLANNING §3.6). Até lá, aceitamos 1 % para economizar ~45 MB. Quem quiser 0 % em repouso desliga `PrivacyDotsMic`.
- **Memória depois de usar** (~38 MB privada) acima de 25 MB; em repouso desde a abertura, 18,6 MB.

## O que mudou no código

Tudo fora do arquivo do mod, exceto três blocos `#ifdef ZEFIRO_ISLAND` marcados com `// ZEFIRO:`. Sem `ZEFIRO_ISLAND`, o arquivo continua sendo um mod válido do Windhawk.

| Onde | O quê |
|---|---|
| `island/wh_shim.h/.cpp` | `Wh_Log` → `%LOCALAPPDATA%\zefiro\logs\island-AAAAMMDD.log` (7 dias). `Wh_GetIntSetting`/`Wh_GetStringSetting` → `%APPDATA%\zefiro\island.ini`, com os padrões **gerados do próprio bloco de configurações do mod** (`gen-settings.ps1`, 100 opções). `Wh_GetIntValue`/`Wh_SetIntValue` → `island-state.ini`, relido quando outro processo o altera. `Wh_SetFunctionHook` → desativado (registra no log). |
| `island/zefiro_island_main.cpp` | `wWinMain`: inclui shim + mod numa unidade só; instância única; `--stop` (evento nomeado, para a barra fechar a ilha); observa o `island.ini` e chama `WhTool_ModSettingsChanged`. Desfaz a macro `small` do SDK, que o MinGW não tem e o mod usa como nome de variável. |
| `GetAnchorWorkRect()` | `// ZEFIRO:` ancoragem na barra. |
| `CreateDCRenderTarget` | `// ZEFIRO:` desenho por software ou GPU. |
| `UpdateLayeredWindowIndirect` | `// ZEFIRO:` mede o primeiro quadro. |
| `island/build.ps1` | MSVC 14.44 + SDK 10.0.26100 (C++/WinRT do SDK), `/std:c++20`, CRT estático, as mesmas bibliotecas do `@compilerOptions` do mod, manifesto igual ao do `windhawk.exe` (PerMonitorV2, Common Controls 6). Compila em ~20 s. |

Compilar com o MSVC exigiu **zero** mudanças no código do mod além disso: o mod já era escrito com cuidado de portabilidade.

## O que não funciona sem o hook do Windhawk

**Nada.** A premissa inicial era que o mod usava um hook no Explorer para as notificações. O código mostra outra coisa: o mod é um *tool mod* do Windhawk, que já roda num `windhawk.exe` próprio. O único `Wh_SetFunctionHook` neutraliza o ponto de entrada desse processo para o mod tomar conta dele. As notificações usam a API WinRT `UserNotificationListener`, que conectou normalmente fora do Windhawk (sem erro no log após a espera de 30 s que o mod impõe na inicialização).

Diferenças que sobram:
- **Configuração:** não há a tela de configurações do Windhawk; edita-se o `island.ini` (aplicado na hora).
- **Anel de progresso:** o mod lê `ProgressPercent` do armazenamento do Windhawk, que algum programa externo precisa escrever. Aqui, esse programa escreve no `island-state.ini`.
- O mod chama a si mesmo de "Windhawk" no nome da classe da janela (`Windhawk.DynamicIslandForWindows`) e guarda cache de letras e bandeja em `%LOCALAPPDATA%\DynamicIslandForWindows\`. Mantido assim, para não divergir do upstream.

## Mídia: sessões pausadas

O mod já faz o que o Zefiro quer, sem código novo: `CollectMediaSources` lista **todas** as sessões do SMTC, tocando ou pausadas, numa ordem estável, e a ilha segue uma sessão pausada quando nada toca. A barra de troca de fonte aparece com 2 ou mais fontes (até 3 posições, ou 2 + "+N"), e o submenu "fonte" do clique direito lista todas. **Validado com o usuário:** com o Spotify (app da Store) e um vídeo no Brave, os dois pausados, ambos apareceram na barra de fontes e deu para trocar entre eles.

## Consequências

- `zefiro-island.exe` é um componente da Fase 1: a barra o inicia e o fecha com `--stop`. A ilha não conversa com a barra; os dois se alinham pela configuração (`BarHeight`).
- O porte para Rust vira trabalho por partes, guiado pelas notas de porte: primeiro a pílula e as molas (no compositor), depois mídia/SMTC e visualizador, depois os módulos.
- **Atualizar o upstream:** trocar o arquivo, reaplicar os três blocos `// ZEFIRO:` e compilar (ver `island/README.md`).
- **Recurso futuro: saída de som por fonte.** Na validação, Spotify e Brave tocavam em saídas diferentes (preferência por app guardada pelo Windows) e não havia como trocar pela ilha. Plano: um submenu "Saída de som" no clique direito, aplicado à fonte atual. A ilha acha o processo da fonte pelas sessões de áudio (`IAudioSessionManager2`; o navegador tem um processo de áudio separado) e grava a saída com a `IAudioPolicyConfig` não documentada (a mesma do EarTrumpet). Código num arquivo próprio (`island/zefiro_audio_route.cpp`) com um gancho `// ZEFIRO:` no menu. Na barra em Rust, entra no painel de mídia.

### Riscos

- **CPU acima da meta** (ver Decisão). Mitigações possíveis sem divergir muito do upstream: ponto de privacidade fixo, sem pulso (zera o repouso), ou limitar o redesenho contínuo a 30 Hz no modo software. Não feito, para manter o fork mínimo.
- **Memória sobe com o uso** (~38 MB privada depois de expandir e trocar de fonte) e não volta ao recolher. Estável, não é vazamento, mas passa do orçamento. Investigar se o desenhador pode liberar caches ao recolher, ou aceitar até o porte.
- **Dois processos sempre no topo** (barra e ilha): a ordem entre eles pode alternar. A ilha precisa ficar acima da barra; verificar quando a barra existir.
- **Pílula mais alta que a barra** (44 px × 32 DIP): a parte de baixo cobre 12 px de janelas maximizadas. Alternativas registradas: escala menor (~0,64) ou pílula abaixo da barra.
- **Atalhos globais:** Ctrl+Alt+D (esconder/mostrar) e Ctrl+Alt+Espaço (pesquisa rápida) podem colidir com outros apps. Nesta máquina, Ctrl+Alt+Espaço já estava em uso (erro 1409 no log).
- **APIs não documentadas:** `SetWindowCompositionAttribute` (desfoque/acrílico, desligado por padrão) e o registro `CapabilityAccessManager\ConsentStore` (pontos de privacidade) podem mudar com atualizações do Windows.
- **Rede:** clima (wttr.in), letras (LRCLIB) e pesquisa (Wiktionary/Wikipedia) fazem requisições externas. Desligáveis no `island.ini`.
- **Rodar junto com o mod do Windhawk** mostra duas ilhas. O usuário deve desativar o mod no Windhawk.
- **Divergência do upstream:** o arquivo muda muito e com frequência; quanto mais blocos `// ZEFIRO:`, mais caro cada atualização.
