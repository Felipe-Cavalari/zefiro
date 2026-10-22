# Zefiro — Planejamento

> **Projeto:** Zefiro (de Zéfiro, o vento oeste: leve e rápido)
> **Proposta:** um **"Seelen-UI nativo"**: um ambiente de desktop para Windows (barra, ilha de mídia, launcher, dock e gerenciador de janelas), sem WebView, mais fluido e com muito menos RAM.
> **Data:** 2026-10-14 · **Autor:** Felipe Cavalari
> **Licença:** MIT OR Apache-2.0
> **Status:** rascunho para revisão

---

## 1. Visão

Projetos como **Seelen-UI** e **Zebar** provaram que dá para reinventar a interface do Windows com muita personalização. Os dois desenham cada widget e cada painel numa **WebView** (Tauri + HTML/JS), e é isso que custa centenas de MB de RAM e tira a fluidez.

O Zefiro faz o mesmo tipo de produto, só que **nativo**: compositor do Windows + Direct2D, animações com física de mola rodando no compositor e um formato de personalização **declarativo e seguro** no lugar de HTML/JS.

Módulos, em ordem de entrega:

1. **Barra** (Fase 1): widgets sempre visíveis no topo, com uma **ilha de mídia** no centro, ao estilo do macOS/Dynamic Island.
2. **Launcher** (Fase 3): atalho global que busca apps, arquivos, projetos e ações.
3. **Dock** (Fase 4): estilo macOS, com apps fixados e abertos.
4. **Gerenciador de janelas** (Fase 6): tiling próprio. Até lá, integração com o **komorebi** (depois da 1.0).

O projeto será **publicado como open source**. Antes disso, eu uso no dia a dia para encontrar bugs (dogfooding), com **logs e diagnóstico embutidos desde o primeiro dia**.

### Princípios

| Princípio | O que significa na prática |
|---|---|
| **Nativo de verdade** | Rust + Win32 + compositor do Windows + Direct2D/DirectWrite. Sem WebView, sem Electron, sem Python. |
| **Leve por padrão** | Nada roda se o módulo ou widget estiver desligado. Atualizações por evento do sistema sempre que possível; polling só quando não há evento. |
| **Fluido** | Animações na taxa do monitor (60–144 Hz+), com física de mola, rodando no **compositor** sempre que possível. Abrir um painel nunca espera dado de rede. |
| **Uma integração, várias caras** | Toda integração expõe *estado* (o que a barra e a dock mostram) e *ações* (o que o launcher busca). |
| **Personalizável e seguro** | Temas e itens da comunidade são **declarativos**: dá para compartilhar sem que eles executem código no PC de ninguém. |
| **Falha isolada** | Uma integração com erro nunca derruba a barra: o widget mostra o erro e tenta de novo sozinho. |
| **Portável no núcleo** | Só a camada de plataforma conhece o Windows. Linux fica "engatilhado", não implementado. |

### Metas mensuráveis (orçamento de performance)

Valores medidos com **todos os widgets da Fase 1 ligados**, em 2 monitores:

| Métrica | Meta |
|---|---|
| RAM (working set) em repouso | **≤ 35 MB** |
| CPU média em repouso | **≤ 0,3 %** |
| Tempo até o primeiro quadro da barra | **≤ 250 ms** |
| Clique → primeiro quadro do painel (flyout) | **≤ 16 ms** |
| Quadros perdidos em animação de abrir/fechar | **0** na taxa nativa do monitor |
| Crashes em 2 semanas de uso diário (fim da Fase 1) | **0** |

> Referência: o YASB (Python + Qt) e o Seelen-UI (WebView2 por widget) usam bem mais que isso. Medir o YASB no mesmo PC na Fase 0 (`docs/baseline.md`) e, se possível, o Seelen-UI também.

---

## 2. Escopo por fases

| Fase | Entrega | Público | Inspiração |
|---|---|---|---|
| **0 — Fundação e spikes** | Provas técnicas das partes arriscadas (S1–S6) + esqueleto do repositório | Só eu | — |
| **1 — Barra (MVP)** | Barra + **ilha de mídia** + painéis + **modo de edição** (komorebi fica fora até a 1.0; os itens declarativos foram para a Fase 5) | Só eu (dogfooding) | YASB, Fancy Toolbar (Seelen), Dynamic Island, Dock do PowerToys |
| **2 — Beta público** | Instalador, documentação, **app de configurações** pesquisável com pré-visualização ao vivo, formato de recursos documentado | Público (beta) | Settings do Seelen |
| **3 — Launcher** | Busca fuzzy em apps, arquivos, projetos e **ações das integrações** | Público | Raycast, Apps Menu do Seelen |
| **4 — Dock** | Apps fixados e abertos, ampliação com mola, badges (itens declarativos na dock só depois da Fase 5) | Público | Weg do Seelen |
| **5 — Plugins e catálogo** | **Itens declarativos** + linguagem de expressões, providers externos (fora do processo) + catálogo de recursos da comunidade | Público | Providers e marketplace do Zebar |
| **6 — Gerenciador de janelas** | Tiling próprio e animado (BSP, colunas, monocle…), **só se ainda fizer falta** depois da integração com o komorebi | Público | Window Manager do Seelen |
| **7 — Linux (opcional)** | Implementação de plataforma para Wayland | Futuro | — |

