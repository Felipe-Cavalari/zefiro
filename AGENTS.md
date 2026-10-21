# AGENTS.md — instruções para agentes (Codex e outros)

Zefiro é um ambiente de desktop nativo para Windows (barra, ilha de mídia, launcher, dock), sem WebView: leve, fluido e personalizável. Ele é escrito em Rust; a ilha de mídia é um fork em C++.

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
| `crates/integrations/usage` | Uso do Claude (resposta do endpoint, token) e do Codex (fim dos `.jsonl`, thread `CodexService`) e datas em pt-BR, portável (ADR-0011) |
| `crates/integrations/vscode` | Projetos recentes do VS Code pelo `workspaceStorage`, portável (ADR-0011) |
| `crates/integrations/terminal` | Perfis do Windows Terminal pelo `settings.json` e fragmentos (JSONC, ocultos, fontes desativadas), ordem e ocultos do `[terminal]` e os argumentos do `wt.exe`, portável (ADR-0015) |
| `crates/integrations/update` | Atualização pelo GitHub Releases, portável: versão, última release, URL confiável e SHA-256 (ADR-0014) |
| `crates/modules/bar` | Chips, menu do Zefiro, calendário, chips e painéis de som, microfone e brilho (`devices.rs`), uso (`usage.rs`), VS Code (`vscode.rs`), energia (`power.rs`), sino e lua (`chips.rs`) e o controlador de painéis |
| `crates/modules/island` (`zefiro-island`) | Ilha embutida, portável: mídia e escolha da fonte, estados, avisos, visões em `DisplayList`, abas (clima, hardware, letras), menu, visualizador e cor da capa (ADR-0009) |
| `crates/modules/island-host` | Ilha externa (`engine = "externa"`): gravação do `island.ini`/`island-state.ini`, política de reinício e o processo |
| `crates/platform-windows` | Janelas, AppBar, compositor (Windows.UI.Composition), `MorphShape`, Direct2D/DirectWrite em WARP, monitores, preferências do sistema e os serviços (mídia, volume de saída e entrada, dispositivos de áudio, brilho por DDC/CI…) |
| `crates/app` (`zefiro.exe`) | Junta tudo: barras por monitor, painéis, laço de eventos, robustez e o ciclo de vida da ilha |
| `island/` | Fork em C++ do Dynamic Island for Windows (MIT) como `zefiro-island.exe`, usado só com `engine = "externa"`; ver ADR-0006 e ADR-0009 |

## Comandos

```powershell
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
pwsh -NoProfile -File scripts/check-windows-imports.ps1

# Barra + ilha
cargo build --release -p zefiro-app
./island/build.ps1                     # MSVC; feche o zefiro-island.exe antes, senão o link falha
Copy-Item island/build/zefiro-island.exe target/release/
./target/release/zefiro.exe

# Instalador (Inno Setup 6: winget install JRSoftware.InnoSetup) → dist\Zefiro-<versão>-setup.exe
pwsh -NoProfile -File scripts/package.ps1      # -BuildIsland recompila a ilha C++ antes
```

**Releases e atualização (ADR-0014):** o Zefiro instalado consulta o GitHub Releases a cada 24 h e oferece "Atualizar para a x.y.z" no menu do Z. Para publicar: subir a versão no `Cargo.toml`, commit e `git tag vX.Y.Z` + push (só quando o usuário pedir); o `.github/workflows/release.yml` gera e anexa o instalador. Guia: `docs/RELEASES.md`. O build de `targetelease` só checa com `ZEFIRO_UPDATE_CHECK=1`.

**Instalador:** `installer/zefiro.iss`, por usuário e sem admin, em `%LOCALAPPDATA%\Programs\Zefiro`. A versão vem do `[workspace.package]` do `Cargo.toml`. Ele fecha o Zefiro com `WM_CLOSE` antes de instalar ou desinstalar e nunca apaga `%APPDATA%\zefiro`. O ícone e a versão do exe vêm do `crates/app/build.rs`.

**Arquivos da barra em uso:**

- A configuração fica em `%APPDATA%\zefiro\config.toml` e é recarregada ao salvar.
  - `acrylic = true | false` (na raiz): a chave "Acrílico" do menu do Z, por cima do `[material] backdrop` do tema; sem a chave, o tema decide
  - `[bar] height = completa | compacta`
  - `[bar] format = faixa | pilulas | ilhas | fluent | vidro`
  - `[bar.sections] start/center/end`: ids do catálogo (`zefiro`, `clock`, `audio`, `mic`, `brightness`, `claude`, `codex`, `vscode`, `terminal`, `notifications`, `dnd`, `island`, `player`, `weather`, `hardware`, `power`)
  - `[terminal] order/hidden`: `guid`s dos perfis do Windows Terminal, gravados pelo painel do chip (lápis → Concluir)
