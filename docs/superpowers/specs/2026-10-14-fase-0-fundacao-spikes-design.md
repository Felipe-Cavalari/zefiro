# Zefiro — Fase 0: Fundação e spikes (spec)

> **Data:** 2026-10-14 · **Autor:** Felipe Cavalari
> **Status:** rascunho para revisão
> **Base:** [`docs/PLANNING.md`](../../PLANNING.md), §12 (Roteiro: Fase 0) e §13 (Riscos)

---

## 1. Objetivo

Responder com **números e testes reais** as perguntas que podem derrubar a arquitetura antes de escrever o código de verdade, e deixar o repositório pronto para a Fase 1.

Ao final da Fase 0 precisamos saber:

1. Se a base visual (AppBar + compositor do Windows + acrílico + Direct2D) entrega a fluidez e a RAM prometidas, e **qual compositor** usar: DirectComposition puro ou Windows.UI.Composition.
2. Se a **ilha de mídia** (pílula que se transforma, ao estilo do macOS) é viável e onde estão os **limites do hardware**.
3. Quais integrações arriscadas funcionam, como funcionam e qual o plano B de cada uma.
4. Como o app vai ser empacotado (depende das notificações).

## 2. Entregáveis

| # | Entregável | Onde |
|---|---|---|
| E1 | Medição de referência do YASB atual | `docs/baseline.md` |
| E2 | Seis ADRs, um por spike, com números medidos e decisão | `docs/adr/0001…0006-*.md` |
| E3 | Repositório com workspace Rust, logs, CI e script de performance | raiz do repo |
| E4 | Tabela de riscos atualizada (o que virou "resolvido", "plano B" ou "corta") | `docs/PLANNING.md` §13 |

## 3. Fora do escopo

- Qualquer widget "de verdade" (isso é a Fase 1).
- Modo de edição, temas completos, configuração do usuário.
- Publicar o repositório como público (fica **privado** até a Fase 2).
- Código dos spikes no `main`: os spikes são **descartáveis** e vivem só em branches (ver §6).

---

## 4. Ambiente de referência

Todas as medições são feitas e anotadas neste ambiente:

| Item | Valor |
|---|---|
| Sistema | Windows 11 Pro, build 26200 |
| GPU | NVIDIA GeForce RTX 5070 |
| Monitores | 2× 1920×1080. `DISPLAY2` = primário (x=0); `DISPLAY1` = à esquerda (x=−1920). Um deles é o LG ULTRAGEAR. |
| Taxa de atualização | 144 Hz no monitor principal (anotar a do segundo no início do S1) |
| Windows 10 | VM no Hyper-V (Windows 10 22H2), usada só no S1 (compositor e acrílico) e no S5 |

## 5. Pré-requisitos (antes de tudo)

Hoje o PC **não tem** Rust nem as ferramentas de compilação da Microsoft. Instalar:

1. **Visual Studio 2022 Build Tools** com a carga de trabalho *Desenvolvimento para desktop com C++* (MSVC v143 + **Windows 11 SDK**). É o linker e as bibliotecas que o Rust usa no Windows.
2. **rustup** com o toolchain `stable-x86_64-pc-windows-msvc` e os componentes `clippy` e `rustfmt`.
3. Alvo extra para verificar portabilidade: `rustup target add x86_64-unknown-linux-gnu` (só para `cargo check`; não precisa de linker Linux).
4. Ferramentas de medição: **PresentMon** (quadros) e o script `scripts/perf.ps1` (RAM/CPU, criado no F1).
5. **Hyper-V** com uma VM Windows 10 22H2 (para S1 e S5). Se não der para montar a VM, o teste no Win10 vira um risco registrado, mas não bloqueia a fase.

**Pronto quando:** `cargo --version` funciona e um "hello world" com a crate `windows` compila e abre uma `MessageBox`.

---

## 6. Organização do trabalho

- **Repositório:** `~/Projects/zefiro`, git local, branch `main`. GitHub **privado** criado no F4 (com sua confirmação na hora).
- **Cada spike numa branch e worktree própria** (`spike/s1-render`, `spike/s2-notifications`…), dentro de `spikes/<nome>/` como binário separado.
- **No `main` entram só:** o ADR do spike, os números medidos e, se for útil, um trecho de código copiado para o ADR. O código do spike fica na branch como referência e nunca é mesclado.
- **Timebox:** cada spike tem um limite de dias. Estourou o limite → para, registra o que descobriu e decide no ADR (plano B, cortar ou estender conscientemente).

