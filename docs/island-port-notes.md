# Notas de porte: Dynamic Island for Windows → barra em Rust

Material de referência para portar a ilha para a barra do Zefiro (Fase 1, módulo `island`). Tudo aqui foi lido do código em [`island/dynamic-island-for-windows.wh.cpp`](../island/dynamic-island-for-windows.wh.cpp) (v1.3.1, upstream `e6cfedb`, MIT, © 2026 devcode90; ver [`THIRD_PARTY_NOTICES.md`](../THIRD_PARTY_NOTICES.md)). Os números de linha citados são desse arquivo e mudam com o upstream; procure pelo nome da função ou constante.

Unidades: "px de conteúdo" são pixels antes da escala; tudo é multiplicado por `sizeScale` (configuração `SizeScale` × DPI do monitor principal, se `AutoDpiScale`).

## 1. Arquitetura em uma página

- **Um processo, várias threads**, cada uma com um laço próprio e `g_stopEvent` para parar: desenho (dona da janela), mídia (SMTC, laço de consulta), áudio (WASAPI loopback), clima (wttr.in), letras (LRCLIB), teclado e mouse (hooks de baixo nível em threads próprias), notificações (`UserNotificationListener`) e Bluetooth (`DeviceWatcher`). Inicialização em `StartThreads()`.
- **Estado compartilhado:** `g_state` (`SharedState`) protegido por `g_stateMutex`. A thread de desenho copia o estado inteiro (`snapshot = g_state`) uma vez por passada e desenha a partir da cópia. As configurações ficam em `g_settings` com um mutex próprio, que é sempre *folha* (nunca se pega outro mutex com ele).
- **Uma janela só:** `WS_EX_LAYERED | WS_EX_TOPMOST | WS_EX_TOOLWINDOW | WS_EX_NOACTIVATE | WS_EX_TRANSPARENT`, desenhada com Direct2D num `ID2D1DCRenderTarget` sobre uma DIB e apresentada com `UpdateLayeredWindowIndirect` com retângulo sujo (`prcDirty`).
- **"Palco fixo":** a janela tem sempre o tamanho da maior pílula possível (880 × 320 px de conteúdo × escala, mais um enchimento transparente de 28 px dos lados e 22 px em cima e embaixo). A pílula anima *dentro* dela, então a janela nunca muda de tamanho nem se move durante a animação. Exceção: com desfoque/acrílico (`BackdropMaterial`), a janela volta a abraçar a pílula, porque o desfoque do DWM cobre o retângulo inteiro.
- **Clique na área transparente:** `WM_NCHITTEST` devolve `HTTRANSPARENT` fora do retângulo da pílula (publicado pelo desenhador em `g_pillClientRect`). No estado ocioso, `SetClickThrough` liga `WS_EX_TRANSPARENT` na janela inteira. Segurar **Ctrl** sobre a ilha a deixa translúcida e "atravessável" em qualquer estado.

## 2. Molas

### 2.1 Integrador

`SpringValue` (≈ linha 1603): Euler semi-implícito com **subpasso fixo de 0,5 ms**, para o resultado não depender da taxa de quadros:

```
a = -k·(x - alvo) - c·v ;  v += a·dt ;  x += v·dt     (repetido em passos de 0,0005 s)
repouso quando |x - alvo| < 0,01 e |v| < 0,01  → x = alvo, v = 0
```

O `dt` de cada quadro é limitado a [1 ms, 50 ms] e multiplicado por `AnimationSpeed` (0,5× a 2,0×).

### 2.2 Constantes

| Mola | Rigidez k | Amortecimento c | Observação |
|---|---|---|---|
| Largura/altura **crescendo** | 380 | 26 | `alvo > valor` |
| Largura/altura **encolhendo** | 200 | 28 | `alvo < valor` |
| Largura/altura parada | 280 | 24 | |
| "Cutucada" (nudge) vertical | 280 | 24 | começa em **−6 px** sempre que o tipo da pílula principal muda (exceto para ocioso) |
| Barra de volume/brilho | 520 | 46 | fixa (não muda com o estilo), superamortecida |
| Troca de abas (roda do mouse) | 250 | 31 | razão de amortecimento ≈ 0,92, assenta em ~0,29 s |
| Rolagem das letras imersivas | 190 | 26 | quase crítica, sem balanço na troca de linha |