### Widgets da Fase 1

| Widget | O que faz | Fonte de dados |
|---|---|---|
| **Ilha de mídia** | Centro da barra, com 3 estados (ver §3.6). **Compacto:** visualizador + capa + data/hora. **Espiando:** título — artista por ~3 s ao trocar de faixa ou passar o mouse. **Expandido:** a pílula "derrete" para baixo com capa, linha do tempo, controles e troca de fonte (Spotify, YouTube, Twitch…). Cor de destaque tirada da capa. | Windows SMTC (`GlobalSystemMediaTransportControlsSessionManager`) + captura do som do sistema (WASAPI loopback) para o visualizador |
| **Relógio e calendário** | Formato personalizável; painel com calendário do mês. Sem mídia tocando, a ilha mostra só data/hora. | Relógio do sistema + formatação de data do Windows por idioma (pt-BR) |
| **Workspaces (komorebi)** — *fora até a 1.0* | Workspaces e layout atual; clique troca de workspace | Eventos do komorebi (`komorebic subscribe-pipe`) + comandos via `komorebic` |
| **Projetos VS Code** | Painel com projetos recentes (locais, WSL, remotos, workspaces), abrir no VS Code | `%APPDATA%\Code\User\workspaceStorage\*\workspace.json` (a chave do `state.vscdb` não existe mais na versão atual; ADR-0011) |
| **Brilho** | % na barra; painel com um slider por monitor; roda do mouse no ícone ajusta | DDC/CI via `dxva2` (`GetPhysicalMonitorsFromHMONITOR`, `Get/SetVCPFeature` código `0x10`); WMI `WmiMonitorBrightness` para tela interna |
| **Uso do Claude** | % da janela de 5 h (e 7 dias no painel), cor por faixa | Token de `~/.claude/.credentials.json` → `GET https://api.anthropic.com/api/oauth/usage`. O token nunca vai para log. |
| **Uso do Codex** | % primária (5 h) e secundária (semanal) | Eventos de uso nos `*.jsonl` de sessão em `~/.codex` (sem abrir credenciais) |
| **Pomodoro** | Timer foco/pausa; liga e desliga no modo de edição | Interno (máquina de estados no núcleo) + notificação ao terminar |
| **Áudio — alto-falantes** | Volume/mudo, roda do mouse, painel com troca de dispositivo de saída | Core Audio: `IMMDeviceEnumerator`, `IAudioEndpointVolume` (+ callbacks), troca de padrão via `IPolicyConfig` |
| **Áudio — microfone** | Mudo/volume do microfone, troca de dispositivo de entrada | Core Audio (mesmas APIs, fluxo de captura) |
| **Modo foco** | Liga/desliga Não Perturbe (perfil "somente prioridade") | Serviço Quiet Hours (COM) + escuta via WNF `WNF_STATE_QUIET_HOURS_CHANGED` |
| ~~Modo noturno~~ | **Fora do plano (2026-10-20):** uma rotina do usuário já liga a luz noturna às 18h e desliga às 9h | — |
| **Notificações (sino)** | Contador; clique abre a central (`Win+N`) | `UserNotificationListener` + escuta via WNF `WNF_SHEL_NOTIFICATIONS` |

**Candidatos a itens declarativos (Fase 5)** (sem código novo de UI, só providers): CPU/GPU/RAM (antigo sysmon), rede, disco, bateria, clima, menu de energia.

---

## 3. Arquitetura

### 3.1 Camadas

```
┌──────────────────────────────────────────────────────────────┐
│ app (binário)                                                │
│   lê config, sobe plataforma, registra módulos e integrações │
├──────────────────────────────────────────────────────────────┤
│ módulos: bar · island · (launcher · dock · wm, fases futuras)│
├──────────────────────────────────────────────────────────────┤
│ integrations/*  (providers: estado + ações)                  │
│   partes do Windows ficam atrás de traits de serviço         │
├──────────────────────────────────────────────────────────────┤
│ resources (sem Windows)                                      │
│   temas, itens declarativos, linguagem de expressões         │
├──────────────────────────────────────────────────────────────┤
│ ui (sem Windows)                                             │
│   árvore de nós, layout, molas, hit-test, modo de edição,    │
│   trait Canvas                                               │
├──────────────────────────────────────────────────────────────┤
│ core (sem Windows)                                           │
│   config, agendador, barramento de eventos,                  │
│   modelo de Integração/Ação, supervisor de falhas            │
├──────────────────────────────────────────────────────────────┤
│ platform (traits) ──► platform-windows (implementação)       │
│   janelas/AppBar, compositor, D2D/DWrite, acrílico,          │
│   monitores, DPI, entrada, atalhos, bandeja, toast           │
└──────────────────────────────────────────────────────────────┘
```

**Regra de ouro:** a crate `windows` só pode ser importada em `platform-windows` e nos módulos `windows.rs` de cada integração. O CI garante isso (ver §8 e §9).

### 3.2 Estrutura do repositório