### Ordem de execução

```
Pré-requisitos → F1 (repo + docs + perf.ps1) → B0 (baseline YASB)
   → S1 (renderização)  ← o mais importante, vem primeiro
   → S6 (ilha de mídia)  ← usa o compositor escolhido no S1
   → S2, S3, S4, S5 (independentes, qualquer ordem)
   → F2 → F3 → F4 (fundação final, já informada pelos spikes)
```

---

## 7. B0 — Medição de referência do YASB

**Por quê:** a meta de RAM/CPU do Zefiro só faz sentido comparada com o que você usa hoje.

**Como:**
1. Subir o YASB com a configuração atual (CavalariDev, 2 monitores) e o komorebi como no dia a dia.
2. `scripts/perf.ps1 -Process yasb -WarmupSeconds 60 -SampleSeconds 300` (somando processos filhos).
3. Repetir 3 vezes e anotar a mediana.

**Anotar em `docs/baseline.md`:** working set, private bytes, CPU média em repouso, tempo até aparecer a barra (cronômetro, 3 medições).

**Timebox:** 0,5 dia.

---

## 8. Spikes

### S1 — Pipeline de renderização (janela, acrílico, Direct2D, animação)

**Pergunta:** qual compositor do Windows usar como base de toda a barra, e ele desenha texto, ícones e um painel animado **sem perder quadro a 144 Hz** e **com pouca RAM**?

**Hipótese:** os dois caminhos ficam abaixo de 25 MB. O **Windows.UI.Composition** vence em fluidez e acrílico porque faz molas e desfoque dentro do próprio compositor, sem gastar CPU do Zefiro por quadro.

**Os dois caminhos comparados:**

| | **(A) DirectComposition puro** | **(B) Windows.UI.Composition** (camada visual do WinRT) |
|---|---|---|
| Ligação com a janela | `DCompositionCreateDevice3` → *target* por janela | `CreateDispatcherQueueController` + `Compositor` + `ICompositorDesktopInterop::CreateDesktopWindowTarget` |
| Conteúdo Direct2D | `IDCompositionSurface` (`BeginDraw`/`EndDraw`) | `CompositionGraphicsDevice` + `ICompositionDrawingSurfaceInterop` (`BeginDraw`/`EndDraw` com D2D) |
| Acrílico | Da janela inteira, via DWM (ver item 4) | `Compositor.CreateHostBackdropBrush()` + efeito de desfoque/tom: o acrílico **recortado pela forma do visual** |
| Molas | `IDCompositionAnimation` com a curva amostrada em segmentos | `SpringScalarNaturalMotionAnimation` / `SpringVector3NaturalMotionAnimation` nativas |
| Recorte arredondado animável | Limitado (`IDCompositionRectangleClip`) | `CompositionGeometricClip` com `CompositionRoundedRectangleGeometry` (tamanho e raio animáveis) |
| Sistemas | Win8+ | Win10 1903+ (desktop interop) |

**Abordagem (igual para A e B):**

1. **Janelas:** uma barra por monitor, `WS_POPUP` + `WS_EX_TOOLWINDOW` + `WS_EX_NOREDIRECTIONBITMAP`, DPI *Per-Monitor v2*.
2. **AppBar:** `SHAppBarMessage` (`ABM_NEW`, `ABM_QUERYPOS`, `ABM_SETPOS`). Tratar `ABN_POSCHANGED` e `ABN_FULLSCREENAPP` (esconder com jogo ou vídeo em tela cheia).
3. **Dispositivos:** D3D11 (com suporte BGRA) → DXGI → dispositivo Direct2D, compartilhado pelos dois caminhos.
4. **Acrílico.** No caminho (A), testar as duas formas e registrar qual funciona com conteúdo transparente e cantos arredondados:
   - (a1) `DwmSetWindowAttribute(DWMWA_SYSTEMBACKDROP_TYPE, DWMSBT_TRANSIENTWINDOW)` + `DwmExtendFrameIntoClientArea` (oficial, Win11 22H2+);
   - (a2) `SetWindowCompositionAttribute` com `ACCENT_ENABLE_ACRYLICBLURBEHIND` (não documentado, funciona no Win10; é o que o Dynamic Island usa).
   - Cantos: `DWMWA_WINDOW_CORNER_PREFERENCE`.

   No caminho (B), usar o `HostBackdropBrush`. Exige `WS_EX_NOREDIRECTIONBITMAP` e a política de backdrop do host habilitada na janela.