**Estilos** (`AnimationStyle`) multiplicam rigidez × amortecimento das molas de tamanho e da cutucada: Suave 1,0 × 1,35 (sem balanço) · Padrão 1,0 × 1,0 · Elástico 1,1 × 0,70 · Ágil 1,5 × 1,25.

### 2.3 O que anima com mola e o que não anima

- Mola: largura, altura, cutucada vertical, barras de volume/brilho, troca de abas, rolagem das letras.
- O raio dos cantos **não** tem mola própria: é derivado da altura a cada quadro (`raio = altura/2`, limitado a 44 px × escala; 8 px no estilo Windows 11; 16 px no estilo notch).
- O conteúdo do painel expandido aparece por **opacidade derivada da altura**: `alfa = clamp((altura − 60) / 60)`. Abaixo de 60 px de altura ainda é a pílula recolhida (e não recebe clique).
- A cor de destaque usa aproximação exponencial, não mola: `k = 1 − e^(−dt/0,18)` (95 % em ~350 ms). No primeiro quadro, salta direto para o alvo.

> **Para o Rust (ADR-0001):** as molas de tamanho viram `SpringScalarNaturalMotionAnimation` no compositor. As constantes acima são o ponto de partida já registrado no PLANNING §3.5. A diferença de rigidez entre crescer e encolher é essencial para a sensação: crescer é rápido e quase sem balanço; encolher é mais lento e amortecido. A cutucada de −6 px na troca de estado é o que dá o "pulo" característico.

## 3. Estados

### 3.1 Tipos (`IslandKind`) e prioridade

`ChooseActivities` monta a lista nesta ordem; o primeiro é a pílula principal e o segundo, se houver, vira uma **segunda pílula ao lado** (estado `Split`, separadas por 12 px × escala):

1. Pesquisa rápida (Quick Lookup)
2. Área de transferência
3. Caps Lock / Num Lock
4. Dispositivo (USB conectado/removido)
5. Bluetooth
6. Não perturbe
7. Volume
8. Brilho
9. Notificação
10. Bateria fraca
11. Progresso
12. Timer (Pomodoro)
13. **Mídia** (qualquer sessão do SMTC *disponível*, tocando ou pausada)
14. Ocioso (relógio + clima), se nada acima estiver ativo

### 3.2 Tamanhos recolhidos (px de conteúdo)

| Estado | Largura × altura | Dura |
|---|---|---|
| Ocioso | medida pelo texto (mín. 72, máx. 280; 96/170 só como semente) × **36** | — |
| Mídia | **150 × 44** (+ seção de relógio, ou 380 de largura com letra no recolhido) | — |
| Progresso | 230 × 48 | enquanto `ProgressPercent` ∈ [0,100] |
| Área de transferência | 340 × 56 | 2,5 s |
| Notificação | 360 × 58 | 4 s |
| Volume / Brilho | 300 × 54 | 1,8 s após a última mudança |
| Bateria fraca | 290 × 52 | 4 s (borda vermelha pulsando a 2,1 Hz) |
| Caps Lock | 180 × 42 | 2,5 s |
| Dispositivo | 240 × 50 | 3 s |
| Bluetooth | 280 × 54 | 4 s |
| Timer | 260 × 54 | enquanto roda |
| Não perturbe | 220 × 42 | 3 s |
| Pesquisa rápida | largura fixa, altura medida pelo resultado | até fechar |

### 3.3 Expansão

- **Mídia expandida: 380 × 184.** Abre com o mouse em cima (`ExpandOnHover`), clique (`g_clickExpanded`), fixada pelo menu (`PinnedExpanded`) ou, com `MediaAutoExpand`, por **5 s após trocar de faixa** (só se estiver tocando e o app não estiver na lista de bloqueio).
- **Roda do mouse** sobre a ilha expandida troca de aba: mídia → letras → calendário → clima → monitor de hardware → bandeja de arquivos (conforme os módulos ligados).
- Em tela cheia, a ilha esconde (`AutoHideFullscreen`), a não ser que haja indicador de privacidade, alerta ou fixação.
- **Esconder:** a mola encolhe até 0 e só então a janela é escondida (`SW_HIDE`) e a thread "estaciona" (CPU ~0). O atalho (Ctrl+Alt+D) traz de volta.

## 4. Tempos de transição