```
zefiro/
├─ crates/
│  ├─ core/
│  ├─ ui/
│  ├─ resources/             # temas, itens declarativos, expressões
│  ├─ platform/              # só traits
│  ├─ platform-windows/
│  ├─ modules/
│  │  ├─ bar/  island/       # Fase 1 (launcher/, dock/, wm/ nas fases seguintes)
│  ├─ integrations/
│  │  ├─ media/  clock/  komorebi/  vscode/  brightness/
│  │  ├─ claude/ codex/  pomodoro/  audio/   focus/
│  │  └─ nightlight/  notifications/
│  └─ app/
├─ resources/                # recursos que vêm com o app
│  ├─ themes/acrylic.toml    # padrão: acrílico do Windows, segue claro/escuro
│  ├─ themes/cavalaridev.toml# tema Nocturne (bg #161826, accent #9184d9)
│  └─ items/                 # itens declarativos embutidos
├─ assets/icons/             # SVGs próprios
├─ docs/
│  ├─ PLANNING.md            # este arquivo
│  ├─ superpowers/specs/     # uma spec por fase/sub-projeto
│  ├─ adr/                   # registros de decisão de arquitetura
│  └─ baseline.md            # medições de referência
├─ scripts/perf.ps1          # mede RAM/CPU (ver §9)
├─ THIRD_PARTY_NOTICES.md    # créditos e licenças de código portado
└─ .github/                  # CI, templates de issue
```

**Nomes:** as crates usam o prefixo `zefiro-` (`zefiro-core`, `zefiro-ui`, `zefiro-resources`, `zefiro-platform-windows`, `zefiro-integration-claude`…). O executável é `zefiro.exe` e o pacote no winget é `zefiro`.

### 3.3 Contrato de integração (provider)

Toda integração implementa o mesmo contrato. A barra e a dock usam o **estado**; o launcher (Fase 3) busca as **ações**; os providers externos (Fase 5) falam o mesmo modelo por JSON-RPC.

```rust
pub trait Integration: Send + 'static {
    fn id(&self) -> &'static str;                  // "media", "cpu", "claude"...
    fn trigger(&self) -> Trigger;                  // Poll(Duration) | Event | Both
    fn refresh(&mut self, ctx: &Ctx) -> Result<State>;
    fn actions(&self, state: &State) -> Vec<Action>;   // ex.: "Abrir uso detalhado"
    fn invoke(&mut self, action: &ActionId, args: &Args, ctx: &Ctx) -> Result<()>;
}
```

`State` é um objeto de chave e valor tipado (`cpu.usage`, `media.title`, `media.cover`…). É ele que as expressões dos itens declarativos leem (§5).

O lado visual de um widget nativo só traduz estado em nós de UI:

```rust
pub trait Widget {
    fn bar_view(&self, state: &State, theme: &Theme) -> Node;
    fn flyout_view(&self, state: &State, theme: &Theme) -> Option<Node>;
}
```

`Node` é uma árvore pequena e declarativa (`Row`, `Column`, `Icon`, `Text`, `Image`, `Slider`, `ProgressRing`, `List`, `Toggle`, `Visualizer`…). A `ui` compara com o quadro anterior e só redesenha o que mudou. **Itens declarativos geram exatamente os mesmos `Node`s**, então não existe "widget de primeira e de segunda classe".

### 3.4 Threads e fluxo de dados

```
[eventos do SO / timers] ──► worker(s) de integração ──► canal ──► thread de UI
                                                          │
                       PostMessage(WM_APP) acorda a UI ◄──┘
thread de UI: aplica estado → marca nós sujos → layout → desenha só o necessário → commit
```

- **Thread de UI:** loop de mensagens Win32 + desenho. Nunca faz I/O.
- **Workers:** um pool pequeno (2 threads) com agendador por prazo. Integrações por evento (áudio, mídia, komorebi, Não Perturbe, notificações) recebem callbacks e só publicam estado.
- **Captura de áudio** do visualizador: thread própria, **ligada só quando a ilha mostra o visualizador e há som tocando**.
- **Sem runtime async na Fase 1.** HTTP com `ureq` (bloqueante, em worker). Reavaliar na Fase 3.
- **Escritas lentas** (DDC/CI): a UI atualiza na hora (otimista) e o worker **agrupa** as escritas, mandando só o último valor.

### 3.5 Desenho e animação (onde vem a fluidez)

- **Compositor:** o S1 decide entre **DirectComposition puro** e **Windows.UI.Composition** (a camada visual que o WinUI usa por baixo). A preferência inicial é o Windows.UI.Composition, porque ele traz:
  - **molas nativas no compositor** (`SpringNaturalMotionAnimation`): a animação roda dentro do Windows, sem CPU do Zefiro por quadro;
  - **acrílico recortado pela forma** (`HostBackdropBrush` + `CompositionGeometricClip` arredondado e animável): o desfoque acompanha a pílula com borda suave.
- **Direct2D + DirectWrite:** desenham o conteúdo (texto, ícones, formas) só quando o estado muda.
- **Molas:** modelo do Dynamic Island (MIT; ver §11). Constantes de partida: crescer com rigidez 380 e amortecimento 26, encolher com 200/28, padrão 280/24. Estilos multiplicam rigidez/amortecimento:

  | Estilo | Rigidez | Amortecimento |
  |---|---|---|
  | Suave | ×1,0 | ×1,35 |
  | Padrão | ×1,0 | ×1,0 |
  | Elástico | ×1,1 | ×0,70 |
  | Rápido | ×1,5 | ×1,25 |

  Mais um multiplicador de velocidade (0,5×–2×). No compositor, a mola é convertida para *período* e *razão de amortecimento* (ω = √k, ζ = c / 2√k). O `Period` do Windows.UI.Composition se comporta como **1/ω**, não 2π/ω: medido na tela, 2π/ω deixava as molas 4–5× mais lentas que as da ilha ([ADR-0007](adr/0007-barra-fase-1a.md)).