5. **Conteúdo de teste:** texto com DirectWrite (Segoe UI Variable), um glifo da Segoe Fluent Icons, um ícone SVG via `ID2D1DeviceContext5::CreateSvgDocument` com troca de cor, e uma "pílula" arredondada.
6. **Painel (flyout):** abrindo sob o item clicado com animação de mola (escala 0,96 → 1, opacidade 0 → 1, deslocamento −8 px → 0), na forma de animação de cada caminho. Como referência extra, implementar também a **animação por quadro na thread de UI**, sincronizada com `DCompositionWaitForCompositorClock` (fallback `DwmFlush`), que é o método do Dynamic Island.
7. **Estatística de quadros:** registrar o tempo de cada quadro (`DCompositionGetFrameStatistics` e PresentMon) e a CPU da thread de UI **durante** a animação.

**Medições (para A e para B):**

| Métrica | Como medir | Meta |
|---|---|---|
| RAM (working set e private bytes) | `perf.ps1`, 60 s de repouso, 2 barras + 1 painel aberto e fechado | **≤ 25 MB** (sobram ~10 MB para os widgets) |
| CPU em repouso | `perf.ps1`, 5 min | **≤ 0,2 %** |
| CPU do Zefiro durante animação | Média em 50 ciclos de abrir/fechar | Menor é melhor (critério de desempate) |
| Primeiro quadro | `QueryPerformanceCounter` do início do `main` até o primeiro quadro visível | **≤ 250 ms** |
| Clique → primeiro quadro do painel | timestamp do `WM_LBUTTONDOWN` até o commit | **≤ 16 ms** |
| Quadros perdidos | 50 ciclos de abrir/fechar a 144 Hz | **≤ 1 no total** |
| Acrílico | Visual nas bordas arredondadas do painel e da barra | Sem serrilhado e sem "caixa" de desfoque fora da forma |
| AppBar | Maximizar uma janela em cada monitor | Janela não fica por baixo da barra |
| Win10 | Rodar o vencedor na VM Win10 22H2 | Funciona (acrílico pode cair para fundo sólido) |

**Como decidir:** o vencedor precisa cumprir **todas** as metas. Se os dois cumprirem, desempata pela qualidade do acrílico recortado (essencial para a ilha do S6), depois pela CPU durante a animação, depois pela simplicidade do código.

**Se falhar:**
- RAM acima da meta → medir sem a camada de debug do D3D, sem SVG e sem fonte de ícones, para achar o culpado. Só revisar a meta depois de entender o porquê.
- (B) não funciona sem pacote ou no Win10 → ficar com (A) e o acrílico (a2) via máscara de região.
- Nenhum acrílico recortado aceitável → fundo com tom sólido semitransparente (sem desfoque) na ilha, e acrílico só na barra.

**Entregável:** `docs/adr/0001-pipeline-de-renderizacao.md`, com a tabela A × B, o compositor escolhido, o método de acrílico e capturas de tela.

**Timebox:** 5 dias.

---

### S2 — Notificações (contador do sino)

**Pergunta:** o `UserNotificationListener` funciona num `.exe` **sem pacote** ou exige identidade de pacote (MSIX/sparse)?

**Hipótese:** o YASB (Python, sem pacote) usa essa API, então talvez funcione sem identidade. Precisa ser confirmado.

**Abordagem:**
1. Binário mínimo chamando `UserNotificationListener::Current()`, `RequestAccessAsync()`, `GetAccessStatus()` e `GetNotificationsAsync(Toast)`.
2. Escutar mudanças via WNF `WNF_SHEL_NOTIFICATIONS` (`RtlSubscribeWnfStateChangeNotification` da `ntdll`).
3. Disparar notificações de teste e conferir se o contador acompanha.
4. **Se exigir identidade:** criar um **pacote sparse** (MSIX com `AllowExternalContent`), assinado com certificado de desenvolvimento, registrado com `Add-AppxPackage -ExternalLocation`, e testar de novo. *Instalar o certificado de teste é uma mudança de segurança do sistema: quem faz é você, seguindo o passo a passo do ADR.*

**Sucesso:** o contador reflete uma notificação nova em **≤ 1 s** e zera ao limpar a central.