| Transição | Duração | Detalhe |
|---|---|---|
| Troca de fonte: sai | 0,17 s | desliza 14 px |
| Troca de fonte: entra | 0,24 s | desliza 14 px; espera no máx. 1,5 s pela nova sessão |
| Barra de fontes (dock) | 0,30 s | deslizamento |
| Player ↔ letras na aba de mídia | 0,32 s | *crossfade* |
| Letra no recolhido (nova linha) | 0,30 s | *ease-out* cúbico, desliza 13 px |
| Letras ao lado do player | 0,30 s | desliza 12 px |
| Capa girando na troca de faixa | 0,62 s | `DrawAlbumArtFlip` (opcional) |
| Barras na troca de faixa | — | "dominó": cada barra cai para um ponto e volta com sobressinal `1 − e^(−5,5v)·cos(9,5v)`, na direção do pulo (anterior/próxima) |
| Cor de destaque | τ = 0,18 s | ver §2.3 |

## 5. Ritmo de quadros (onde a CPU vai)

- **Nada mudou:** a thread de desenho acorda a cada **16 ms** (`WaitForSingleObject(g_stopEvent, 16)`), refaz a escolha de estados e a detecção de mudanças, e volta a dormir. É uma consulta, não um quadro.
- **Conteúdo contínuo** (barras com mídia tocando, letreiro rolando, alertas com contagem, **pontos de privacidade acesos**): no máximo **60 Hz**.
- **Molas em movimento:** taxa do monitor (`TargetFPS=auto`, até 500 Hz), sincronizada com `DwmFlush()`; nas taxas altas, gira (`yield`) no último milissegundo para acertar o quadro.
- Resolução do timer do sistema (`timeBeginPeriod(1)`) só enquanto anima, mais 0,5 s.
- A captura de áudio só fica ligada quando o visualizador está visível e há mídia tocando (`g_audioCaptureNeeded`).
- **Consequência medida no ADR-0006:** com o microfone em uso (ponto laranja), a ilha redesenha a 60 Hz o tempo todo, mesmo parada.

> **Para o Rust:** no compositor, nada disso precisa de laço. Molas e barras são animações do Windows.UI.Composition; o relógio é um temporizador por minuto; os dados chegam por eventos. O laço de 16 ms e o redesenho a 60 Hz com os pontos de privacidade são os dois custos fixos que a versão em Rust elimina.

## 6. Layout

### 6.1 Recolhido

- **Mídia (150 × 44):** capa à esquerda (quadrada, com *padding* interno; ela "cresce" até a capa de 64 px quando a ilha expande, interpolando pela altura), visualizador à direita, ponto de privacidade na ponta direita. Opcional: seção de relógio à esquerda (`MediaPillClock`), com divisor de 1 px (margem vertical de 10 px), espaçamento de 9 px e largura arredondada para múltiplos de 2 px.
- **Ocioso (altura 36):** relógio (+ divisor + clima), *padding* de 14 px, espaçamento de 9 px, divisor com margem vertical de 9 px, 18 px reservados à direita para o ponto de privacidade, largura medida com DirectWrite e arredondada para 2 px (evita redimensionar a cada quadro por métrica sub-pixel).

### 6.2 Expandido (mídia, 380 × 184)

| Elemento | Posição (px de conteúdo) |
|---|---|
| Capa | 64 × 64 em (24, 20) |
| Título / artista / álbum | à direita da capa |
| Linha do tempo | y = 114; margem 24 + 48 de cada lado; clique ±14 px na vertical |
| Controles | y = 148; espaçamento 64; raios 16 (anterior/próximo) e 22 (tocar/pausar); folga de clique 6 |
| Barra de fontes | x = 20, altura 30, posições de 26 px, espaçamento 2, ícone 16; até 3 posições (ou 2 + "+N") |
| Letras ao lado (opcional) | +220 px de largura |

Estilos da linha do tempo (`ProgressStyle`): fina, ondulada, "squiggle" (onda mais lenta) e barra grossa. As ondas animam tocando e achatam ao pausar.

## 7. Visualizadores

### 7.1 Forma de onda (estilo "barras", padrão)

- Thread de áudio: WASAPI loopback do dispositivo de saída padrão (troca de dispositivo reinicia a captura).
- **Nível RMS × 4**, limitado a [0, 1], com envelope: subida `0,3·anterior + 0,7·novo`, descida `0,85·anterior + 0,15·novo`.
- Anel de **48 amostras** (`SharedState::waveform`).
- Desenho: até **32 barras**, largura ≥ 2 px, espaçamento 2,5 px, centradas na vertical, altura até 86 % da área (mínimo 3 px). Cada barra lê o anel com **passo de 4 amostras (~40 ms)** para as barras não ficarem iguais. Opacidade `0,45 + 0,5·amplitude`. Cantos totalmente arredondados. Cor: destaque.
- Pausado: barras planas, sem redesenho contínuo.