- **Pílula → painel** (Fase 1a): cada painel é uma forma só (fundo, contorno, sombra e recorte) cujos x, y, largura, altura e raio são escalares de um `CompositionPropertySet`; as molas animam esses escalares e **expressões** ligam a eles a geometria, o recorte do conteúdo e a opacidade do conteúdo (aparece nos últimos 40% da altura, regra da ilha). O conteúdo é desenhado uma vez, já no lugar final; abrir é a forma crescer do chip até o painel com a mola de crescer e uma "cutucada" de −6 DIP; fechar é o caminho de volta com a mola de encolher. Trocar de painel no meio da animação só redireciona a mola.
- **Clique atravessando:** barra e painel têm uma janela de desenho que deixa todo clique passar (`WS_EX_LAYERED | WS_EX_TRANSPARENT`) e uma janela de entrada invisível recortada (`SetWindowRgn`) nas áreas clicáveis. `HTTRANSPARENT` não serve: só repassa o clique para janelas da mesma thread.
- **AppBar:** a barra reserva espaço na tela (`SHAppBarMessage`). Uma barra por monitor, DPI por monitor (Per-Monitor v2).
- **Degradação automática** (validada no S6):
  - "Efeitos de animação" do Windows desligados → sem molas, transições instantâneas.
  - "Transparência" desligada → fundo sólido.
  - Jogo ou vídeo em tela cheia, ou tela bloqueada → visualizador e captura de áudio param.
  - Economia de energia → visualizador a 30 fps.
  - Quadros atrasando → desliga o desfoque da ilha antes de perder fluidez.

### 3.6 Ilha de mídia

```
COMPACTO (na barra)      ┃ ▁▃▅▂▇  [capa]  qua 14  22:53 ┃
ESPIANDO (troca faixa)   ┃ ▁▃▅▂▇  [capa]  Música — Artista ⟶ ┃   ← cresce na largura ~3 s
EXPANDIDO (clique)       ╭─────────────────────────────────╮
                         │ [CAPA]  Título                  │
                         │         Artista · Álbum         │
                         │ 1:12 ━━━━━●──────── 3:40        │
                         │      ⏮    ⏯    ⏭               │
                         │ [Spotify] [YouTube] [Twitch]    │
                         ╰─────────────────────────────────╯
```

- **Janela própria**, sobre o centro da barra do monitor principal, transparente e com área clicável que acompanha a pílula. A área transparente deixa o clique passar para as janelas de baixo (método decidido no S6).
- **Transformação contínua:** largura, altura e raio dos cantos animados por mola. O conteúdo troca com *crossfade* (0,32 s); a troca de fonte desliza para fora (0,17 s) e entra (0,24 s), com deslocamento de 14 px (valores do Dynamic Island).
- **Visualizador:** WASAPI loopback (float), janela Hann de 1024 amostras, FFT, 24 bandas logarítmicas, ganho automático (decaimento 0,9985), subida 0,65 / queda 0,14, silêncio continua silêncio. Cada barra é um *visual* com escala Y atualizada a ≤ 60 Hz, **sem redesenhar com Direct2D**.
- **Cor da capa:** amostrada da imagem e usada como tom da ilha (opcional no tema).

### 3.7 Ícones (controle total do estilo)

| Tipo | Uso | Como |
|---|---|---|
| **Fonte de ícones** | Ícones gerais (volume, brilho, sino…) | DirectWrite com Segoe Fluent Icons (nativa do Win11) e Material Design Icons. Cor, tamanho e peso vêm do tema. |
| **SVG** | Ícones próprios e de marcas | `ID2D1SvgDocument`, com troca de cor pelo tema |
| **Imagem** | Capa do álbum, ícone de app | WIC → bitmap D2D, com cache por tamanho |
| **Pacote de ícones** | Trocar o conjunto inteiro | Recurso declarativo (§5) |

---

## 4. Tema e configuração

### 4.1 Tema por tokens (a "sensação de CSS" sem navegador)

Arquivo TOML com tokens, recarregado na hora ao salvar. O padrão é o **Obsidian** (preto verdadeiro), e vêm junto os outros 7 temas da ilha: Graphite, Slate, Nord, Evergreen, Espresso, Plum e Porcelain (claro). Formato (Fase 1a, [spec §8](superpowers/specs/2026-10-15-fase-1a-barra-design.md)):

```toml
name = "Obsidian"

[colors]
background     = "#08080A"
text           = "#FFFFFF"
text_secondary = "#9B9BA5"
border         = "#1E1E22"   # "none" = sem contorno
accent         = "auto"      # auto (capa da ilha) | system | "#RRGGBB"

[material]
opacity        = 0.96
depth          = true
shadow         = true
backdrop       = "none"      # none | acrylic
backdrop_fill  = 0.45

[shape]
style          = "pill"      # pill | w11
radius_max     = 44
panel_radius   = 24

[font]
family         = "Segoe UI Variable"
scale          = 1.0

[animation]
spring         = "default"   # smooth | default | bouncy | snappy
speed          = 1.0
```