**Se falhar:** o sino vira só um botão que abre a central (`Win+N`) sem contador, e o contador fica para a Fase 2 (com o pacote).

**Entregável:** `docs/adr/0002-notificacoes-e-empacotamento.md`, com a decisão de empacotamento para a Fase 2 (zip portátil vs. sparse vs. MSIX).

**Timebox:** 1 dia.

---

### S3 — Brilho via DDC/CI

**Pergunta:** qual a latência real de leitura e escrita de brilho nos dois monitores, e qual estratégia de agrupamento deixa o slider fluido?

**Abordagem:**
1. `EnumDisplayMonitors` → `GetNumberOfPhysicalMonitorsFromHMONITOR` → `GetPhysicalMonitorsFromHMONITOR`.
2. Ler com `GetVCPFeatureAndVCPFeatureReply(0x10)` e escrever com `SetVCPFeature(0x10, valor)`.
3. 100 leituras e 100 escritas por monitor: anotar p50 e p95.
4. Simular arrasto de slider (30 valores por segundo durante 2 s) com e sem agrupamento. Agrupamento = o worker guarda só o último valor e escreve quando a escrita anterior termina, com um intervalo mínimo a descobrir.
5. Desconectar e reconectar um monitor: os *handles* precisam ser recriados sem travar.

**Sucesso:**
- Os dois monitores respondem.
- p95 da escrita **≤ 100 ms**.
- Com agrupamento, o valor final fica correto em **20 de 20** arrastos, sem o monitor travar ou piscar.

**Se falhar:** monitor sem DDC/CI → widget mostra "brilho indisponível" só para ele. Escrita lenta demais → aumentar o intervalo mínimo e manter a UI otimista.

**Entregável:** `docs/adr/0003-brilho-ddc-ci.md`, com a tabela de latência e os parâmetros de agrupamento.

**Timebox:** 1 dia.

---

### S4 — Mídia via SMTC

**Pergunta:** o SMTC enxerga Spotify, YouTube e Twitch ao mesmo tempo, e como os navegadores expõem várias abas?

**Abordagem:**
1. `GlobalSystemMediaTransportControlsSessionManager::RequestAsync()` → `GetSessions()`.
2. Para cada sessão: `SourceAppUserModelId`, `TryGetMediaPropertiesAsync()` (título, artista, capa), `GetPlaybackInfo()`, `GetTimelineProperties()`.
3. Assinar `SessionsChanged`, `MediaPropertiesChanged`, `PlaybackInfoChanged` e `TimelinePropertiesChanged`.
4. Testar `TryTogglePlayPauseAsync`, `TrySkipNextAsync` e `TrySkipPreviousAsync`.

**Cenários (anotar o resultado de cada um):**

| # | Cenário |
|---|---|
| 1 | Só Spotify desktop |
| 2 | Só YouTube no Brave |
| 3 | Só Twitch no Brave |
| 4 | YouTube + Twitch em **abas diferentes do mesmo Brave** |
| 5 | Spotify + YouTube tocando ao mesmo tempo |
| 6 | YouTube no Chrome ou Edge, se instalado, para comparar |

**Para cada cenário, anotar:** quantas sessões aparecem, o AUMID de cada uma (para mapear nome e ícone do app), se vem capa, com que frequência a linha do tempo atualiza (se precisa interpolar) e se os controles funcionam.

**Sucesso:** apps diferentes aparecem como sessões diferentes e os controles funcionam em todas. A limitação de várias abas fica documentada.

**Se falhar:** navegador expõe uma sessão só → o painel mostra "Brave — aba ativa" e documentamos a limitação.

**Entregável:** `docs/adr/0004-midia-smtc.md`, com a tabela de cenários.

**Timebox:** 1 dia.

---

### S5 — Modo foco (Não Perturbe) e modo noturno

**Pergunta:** dá para **ler e alternar** o Não Perturbe e a luz noturna de forma confiável no Windows 11 (e no 10)?

**Abordagem — Não Perturbe:**
1. Levantar os identificadores COM do serviço **Quiet Hours** e os nomes de estado WNF que o YASB usa (módulo `dnd_api`: `WNF_STATE_QUIET_HOURS_CHANGED` e os perfis `Microsoft.QuietHoursProfile.Unrestricted`, `PriorityOnly`, `AlarmsOnly`), conferindo com projetos open source conhecidos.
2. Ler o perfil atual, alternar entre `Unrestricted` e `PriorityOnly` e escutar mudanças feitas pelo próprio Windows (pela central de ações).