- `ZEFIRO_CONFIG_DIR=<pasta>` troca a pasta da configuração: use um perfil isolado para testes, sem mexer no do usuário.
- Os temas do usuário ficam em `%APPDATA%\zefiro\themes\`.
- Os logs ficam em `%LOCALAPPDATA%\zefiro\logs`.

**Fechar a barra sem deixar a AppBar presa:** use **Sair** no menu (clique em "Z" ou botão direito na barra) ou mande `WM_CLOSE` para a janela `ZefiroBar`. Evite `Stop-Process -Force`.

**Medir:**

- `./scripts/perf.ps1 -Process zefiro …`: RAM e CPU. Ele soma os processos filhos, e a ilha é filha do zefiro, então para medir só a barra use `[island] enabled = false`.
- `ZEFIRO_CYCLES=50`: abre e fecha o calendário 50 vezes e registra no log o tempo do clique até o quadro.
- `./scripts/frames.ps1`: quadros perdidos a partir de um CSV do PresentMon, que precisa de um PowerShell elevado.

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
- **Chip Terminal (ADR-0015):** lê os perfis do Terminal ao abrir o painel e nunca grava nos arquivos dele. A edição (arrastar, olho) é no próprio painel e grava só `[terminal]`; mudança só nessa seção não fecha o painel (`Config::same_except_terminal`). O `wt.exe` é um alias de execução: `Path::exists` dá `false`, use `symlink_metadata`.
- **Serviços seguem a barra:** Claude, Codex e notificações só rodam com o chip em algum layout (`BarConfig::uses`); `refresh_services()` roda ao salvar o config e ao concluir o modo de edição.
- **Claude:** o Zefiro só lê `~/.claude/.credentials.json` (a cada consulta) e nunca renova nem grava o token, porque isso derrubaria o login do Claude Code. O token nunca vai para log, teste ou sonda (ADR-0011).
- **Notificações:** sem identidade de pacote o `ClearNotifications` falha (0x80070490); limpar remove uma a uma. A central abre com Win+N por `SendInput`.
- **Sondas de tela sem roubar o mouse:** `PostMessage` na janela `ZefiroInput` simula cliques, mas não dá ao Zefiro o direito de trazer janelas para frente; para isso é preciso entrada real (`SendInput`).
- **DPI:** o processo é Per-Monitor v2 (`system::enable_per_monitor_dpi`, chamado no início do `main`).
- **Tela cheia:** com `ABN_FULLSCREENAPP`, a barra do monitor coberto some. As janelas do próprio Zefiro nunca contam como tela cheia.
- **Nenhuma janela do Zefiro do tamanho exato do monitor:** em primeiro plano, o Windows a trata como app em tela cheia e liga o Não perturbe sozinho. A janela do painel tem uma linha a menos (`panel_window`, ADR-0007).
- **Fork da ilha:** toda mudança no `dynamic-island-for-windows.wh.cpp` vai dentro de `#ifdef ZEFIRO_ISLAND` e é marcada com `// ZEFIRO:`, para acompanhar o upstream com diffs mínimos. A camada `wh_shim.*` substitui a API do Windhawk.
- **Ilha embutida (padrão, ADR-0009 e ADR-0017):** é o chip `island`, em qualquer seção de qualquer monitor; a barra desenha a ilha no próprio compositor, centrada no chip, que reserva a largura máxima dela. Sem o chip, o centro do principal fica livre. Clique abre o painel com abas. Os serviços só da ilha seguem `Config::shows_embedded_island()`. Ilha e player não têm fundo próprio (`SurfaceKind::Merged`): uma segunda camada a 96 % por cima da faixa ficava mais escura. Cada fonte de dados (SMTC, volume, privacidade, notificações, Bluetooth, clima, letras, hardware) é um serviço em `platform-windows` com thread própria que avisa a UI por `post_user`; nada consulta por quadro. O hardware só amostra com a aba aberta, e a captura de áudio só roda com mídia tocando e a barra visível.
- **Chips da ilha (ADR-0012):** `player`, `weather` e `hardware` levam partes da ilha para qualquer seção e monitor. O player é uma `IslandView` própria no retângulo do chip, sem os avisos; a largura dele segue a pílula e refaz o layout. Os painéis reaproveitam o painel com abas da ilha (`PanelKind::tabbed`, `tabs::for_set`). Mídia e clima rodam com a ilha embutida ou com o chip na barra.
- **HUD e diagnóstico (ADR-0013):** `Ctrl+Alt+Shift+D` abre o HUD (quadros do DWM, memória, último dado de cada integração e os últimos avisos do log, pela `RecentLayer`); ele só atualiza aberto. "Exportar diagnóstico" no menu do Z grava um zip limpo (`zefiro_core::diagnostics::sanitize`) em Downloads.
- **Ilha externa e temas novos:** a ilha C++ tem só 8 paletas; temas embutidos depois delas vão como personalizados (`ISLAND_PRESETS`).
- **Barra ↔ ilha externa (`engine = "externa"`):**
  - A barra grava `island.ini` (seção `[Zefiro]`, cores do tema, e no formato faixa sem contorno, sem sombra e sem o tom da capa) e o `ColorThemeV2` no `island-state.ini`.
  - A ilha publica `ZefiroAccent` no `island-state.ini`. A barra usa esse valor quando o tema tem `accent = "auto"`.
  - O calendário fica só na barra; a ilha embutida não tem essa aba.

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