- Os embutidos ficam em `resources/themes/<id>.toml`; os do usuário em `%APPDATA%\zefiro\themes\<id>.toml` (mesmo id substitui o embutido). **Abrir pasta de temas** no menu do Zefiro copia o tema atual para lá.
- Erro no arquivo: a barra mantém o tema anterior e o log mostra o arquivo, a linha e o motivo.
- No `config.toml`: `bar.height = completa | compacta` (44 ou 32 DIP) e `bar.format = faixa | pilulas | ilhas | fluent | vidro` (ponta a ponta, pílulas flutuantes, ilhas penduradas num trilho (ADR-0016), cartões do Win11 ou uma barra de vidro flutuante (ADR-0018)). A ilha embutida segue a altura, o formato e o tema.
- O `cavalaridev.toml` saiu do plano (2026-10-20); no lugar dele, o tema embutido **Cobalto**, com acrílico e destaque `#3B82F6` (ADR-0013).

### 4.2 Configuração do layout

`%APPDATA%\zefiro\config.toml` guarda os módulos ligados, os itens de cada **seção** (início, centro, fim), a ordem, por monitor, e as opções de cada item. Editável à mão **e** pelo modo de edição; os dois sempre se mantêm em sincronia. Arquivos em texto puro, bons para versionar e sincronizar (como no Seelen).

### 4.3 Modo de edição ("Editar encaixe")

Inspirado na Dock do PowerToys:

1. Botão direito na barra → **Editar barra**.
2. A barra destaca as seções **Início · Centro · Fim**.
3. Arrastar itens entre seções e reordenar, com animação de mola.
4. Botão **+** abre a galeria (widgets nativos **e** itens declarativos instalados).
5. Engrenagem em cada item abre as opções dele.
6. Opção por monitor: **mesmo layout em todos** ou **layout próprio**.
7. **Concluir** salva no `config.toml`.

O **app de configurações** completo (pesquisável, com pré-visualização ao vivo) vem na Fase 2.

---

## 5. Recursos e extensões (personalização sem WebView)

A comunidade personaliza o Zefiro com **recursos**: arquivos declarativos que só descrevem aparência e comportamento. **Nenhum recurso executa código.**

### 5.1 Tipos de recurso

| Tipo | O que é | Fase |
|---|---|---|
| **Tema** | Tokens (§4.1) + ajustes por módulo | 1 |
| **Item** | Elemento de barra (e depois de dock): ícone, texto, cor, painel e ações, ligado a um provider | 5 |
| **Pacote de ícones** | Mapeia nomes de ícones para glifos/SVGs | 2 |
| **Idioma** | Textos da interface | 2 |
| **Sons** | Sons de eventos (opcional) | Futuro |

### 5.2 Exemplo de item

```toml
schema = 1
id = "@felipe/cpu"
name = { pt = "Uso de CPU", en = "CPU usage" }
provider = "cpu"            # dado ao vivo vindo de uma integração
refresh = "2s"

[bar]
icon  = "fluent:processor"
text  = "{cpu.usage | round}%"
color = "{cpu.usage > 80 ? colors.amber : colors.text}"

[on]
click = "flyout"            # painel montado com nós nativos (row, text, ring…)
right_click = "action:open-task-manager"
```

### 5.3 Linguagem de expressões

- Lê o `State` dos providers (`cpu.usage`, `media.title`), os tokens do tema (`colors.accent`) e literais.
- Operadores: comparação, `&&`/`||`, ternário `a ? b : c`.
- Filtros: `round`, `truncate(n)`, `upper`, `lower`, `percent`, `duration`, `date(fmt)`.
- **Sem laços, sem chamadas ao sistema, sem acesso a arquivos.** Avaliação com limite de tempo e de tamanho. Erro na expressão mostra um aviso no item, não derruba nada.
- Ações só podem chamar **ações registradas** pelas integrações (`action:<id>`), que já são as mesmas que o launcher vai listar.

### 5.4 Onde ficam e como são carregados