**Abordagem — luz noturna:**
1. Ler o blob `Data` em `HKCU\Software\Microsoft\Windows\CurrentVersion\CloudStore\Store\DefaultAccount\Current\default$windows.data.bluelightreduction.bluelightreductionstate\windows.data.bluelightreduction.bluelightreductionstate`.
2. **Fazer backup do blob antes de qualquer escrita.**
3. Descobrir o byte de ligado/desligado, alternar e verificar se aplica na hora, sem fazer logoff.
4. Escutar mudanças externas com `RegNotifyChangeKeyValue`.

**Sucesso:**
- No Win11, alternar funciona e mudanças feitas fora do app aparecem em **≤ 1 s**, nos dois casos.
- Restaurar o backup sempre volta ao estado original.
- Resultado no Win10 anotado (funciona, funciona diferente ou não funciona).

**Se falhar:** o widget abre a página de configuração correspondente (`ms-settings:nightlight`, configurações de notificação) em vez de alternar direto, e o risco fica registrado.

**Entregável:** `docs/adr/0005-foco-e-modo-noturno.md`.

**Timebox:** 2 dias.

---

### S6 — Ilha de mídia e limites do hardware

**Pergunta:** a ilha de mídia (compacto → espiando → expandido, como no `PLANNING.md` §3.6) roda **lisa a 144 Hz** com o compositor escolhido no S1, **deixa o clique passar** na área transparente e **quanto custa** em CPU e GPU, inclusive com o visualizador ligado?

**Hipótese:** com as molas no compositor e o visualizador feito de *visuals* escalados (sem redesenho em Direct2D), a ilha custa menos de 1 % de CPU durante as transições e menos de 1,5 % com o visualizador ligado.

**Base:** Dynamic Island for Windows (MIT). Portar a lógica, não o código C++ literal, e registrar em `THIRD_PARTY_NOTICES.md`.

**Escopo do spike:** visual e desempenho. Os dados de mídia vêm de uma **fonte falsa** (título, artista e capa fixos, com troca de faixa a cada 10 s), mas o **áudio do visualizador é real** (loopback). A integração real com o SMTC é o S4.

**Abordagem:**

1. **Janela da ilha:** janela própria, sobre o centro da barra do monitor principal, transparente (`WS_EX_NOREDIRECTIONBITMAP`, sem foco ao clicar com `WS_EX_NOACTIVATE`). A pílula é um *visual* dentro dela.
2. **Estados e tamanhos de partida** (em pixels lógicos, ajustáveis no ADR):

   | Estado | Largura | Altura | Raio |
   |---|---|---|---|
   | Compacto | conforme o conteúdo (~220) | altura da barra − 4 | metade da altura |
   | Espiando | até ~380 | igual ao compacto | igual ao compacto |
   | Expandido | ~400 | ~184 | ~22 |

3. **Molas:** constantes do Dynamic Island (crescer 380/26, encolher 200/28, padrão 280/24; estilos e multiplicador de velocidade como no `PLANNING.md` §3.5), convertidas para o modelo do compositor (ω = √k, ζ = c / 2√k). Anotar se o resultado bate visualmente com a física por quadro do mod (comparar gravando os dois).
4. **Conteúdo:** *crossfade* de 0,32 s entre estados; troca de fonte saindo em 0,17 s e entrando em 0,24 s, com 14 px de deslocamento.
5. **Clique na área transparente:** testar três métodos e escolher o mais simples que funcione sem falhas:
   - (i) **redimensionar a janela** para o tamanho-alvo no início de cada transição de crescer e no fim de cada transição de encolher;
   - (ii) **região de janela** (`SetWindowRgn`) atualizada com a forma da pílula;
   - (iii) **transparência dinâmica**: liga/desliga `WS_EX_TRANSPARENT` conforme o cursor está dentro ou fora da pílula.

   Teste: clicar 20 vezes em ícones e links logo abaixo da ilha (fora da pílula) com ela em cada estado. Todos os cliques precisam chegar ao app de baixo.
