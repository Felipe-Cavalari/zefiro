# AGENTS.md — instruções para agentes (Codex e outros)

Zefiro é um ambiente de desktop nativo para Windows (barra, ilha de mídia, launcher), sem WebView: leve, fluido e personalizável. Ele é todo escrito em Rust, inclusive a ilha de mídia.

Leia antes de mudar qualquer coisa:

| Documento | Quando ler |
| --- | --- |
| `docs/PLANNING.md` | Sempre. É a visão, o roteiro de fases (1a a 1e feitas: barra, modo de edição, ilha embutida em Rust, áudio e brilho, widgets do YASB; 1g feita no código: Cobalto, HUD e diagnóstico; falta o uso diário de 2 semanas; a 1f foi removida e os itens declarativos foram para a Fase 5) e as metas de performance. |
| `docs/adr/` | Antes de mexer em desenho, animação, janelas ou na ilha. Registra as decisões tomadas e o motivo. Não desfaça uma decisão de um ADR sem escrever outro ADR que a substitua. |
| `docs/superpowers/specs/` e `docs/superpowers/plans/` | Antes de começar uma fase. Mostra como as fases 0, 1a e 1b foram especificadas e planejadas; as próximas seguem o mesmo modelo. |
| `docs/superpowers/handoff/` | Para saber o estado da última passagem e o que ficou pendente. |

## Idioma

- Docs, comentários, mensagens de log e textos da interface: **português do Brasil**.
- Identificadores (tipos, funções, variáveis), nomes de commits e de branches: **inglês**.
- Converse com o usuário em português.

## Regras que não se quebram

1. **Nunca faça `git push`** nem crie PR sem o usuário pedir explicitamente. Commits locais são normais.
2. **Regra de ouro da crate `windows`:** ela só pode aparecer em:
   - `crates/platform-windows`;
   - `crates/integrations/clock/src/windows.rs`;
   - `crates/app`.

   As crates portáveis (`core`, `platform`, `resources`, `ui`, `modules/*`, `integrations/*`) produzem dados: config, tokens de tema, retângulos de layout, `DisplayList`s e regras de painel. A `platform-windows` desenha e anima. O script `scripts/check-windows-imports.ps1` confere isso, e o CI compila as crates portáveis no Linux.