- Recursos do usuário: `%APPDATA%\zefiro\resources\{themes,items,icons}\<id>\`.
- Recarregados na hora ao salvar (como os temas).
- Identificador no formato `@autor/nome` (como no Seelen), com `schema` para migrar formatos futuros.
- **Catálogo da comunidade** (instalar com um clique) e **providers externos** chegam na Fase 5.
- **Formato:** TOML por coerência com a configuração. Se os layouts aninhados dos painéis ficarem ilegíveis em TOML, reavaliar na spec da Fase 1.

---

## 6. Gerenciadores de janela

- **Até a 1.0:** o komorebi fica de fora por completo (decisão da Fase 1a).
- **Depois da 1.0:** integração com o **komorebi**. Escutar eventos via `komorebic subscribe-pipe` (JSON com workspaces, layout e janela focada) e mandar comandos via `komorebic`. Cuidar da convivência: espaço reservado da AppBar, regras de ignorar as janelas do Zefiro.
- **Depois do beta:** provider do **GlazeWM** (IPC próprio), no mesmo modelo.
- **Fase 6:** gerenciador de janelas próprio, com tiling animado pelo compositor, **só se** a integração com o komorebi não bastar. Spec própria.

---

## 7. Observabilidade (para o dogfooding)

| Recurso | Detalhe |
|---|---|
| **Logs estruturados** | `tracing` + arquivo diário em `%LOCALAPPDATA%\zefiro\logs`, mantendo 7 dias. Nível ajustável no config. |
| **Crash report** | Hook de panic grava mensagem, backtrace e versão; minidump em crash nativo. |
| **Supervisor de integração** | Cada integração roda isolada (`catch_unwind`). Se falhar, o widget mostra um "!" com o motivo na dica e tenta de novo com espera crescente (1 s → 5 min). |
| **HUD de debug** | Atalho `Ctrl+Alt+Shift+D`: FPS, tempo de quadro, RAM, última atualização e erro de cada integração. |
| **Monitor de vazamento** | Registra no log a RAM a cada 10 min, para ver se ela cresce com o tempo. |
| **Exportar diagnóstico** | Ação no menu: gera um `.zip` com logs + config, **removendo tokens e caminhos pessoais**, pronto para anexar numa issue. |
| **Sem telemetria remota** | Nada sai do PC. Talvez opcional (opt-in) depois do beta. |

---

## 8. Linux "engatilhado"

Não implementar nada de Linux agora, só manter a porta aberta:

- `core`, `ui` e `resources` sem nenhuma dependência do Windows.
- Toda capacidade do sistema é uma trait em `platform`: `WindowHost`, `Canvas`, `Backdrop`, `Monitors`, `Input`, `Hotkeys`, além de traits de serviço para `Audio`, `AudioCapture`, `Media`, `Brightness`, `Notifications`, `DoNotDisturb`, `NightLight`.
- O CI compila `core`, `ui` e `resources` para o alvo Linux (`cargo check --target x86_64-unknown-linux-gnu`) para detectar dependência acidental.
- No futuro, `platform-linux` usaria Wayland `layer-shell` (barra), PipeWire (áudio e captura), MPRIS (mídia) e `ddcutil` (brilho).

---

## 9. Testes e qualidade

| Nível | O quê |
|---|---|
| **Unitários (`core`, `ui`, `resources`)** | Config/tema, layout, molas, agendador, modo de edição, **parser de itens e linguagem de expressões** (incluindo entradas maliciosas: expressão gigante, recursão, tipos errados) |
| **Integrações com fixtures** | JSON real (anonimizado) da API de uso do Claude, `*.jsonl` do Codex, `state.vscdb` de exemplo, eventos do komorebi. Cobrir erros: token expirado, arquivo ausente, formato novo. |
| **Visualizador** | Sinais sintéticos (senoide, ruído, silêncio) → bandas esperadas |
| **Golden de renderização** | Desenhar widgets com D2D num bitmap WIC e comparar com imagens de referência |
| **Performance** | `scripts/perf.ps1`: abre o app, espera 60 s, mede RAM/CPU e falha se passar do orçamento da §1. Rodar antes de cada release. |
| **Checklist manual** | 2 monitores, DPI diferente, tema claro/escuro, desconectar monitor, suspender/retomar, komorebi ligado/desligado, efeitos de animação e transparência desligados |
| **CI (GitHub Actions)** | `cargo fmt --check`, `clippy -D warnings`, testes, `cargo check` para Linux em `core`/`ui`/`resources`, build de release no Windows |

---

## 10. Stack técnica

| Área | Escolha | Por quê |
|---|---|---|
| Linguagem | **Rust** (edição 2024, stable) | Rápido, leve, seguro; núcleo portável |
| APIs do Windows | crate **`windows`** (Microsoft) | Win32, COM e WinRT (compositor, D2D, SMTC, Core Audio) |
| Config, tema, recursos | `serde` + `toml` | Legível, fácil de editar à mão |
| Recarregar ao salvar | `ReadDirectoryChangesW` (via plataforma) | Sem dependência extra |
| Expressões | Parser próprio pequeno (sem engine de script) | Seguro e previsível |
| FFT | Implementação própria radix-2 (1024 pontos) | Pequena; evita dependência |
| HTTP | `ureq` + `rustls` | Bloqueante e leve, sem runtime async |
| JSON | `serde_json` | — |
| SQLite (VS Code) | `rusqlite` (bundled, somente leitura) | Ler projetos recentes |
| Logs | `tracing` + `tracing-appender` | Estruturado, arquivo rotativo |
| Licença | **MIT OR Apache-2.0** | Padrão do ecossistema Rust; permissiva |

---

## 11. Inspirações, créditos e licenças

| Projeto | Licença | O que usamos |
|---|---|---|
| [Seelen-UI](https://github.com/eythaann/Seelen-UI) | AGPL-3.0 | **Só ideias e experiência de uso**: módulos (toolbar, dock, WM, launcher), recursos com `@autor/nome`, app de configurações com pré-visualização. **Nenhum código copiado.** |
| [Zebar](https://github.com/glzr-io/zebar) | GPL-3.0 | **Só ideias**: modelo de providers reativos, pacotes de widgets, marketplace. **Nenhum código copiado.** |
| [Dynamic Island for Windows](https://github.com/devcode90/Dynamic-Island-for-Windows) | MIT | Lógica e constantes das molas, visualizador (FFT/bandas/ganho), tempos de transição. **Pode ser portado**, com o aviso de licença em `THIRD_PARTY_NOTICES.md`. |
| YASB | — | Fontes de dados dos widgets (Claude, Codex, Não Perturbe, notificações, brilho) e o visual CavalariDev atual |
| PowerToys (Dock da Paleta de Comandos) | MIT | Experiência do modo de edição e da fluidez. Só ideias. |

**Regra:** antes de portar qualquer trecho, conferir a licença do arquivo de origem e registrar em `THIRD_PARTY_NOTICES.md`.

---

## 12. Roteiro detalhado

### Fase 0 — Fundação e spikes

Spec: `docs/superpowers/specs/2026-10-14-fase-0-fundacao-spikes-design.md`. Código dos spikes é **descartável**.

| # | Pergunta |
|---|---|
| S1 | Qual compositor (DirectComposition × Windows.UI.Composition) é a base da barra, com fluidez a 144 Hz, acrílico recortado e pouca RAM? |
| S2 | O `UserNotificationListener` funciona sem pacote, ou exige identidade (MSIX/sparse)? |
| S3 | Qual a latência do DDC/CI nos 2 monitores e qual estratégia de agrupamento usar? |
| S4 | O SMTC enxerga Spotify + YouTube + Twitch ao mesmo tempo? Como os navegadores expõem várias abas? |
| S5 | Dá para alternar Não Perturbe e luz noturna de forma confiável? |
| S6 | A ilha de mídia é fluida, deixa o clique passar na área transparente, e onde estão os limites do hardware? |

Depois dos spikes: workspace Rust, CI, logs e esqueleto de `core`/`ui`/`platform`.

### Fase 1 — Barra (MVP)

| Etapa | Entrega |
|---|---|
| **1a** | Barra AppBar por monitor no design da ilha: tema **Obsidian** + 7 temas editáveis, alturas completa/compacta, formatos faixa/pílulas, painéis pílula → painel, **Relógio e calendário**, menu do Zefiro e a ilha embutida no centro (fork, ADR-0006). [Spec](superpowers/specs/2026-10-15-fase-1a-barra-design.md), [ADR-0007](adr/0007-barra-fase-1a.md) |
| **1b** | **Modo de edição implementado**: três seções, arrastar/teclado, galeria, opções do relógio, layouts por monitor e confirmação transacional. [Spec](superpowers/specs/2026-10-15-fase-1b-modo-de-edicao-design.md), [ADR-0008](adr/0008-modo-de-edicao-da-barra.md), [handoff e validações de ambiente pendentes](superpowers/handoff/2026-10-15-fase-1b-codex.md). A ilha continua fixa no primário até a 1c. |
| **1c** | **Ilha embutida na barra (Rust)**: pílula no centro do primário (ocioso com clima, mídia, espiada, avisos de volume, notificação, área de transferência, Caps Lock, Bluetooth, USB, Não perturbe, timer de foco), pontos de privacidade, visualizador, painel com abas (mídia, clima, hardware, letras com karaokê) e menu com fonte, saída de som por app, saída e microfone padrão. `[island] engine = "embutida"` é o padrão; a C++ continua com `"externa"`. [ADR-0009](adr/0009-ilha-embutida-na-barra.md) |
| **1d** | **Áudio e brilho**: chips `audio`, `mic` e `brightness` com %, roda (2 % no volume, 5 % no brilho) e painéis (slider, mudo, troca de saída/entrada; um slider por monitor por DDC/CI, gravado numa thread com escrita agrupada). [ADR-0010](adr/0010-audio-e-brilho-fase-1d.md) |
| **1e** | **Widgets do YASB que faltam**: chips `claude`, `codex` (uso), `vscode` (projetos recentes), `notifications` (contador, central, limpar), `dnd` (Não perturbe) e `power` (energia com confirmação). O chip de apps fixados/abertos foi feito e removido a pedido do usuário (barra de tarefas do Windows e Raycast cobrem). [Spec](superpowers/specs/2026-10-19-fase-1e-widgets-do-yasb-design.md), [ADR-0011](adr/0011-widgets-do-yasb-fase-1e.md) |
| **1e+** | **Chips da ilha**: `player` (a pílula de mídia da ilha, idêntica, sem os avisos), `weather` (temperatura e cidade) e `hardware` (CPU, GPU, RAM ao clicar), em qualquer seção de qualquer monitor; a ilha continua fixa no centro do principal. [Spec](superpowers/specs/2026-10-20-chips-da-ilha-design.md), [ADR-0012](adr/0012-chips-da-ilha.md) |
| **1e++** | **Chip Terminal**: `terminal` com os perfis do Windows Terminal (ícones de verdade, selo do padrão) e edição no próprio painel (lápis → arrastar para ordenar, olho para esconder → Concluir), gravada em `[terminal]`. [Spec](superpowers/specs/2026-10-20-chip-terminal-design.md), [ADR-0015](adr/0015-chip-terminal.md) |
| **1e+++** | **Formato ilhas** (Adaptive Islands do YASB): trilho de 4 px no topo e ilhas de início, centro e fim com cantos de 16 e concordâncias de 8; a ilha do centro envolve a ilha de mídia. Primeiro dos formatos novos de barra. [ADR-0016](adr/0016-formato-ilhas.md) |
| **1e++++** | **Ilha como chip**: `island` no catálogo, em qualquer seção e monitor; sem ele, o centro do principal fica livre. Ilha e player sem fundo próprio (a segunda camada escurecia a pílula). [ADR-0017](adr/0017-ilha-como-chip.md) |
| **1e+++++** | **Formatos Fluent e Vidro** (dos temas Win11 Fluent Onyx e Frosted Glass Revamp do YASB) e a chave **Acrílico** no menu do Z, que vale para qualquer tema; o acrílico agora funciona de verdade (`DWMWA_USE_HOSTBACKDROPBRUSH`). [ADR-0018](adr/0018-formatos-fluent-e-vidro.md) |
| ~~1f~~ | **Removida em 2026-10-20.** Os itens declarativos e a linguagem de expressões foram para a Fase 5. O modo noturno saiu do plano: uma rotina do usuário já liga a luz noturna às 18h e desliga às 9h. O Pomodoro ficou coberto pelo timer de foco da ilha |
| **1g** | Tema **Cobalto** (acrílico, destaque `#3B82F6`) no lugar do `cavalaridev.toml`, **HUD de debug** (`Ctrl+Alt+Shift+D`) e **exportar diagnóstico** feitos ([Spec](superpowers/specs/2026-10-20-fase-1g-design.md), [ADR-0013](adr/0013-fase-1g-cobalto-hud-diagnostico.md)); falta **2 semanas de uso diário** substituindo o YASB, **contando desde 2026-10-22** (`scripts/stability.ps1` soma panics, crashes e travamentos do Windows; com um incidente, a contagem recomeça no dia seguinte e esta data muda) |