6. **Visualizador:** WASAPI loopback (float) numa thread própria, janela Hann de 1024 amostras, FFT radix-2, 24 bandas logarítmicas, ganho automático (decaimento 0,9985), subida 0,65 / queda 0,14, portão de silêncio. Cada barra é um *visual* com escala Y atualizada a ≤ 60 Hz.
7. **Cor da capa:** amostrar a cor dominante da capa (redução para 16×16, média ponderada por saturação) e aplicar como tom da ilha.
8. **Degradação automática**, cada regra testada uma vez:

   | Situação | Como detectar | Comportamento |
   |---|---|---|
   | Efeitos de animação desligados | `SPI_GETCLIENTAREAANIMATION` | Transições instantâneas |
   | Transparência desligada | `HKCU\…\Themes\Personalize\EnableTransparency = 0` | Fundo sólido |
   | Jogo/vídeo em tela cheia | `ABN_FULLSCREENAPP` | Visualizador e captura param; ilha esconde |
   | Tela bloqueada | `WTS_SESSION_LOCK` | Visualizador e captura param |
   | Mídia pausada ou silêncio > 2 s | estado da mídia + portão de silêncio | Captura de áudio para |
   | Economia de energia | `GUID_ENERGY_SAVER_STATUS` / `GetSystemPowerStatus` | Visualizador a 30 fps |
   | Quadros atrasando | > 2 quadros perdidos por segundo, 3 s seguidos | Desliga o desfoque da ilha |

**Matriz de medição do hardware:**

| Cenário | 60 Hz | 144 Hz |
|---|---|---|
| 50 ciclos compacto ↔ expandido, sem visualizador | ✔ | ✔ |
| 50 ciclos compacto ↔ espiando, sem visualizador | ✔ | ✔ |
| 50 ciclos compacto ↔ expandido, **com** visualizador | ✔ | ✔ |
| 30 min de música com visualizador, ilha compacta | — | ✔ |
| 50 ciclos com a GPU sob carga (jogo ou benchmark em janela) | — | ✔ |

Para testar a 60 Hz, mudar a taxa do monitor nas Configurações do Windows durante o teste.

Em cada cenário, anotar: quadros perdidos (PresentMon e estatística do compositor), CPU do processo, uso da GPU (PresentMon/Gerenciador de Tarefas) e RAM.

**Metas:**

| Métrica | Meta |
|---|---|
| Quadros perdidos em 50 ciclos (sem GPU sob carga) | **≤ 1** |
| CPU do processo durante transições | **≤ 1 %** |
| CPU com visualizador ligado | **≤ 1,5 %** |
| GPU com visualizador ligado | **≤ 2 %** |
| Crescimento de RAM em 30 min | **≤ 1 MB** |
| Cliques fora da pílula que chegam ao app de baixo | **20 de 20** em cada estado |
| Captura de áudio desligada com mídia pausada | Confirmado no log |

**Se falhar:**
- Molas no compositor diferentes demais do mod → animar por quadro na thread de UI, como o Dynamic Island (`DwmFlush` + relógio de alta resolução só enquanto a mola se mexe).
- Nenhum método de clique funciona bem → manter a janela do tamanho exato da pílula e aceitar redimensionar a cada quadro, medindo o custo.
- Visualizador caro demais → 30 fps fixo ou desligado por padrão.
- GPU sob carga derruba a fluidez → anotar o limite e documentar como limitação conhecida.

**Entregável:** `docs/adr/0006-ilha-de-midia-e-limites-de-hardware.md`, com a matriz preenchida, o método de clique escolhido, a conversão das molas, as regras de degradação validadas e um vídeo curto dos 3 estados. Mais a entrada do Dynamic Island em `THIRD_PARTY_NOTICES.md`.

**Timebox:** 3 dias.

---

## 9. Fundação (F1–F4)

### F1 — Repositório e documentação (antes dos spikes)

- `git init` em `~/Projects/zefiro`, `.gitignore` para Rust, `README.md` curto ("em desenvolvimento").
- `docs/` com o `PLANNING.md`, as specs e um modelo de ADR (`docs/adr/0000-modelo.md`: contexto → opções → decisão → consequências → números).
- `LICENSE-MIT` e `LICENSE-APACHE` (licença dupla MIT OR Apache-2.0).
- `THIRD_PARTY_NOTICES.md` começando com a seção de créditos do `PLANNING.md` §11 (Seelen-UI e Zebar como **só inspiração**; Dynamic Island como código portável, MIT).
- `scripts/perf.ps1`, com os parâmetros `-Process`, `-WarmupSeconds` e `-SampleSeconds`, somando processos filhos e gerando JSON com working set, private bytes e CPU média. É usado no B0, no S1 e no S6.

