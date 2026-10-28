# ADR-0009: Ilha de mídia embutida na barra (Rust), no lugar do processo C++

- **Status:** aceito (substitui a decisão de processo separado do [ADR-0006](0006-ilha-de-midia-fork.md) como padrão). O fork C++ e o `engine = "externa"` saíram na [ADR-0026](0026-remocao-da-ilha-cpp.md).
- **Data:** 2026-10-16
- **Branch:** `main` (commits `32c0e2f` … `8af6770`)

## Contexto

Na 1a/1b a ilha de mídia era o `zefiro-island.exe`, um fork C++ do Dynamic Island for Windows rodando em processo próprio (ADR-0006). A 1c pedia o porte para Rust **dentro da barra**: a ilha deixa de ser um programa separado e passa a ser o centro da própria barra, no mesmo compositor dos chips, com o painel expandido nascendo da pílula como o calendário. O usuário pediu explicitamente que nada do que ele usa saísse; a C++ ficou disponível até a embutida cobrir o uso dele.

## Opções consideradas

1. **Continuar com o processo C++** (ADR-0006). Zero trabalho, mas dois processos, desenho por quadro na CPU (1 % com o microfone em uso), janela própria sobre a barra e reserva fixa de 880 px no centro.
2. **Portar tudo antes de trocar.** Uma troca só, mas meses sem nada usável e sem validação parcial.
3. **Ilha embutida com chave `[island] engine`** (`embutida` | `externa`), portando por partes na `main` e trocando o padrão quando o uso do usuário estivesse coberto. Escolhida.

## Números medidos

Ambiente: Windows 11 Pro 26200, dois monitores a 100 %, `compacta`/`faixa`, Obsidian, build release, 2026-10-16. Ilha embutida parada, com o ponto do microfone aceso (Discord), clima, notificações, Bluetooth e volume ligados. `scripts/perf.ps1 -Process zefiro -WarmupSeconds 20 -SampleSeconds 60`.

| Métrica | Barra (1b) + ilha C++ (ADR-0006) | Barra com a ilha embutida |
|---|---|---|
| Processos | 2 | 1 |
| Memória privada | 14,9 + 18,6 = **33,5 MB** | **32,5 MB** |
| Working set | 41,5 + 55,4 MB | 67,3 MB |
| CPU em repouso com o ponto do microfone | **1,0 %** (redesenho a 60 Hz) | **0,0 %** |

O pulso do ponto de privacidade é uma animação do compositor e o visualizador só redesenha barras (sprites) a ≤ 60 Hz enquanto há mídia tocando; nada roda por quadro na CPU parado. A memória privada total ficou igual à dos dois processos juntos; os serviços WinRT (HttpClient, UserNotificationListener, DeviceWatcher) explicam a maior parte. Não se declara cumprida a meta global de 35 MB de working set.

## Decisão

A ilha é desenhada pela barra do monitor principal (`engine = "embutida"`, padrão). O `zefiro-island.exe` continua disponível com `engine = "externa"` e não é iniciado no modo embutido.

### Arquitetura

| Camada | O que tem |
|---|---|
| `crates/modules/island` (`zefiro-island`, portável) | Modelo de mídia e escolha da fonte (§11 das notas), estado recolhido (ocioso, mídia, espiada, avisos com prioridade da ilha, timer), visões em `DisplayList` (pílulas, painel, abas), forma de onda e analisador (S6), cor da capa, menu do botão direito, clima (wttr.in), letras (LRCLIB), saída por app (casamento AUMID → processo). |
| `crates/platform-windows` | SMTC (`media.rs`), captura em loopback, volume por evento, dispositivos de áudio (`IPolicyConfig`/`IAudioPolicyConfig`), privacidade (`RegNotifyChangeKeyValue`), notificações, Não perturbe (WNF), Bluetooth (`DeviceWatcher`), HTTP (`Windows.Web.Http`), hardware (PDH), área de transferência e Caps/Num Lock (entrada bruta), menu nativo. Cada serviço tem sua thread e avisa a UI por `post_user`. |
| `crates/app` | `island_view.rs` (forma, conteúdo, barras e pontos no compositor da barra), `island_loop.rs` (eventos, serviços, abas, menu), `bar_view.rs` (reserva estável no centro e o "chip" `island`). |

### Decisões de comportamento

- **Pílula:** altura da ilha da barra (`island_pill`), largura do estado escalada pela altura (como `[Zefiro] PillHeight`). Na faixa, a pílula se funde à barra; nas pílulas, é uma pílula com contorno e sombra.
- **Reserva no centro:** largura fixa da maior pílula recolhida (espiada + folga), para os chips laterais não pularem. Substitui a reserva de 880 px do fork.
- **Expandido = painel da barra:** clique abre o painel que nasce da pílula, com abas pela roda (mídia, clima, hardware, letras). Abre sempre na primeira aba.
- **Animações do painel no compositor:** cada aba é um visual desenhado uma vez (a mídia em camadas: fixa, indicador, ícones, capa + textos + tempo). O carrossel das abas é uma propriedade `pos` animada pela mola 250/31, com posição, escala e opacidade de cada aba ligadas por expressão à fórmula do `DrawTabTransition`; a troca de fonte e o deslize do indicador são animações de quadro-chave disparadas uma vez. Uma primeira versão redesenhava o painel inteiro a cada `WM_TIMER` de 16 ms com o Direct2D em WARP e ficou travada (o temporizador acorda a cada 16–31 ms); no compositor, a animação roda na taxa do monitor sem CPU do Zefiro por quadro.
- **Desvios conscientes do C++:** visualizador com curva raiz quadrada (fala baixa ainda mexe as barras); privacidade e Caps Lock por evento em vez de consulta; o calendário segue só no chip do relógio.
- **Robustez:** sem o isolamento de processo do ADR-0006, uma falha na ilha afeta a barra; cada serviço falha sozinho (registra no log e a ilha segue sem ele).

## Consequências

- `[island] engine` (padrão `embutida`). O `island.ini`/`island-state.ini` só valem para a externa.
- **Ainda só na externa:** pesquisa rápida (Ctrl+Alt+Espaço; o atalho já falhava nesta máquina, erro 1409), anel de progresso externo (`ProgressPercent`), bandeja de arquivos e sobreposição de jogo (desligadas no `island.ini` do usuário), bateria fraca (o PC é desktop), aviso de brilho (o C++ só lia brilho de tela interna e falhava aqui), "fixar expandido", atalho Ctrl+Alt+D e letras na pílula recolhida.
- **Configuração:** as opções da ilha C++ (`island.ini`) ainda não foram trazidas para o `config.toml` (cidade do clima, módulos, deslocamento das letras). Hoje valem os padrões da ilha do usuário.
- O menu nativo usa o tema claro do Windows; um menu desenhado pela barra fica para depois.
- Validações pendentes: troca de faixa com espiada automática, comandos de mídia e de saída por app com som tocando, notificação real, Bluetooth real, formato pílulas e altura completa, DPI 125/150 %, e PresentMon elevado.