**Pronto quando:** todas as metas da §1 forem atingidas e houver 2 semanas sem crash.

### Fase 2 — Beta público

App de configurações (pesquisável, pré-visualização ao vivo), documentação do formato de recursos, pacotes de ícones, README com GIFs, instalador (zip portátil + MSIX/winget conforme o S2), GitHub Releases, templates de issue com "exportar diagnóstico".

### Fase 3 — Launcher

Atalho global, janela acrílica, busca fuzzy em apps, arquivos, projetos e **ações de todas as integrações**. Spec própria.

### Fase 4 — Dock

Estilo macOS: apps fixados e abertos, indicadores, badges, ampliação com mola. Itens declarativos na dock só depois da Fase 5. Spec própria.

### Fase 5 — Plugins e catálogo

**Itens declarativos** e a linguagem de expressões (§5; vieram da antiga 1f, e Claude e Codex podem virar itens sobre os parsers da 1e), providers externos **fora do processo** (JSON-RPC por named pipe, mesmo modelo Estado + Ações) e catálogo de recursos da comunidade. Spec própria.

### Fase 6 — Gerenciador de janelas (condicional)

Só se a integração com o komorebi não bastar. Spec própria.

### Fase 7 — Linux (opcional)

`platform-linux` conforme a §8.

---

