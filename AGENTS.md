# AGENTS.md — instruções para agentes (Codex e outros)

Zefiro é um ambiente de desktop nativo para Windows (barra, ilha de mídia, launcher, dock), sem WebView: leve, fluido e personalizável. Ele é escrito em Rust; a ilha de mídia é um fork em C++.

Leia antes de mudar qualquer coisa:

| Documento | Quando ler |
| --- | --- |
| `docs/PLANNING.md` | Sempre. É a visão, o roteiro de fases (1a a 1d feitas: barra, modo de edição, ilha embutida em Rust, áudio e brilho; próxima: 1e itens declarativos v1, depois 1f e 1g) e as metas de performance. |
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
| `crates/resources` | Os 8 temas embutidos (`themes/*.toml`, padrão Obsidian), o parser de tema com erros por linha, os tokens e o registro de temas com os do usuário |
| `crates/ui` | Molas (constantes da ilha), `DisplayList`, layout da barra e regras do painel (pílula → painel) |
| `crates/integrations/clock` | Grade do mês e formatação de data (pt-BR e Windows) |
| `crates/integrations/devices` | Brilho e níveis, portável: escrita agrupada do DDC/CI (`Coalescer`), passo da roda e posição no slider (ADR-0010) |
| `crates/modules/bar` | Chips, menu do Zefiro, calendário, chips e painéis de som, microfone e brilho (`devices.rs`) e o controlador de painéis |
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
```

**Arquivos da barra em uso:**

- A configuração fica em `%APPDATA%\zefiro\config.toml` e é recarregada ao salvar.
  - `[bar] height = completa | compacta`
  - `[bar] format = faixa | pilulas`
  - `[bar.sections] start/center/end`: ids do catálogo (`zefiro`, `clock`, `audio`, `mic`, `brightness`)
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
- **Sombra:** `DropShadow` num `LayerVisual` deixa a janela preta. A sombra usa 7 retângulos concêntricos, como a ilha (`DrawSoftShadow`).
- **Mover "na hora" no compositor é animação:** `StopAnimation` + `InsertScalar` logo depois de uma mola perde para a parada, que grava o valor do meio do caminho por cima. `Comp::move_offset` usa um quadro-chave de 1 ms (ADR-0010).
- **Chips de nível com largura fixa:** o painel aberto nasce do chip; se o chip mudar de largura com o nível, o painel anda sob o cursor. Os chips de som, microfone e brilho reservam "100%" (ADR-0010).
- **Brilho por DDC/CI:** ~60 ms por chamada. Tudo na thread `zefiro-brightness`; a interface é otimista e só o último valor de cada monitor é gravado.
- **DPI:** o processo é Per-Monitor v2 (`system::enable_per_monitor_dpi`, chamado no início do `main`).
- **Tela cheia:** com `ABN_FULLSCREENAPP`, a barra do monitor coberto some. As janelas do próprio Zefiro nunca contam como tela cheia.
- **Fork da ilha:** toda mudança no `dynamic-island-for-windows.wh.cpp` vai dentro de `#ifdef ZEFIRO_ISLAND` e é marcada com `// ZEFIRO:`, para acompanhar o upstream com diffs mínimos. A camada `wh_shim.*` substitui a API do Windhawk.
- **Ilha embutida (padrão, ADR-0009):** a barra do primário desenha a ilha no próprio compositor; o "chip" `island` abre o painel com abas. Cada fonte de dados (SMTC, volume, privacidade, notificações, Bluetooth, clima, letras, hardware) é um serviço em `platform-windows` com thread própria que avisa a UI por `post_user`; nada consulta por quadro. O hardware só amostra com a aba aberta, e a captura de áudio só roda com mídia tocando e a barra visível.
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