### F2 — Workspace Rust e observabilidade (depois dos spikes)

- `Cargo.toml` de workspace (edição 2024) com as crates:
  - `zefiro-core`
  - `zefiro-ui`
  - `zefiro-resources` (vazia por enquanto; o motor de itens declarativos é da Fase 1)
  - `zefiro-platform`
  - `zefiro-platform-windows`
  - `zefiro-app`
- Versão da crate `windows` fixada no workspace.
- Lints do workspace: `clippy::all` como erro e `unsafe_op_in_unsafe_fn` negado. Todo `unsafe` com comentário `// SAFETY:`.
- **Logs:** `tracing` + `tracing-appender`, arquivo diário em `%LOCALAPPDATA%\zefiro\logs`, mantendo 7 dias.
- **Crash:** hook de panic que grava `crash-<data>.txt` com versão, mensagem e backtrace.
- **Segredos:** tipo `Secret<T>` cujo `Debug` e `Display` imprimem `***`. Tokens (como o do Claude) só podem existir dentro de `Secret`. Teste garante que nunca aparecem formatados.

### F3 — Contratos iniciais

- Em `zefiro-core`: o trait `Integration`, os tipos `State`, `Action` e `Trigger` (conforme o `PLANNING.md` §3.3), um agendador simples e o **supervisor** (isola panics com `catch_unwind` e refaz com espera crescente). Tudo com testes, usando uma integração falsa.
- Em `zefiro-platform`: assinaturas de `WindowHost`, `Canvas`, `Backdrop`, `Monitors` e `AudioCapture`, **desenhadas a partir do que o S1 e o S6 mostraram**. Sem implementação completa ainda.
- Em `zefiro-ui`: o modelo de mola (rigidez, amortecimento, estilos, multiplicador de velocidade e conversão para o compositor) com testes, a partir das constantes validadas no S6.

### F4 — CI e GitHub

- Criar o repositório **privado** no GitHub (`Felipe-Cavalari/zefiro`). *Pedir sua confirmação antes de criar.*
- GitHub Actions:
  - `windows-latest`: `cargo fmt --check`, `cargo clippy -- -D warnings`, `cargo test`.
  - `ubuntu-latest`: `cargo check -p zefiro-core -p zefiro-ui -p zefiro-resources`. **Garante que o núcleo continua sem depender do Windows.**

---

## 10. Critério de pronto da Fase 0

- [ ] Pré-requisitos instalados e "hello world" com a crate `windows` rodando.
- [ ] `docs/baseline.md` com os números do YASB.
- [ ] ADRs 0001–0006 escritos, cada um com números medidos e uma decisão clara.
- [ ] Metas do S1 e do S6 atingidas **ou** decisão consciente registrada nos ADRs 0001 e 0006.
- [ ] `THIRD_PARTY_NOTICES.md` com a entrada do Dynamic Island.
- [ ] F1–F4 no `main`, com CI verde nos dois sistemas.
- [ ] `PLANNING.md` §13 atualizado com o resultado de cada risco.
- [ ] Pronto para abrir o brainstorming/spec da **Fase 1**.

## 11. Riscos desta fase

| Risco | Impacto | Mitigação |
|---|---|---|
| Acrílico não combina com o compositor transparente | Alto (visual base) | Dois compositores testados no S1 + plano de fundo sólido semitransparente |
| `HostBackdropBrush` exige condições não atendidas (sem pacote, Win10) | Alto (ilha) | S1 testa sem pacote e no Win10; plano B com DirectComposition + acrílico do DWM |
| Área transparente da ilha "rouba" cliques | Médio | S6 testa três métodos com critério objetivo (20 de 20 cliques) |
| Visualizador caro em hardware mais fraco | Médio | Regras de degradação do S6; visualizador pode vir desligado por padrão |
| VM Windows 10 não disponível | Baixo | Registrar como risco aberto; testar no Win10 antes da Fase 2 |
| APIs não documentadas (WNF, Quiet Hours, blob de registro) | Médio | Isolamento por módulo + plano B (abrir Configurações) |
| Spikes estourando o tempo | Médio | Timebox rígido; decisão registrada no ADR |
| Curva de aprendizado de Rust + COM/WinRT | Médio | S1 primeiro (concentra a maior parte do COM); padrões do S1 viram referência |