### 7.2 Analisador de espectro (estilos LED, plasma e orbe)

`SpectrumAnalyzer` (≈ linha 6125), só ligado quando o estilo precisa (`g_spectrumBandsNeeded`):

- Janela de **1024 amostras com Hann**, salto de 512, FFT radix-2 própria (tabelas de *twiddle* e bit-reverso pré-calculadas).
- **24 bandas logarítmicas de 50 Hz a 14 kHz** (ou 0,45 × taxa de amostragem). Potência da banda = 0,5 · média + 0,5 · pico dos bins. Amplitude = √potência · 4/1024.
- Inclinação para os agudos: × (1 + 2,2 · b/23).
- **Ganho automático:** `agc = max(pico, agc · 0,9985)`, piso 0,02; valor = √(clamp(bruto/agc)).
- **Silêncio continua silêncio:** multiplica por clamp(pico · 40).
- Suavização: sobe com 0,65, desce com 0,14. Sem dados: decai × 0,88 por bloco.

Os estilos: **LED** (matriz de picos com queda), **plasma** (linha contínua) e **orbe** (círculo pulsante). Cada um tem sua própria animação de troca de faixa.

## 8. Letras

- Fonte: **LRCLIB** (`lrclib.net/api/...`), letras sincronizadas (LRC), *user-agent* próprio. Falha → nova tentativa com espera; sem letra → mostra o título.
- Cache em disco: `%LOCALAPPDATA%\DynamicIslandForWindows\lyrics`, um arquivo por música, **LRU com limite de 25 MB** (10/25/100/desligado), renovação em 90 dias. Só guarda músicas que têm letra.
- **Antecipação de 0,49 s** + deslocamento do usuário (±3000 ms), com um valor separado escolhido automaticamente quando a saída padrão é Bluetooth (latência maior).
- **Karaokê:** a linha ativa acende da esquerda para a direita conforme o tempo. Parte não cantada com opacidade 0,6, brilho de 46 px atrás da frente de varredura e 6 px à frente, alfa de pico 0,30. Linha ativa em 19,5 px. Vizinhas como "fantasma", ou legíveis (`LyricsNeighborLinesVisible`).
- Vista imersiva (aba de mídia com letras): linhas a 25 px de distância, as de fora a 14 px e desaparecendo; o bloco do player encolhe para 0,90 em torno de y = 176; mola 190/26 na rolagem.
- Letra na pílula recolhida (`CollapsedLyrics`): pílula mais larga, mesma altura, linha nova entrando com *ease-out* cúbico de 0,30 s.

## 9. Indicadores

### 9.1 Câmera e microfone (pontos de privacidade)

- **Detecção pelo registro**, a cada **2 s** (pulada enquanto há animação): `HKCU` e depois `HKLM` `\SOFTWARE\Microsoft\Windows\CurrentVersion\CapabilityAccessManager\ConsentStore\{microphone|webcam}\*`, incluindo as subchaves de `NonPackaged` (apps Win32).
- Uma entrada está **em uso** quando `LastUsedTimeStart ≠ 0` **e** `LastUsedTimeStop == 0`. Testar só o `Stop` acendia o ponto para apps que nunca usaram o dispositivo (bug corrigido no upstream).
- O nome do app em uso vem do nome da subchave (limpo por `CleanActiveAppName`).
- Desenho: ponto de raio 4 px a `rect.right − 20`, **laranja `#FF9500`** (microfone) e **verde `#34C759`** (câmera), cores configuráveis, pulsando, o que força redesenho contínuo (ver §5).
- O ponto mantém a ilha visível mesmo com auto-esconder ou tela cheia.

> **Para o Rust:** dá para trocar a consulta ao registro por `RegNotifyChangeKeyValue` nas duas chaves (evento em vez de consulta) e fazer o pulso com uma animação de opacidade no compositor.

### 9.2 Monitor de hardware e sobreposição de jogo