## 13. Riscos e decisões em aberto

| Item | Risco/dúvida | Mitigação |
|---|---|---|
| **Escopo de "ambiente de desktop"** | Querer tudo do Seelen de uma vez impede chegar a algo usável | Fases estritas; cada módulo só começa com o anterior em uso diário; WM próprio é condicional |
| Compositor e acrílico | `HostBackdropBrush` pode exigir condições específicas (sem pacote, Win10) | S1 compara os dois caminhos; plano B com DirectComposition + acrílico do DWM |
| Clique na área transparente da ilha | Janela transparente pode "roubar" cliques | S6 testa três métodos (redimensionar, região, transparência dinâmica) |
| Notificações | `UserNotificationListener` pode exigir identidade de pacote | S2; se exigir, pacote *sparse* |
| Não Perturbe / luz noturna | APIs não documentadas; podem mudar em atualizações do Windows | Isolar em módulo próprio; plano B abre as Configurações |
| Trocar dispositivo de áudio | `IPolicyConfig` não é documentada (usada há anos por EarTrumpet/YASB) | Isolar; fallback: abrir as configurações de som |
| Mídia em navegador | Chromium pode expor só uma sessão por navegador | S4; documentar a limitação |
| DDC/CI | Lento; alguns monitores não respondem | Escrita agrupada, UI otimista, timeout por monitor |
| Uso do Claude/Codex | Endpoints e formatos podem mudar sem aviso | Parser tolerante + fixtures + "formato desconhecido" em vez de quebrar |
| komorebi | Formato dos eventos muda entre versões | Parser tolerante; fixar versão testada no README |
| Licenças | Código AGPL/GPL não pode entrar num projeto MIT/Apache | Regra da §11 + revisão em todo PR que porta código |
| Itens declarativos | Linguagem de expressões crescer até virar uma linguagem de script | Lista fechada de operadores/filtros; nada de laços nem acesso ao sistema |
| **Volume por app** | Entra na Fase 1 ou fica para 1.x? | Decidir na spec da Fase 1 |
| **Eventos no calendário** | Integrar Google/Outlook? | Fora da Fase 1; reavaliar depois do beta |

---

## 14. Próximos passos

1. Revisar este planejamento e a spec da Fase 0.
2. Instalar os pré-requisitos (Build Tools + Rust) e medir o YASB atual.
3. Executar o **S1** primeiro (decide o compositor), depois o **S6** (ilha), depois S2–S5.
4. Com os spikes respondidos, escrever a **spec da Fase 1** e o plano de implementação da etapa 1a.