3. **Lint:** `cargo clippy --workspace --all-targets -- -D warnings` precisa passar (o workspace usa `clippy::all = deny`). `unsafe` só em blocos pequenos, cada um com um comentário `// SAFETY:` explicando por que está certo.
4. **Medições:** feche o YASB antes de medir (`perf.ps1`, `frames.ps1`, primeiro quadro) e reabra no fim.
5. **Arquivos do usuário:** não apague nada em `%APPDATA%\zefiro\` sem pedir. Em especial, `codex-backup\` guarda a configuração da barra antiga.
6. **O usuário pode estar usando o PC.** Antes de testes que movem o mouse, mandam teclas (principalmente Alt+F4) ou reiniciam a barra, peça um momento. Só mande Alt+F4 depois de conferir que a janela em primeiro plano é do Zefiro.

## Estrutura

| Pasta | O que tem |
| --- | --- |
| `crates/core` (`zefiro-core`) | Config (`config.toml`, gravação com `toml_edit` que preserva comentários), observador de arquivos, logs e panic hook |
| `crates/platform` | Tipos portáveis: `Rgba`, `Rect`, `TextStyle` e medição de texto |
| `crates/resources` | Os 9 temas embutidos (`themes/*.toml`, padrão Obsidian; o Cobalto usa acrílico), o parser de tema com erros por linha, os tokens e o registro de temas com os do usuário |
| `crates/ui` | Molas (constantes da ilha), `DisplayList`, layout da barra (faixa, pílulas, ilhas com trilho e concordâncias, Fluent com cartões e Vidro flutuante) e regras do painel (pílula → painel) |
| `crates/integrations/clock` | Grade do mês e formatação de data (pt-BR e Windows) |
| `crates/integrations/devices` | Brilho e níveis, portável: escrita agrupada do DDC/CI (`Coalescer`), passo da roda e posição no slider (ADR-0010) |
| `crates/integrations/usage` | Uso do Claude (resposta do endpoint, token, contas), do Codex (fim dos `.jsonl`, contas, thread `CodexService`) e do Grok (`_x.ai/billing`, contas pelo `principalId`), origens Windows/WSL (`origins.rs`) e datas em pt-BR, portável (ADR-0011, ADR-0019, ADR-0027) |
| `crates/integrations/vscode` | Projetos recentes do VS Code pelo `workspaceStorage`, portável (ADR-0011) |
| `crates/integrations/systray` | Bandeja do sistema, portável: os bytes do `WM_COPYDATA` de ícone (`wire`), a lista de ícones (`model`) e as mensagens de clique por versão (`click`) (ADR-0023) |
| `crates/integrations/terminal` | Perfis do Windows Terminal pelo `settings.json` e fragmentos (JSONC, ocultos, fontes desativadas), ordem e ocultos do `[terminal]` e os argumentos do `wt.exe`, portável (ADR-0015) |
| `crates/integrations/chat` | Chat com o Claude, o Codex e o Grok, portável: linhas do `claude -p` em stream-json, do `codex app-server` (JSON-RPC) e do `grok agent stdio` (ACP, `grok.rs`), argumentos e perfil do Grok (Windows e WSL), origem, modelos, a conversa e as conversas antigas em JSON (ADR-0020, ADR-0027) |
| `crates/integrations/spotify` | Spotify pela Web API, portável: login por PKCE, os pedidos (`api`), a leitura das respostas de 2026 (`model`), a sessão com renovação e 429 (`session`, testável com um `Transport` falso) e o cache de capas (ADR-0024) |
| `crates/integrations/update` | Atualização pelo GitHub Releases, portável: versão, última release, URL confiável e SHA-256 (ADR-0014) |
| `crates/modules/bar` | Chips, menu do Zefiro, calendário, chips e painéis de som, microfone e brilho (`devices.rs`), uso (`usage.rs`), chat (`chat.rs`: campo, quebra de linha e painel), VS Code (`vscode.rs`), energia (`power.rs`), sino e lua (`chips.rs`) e o controlador de painéis |
| `crates/modules/island` (`zefiro-island`) | Ilha embutida, portável: mídia e escolha da fonte, estados, avisos, visões em `DisplayList`, abas (clima, hardware, letras), menu, visualizador e cor da capa (ADR-0009) |
| `crates/platform-windows` | Janelas, AppBar, compositor (Windows.UI.Composition), `MorphShape`, Direct2D/DirectWrite em WARP, monitores, preferências do sistema e os serviços (mídia, volume de saída e entrada, dispositivos de áudio, brilho por DDC/CI…) |
| `crates/app` (`zefiro.exe`) | Junta tudo: barras por monitor, painéis, laço de eventos e robustez |

## Comandos

```powershell
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
pwsh -NoProfile -File scripts/check-windows-imports.ps1

# Barra
cargo build --release -p zefiro-app
./target/release/zefiro.exe

# Instalador (Inno Setup 6: winget install JRSoftware.InnoSetup) → dist\Zefiro-<versão>-setup.exe
pwsh -NoProfile -File scripts/package.ps1
```

**Releases e atualização (ADR-0014):** o Zefiro instalado consulta o GitHub Releases a cada 24 h e oferece "Atualizar para a x.y.z" no menu do Z. Para publicar: subir a versão no `Cargo.toml`, commit e `git tag vX.Y.Z` + push (só quando o usuário pedir); o `.github/workflows/release.yml` gera e anexa o instalador. Guia: `docs/RELEASES.md`. O build de `target
elease` só checa com `ZEFIRO_UPDATE_CHECK=1`.

**Instalador:** `installer/zefiro.iss`, por usuário e sem admin, em `%LOCALAPPDATA%\Programs\Zefiro`. A versão vem do `[workspace.package]` do `Cargo.toml`. Ele fecha o Zefiro com `WM_CLOSE` antes de instalar ou desinstalar e nunca apaga `%APPDATA%\zefiro`. O ícone e a versão do exe vêm do `crates/app/build.rs`.

**Arquivos da barra em uso:**

- A configuração fica em `%APPDATA%\zefiro\config.toml` e é recarregada ao salvar.
  - `acrylic = true | false` (na raiz): a chave "Acrílico" da tela de temas do menu do Z, por cima do `[material] backdrop` do tema; sem a chave, o tema decide
  - `[bar] height = completa | compacta`
  - `[bar] format = faixa | pilulas | ilhas | fluent | vidro`
  - `[bar] autohide = true | false` e `[bar] display = todos | principal`: "Ocultar automaticamente" e "Só no monitor principal", gravados pelo Editar barra (ADR-0022)
  - `[bar.sections] start/center/end`: ids do catálogo (`zefiro`, `clock`, `audio`, `mic`, `brightness`, `claude`, `codex`, `grok`, `vscode`, `terminal`, `chat`, `notifications`, `dnd`, `island`, `player`, `weather`, `hardware`, `tray`, `spotify`, `power`)
  - `[chat] agent = claude | codex | grok`; `[chat.claude]`, `[chat.codex]` e `[chat.grok]` com `origin = auto | windows | wsl | <distro>`, `prefer = windows | wsl` (o que o automático tenta primeiro) e `model` (vazio = o padrão), gravados pelo painel
  - `[spotify] client_id`: o Client ID do app que o usuário criou no Spotify, gravado pelo painel do chip
  - `[terminal] order/hidden`: `guid`s dos perfis do Windows Terminal, gravados pelo painel do chip (lápis → Concluir)
- `ZEFIRO_CONFIG_DIR=<pasta>` troca a pasta da configuração: use um perfil isolado para testes, sem mexer no do usuário.
- Os temas do usuário ficam em `%APPDATA%\zefiro\themes\`.
- Os logs ficam em `%LOCALAPPDATA%\zefiro\logs`.

**Fechar a barra sem deixar a AppBar presa:** use **Sair** no menu (clique em "Z" ou botão direito na barra) ou mande `WM_CLOSE` para a janela `ZefiroBar`. Evite `Stop-Process -Force`.

**Medir:**

- `./scripts/perf.ps1 -Process zefiro …`: RAM e CPU. Ele soma os processos filhos. A ilha roda dentro do próprio `zefiro.exe`: para medir a barra sem ela, use `[island] enabled = false`.
- `ZEFIRO_CYCLES=50`: abre e fecha o calendário 50 vezes e registra no log o tempo do clique até o quadro.
- `./scripts/frames.ps1`: quadros perdidos a partir de um CSV do PresentMon, que precisa de um PowerShell elevado.
- `./scripts/stability.ps1`: dia da contagem das 2 semanas sem crash (início no `-Since`, padrão 2026-10-22), com os panics (`crash-*.txt`), os crashes e travamentos do Windows (eventos 1000 e 1002 do `zefiro.exe`) e os inícios da barra. Os logs ficam 21 dias (`LOG_FILES_KEPT`).

## Coisas não óbvias (detalhes nos ADRs)

- **Mola no compositor:** `Period = 1/ω`, não 2π/ω. Isso foi medido; com 2π/ω as molas ficam 4–5× lentas (ADR-0007).
- **Clique atravessando:** `HTTRANSPARENT` só repassa o clique para janelas da mesma thread. Por isso barra e painel têm uma janela de desenho que deixa todo clique passar (`WS_EX_LAYERED | WS_EX_TRANSPARENT`) e uma janela de entrada invisível (`ZefiroInput`) recortada por região nas áreas clicáveis.
- **Acrílico:** o fundo do Windows (`CreateHostBackdropBrush`) só funciona numa janela Win32 com `DWMWA_USE_HOSTBACKDROPBRUSH` ligado (`window::create`); sem isso ele sai preto e o Cobalto fica com cor fixa. Quem decide se há acrílico é `theme_state::backdrop_on` (tema, escolha do menu e a transparência do Windows).
- **Sombra:** `DropShadow` num `LayerVisual` deixa a janela preta. A sombra usa 7 retângulos concêntricos, como a ilha (`DrawSoftShadow`).
- **Mover "na hora" no compositor é animação:** `StopAnimation` + `InsertScalar` logo depois de uma mola perde para a parada, que grava o valor do meio do caminho por cima. `Comp::move_offset` usa um quadro-chave de 1 ms (ADR-0010). Pelo mesmo motivo, `set` seguido de `animate` não faz a forma nascer num lugar: use `MorphShape::animate_from` (mola com `InitialValue`, ADR-0016).
- **Formato ilhas (ADR-0016):** as peças não se sobrepõem, senão a opacidade do tema e o acrílico fazem emenda mais escura. Cada ilha é um `MorphShape` que começa um raio acima da tela (e passa da borda nas pontas), o trilho são retângulos só nos vãos e as concordâncias são `CompositionPath` (`platform-windows/src/fillet.rs`) presas à ilha por expressão. `CompositionMaskBrush` não aceita o fundo do Windows como fonte. Os grupos ficam por seção (`BarLayout::group_slot`).
- **Fluent e Vidro (ADR-0018):** o fundo de sempre dos chips (cartão do Fluent, vidro do Vidro) é o próprio retângulo de hover do chip, ligado por `BarLayout::has_backing`. No Vidro, `band` é a reserva da AppBar (barra flutuante + 6 DIP em cima e embaixo) e a faixa útil é `content_top/content_h`.
- **Chips de nível com largura fixa:** o painel aberto nasce do chip; se o chip mudar de largura com o nível, o painel anda sob o cursor. Os chips de som, microfone e brilho mostram só o ícone (a % fica no painel); qualquer chip de nível com texto precisa reservar "100%" (ADR-0010).
- **Brilho por DDC/CI:** ~60 ms por chamada. Tudo na thread `zefiro-brightness`; a interface é otimista e só o último valor de cada monitor é gravado.
- **Chip Terminal (ADR-0015):** lê os perfis do Terminal ao abrir o painel e nunca grava nos arquivos dele. A edição (arrastar, olho) é no próprio painel e grava só `[terminal]`; mudança só nessa seção não fecha o painel (`Config::same_except_panels`). O `wt.exe` é um alias de execução: `Path::exists` dá `false`, use `symlink_metadata`.
- **Serviços seguem a barra:** Claude, Codex e notificações só rodam com o chip em algum layout (`BarConfig::uses`); `refresh_services()` roda ao salvar o config e ao concluir o modo de edição.
- **Claude (ADR-0025):** o uso vem do `/usage` do próprio Claude Code (`claude -p /usage --output-format stream-json`, campo `usage_report.rate_limits`), não do token: o Zefiro nunca lê o `.credentials.json` (só confere se existe), porque o token da assinatura só pode ser usado no Claude Code. Conta e plano saem do `.claude.json`. Use `--no-session-persistence`, senão cada consulta grava uma sessão em `~/.claude/projects`.
- **HTTP sem cache:** o `HttpClient` do Windows com o filtro padrão devolve do cache a 1ª resposta do processo (era o chip do Claude parado até reiniciar a barra). `http::client()` usa `HttpBaseProtocolFilter` com `NoCache`; sonda `http_cache_probe` (ADR-0019).
- **Uso no WSL (ADR-0019):** os chips `claude` e `codex` juntam as contas do Windows e das distros rodando (`wsl::discover`). Ler `\\wsl.localhost\<distro>` de uma distro parada a **liga**: só se lê distro que está em `wsl --list --running`, com cache de 3 s, e o Claude lê todas as credenciais logo depois de descobrir as origens. `docker-desktop*` e `/root` ficam de fora. O e-mail da conta só aparece no painel; o log diz "2 contas (Windows, WSL)". `ZEFIRO_USAGE_WINDOWS_HOME=<pasta>` simula um tester que só usa o WSL.
- **Chat (ADR-0020):** Codex pelo `codex app-server` (JSON-RPC; `thread/start`, `turn/start` com modelo explícito do `model/list`, `turn/interrupt`, `thread/resume`). Claude: um `claude -p --input-format stream-json --output-format stream-json` vivo por conversa, sem ferramentas e sem hooks/plugins/MCP do usuário (`--setting-sources ""`). Sobe ao abrir o painel, sai com 10 min sem uso e retoma por `--resume` (sessões em `%LOCALAPPDATA%\zefiro\chat` ou `~/.cache/zefiro-chat` no WSL). Parar é um `control_request` `interrupt`. O campo é desenhado pelo painel (não é o EDIT); `WM_CHAR` chega por `chat_char`. Cada conversa fica em `chat\conversas\<id>.json` e nunca vai para o log. Ctrl+V sem texto cola imagem (PNG, arquivo do Explorer ou DIB), convertida numa thread para PNG em `chat\imagens\`; Claude recebe base64 no stream-json, Codex `data:` URL no `turn/start` (sonda `chat_image_probe`). Imagens geradas pelo Codex chegam em base64 no `result` do item `imageGeneration` (o `savedPath` não vale no WSL) e viram `chat\imagens\<id>.png` ligado à resposta; um turno do Codex pode ter vários `agentMessage`, separados por linha em branco pelo `itemId` dos deltas (sonda `chat_generated_probe`). Mudança só em `[terminal]`/`[chat]` não fecha o painel (`Config::same_except_panels`). Sonda: `chat_probe` (`ZEFIRO_CHAT_AGENT=codex|grok`, `ZEFIRO_CHAT_ORIGIN=<distro>`).
- **Grok (ADR-0027):** chat pelo `grok agent --agent-profile grok-perfil.md stdio` do Grok Build (ACP: `initialize`, `authenticate` só com `cached_token`, `session/new`/`session/load`, `session/prompt`, `session/cancel`, `session/set_config_option` com o modelo em texto puro). O `--deny` e o `--disallowed-tools` antes de `agent` **não valem** no ACP, e o `always-approve` do `~/.grok/config.toml` roda ferramentas sem pedir: quem as tira é o perfil de agente (`tools: todo_write`, `mcpInheritance: none`), gravado pelo Zefiro na pasta do chat antes de cada partida (no WSL, com `echo` no script do `bash -lc`). O `GROK_SANDBOX=strict` faz o `grok` recusar a partida sem bubblewrap. O `session/load` reenvia o histórico como `session/update` antes da resposta: até ela, nada vai para a tela. Chip `grok`: um `grok agent stdio` curto por origem com `initialize`, `_x.ai/auth/info` (o `principalId` junta as contas) e `_x.ai/billing` (com sublinhado; não chama o modelo), thread `zefiro-grok-usage`. O Zefiro nunca lê o `~/.grok/auth.json`. Sondas: `chat_probe` com `ZEFIRO_CHAT_AGENT=grok` e `grok_usage_probe`.
- **Bandeja (ADR-0023):** o chip `tray` mostra os ícones da bandeja dos outros apps. Com o chip em algum layout, a thread `zefiro-tray-host` (`tray_host.rs`) cria uma janela `Shell_TrayWnd` acima da barra de tarefas: o `Shell_NotifyIcon` dos apps acha ela primeiro, o host lê o `WM_COPYDATA` (`dwData = 1`, 1484 bytes no Windows 11, com o caminho do exe no fim) e repassa **tudo** à barra de tarefas de verdade, que continua funcionando. Para pegar os ícones que já existem, o host manda `TaskbarCreated` com `lParam = OWN_BROADCAST`; o Zefiro recebe `WinEvent::TaskbarCreated { own: true }` e não refaz nada. Quando o Explorer cai, o host sai da frente e volta no `TaskbarCreated` dele. O clique volta ao app por `PostMessage` na mensagem de callback, depois de `AllowSetForegroundWindow`. `tray.rs` é outra coisa: o ícone do próprio Zefiro. Sonda: `tray_host_probe` (`--check` diz de quem é a primeira `Shell_TrayWnd`).
- **Spotify (ADR-0024):** o chip `spotify` usa a Web API com o Client ID do próprio usuário (modo de desenvolvimento de 2026: dono Premium, 5 usuários, busca de 10 por página, playlists de outro dono sem as músicas). Login por PKCE: o app do Spotify tem `http://127.0.0.1/callback` **sem porta** e o listener (`spotify_auth.rs`) pede ao Windows uma porta livre, que vai no pedido; porta fixa não serve, porque o WSL em modo espelhado reserva blocos de 4096 portas que mudam. O token de renovação fica cifrado pelo DPAPI em `%LOCALAPPDATA%\zefiro\spotify\token.bin` (com o `client_id` dono dele); o de acesso só existe na thread `zefiro-spotify`; nenhum vai para log, nem o texto da busca. Painel fechado = nenhuma consulta; aberto, o player é relido a cada 5 s. Estado do painel em `zefiro_bar::spotify`, desenho em `spotify_view`, ligação em `spotify_loop.rs`. Sonda: `spotify_probe` (`--login`, `--dump <pasta>`).
- **Notificações:** sem identidade de pacote o `ClearNotifications` falha (0x80070490); limpar remove uma a uma. A central abre com Win+N por `SendInput`.
- **Sondas de tela sem roubar o mouse:** `PostMessage` na janela `ZefiroInput` simula cliques, mas não dá ao Zefiro o direito de trazer janelas para frente; para isso é preciso entrada real (`SendInput`).
- **DPI:** o processo é Per-Monitor v2 (`system::enable_per_monitor_dpi`, chamado no início do `main`).
- **Tela cheia:** com `ABN_FULLSCREENAPP`, a barra do monitor coberto some. O aviso chega uma vez só, então a troca de foco (`window::ForegroundHook`) e um temporizador de 1 s (enquanto há barra escondida) também conferem, por `robustness::reconcile_fullscreen`. Só a janela em primeiro plano **no próprio monitor** decide a barra dele (`system::fullscreen_verdict`): foco no outro monitor não traz a barra por cima do jogo, mas ela volta se o app que a escondeu fechar, minimizar ou sair da tela cheia. Jogos sem bordas (Cyberpunk) também são sempre-no-topo; quem fica na frente é quem subiu por último. As janelas do próprio Zefiro nunca contam como tela cheia.
- **Barra limpa (ADR-0022):** com `autohide`, a barra não registra AppBar (sem reserva) e o hook `WH_MOUSE_LL` do `bar_context` só observa o cursor: avisa o laço (`WinEvent::AutoHideCursor`) quando ele entra ou sai de um gatilho do topo (`zefiro_ui::autohide::top_triggers`: a linha de cima na borda de verdade e uma faixa de 6 DIP onde outro monitor está logo acima, para a barra nunca ficar inacessível) ou da faixa, sem reter clique. A regra (`AutoHide`, 300 ms para descer, 500 ms para subir, painel aberto segura, tela cheia manda) roda depois de cada evento (`Loop::autohide_update`). A barra desliza no `Offset` do visual raiz e a janela some quando a mola assenta. `is_hidden()` é só a tela cheia; `is_visible()` junta as duas coisas. `display = "principal"` cria barra só no principal (`robustness::bar_monitors`). O menu do Z não tem mais Acrílico (foi para a tela de temas), mosaico, HUD, diagnóstico e iniciar com o Windows: os três últimos estão no painel de Configurações (`PanelKind::Settings`, pseudo-chip `__settings`).
- **Nenhuma janela do Zefiro do tamanho exato do monitor:** em primeiro plano, o Windows a trata como app em tela cheia e liga o Não perturbe sozinho. A janela do painel tem uma linha a menos (`panel_window`, ADR-0007).
- **Ilha (ADR-0009, ADR-0017 e ADR-0026):** é o chip `island`, em qualquer seção de qualquer monitor; a barra desenha a ilha no próprio compositor, centrada no chip, que reserva a largura máxima dela. Sem o chip, o centro do principal fica livre. Clique abre o painel com abas. Os serviços só da ilha seguem `Config::shows_embedded_island()`. Ilha e player não têm fundo próprio (`SurfaceKind::Merged`): uma segunda camada a 96 % por cima da faixa ficava mais escura. Cada fonte de dados (SMTC, volume, privacidade, notificações, Bluetooth, clima, letras, hardware) é um serviço em `platform-windows` com thread própria que avisa a UI por `post_user`; nada consulta por quadro. O hardware só amostra com a aba aberta, e a captura de áudio só roda com mídia tocando e a barra visível.
- **YouTube no Brave/Chrome:** a cada troca de vídeo (próxima, anterior, fim no Mix) a sessão some do SMTC por ~5 s e volta com uma miniatura genérica; a definitiva chega ~150 ms depois, às vezes sem evento. O serviço segura a sessão sumida sem controles (`VanishHold`), guarda a capa pelos bytes da miniatura (`CoverCache`) e relê a capa por 3 s depois de faixa nova. Sonda: `media_watch_probe` (`ZEFIRO_PROBE_NEXT=5` pula a faixa).
- **Chips da ilha (ADR-0012):** `player`, `weather` e `hardware` levam partes da ilha para qualquer seção e monitor. O player é uma `IslandView` própria no retângulo do chip, sem os avisos; a largura dele segue a pílula e refaz o layout. Os painéis reaproveitam o painel com abas da ilha (`PanelKind::tabbed`, `tabs::for_set`). Mídia e clima rodam com a ilha embutida ou com o chip na barra.
- **HUD e diagnóstico (ADR-0013):** `Ctrl+Alt+Shift+D` abre o HUD (quadros do DWM, memória, último dado de cada integração e os últimos avisos do log, pela `RecentLayer`); ele só atualiza aberto. "Exportar diagnóstico" nas Configurações (menu do Z) grava um zip limpo (`zefiro_core::diagnostics::sanitize`) em Downloads.
- **Sem ilha C++ (ADR-0026):** o fork do Dynamic Island for Windows e o `engine = "externa"` saíram. Uma config com `[island] engine` ou `exe` abre normalmente com a ilha em Rust, e o log avisa uma vez (`removed_island_keys`). O `island.ini` e o `island-state.ini` antigos em `%APPDATA%\zefiro` não são lidos nem apagados. O destaque da capa (`accent = "auto"`) vem do `island_loop`, e o calendário fica só na barra.

## Fluxo de trabalho

**Mudança grande (fase nova, subsistema novo):**

1. Escreva uma spec em `docs/superpowers/specs/AAAA-MM-DD-<tema>-design.md` e peça a aprovação do usuário.
2. Escreva um plano em `docs/superpowers/plans/`, com tarefas pequenas, cada uma com os testes e o comando de verificação, e peça a aprovação.
3. Execute as tarefas e mantenha um ledger em `.superpowers/sdd/<plano>/progress.md` (pasta ignorada pelo git):
   - uma linha `Task N: complete (commits …, tests: … → …)` por tarefa;
   - cada desvio do plano numa linha `Task N: Ruling: <decisão> — <por quê> — <custo se errado>`.
4. **TDD:** escreva o teste, veja ele falhar, implemente e veja passar. Quando o teste não dá para automatizar (comportamento na tela), use uma sonda que falha antes e passa depois, por exemplo `WindowFromPoint` para saber quem recebe um clique.
5. No fim, faça uma revisão da branch inteira contra a spec. Corrija o que for Critical e Important, cada correção com um teste que falhe antes. Registre os Minor no ledger. Entregue ao usuário a lista de rulings e de minors adiados.
6. Documente as decisões num ADR (`docs/adr/NNNN-*.md`, modelo em `docs/adr/0000-modelo.md`) e atualize o `docs/PLANNING.md` quando o roteiro mudar.

**Mudança pequena:** descreva a ideia em poucas frases, espere o "sim" e implemente com TDD.

**Commits:** em inglês, no estilo `feat(app): …`, `fix(platform-windows): …`, `docs(adr): …`.