| Métrica | Fonte | Frequência |
|---|---|---|
| CPU | `GetSystemTimes` (delta de ocioso / total) | 1 s, só se visível |
| GPU | PDH `\GPU Engine(*)\Utilization Percentage`, somando as instâncias `engtype_3D` | só se visível (enumerar os motores de todos os processos é caro) |
| RAM | `GlobalMemoryStatusEx` (% de carga, usado/total em GB) | 1 s |
| Disco | `GetDiskFreeSpaceExW(L"C:\\")` | 1 s |
| Rede | PDH `\Network Interface(*)\Bytes Sent/sec` e `Received/sec` | só se visível |
| "FPS" | **a taxa de quadros da própria ilha** (`renderFps`), não a do jogo | — |

Cores das barras de carga: normal até 75 %, âmbar acima de 75 %, vermelho acima de 90 %.

## 10. Temas

### 10.1 Paletas (`kThemePalettes`)

| Tema | Fundo | Texto | Secundário | Contorno |
|---|---|---|---|---|
| Obsidian (padrão) | `#08080A` | `#FFFFFF` | `#9B9BA5` | `#1E1E22` |
| Graphite | `#1C1C1E` | `#FFFFFF` | `#A8A8AE` | `#323236` |
| Slate | `#111721` | `#E9EEF6` | `#92A2B8` | `#232C3A` |
| Nord | `#2E3440` | `#ECEFF4` | `#A7B0C0` | `#3B4252` |
| Evergreen | `#0C1512` | `#E4F1EA` | `#8CAE9D` | `#1A2A23` |
| Espresso | `#1A1512` | `#F6EEE7` | `#B7A395` | `#2E2420` |
| Plum | `#16111C` | `#F1EAF6` | `#AC9BB9` | `#281F33` |
| Porcelain (claro) | `#F5F6F8` | `#14161A` | `#5B6270` | `#D8DBE1` |

O secundário é mantido em ~4,5:1 de contraste com o fundo de cada tema. Graphite muda a opacidade: 0,88 recolhido e 0,98 expandido.

### 10.2 Resolução das cores

- Tema escolhido no menu é salvo como inteiro (valor `ThemePreset`), para não reescrever o arquivo de configuração.
- Um hex personalizado **só vale se for diferente do padrão** (comparando a cor interpretada, não o texto; aceita 3, 4, 6 ou 8 dígitos, com alfa `#RRGGBBAA`). Voltar ao padrão devolve o controle ao tema.
- **Tokens derivados** (`BuildMaterialTokens`), a partir do fundo e de "fundo escuro?" (luminância relativa < 0,45):
  - cartões: branco (ou preto, em tema claro) com alfa 0,085 (0,062); pressionado 0,150 (0,105); divisórias 0,100 (0,085);
  - sombra: preto com alfa 0,55;
  - texto: primário com alfa 0,98, secundário 0,88; o terciário é derivado (secundário 35 % misturado ao fundo, alfa 0,72).
- **Destaque** (`AccentColorMode`): da capa (padrão), do sistema ou hex fixo. Da capa: histograma em **16 × 16 × 16 baldes** RGB, vence o balde mais "vibrante" entre os mais votados, a cor final é a média do balde com seus 26 vizinhos (estável quando a cor cai na fronteira entre baldes) e depois é corrigida para ter contraste com o fundo da ilha (`EnsureContrastAgainstBackground`).
- **Material:** sombreamento de profundidade para baixo + lavagem de destaque vinda do topo (`AccentBloom`, 0–200 %) + sombra suave. Sem bordas claras nem brilho de aro, por decisão de design do upstream.
- **Fundo real do Windows** (`BackdropMaterial`): desfoque ou acrílico via `SetWindowCompositionAttribute` (API não documentada), com opacidade do preenchimento e força da tinta do acrílico configuráveis.

> **Para o Rust:** as paletas e a derivação dos tokens viram dados do tema declarativo do Zefiro. O acrílico do Zefiro já é o `HostBackdropBrush` recortado pela forma (ADR-0001), melhor que o desfoque retangular do mod. A extração de cor da capa é portável como função pura.

## 11. Outras peças que valem o porte

- **Escolha da fonte de mídia** (`ChooseMediaSourceAumid`): (1) escolha na barra de fontes, temporária, termina quando algo **novo** começa a tocar ou o app some; (2) app travado pelo menu; (3) automático: o app que **começou** a tocar por último; sem nada tocando, o último que tocou. A lista de fontes inclui sessões pausadas e mantém a ordem estável (novas no fim).
- **Ícone da fonte**: procurado nas janelas do processo, com nova tentativa a cada 8 consultas (enumerar janelas não é de graça).
- **Clima**: wttr.in (`?format=j1`), cidade pela configuração ou por geolocalização do IP.
