# Gerenciador de janelas em mosaico — plano

> **Para agentes:** SUB-SKILL OBRIGATÓRIA: use superpowers:subagent-driven-development (recomendado) ou
> superpowers:executing-plans para executar este plano tarefa por tarefa. Os passos usam caixas
> (`- [ ]`) para acompanhar.

**Objetivo:** um gerenciador de janelas em mosaico (dwindle do Hyprland) dentro do `zefiro.exe`, com
áreas de trabalho por monitor, Win+setas, borda de foco, animações por mola no compositor, chave no
menu do Z e chip `workspaces`.

**Arquitetura:** uma crate portável nova (`zefiro-wm`, em `crates/modules/wm`) é uma máquina de estados
pura: recebe `Event`s e devolve `Command`s. A `platform-windows` (`src/wm/`) tem a thread `zefiro-wm`
(gancho de eventos, modelo, execução dos comandos sem animação), a thread `zefiro-wm-keys` (gancho de
teclado) e o `Animator`, que roda na thread da interface reaproveitando o `Comp` e o dispositivo WARP
da barra. O `zefiro-app` liga e desliga tudo pelo config (`wm_loop.rs`).

**Stack:** Rust 2024, crate `windows` 0.61.3 (features novas: `Win32_UI_Accessibility`,
`Graphics_Capture`, `Win32_System_WinRT_Graphics_Capture`), Windows.UI.Composition, DWM.

**Spec:** `docs/superpowers/specs/2026-10-21-gerenciador-de-janelas-design.md` (leia junto).

Ledger: `.superpowers/sdd/2026-10-21-gerenciador-de-janelas/progress.md`. Trabalho direto na `main`.
Verificação de cada tarefa: `cargo test -p <crate>`; no fim de cada tarefa que toca Rust,
`cargo clippy --workspace --all-targets -- -D warnings` e
`pwsh -NoProfile -File scripts/check-windows-imports.ps1`.

## Restrições globais

- Crate `windows` só em `crates/platform-windows`, `crates/integrations/clock/src/windows.rs` e
  `crates/app`. `zefiro-wm` e `zefiro-core` não a usam; o CI compila `zefiro-wm` no Linux.
- `clippy::all = deny`; `unsafe` só em blocos pequenos com `// SAFETY:`.
- Textos de interface, comentários e logs em pt-BR; identificadores e commits em inglês
  (`feat(wm): …`, `feat(platform-windows): …`, `feat(app): …`, `feat(bar): …`, `docs(adr): …`).
- Título de janela nunca vai para log, HUD ou diagnóstico (exe e classe podem).
- Animação só no compositor; nada de `WM_TIMER` nem `SetWindowPos` por quadro.
- Um único dispositivo D3D11 WARP no processo (ADR-0001). Se o S1 provar que a captura precisa de
  um dispositivo de GPU, isso vira ADR antes do código.
- Nenhuma janela do Zefiro do tamanho exato do monitor: a camada da animação cobre a área útil; se
  a área útil for igual ao monitor, fica com 1 px a menos de altura.
- `USER_*` novos: use o próximo número livre em `crates/app/src/event_loop.rs` **no momento** da
  tarefa (há outras sessões adicionando códigos).
- Sondas que movem janelas ou mandam teclas: peça um momento ao usuário antes (AGENTS.md, regra 6).
- Textos exatos: item do menu "Janelas em mosaico"; catálogo `workspaces`, nome "Áreas de trabalho",
  descrição "Áreas de trabalho do mosaico neste monitor; clique troca"; HUD "Mosaico"; aviso de
  elevação "janela de app elevado fica fora do mosaico ({exe})"; aviso do cloak "esconder pelo shell
  indisponível nesta versão do Windows; usando minimizar".

## Foco da revisão

1. **App que recusa o tamanho** (tamanho mínimo, janela que se redimensiona sozinha): o Zefiro nunca
   reaplica em laço. Teste `app_moving_itself_never_rearranges` (Tarefa 6).
2. **Janela destruída enquanto escondida em outra área**: sai da área e da lista de recuperação.
   Teste `destroyed_hidden_window_leaves_workspace` (Tarefa 7) e `hidden_list_remove` (Tarefa 5).
3. **Monitor desconectado com janelas escondidas nas áreas dele**: todas vão para a área ativa do
   principal e aparecem. Teste `unplugged_monitor_brings_every_window_to_primary` (Tarefa 7).
4. **Atalhos com uma janela fora do mosaico em foco** (app elevado, flutuante, área de trabalho):
   Win+setas passa para o Windows; trocar de área funciona. Teste
   `move_keys_need_a_tiled_foreground` (Tarefa 4).
5. **Desligar o WM com janelas escondidas**: todas voltam. Teste
   `release_all_shows_every_hidden_window` (Tarefa 7).

---

### Tarefa 1: Spikes S1, S2 e S3 (sondas + rascunho do ADR-0021)

Sem TDD (spike). Cada sonda é um `example` da `platform-windows`; o resultado vai no ledger e num
rascunho `docs/adr/0021-gerenciador-de-janelas.md` (modelo em `docs/adr/0000-modelo.md`, status
"proposto"). Peça um momento ao usuário antes de rodar S2 e S3.

**Files:**
- Modify: `Cargo.toml` (features da crate `windows`: `Win32_UI_Accessibility`, `Graphics_Capture`,
  `Win32_System_WinRT_Graphics_Capture`)
- Create: `crates/platform-windows/examples/wm_capture_spike.rs`
- Create: `crates/platform-windows/examples/wm_cloak_spike.rs`
- Create: `crates/platform-windows/examples/wm_keys_spike.rs`
- Create: `docs/adr/0021-gerenciador-de-janelas.md`

- [ ] **Passo 1: S1, imagem da janela.** `wm_capture_spike <exe>` acha a primeira janela visível do
  exe (`EnumWindows`) e, sobre o dispositivo WARP de `Gfx::new()`, captura **um** quadro com
  `Windows.Graphics.Capture` (`IGraphicsCaptureItemInterop::CreateForWindow`,
  `Direct3D11CaptureFramePool::CreateFreeThreaded`, 1 buffer, `IsBorderRequired(false)`,
  `IsCursorCaptureEnabled(false)`), copia para uma `CompositionDrawingSurface` e grava um PNG em
  `%TEMP%` para conferir. Imprime: ms até o quadro, memória privada antes/depois/depois de soltar, se
  apareceu a borda amarela, se houve pedido de permissão. Rodar com `notepad.exe`, `brave.exe`,
  `Code.exe` e com uma janela escondida pelo S2 (cloak).
  **Aprovado se:** ≤ 16 ms por janela, sem borda nem pedido, imagem correta nos apps de GPU, memória
  devolvida depois de soltar. Janela escondida sem imagem não reprova (vira a regra "a área que
  entra aparece no fim", Tarefa 13). Reprovado → testar o mesmo com um dispositivo D3D11 de hardware
  só durante a captura e medir RAM/tempo; se ainda assim não servir, o caminho é o 2 da spec
  (miniatura do DWM). Registrar a decisão no ADR antes da Tarefa 13.
- [ ] **Passo 2: S2, esconder pelo shell.** `wm_cloak_spike <exe>` esconde a janela por
  `IApplicationView::SetCloak` (via `CoCreateInstance(CLSID_ImmersiveShell)` →
  `IServiceProvider::QueryService(IApplicationViewCollection)` → `GetViewForHwnd`), espera 3 s,
  mostra de novo. Escreva as interfaces a partir da descrição (vtable, GUIDs da build 26200), **sem
  copiar código do komorebi**. Conferir: some da tela, da barra de tarefas e do Alt+Tab; volta no
  mesmo lugar e tamanho; Electron (VS Code) volta desenhado. Imprimir a build (`RtlGetVersion`).
- [ ] **Passo 3: S3, teclado.** `wm_keys_spike` instala `WH_KEYBOARD_LL` numa thread, engole
  Win+←/→/↑/↓ (down e up da seta), injeta `VK 0xE8` (down+up, `SendInput`) ao engolir com Win
  pressionado e imprime cada atalho visto. Conferir: o encaixe nativo não acontece, o Iniciar não
  abre ao soltar o Win, digitar normal não atrasa, eventos com `LLKHF_INJECTED` passam direto.
- [ ] **Passo 4: Registrar.** Números e decisões dos três no ADR-0021 (proposto) e no ledger.
- [ ] **Passo 5: Commit** `feat(platform-windows): window manager spikes (capture, cloak, keys)`.

---

### Tarefa 2: Crate `zefiro-wm` e a árvore dwindle

**Files:**
- Modify: `Cargo.toml` (membro `crates/modules/wm`, dependência `zefiro-wm`)
- Create: `crates/modules/wm/Cargo.toml` (deps: `zefiro-core`; `[lints] workspace = true`)
- Create: `crates/modules/wm/src/lib.rs`, `crates/modules/wm/src/geom.rs`,
  `crates/modules/wm/src/tree.rs`

**Interfaces:**
- Produces (`geom.rs`):
  - `pub type WinId = u64;` (HWND como inteiro)
  - `#[derive(Copy, Clone, Debug, PartialEq, Eq)] pub struct PxRect { pub x: i32, pub y: i32, pub w: i32, pub h: i32 }`
    com `right()`, `bottom()`, `center() -> (i32, i32)`, `contains(x, y) -> bool`
  - `#[derive(Copy, Clone, Debug, PartialEq, Eq)] pub enum Dir { Left, Right, Up, Down }`
- Produces (`tree.rs`):
  - `#[derive(Copy, Clone, Debug, PartialEq, Eq)] pub enum Split { Side, Stack }` (lado a lado / em
    cima e embaixo)
  - `pub struct Tree` (arena; nós internos guardam `Split` e `ratio: f32`)
  - `Tree::new()`, `is_empty()`, `len()`, `contains(WinId)`, `windows() -> Vec<WinId>` (ordem da
    árvore, esquerda→direita)
  - `insert(&mut self, w: WinId, target: Option<(WinId, PxRect)>)`: divide a folha `target`; o
    sentido vem do retângulo (`w >= h` → `Side`); a nova fica à direita/embaixo; razão 0,5. Sem
    `target`, divide a última folha inserida; árvore vazia → raiz.
  - `remove(&mut self, w: WinId) -> bool` (a irmã ocupa o lugar do pai)
  - `swap(&mut self, a: WinId, b: WinId)`
  - `toggle_split(&mut self, w: WinId)` (inverte o `Split` do pai de `w`)
  - `set_ratio(&mut self, w: WinId, edge: Dir, ratio: f32)`: acha o ancestral mais próximo cuja
    divisão é a borda `edge` de `w` e grava a razão, presa em `0.1..=0.9`

- [ ] **Passo 1: Testes que falham** em `tree.rs`:
  - `first_window_is_root`: `insert(1, None)` → `windows() == [1]`
  - `wide_target_splits_side_by_side_new_goes_right`: `insert(1,None)`; `insert(2, Some((1, PxRect{x:0,y:0,w:1600,h:900})))`
    → `windows() == [1,2]` e o pai é `Split::Side`
  - `tall_target_stacks`: alvo 800×900 → `Split::Stack`
  - `remove_promotes_sibling`: 1,2,3 (3 dividindo 2) → `remove(2)` → `windows() == [1,3]`, `len()==2`
  - `swap_exchanges_leaves`: `swap(1,3)` → `windows() == [3,2,1]`
  - `ratio_is_clamped`: `set_ratio(1, Dir::Right, 0.95)` → razão do pai `0.9`
  - `toggle_split_flips_parent`
- [ ] **Passo 2:** `cargo test -p zefiro-wm` → FAIL (não compila).
- [ ] **Passo 3:** implementar `geom.rs` e `tree.rs`.
- [ ] **Passo 4:** `cargo test -p zefiro-wm` → PASS.
- [ ] **Passo 5: Commit** `feat(wm): dwindle tree`.

---

### Tarefa 3: Layout, vizinhos e DPI

**Files:**
- Create: `crates/modules/wm/src/layout.rs`

**Interfaces:**
- Consumes: `Tree`, `PxRect`, `Dir`, `WinId` (Tarefa 2)
- Produces:
  - `#[derive(Copy, Clone, Debug, PartialEq, Eq)] pub struct Gaps { pub inner: i32, pub outer: i32 }` (px)
  - `pub fn px(dip: f32, dpi: u32) -> i32` (`(dip * dpi / 96).round()`)
  - `pub fn layout(tree: &Tree, area: PxRect, gaps: Gaps) -> Vec<(WinId, PxRect)>`
  - `pub fn neighbor(placed: &[(WinId, PxRect)], from: WinId, dir: Dir) -> Option<WinId>`
  - `pub fn neighbor_monitor(monitors: &[(String, PxRect)], from: &str, dir: Dir) -> Option<String>`

Semântica dos espaços (como no Hyprland): `outer` é tirado da área uma vez; entre duas janelas
irmãs ficam `2 * inner` px. Ruling: a spec diz só "entre janelas (`gaps_in`)"; adotamos o 2× do
Hyprland para os padrões 5/20 renderem igual a ele.

Arredondamento: a borda da divisão é `round(início + razão × (tamanho − 2·inner))`; a irmã B começa
em `fim de A + 2·inner`, de modo que A e B somados com o espaço dão exatamente o tamanho do pai.

Vizinho: candidatos inteiramente do lado `dir` (tolerância de `2·inner + 2` px) com sobreposição
positiva no eixo perpendicular; menor distância no eixo de `dir`, depois maior sobreposição, depois
centro mais próximo. `neighbor_monitor` usa a mesma regra sobre os retângulos dos monitores.

- [ ] **Passo 1: Testes que falham:**
  - `single_window_fills_area_minus_outer`: área `{0,0,1920,1040}`, gaps `{5,20}` → `{20,20,1880,1000}`
  - `two_windows_have_twice_inner_between`: as duas lado a lado → `a.right() + 10 == b.x` e
    `a.w + 10 + b.w == 1880`
  - `odd_sizes_leave_no_one_pixel_hole`: área de largura ímpar 1881, três janelas → soma das
    larguras + espaços == largura útil
  - `px_scales_with_dpi`: `px(5.0,144) == 8`, `px(20.0,96) == 20`
  - `neighbor_right_and_none_at_edge`: layout de 3 janelas (1 | 2/3) → `neighbor(1,Right)` é a de
    maior sobreposição (2, a de cima, com empate pelo centro) e `neighbor(2,Right) == None`
  - `neighbor_monitor_left_of_primary`: monitores `("a",{0,0,1920,1080})`, `("b",{-1920,0,1920,1080})`
    → `neighbor_monitor("a", Left) == Some("b")`, `Up == None`
- [ ] **Passo 2:** `cargo test -p zefiro-wm layout` → FAIL.
- [ ] **Passo 3:** implementar.
- [ ] **Passo 4:** → PASS.
- [ ] **Passo 5: Commit** `feat(wm): layout with gaps and directional neighbors`.

---

### Tarefa 4: Config `[wm]`, regras e atalhos

**Files:**
- Modify: `crates/core/src/config.rs` (`WmConfig` em `Config`, `DEFAULT_CONFIG`, `Config::default`,
  `same_except_panels` não muda)
- Create: `crates/modules/wm/src/rules.rs`, `crates/modules/wm/src/keys.rs`,
  `crates/modules/wm/src/settings.rs`

**Interfaces:**
- Produces (`zefiro-core`):
  - `pub struct WmConfig { pub enabled: bool, pub workspaces: u8, pub gaps_in: f32, pub gaps_out: f32, pub animations: bool, pub focus_border: bool, pub keys: WmKeys, pub rules: Vec<WmRule> }`
    (`#[serde(default)]`; padrões `false, 5, 5.0, 20.0, true, true`)
  - `pub struct WmKeys { pub move_left, move_right, move_up, move_down, focus_left, focus_right, focus_up, focus_down, workspace_prev, workspace_next, move_to_prev, move_to_next, workspace_n, move_to_n, toggle_float, toggle_split: String }`
    com padrões `"win+left"`, `"win+right"`, `"win+up"`, `"win+down"`, `"win+alt+left"`, …,
    `"win+ctrl+left"`, `"win+ctrl+right"`, `"win+ctrl+shift+left"`, `"win+ctrl+shift+right"`,
    `"win+ctrl"` (prefixo; recebe `+1`…`+9`), `"win+ctrl+shift"`, `"win+ctrl+f"`, `"win+ctrl+s"`
  - `pub struct WmRule { pub exe: Option<String>, pub class: Option<String>, pub title: Option<String>, pub action: WmRuleAction }`,
    `pub enum WmRuleAction { Float, Ignore }` (`rename_all = "lowercase"`)
  - `Config.wm: WmConfig`. `validate()`: `workspaces` em `1..=9`, gaps em `0..=100`; erro com
    `"wm.workspaces: …"`.
  - `DEFAULT_CONFIG` ganha, no fim, a seção comentada:
    ```toml
    [wm]
    enabled = false       # janelas em mosaico (a chave "Janelas em mosaico" do menu do Z grava aqui)
    workspaces = 5        # áreas de trabalho por monitor (1 a 9)
    gaps_in = 5           # DIP; entre duas janelas ficam 2 × gaps_in, como no Hyprland
    gaps_out = 20         # DIP, entre as janelas e a borda da área útil
    ```
- Produces (`rules.rs`):
  - `pub struct WindowFacts { pub id: WinId, pub exe: String, pub class: String, pub title: String, pub visible: bool, pub owned: bool, pub caption: bool, pub thick_frame: bool, pub tool_window: bool, pub no_activate: bool, pub own_process: bool, pub elevated: bool, pub system_cloaked: bool, pub rect: PxRect, pub monitor: String }`
  - `#[derive(Copy, Clone, Debug, PartialEq, Eq)] pub enum Verdict { Tile, Float, Ignore }`
  - `pub fn classify(f: &WindowFacts, rules: &[WmRule]) -> Verdict` (regras do usuário primeiro,
    depois as padrão; comparação de exe e classe sem diferenciar maiúsculas, título por trecho)
  - `pub fn default_rules() -> Vec<WmRule>`: as da spec ("Quem entra no mosaico")
- Produces (`keys.rs`):
  - `#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)] pub enum Key { Left, Right, Up, Down, Digit(u8), Letter(char), Space, Enter, Tab, F(u8) }`
  - `#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)] pub struct Chord { pub win: bool, pub ctrl: bool, pub alt: bool, pub shift: bool, pub key: Key }`
  - `pub fn parse_chord(s: &str) -> Result<Chord, String>` (minúsculas, `+` como separador; `""` →
    erro "atalho vazio" tratado como desligado pelo chamador)
  - `#[derive(Copy, Clone, Debug, PartialEq, Eq)] pub enum Action { Move(Dir), Focus(Dir), WorkspacePrev, WorkspaceNext, MoveToPrev, MoveToNext, Workspace(u8), MoveTo(u8), ToggleFloat, ToggleSplit, SwitchTo { monitor_index: u8, workspace: u8 } }`
    (`SwitchTo` vem do clique no chip, não de tecla)
  - `#[derive(Copy, Clone, Debug, PartialEq, Eq)] pub enum Needs { Tiled, Managed, Nothing }`;
    `Action::needs(self) -> Needs` (Move/Focus/ToggleSplit → `Tiled`; MoveTo*/ToggleFloat →
    `Managed`; Workspace*/SwitchTo → `Nothing`)
  - `pub struct Keymap`; `Keymap::from_config(k: &WmKeys) -> (Keymap, Vec<String>)` (erros com o nome
    do campo); `lookup(&self, c: &Chord) -> Option<Action>`
- Produces (`settings.rs`): `pub struct WmSettings { pub workspaces: u8, pub gaps_dip: (f32, f32), pub animations: bool, pub focus_border: bool, pub keymap: Keymap, pub rules: Vec<WmRule> }`,
  `WmSettings::from_config(c: &WmConfig) -> (WmSettings, Vec<String>)`

- [ ] **Passo 1: Testes que falham:**
  - core: `wm_defaults_when_section_missing` (`parse_config("theme=\"obsidian\"")` → `wm.enabled == false`, `workspaces == 5`);
    `wm_rules_parse` (`[[wm.rules]] exe="Spotify.exe" action="float"`);
    `wm_workspaces_out_of_range_is_an_error` (`workspaces = 12` → `validate()` erro com `wm.workspaces`);
    `default_config_parses_with_wm_section`
  - rules: `normal_window_tiles`; `zefiro_and_tool_windows_are_ignored` (`own_process` →
    `Ignore`; `tool_window` → `Ignore`; `owned` → `Ignore`); `elevated_is_ignored`;
    `fixed_size_floats` (`thick_frame=false, caption=true` → `Float`);
    `user_rule_wins_over_default` (`exe="SPOTIFY.EXE"` casa `Spotify.exe`);
    `pip_title_floats` (título contém "Picture-in-Picture")
  - keys: `parses_win_ctrl_shift_left`; `rejects_unknown_key`;
    `digits_expand_from_prefix` (`workspace_n="win+ctrl"` → `win+ctrl+3` → `Workspace(3)`, `+0` não existe);
    `move_keys_need_a_tiled_foreground` (`Move(Left).needs()==Tiled`, `Workspace(1).needs()==Nothing`,
    `ToggleFloat.needs()==Managed`); `empty_binding_disables_action`
- [ ] **Passo 2:** `cargo test -p zefiro-core -p zefiro-wm` → FAIL.
- [ ] **Passo 3:** implementar.
- [ ] **Passo 4:** → PASS.
- [ ] **Passo 5: Commit** `feat(wm): wm config, window rules and keymap`.

---

### Tarefa 5: Lista de recuperação das janelas escondidas

**Files:**
- Create: `crates/modules/wm/src/recovery.rs`

**Interfaces:**
- Produces: `pub struct HiddenList(BTreeSet<WinId>)` com `add`, `remove`, `iter`, `is_empty`,
  `to_text(&self) -> String` (um HWND decimal por linha) e `parse(text: &str) -> HiddenList`
  (ignora linhas inválidas)

- [ ] **Passo 1: Testes:** `round_trip`; `hidden_list_remove`; `garbage_lines_are_skipped`
  (`"12\nabc\n\n34"` → `{12,34}`).
- [ ] **Passo 2:** FAIL. **Passo 3:** implementar. **Passo 4:** PASS.
- [ ] **Passo 5: Commit** `feat(wm): hidden window recovery list`.

---

### Tarefa 6: Modelo `Wm` — janelas, foco e arranjo

**Files:**
- Create: `crates/modules/wm/src/state.rs`, `crates/modules/wm/src/model.rs`

**Interfaces:**
- Consumes: Tarefas 2–4.
- Produces (`state.rs`):
  - `pub struct MonitorInfo { pub id: String, pub work: PxRect, pub dpi: u32, pub primary: bool }`
    (`id` = `identity.id` ou, sem ele, o `device`)
  - `pub enum Event { Shown(WindowFacts), Destroyed(WinId), Hidden(WinId), Minimized(WinId), Restored(WindowFacts), Focused(WinId), Maximized { id: WinId, on: bool }, Fullscreen { id: WinId, on: bool }, MovedToMonitor { id: WinId, monitor: String }, MoveSizeEnded { id: WinId, rect: PxRect, cursor: (i32, i32) }, Action(Action), Monitors(Vec<MonitorInfo>), Settings(WmSettings) }`
  - `pub enum MoveKind { Open, Move }`
  - `pub struct Move { pub id: WinId, pub from: Option<PxRect>, pub to: PxRect, pub kind: MoveKind }`
  - `pub enum Command { Arrange { monitor: String, moves: Vec<Move> }, Slide { monitor: String, out: Vec<(WinId, PxRect)>, into: Vec<(WinId, PxRect)>, toward: Dir }, Hide(Vec<WinId>), Show(Vec<WinId>), Focus(WinId), Border { focused: Option<WinId> }, Manage(WinId), Unmanage(WinId) }`
  - `pub struct WorkspaceView { pub monitor: String, pub count: u8, pub active: u8, pub occupied: Vec<u8> }`
  - `#[derive(Copy, Clone, Debug, PartialEq, Eq)] pub enum Membership { Tiled, Floating, None }`
- Produces (`model.rs`): `pub struct Wm` com
  - `Wm::new(settings: WmSettings, monitors: Vec<MonitorInfo>) -> Wm`
  - `adopt(&mut self, existing: Vec<WindowFacts>) -> Vec<Command>` (ordem de Z de baixo para cima;
    tudo na área 1 do monitor de cada janela)
  - `handle(&mut self, e: Event) -> Vec<Command>`
  - `release_all(&mut self) -> Vec<Command>` (`Show` de todas as escondidas, `Unmanage` de todas,
    `Border { focused: None }`)
  - `membership(&self, id: WinId) -> Membership`
  - `views(&self) -> Vec<WorkspaceView>` (ordem dos monitores recebida)

Regras desta tarefa (as de áreas e monitores são da Tarefa 7, as de teclas e mouse da Tarefa 8):
- `Shown`: `classify`; `Tile` → entra na área ativa do monitor da janela (`target` = foco daquela
  área com o retângulo atual do layout) e sai `Manage(id)` + `Arrange` (a nova com `MoveKind::Open`,
  `from: None`; as outras com `from` = retângulo antigo); `Float` → membro flutuante, `Manage`;
  `Ignore` → nada. Já conhecido → nada.
- `Destroyed`/`Hidden`/`Minimized`: sai da árvore e do foco; `Arrange` das restantes; sem
  `Unmanage` (a janela não existe ou não está visível). `Restored` = `Shown`.
- `Focused(id)`: guarda como o foco da área dela; sai `Border { focused: Some(id) }` se for gerenciada
  e `focus_border`, senão `Border { focused: None }`.
- `Maximized { on: true }`: nada; `on: false` → `Arrange` só dela (`from: None`).
- `Fullscreen { on: true }`: sai da árvore (fica lembrada como "suspensa" na área) + `Arrange`;
  `on: false` → volta como `Shown`.
- `MovedToMonitor` para o monitor em que ela já está → nada (é a defesa contra app que se move).
- `Settings`: troca as configurações e reaplica o layout de todas as áreas ativas (`Arrange`).
- `Arrange` só lista janelas cujo retângulo mudou.

- [ ] **Passo 1: Testes que falham** (`model.rs`, com um helper `facts(id, monitor, rect)` e dois
  monitores `"a"` `{0,0,1920,1040}` e `"b"` `{1920,0,1920,1040}`, dpi 96, gaps 5/20):
  - `shown_window_is_managed_and_fills_area`: → `[Manage(1), Arrange{ "a", [Move{1, None, {20,20,1880,1000}, Open}] }]`
  - `second_window_splits_focused_and_moves_first`: foco em 1, `Shown(2)` → `Arrange` com 1 de
    `{20,20,1880,1000}` para `{20,20,935,1000}` (`Move`) e 2 em `{965,20,935,1000}` (`Open`)
  - `ignored_window_produces_nothing`
  - `destroyed_window_rearranges_rest`
  - `focus_sets_border_only_for_managed`
  - `app_moving_itself_never_rearranges`: `MovedToMonitor{1,"a"}` com 1 já em `"a"` → `[]`
  - `fullscreen_leaves_and_returns`
  - `settings_change_reapplies_gaps`: gaps 0/0 → `Arrange` com `{0,0,1920,1040}`
- [ ] **Passo 2:** FAIL. **Passo 3:** implementar. **Passo 4:** PASS.
- [ ] **Passo 5: Commit** `feat(wm): window lifecycle model`.

---

### Tarefa 7: Modelo `Wm` — áreas de trabalho e monitores

**Files:**
- Modify: `crates/modules/wm/src/model.rs`

**Interfaces:**
- Consumes: Tarefa 6.
- Regras:
  - `Action(Workspace(n))` / `WorkspacePrev`/`Next` (sem volta: na 1 o Prev não faz nada, na última
    o Next não faz nada; `n > workspaces` → nada) no monitor do foco (sem foco: o monitor principal)
    → `Slide { monitor, out: [(id, rect atual)], into: [(id, rect do layout)], toward }` (`toward`
    = `Left` quando o índice cai, `Right` quando sobe; `Workspace(n)` segue a mesma regra) e
    `Focus(último foco da área nova)` se houver.
  - `Action(SwitchTo { monitor_index, workspace })`: igual, no monitor de índice `monitor_index`
    da lista recebida.
  - `Action(MoveTo(n))` / `MoveToPrev`/`Next`: a janela em foco sai da árvore (`Arrange` do resto),
    entra na área destino e o monitor troca para ela (`Slide`, com a janela movida no `into`).
  - `Monitors(list)`: monitor que sumiu → todas as janelas de **todas** as áreas dele vão para a área
    ativa do principal (`Show` das escondidas + `Arrange`). Monitor novo → áreas vazias. Área útil
    mudou → `Arrange` da área ativa.
  - `Shown` sempre entra na área **ativa** do monitor da janela.
  - O modelo guarda o conjunto de escondidas: `Slide.out` e `Hide` entram; `Slide.into` e `Show`
    saem.

- [ ] **Passo 1: Testes que falham:**
  - `switch_workspace_slides_out_and_in`: 1 na área 1 de "a"; `Workspace(2)` → `Slide{ "a", out:[(1,…)], into:[], toward: Right }`;
    `views()[0].active == 2`, `occupied == [1]`
  - `other_monitor_is_untouched`: janela em "b" não aparece em nenhum comando
  - `workspace_beyond_count_is_noop`: `Workspace(7)` com 5 → `[]`
  - `move_to_workspace_takes_window_along`
  - `focus_returns_to_last_focused_of_workspace`
  - `destroyed_hidden_window_leaves_workspace`: 1 escondida na área 1; `Destroyed(1)` →
    `views()[0].occupied` sem a 1 e nenhum `Show(1)` depois
  - `unplugged_monitor_brings_every_window_to_primary`: janelas em "b" áreas 1 e 3 (a da 3
    escondida); `Monitors([só "a"])` → `Show([id da área 3])` e `Arrange` em "a" com as duas
  - `release_all_shows_every_hidden_window`
- [ ] **Passo 2:** FAIL. **Passo 3:** implementar. **Passo 4:** PASS.
- [ ] **Passo 5: Commit** `feat(wm): per-monitor workspaces`.

---

### Tarefa 8: Modelo `Wm` — mover, focar, flutuar e arrastar

**Files:**
- Modify: `crates/modules/wm/src/model.rs`

**Interfaces:**
- Consumes: `neighbor`, `neighbor_monitor`, `Tree::swap`, `set_ratio`, `toggle_split`.
- Regras:
  - `Move(dir)`: vizinha na área → `swap` + `Arrange`; sem vizinha e com monitor naquele lado → sai
    da árvore, entra na área ativa do outro monitor dividindo o foco dela; senão → `[]`.
  - `Focus(dir)`: vizinha → `Focus(id)`; senão o último foco do monitor vizinho; senão `[]`.
  - `ToggleFloat`: lado a lado ↔ flutuante (`Arrange` ao entrar; ao sair, `Arrange` do resto e a
    janela fica onde está).
  - `ToggleSplit`: `toggle_split` + `Arrange`.
  - `MoveSizeEnded { id, rect, cursor }` de uma janela no mosaico: se `w`/`h` diferem do slot em no
    máximo 2 px é **movimento**: janela gerenciada sob `cursor` (pelo layout de qualquer monitor)
    → troca de lugar (ou muda de monitor); nenhuma → `Arrange` de volta com `from: Some(rect)`. Se
    o tamanho mudou é **redimensionar**: para cada borda que andou mais de 2 px, `set_ratio` com a
    nova posição relativa ao pai, depois `Arrange`. Janela flutuante: só atualiza o monitor.

- [ ] **Passo 1: Testes que falham:**
  - `move_right_swaps_with_neighbor`
  - `move_right_at_edge_goes_to_next_monitor`
  - `move_with_no_neighbor_and_no_monitor_is_noop`
  - `focus_left_from_monitor_b_reaches_monitor_a`
  - `toggle_float_removes_from_tree`
  - `drag_drop_on_other_window_swaps`
  - `drag_drop_on_nothing_snaps_back` (`Arrange` com `from == Some(rect solto)`)
  - `resize_right_edge_updates_ratio` (borda direita de 1 vai de 955 para 1255 → razão ≈ 0,66 e o
    `Arrange` reflete)
- [ ] **Passo 2:** FAIL. **Passo 3:** implementar. **Passo 4:** PASS.
- [ ] **Passo 5: Commit** `feat(wm): directional move/focus, float toggle and mouse drag`.

---

### Tarefa 9: Plano de animação (portável)

**Files:**
- Create: `crates/modules/wm/src/anim.rs`

**Interfaces:**
- Consumes: `Command`, `Move`, `MoveKind`, `PxRect`.
- Produces:
  - `#[derive(Copy, Clone, Debug, PartialEq)] pub struct SpriteState { pub rect: PxRect, pub opacity: f32 }`
  - `#[derive(Clone, Debug, PartialEq)] pub struct Sprite { pub id: WinId, pub from: SpriteState, pub to: SpriteState, pub reveal: bool }`
    (`reveal`: mostrar a janela real no fim)
  - `pub fn plan(cmd: &Command, work: PxRect) -> Vec<Sprite>`, com retângulos relativos a `work`
  - `pub const OPEN_SCALE: f32 = 0.85;`

Regras: `Move` com `from` → de `from` para `to`, opacidade 1→1, `reveal`. `Open` (ou `from: None`)
→ de um retângulo com 85 % do `to`, centrado nele, opacidade 0→1, `reveal`. `Slide` com `toward:
Right` → `out` vai de `rect` para `rect` deslocado `−work.w` em x, sem `reveal`; `into` vem de
`+work.w` para `rect`, com `reveal` (`Left` é o espelho). Outros comandos → `vec![]`.

Fechar não tem sprite próprio: quando o `Destroyed`/`Hidden` chega, a janela já sumiu e não há o
que capturar; só as restantes se movem (ruling; a spec já previa esse caso).

- [ ] **Passo 1: Testes:** `move_goes_from_old_to_new_relative_to_work`; `open_pops_in_from_85_percent`;
  `slide_right_pushes_out_left`; `hide_has_no_sprites`.
- [ ] **Passo 2:** FAIL. **Passo 3:** implementar. **Passo 4:** PASS.
- [ ] **Passo 5: Commit** `feat(wm): animation plan`.

---

### Tarefa 10: Plataforma — informações da janela, eventos e a thread `zefiro-wm`

**Files:**
- Modify: `crates/platform-windows/Cargo.toml` (dep `zefiro-wm`), `crates/platform-windows/src/lib.rs`
  (`pub mod wm;`)
- Create: `crates/platform-windows/src/wm/mod.rs`, `wm/window_info.rs`, `wm/events.rs`,
  `wm/host.rs`
- Create: `crates/platform-windows/examples/wm_probe.rs`

**Interfaces:**
- Consumes: `zefiro_wm::{Wm, Event, Command, WindowFacts, MonitorInfo, WmSettings, WorkspaceView, Membership}`.
- Produces:
  - `window_info::facts(hwnd: HWND, monitors: &[MonitorInfo]) -> Option<WindowFacts>` (estilos,
    classe, exe por `QueryFullProcessImageNameW`, elevação por `GetTokenInformation(TokenElevation)`
    com cache por PID, `DWMWA_CLOAKED`, `GetWindow(GW_OWNER)`, monitor por `MonitorFromWindow`)
  - `window_info::frame_inset(hwnd: HWND) -> (i32, i32, i32, i32)` (diferença entre
    `GetWindowRect` e `DWMWA_EXTENDED_FRAME_BOUNDS`: esquerda, cima, direita, baixo)
  - `window_info::monitor_infos() -> Vec<MonitorInfo>` (a partir de `monitors::list()` e `rcWork`)
  - `pub struct WmHost` com
    `WmHost::start(settings: WmSettings, on_views: Arc<dyn Fn() + Send + Sync>, on_transition: Arc<dyn Fn(Transition) + Send + Sync>) -> windows::core::Result<WmHost>`,
    `send(&self, e: Event)`, `views(&self) -> Vec<WorkspaceView>`, `foreground_needs(&self) -> Membership`
    (lido sem trava, de um `AtomicU64` com o HWND e um `AtomicU8` com a membership, atualizados a cada
    `Focused`), `stop(self)` (manda `release_all`, executa, sai do laço e junta a thread)
  - `pub struct Transition { pub monitor_work: PxRect, pub command: Command }` (só para comandos
    animáveis: `Arrange` e `Slide`; o `on_transition` é chamado no lugar de executá-los quando
    `animations` está ligado)
- `events.rs`: `SetWinEventHook` com `WINEVENT_OUTOFCONTEXT | WINEVENT_SKIPOWNPROCESS` nas faixas
  `EVENT_SYSTEM_FOREGROUND`, `EVENT_SYSTEM_MOVESIZESTART..=EVENT_SYSTEM_MINIMIZEEND`,
  `EVENT_OBJECT_DESTROY..=EVENT_OBJECT_HIDE`, `EVENT_OBJECT_LOCATIONCHANGE`,
  `EVENT_OBJECT_CLOAKED..=EVENT_OBJECT_UNCLOAKED`; só `idObject == OBJID_WINDOW && idChild ==
  CHILDID_SELF` de janelas de nível superior. `LOCATIONCHANGE` só vale para janelas gerenciadas e
  só vira evento numa transição (maximizada, tela cheia = retângulo igual ao do monitor e sem
  `WS_CAPTION`, troca de monitor). `CLOAKED`/`UNCLOAKED` de janelas que o próprio Zefiro escondeu
  são descartados; os outros viram `Hidden`/`Shown` (UWP suspenso não é "fechada").
- Veredito `Ignore` por elevação: `tracing::info!` "janela de app elevado fica fora do mosaico
  ({exe})", uma vez por exe enquanto o host vive.
- A thread `zefiro-wm` inicializa COM (STA), instala o gancho, chama `Wm::adopt` com
  `EnumWindows` (ordem de Z invertida), roda o laço de mensagens e executa os comandos (Tarefas 11
  e 12). Nesta tarefa, os comandos são só registrados no log (`debug`).

- [ ] **Passo 1: Sonda que falha antes:** `wm_probe list` imprime, para cada janela visível de
  nível superior, `exe`, `classe`, veredito e monitor (sem título). Antes da implementação não
  compila.
- [ ] **Passo 2:** implementar `window_info.rs`, `events.rs`, `host.rs`.
- [ ] **Passo 3:** `cargo run -p zefiro-platform-windows --example wm_probe -- list` com Notepad,
  VS Code, Brave e o Gerenciador de Tarefas abertos → os três primeiros `Tile`, o Gerenciador
  `Ignore` (elevado), nenhuma janela do Zefiro, Iniciar ou barra de tarefas como `Tile`.
- [ ] **Passo 4:** `wm_probe events` imprime 20 s de eventos do modelo; abrir/fechar/minimizar um
  Notepad gera `Shown`, `Minimized`, `Restored`, `Destroyed` uma vez cada.
- [ ] **Passo 5: Commit** `feat(platform-windows): wm window facts, event hook and host thread`.

---

### Tarefa 11: Plataforma — posicionar, bordas e esconder com recuperação

**Files:**
- Create: `crates/platform-windows/src/wm/place.rs`, `wm/border.rs`, `wm/cloak.rs`
- Modify: `crates/platform-windows/src/wm/host.rs` (executar `Arrange` sem animação, `Hide`,
  `Show`, `Slide` sem animação = `Hide(out)` + `Arrange(into)` + `Show(into)`, `Focus`, `Border`,
  `Manage`, `Unmanage`)
- Modify: `crates/platform-windows/examples/wm_probe.rs`

**Interfaces:**
- Produces:
  - `place::apply(moves: &[(HWND, PxRect)])`: `BeginDeferWindowPos`/`DeferWindowPos`/`EndDeferWindowPos`
    com `SWP_NOACTIVATE | SWP_NOZORDER | SWP_NOOWNERZORDER | SWP_ASYNCWINDOWPOS`, retângulo
    acrescido do `frame_inset` (o retângulo do modelo é o visível). Janela maximizada recebe
    `ShowWindow(SW_RESTORE)` antes.
  - `border::set_focused(hwnd: Option<HWND>, color: Rgba)` (`DWMWA_BORDER_COLOR`; a anterior volta a
    `DWMWA_COLOR_DEFAULT`), `border::manage(hwnd)` (`DWMWA_WINDOW_CORNER_PREFERENCE = DWMWCP_ROUND`),
    `border::unmanage(hwnd)` (`DWMWCP_DEFAULT` + `DWMWA_COLOR_DEFAULT`)
  - `cloak::Cloaker::new(list_path: PathBuf) -> Cloaker` (testa a interface do S2; sem ela, avisa e
    usa `SW_SHOWMINNOACTIVE`/`SW_RESTORE`), `hide(&mut self, hwnd)`, `show(&mut self, hwnd)`,
    `is_hidden(&self, hwnd) -> bool`. Cada mudança grava o `HiddenList` (temporário + renomear)
    **antes** de esconder e depois de mostrar.
  - `cloak::restore_from_file(list_path: &Path) -> usize` (mostra as que existem, zera o arquivo;
    usada na partida, no panic e no `--wm-restore`)
  - `cloak::default_list_path() -> PathBuf` = `%LOCALAPPDATA%\zefiro\wm-hidden.txt`
- A cor da borda vem de `WmHost::set_accent(Rgba)` (o app passa o `accent` do tema).

- [ ] **Passo 1: Sonda que falha antes:** `wm_probe tile` sobe um `WmHost` com animações desligadas
  e confere, depois de abrir 3 Notepads, que os retângulos visíveis (`DWMWA_EXTENDED_FRAME_BOUNDS`)
  batem com `zefiro_wm::layout` (±1 px) e que o espaço entre vizinhas é `2 × px(5)`; imprime
  `OK`/`FALHOU` por janela. Antes: `FALHOU` (comandos só no log).
- [ ] **Passo 2:** implementar `place.rs`, `border.rs`, `cloak.rs` e a execução no `host.rs`.
- [ ] **Passo 3:** pedir um momento ao usuário; `wm_probe tile` → `OK` nas 3.
- [ ] **Passo 4:** `wm_probe hide-kill`: esconde um Notepad, encerra o processo da sonda com
  `std::process::exit` sem mostrar; `wm_probe restore` (chama `restore_from_file`) → o Notepad volta
  e o arquivo fica vazio.
- [ ] **Passo 5: Commit** `feat(platform-windows): wm placement, borders and recoverable hiding`.

---

### Tarefa 12: Plataforma — gancho de teclado

**Files:**
- Create: `crates/platform-windows/src/wm/keys.rs`
- Modify: `crates/platform-windows/src/wm/host.rs` (sobe/desce a thread de teclas junto com o host)

**Interfaces:**
- Consumes: `Keymap::lookup`, `Action::needs`, `WmHost::foreground_needs`.
- Produces: `keys::KeyHook::start(keymap: Keymap, needs: Arc<dyn Fn() -> Membership + Send + Sync>, on_action: Arc<dyn Fn(Action) + Send + Sync>) -> windows::core::Result<KeyHook>`,
  `set_keymap(&self, Keymap)`, `Drop` desinstala e junta a thread.
- Comportamento: rastreia Win/Ctrl/Alt/Shift pelos próprios eventos; na descida de uma tecla que
  forma um `Chord` do keymap, engole se `action.needs()` for satisfeito pela membership atual
  (`Tiled` exige `Tiled`; `Managed` aceita `Tiled` ou `Floating`; `Nothing` sempre), engole também a
  subida dessa tecla, injeta `VK 0xE8` (desce+sobe) quando Win está pressionado e manda a ação por
  `on_action` (que só faz `PostThreadMessage` para a thread `zefiro-wm`). Não satisfeito → passa.
  `LLKHF_INJECTED` → passa sem olhar. Nada de alocação nem trava no callback (o keymap fica num
  `ArcSwap`-like feito com `AtomicPtr` ou num `RwLock` lido com `try_read`, passando a tecla se
  estiver ocupado).

- [ ] **Passo 1: Sonda que falha antes:** `wm_probe keys` sobe o host e, com um Notepad no
  mosaico, manda `Win+→` por `SendInput` e confere que a janela trocou de lugar com a vizinha e que
  `GetForegroundWindow` depois de soltar o Win ainda é o Notepad (o Iniciar não abriu). Com o
  Gerenciador de Tarefas em foco, confere que o `Win+→` passou (a janela dele encaixou). Antes:
  falha (sem gancho).
- [ ] **Passo 2:** implementar.
- [ ] **Passo 3:** pedir um momento ao usuário; `wm_probe keys` → `OK`.
- [ ] **Passo 4: Commit** `feat(platform-windows): wm keyboard hook without start menu`.

---

### Tarefa 13: Animações (captura, camada e `Animator`)

**Files:**
- Create: `crates/platform-windows/src/wm/capture.rs`, `wm/overlay.rs`, `wm/animator.rs`
- Modify: `crates/ui/src/spring.rs` (`SpringParams::WM_MOVE`, `WM_OPEN`, `WM_WORKSPACE`; partir de
  `SETTLE` e ajustar na tela)
- Modify: `crates/platform-windows/src/compositor.rs` (`spring_rect(&self, visual: &Visual, r: Rect, opacity: f32) -> Result<CompositionPropertySet>`
  e `move_rect(&self, props: &CompositionPropertySet, r: Rect, opacity: f32, spring: SpringParams) -> Result<()>`,
  no molde de `spring_offset`/`move_offset`: `Offset`, `Size` e `Opacity` ligados por expressão a
  `x, y, w, h, o`)

**Interfaces:**
- Consumes: `anim::plan`, `Sprite`, `Transition`, `Cloaker`, `place::apply`, `Comp`, `Gfx`.
- Produces:
  - `capture::snapshot(gfx: &Gfx, comp: &Comp, hwnd: HWND) -> Result<CompositionDrawingSurface>`
    (caminho decidido no S1; com timeout de 50 ms)
  - `overlay::Overlay::new(comp: &Comp, work: Rect) -> Result<Overlay>` (janela
    `WS_EX_LAYERED | WS_EX_TRANSPARENT | WS_EX_NOACTIVATE | WS_EX_TOPMOST | WS_EX_TOOLWINDOW`, sem
    entrada, regra do 1 px), `show`, `hide`, `root() -> &ContainerVisual`
  - `animator::Animator::new() -> Animator`, `run(&mut self, gfx: &Gfx, comp: &Comp, cloaker: &mut Cloaker, t: Transition) -> Result<()>`,
    `settled(&mut self, ticket: usize)` (chamado por `Comp::on_settled` via `post_user`)
- Sequência do `run`: `plan` → `snapshot` de cada sprite (falhou → aquela janela vai direto, sem
  sprite) → sprites na camada com `CompositionSurfaceBrush` (`Stretch::Fill`) e clipe arredondado
  de 8 DIP, no `from` → esconder as janelas reais com sprite → `place::apply` → `move_rect` até o
  `to` → `on_settled`: mostrar as `reveal`, fade de 80 ms nos sprites, esconder a camada, soltar as
  superfícies e chamar `gfx.trim_idle_resources()`. Uma `Transition` nova para uma janela com sprite
  vivo reaproveita o sprite (redireciona a mola, sem nova captura).
- `Slide.into` sem imagem (janela escondida, se o S1 reprovou a captura delas): essas janelas são
  mostradas só no `settled`, sem sprite (ruling já previsto na spec).
- Ruling desta tarefa (substitui o "a thread `zefiro-wm` tem o próprio `Comp`" da spec): a camada e
  a captura rodam na thread da interface, com o `Comp` e o dispositivo WARP da barra, porque o
  ADR-0001 fixa um único dispositivo e um segundo `Gfx`+`Comp` custaria ~5 MB de working set.
- O `Cloaker` passa a ser usado pela thread da interface quando há animação:
  o `WmHost` manda `Transition` e o `Animator` executa esconder/posicionar/mostrar dessas janelas;
  sem animação o host executa tudo. Os dois usam o mesmo `Cloaker` atrás de um `Arc<Mutex<_>>`
  (contenção só em transições).

- [ ] **Passo 1: Sonda que falha antes:** `wm_probe animate` abre 2 Notepads com animações ligadas,
  manda `Win+→` e mede: CPU do processo durante a animação (`GetProcessTimes` antes/depois) e se as
  duas janelas terminam nos retângulos certos (±1 px) e visíveis. Antes: não compila.
- [ ] **Passo 2:** implementar `compositor.rs`, `capture.rs`, `overlay.rs`, `animator.rs` e o
  `on_transition` no host.
- [ ] **Passo 3:** pedir um momento ao usuário; `wm_probe animate` → janelas certas e visíveis, CPU
  do Zefiro < 5 ms durante 300 ms de animação (a captura conta).
- [ ] **Passo 4: Commit** `feat(platform-windows): compositor-driven window animations`.

---

### Tarefa 14: App — liga/desliga, menu do Z, chip `workspaces`, HUD e recuperação

**Files:**
- Create: `crates/app/src/wm_loop.rs`
- Modify: `crates/app/src/main.rs` (`--wm-restore`; `restore_from_file` antes de tudo; panic hook
  encadeado), `crates/app/src/event_loop.rs` (`USER_WM_VIEWS`, `USER_WM_ANIM`, menu, clique no chip,
  troca de monitores e de área útil → `Event::Monitors`), `crates/app/src/system_loop.rs`
  (`refresh_services` chama `wm_services`), `crates/app/src/bar_view.rs` (conteúdo do chip),
  `crates/app/src/debug_loop.rs` (linha "Mosaico" no HUD e no diagnóstico)
- Modify: `crates/modules/bar/src/menu.rs` (`MenuAction::ToggleWm`, linha "Janelas em mosaico" com
  chave, logo abaixo de "Acrílico"; `MenuModel::set_wm(on: bool)`)
- Modify: `crates/modules/bar/src/catalog.rs` (item `workspaces`)
- Create: `crates/modules/bar/src/workspaces.rs` (`pub fn workspaces_chip(view: Option<&WorkspaceViewLite>, chip_h: f32, scale: f32, tokens: &Tokens) -> ChipContent`
  e `pub fn workspace_at(view: &WorkspaceViewLite, x: f32, scale: f32) -> Option<u8>`, com
  `pub struct WorkspaceViewLite { pub count: u8, pub active: u8, pub occupied: Vec<u8> }` (o bar não
  depende de `zefiro-wm`))

**Interfaces:**
- Consumes: `WmHost`, `WmSettings::from_config`, `cloak::restore_from_file`, `default_list_path`.
- Produces (`wm_loop.rs`): `Loop::wm_services(&mut self)` (liga/desliga o host por
  `config.wm.enabled`, troca as settings se só elas mudaram, passa o accent; `animations` efetivo =
  `config.wm.animations && system::animations_enabled()`),
  `Loop::wm_views_changed(&mut self) -> Result<()>` (redesenha os chips `workspaces`),
  `Loop::wm_animate(&mut self) -> Result<()>` (tira as `Transition` da fila e chama o `Animator`),
  `Loop::wm_chip_click(&mut self, monitor_device: &str, x: f32)`.
- Chip: uma bolinha por área (diâmetro 8 DIP, passo 14 DIP); ativa = `accent`; com janelas =
  `text_secondary`; vazia = `text_tertiary` com 50 % de opacidade. Largura fixa = `count × 14 DIP`.
  Sem `WmHost` (WM desligado), o chip não aparece (largura 0).
- Panic: `install_panic_hook` continua igual; logo depois, no `main`, o hook é encadeado:
  `restore_from_file(default_list_path())` e depois o hook anterior.
- Saída (Sair, `WM_CLOSE`, `WM_ENDSESSION`): `WmHost::stop()` antes de destruir as barras.

- [ ] **Passo 1: Testes que falham** (`zefiro-bar`):
  `wm_row_follows_acrylic_with_a_switch` (no molde de `acrylic_row_follows_the_theme_row_with_a_switch`);
  `workspaces_chip_has_fixed_width` (`count 5`, escala 1 → `content_w == 70.0`, igual com
  `active` 1 ou 5); `workspace_at_maps_x_to_index` (`x = 15.0` → `Some(2)`, `x = 80.0` → `None`);
  `new_items_are_in_the_catalog` passa a incluir `workspaces`.
- [ ] **Passo 2:** `cargo test -p zefiro-bar` → FAIL. **Passo 3:** implementar o bar.
  **Passo 4:** PASS.
- [ ] **Passo 5:** implementar o app. Verificação manual (pedir um momento ao usuário), com
  `ZEFIRO_CONFIG_DIR` apontando para um perfil de teste com `workspaces` na barra:
  1. menu do Z → "Janelas em mosaico" liga; as janelas abertas se organizam; `config.toml` ganha
     `[wm] enabled = true`;
  2. Win+Ctrl+→ troca a área do monitor do foco e o chip daquela barra acende a 2; o outro monitor
     não muda;
  3. clique numa bolinha do chip troca de área;
  4. com uma janela escondida na área 2, `Stop-Process -Force` no zefiro e `zefiro.exe --wm-restore`
     → a janela volta;
  5. desligar pela chave → tudo aparece e as bordas voltam ao padrão.
- [ ] **Passo 6: Commit** `feat(app): tiling window manager toggle, workspaces chip and recovery`.

---

### Tarefa 15: Medição, ADR e documentos

**Files:**
- Modify: `docs/adr/0021-gerenciador-de-janelas.md` (status "aceito"), `docs/PLANNING.md` (Fase 6
  adiantada, sem a condição do komorebi; árvore de crates com `modules/wm`), `AGENTS.md` (tabela de
  estrutura: `crates/modules/wm`; "Coisas não óbvias": gancho de teclado e `VK 0xE8`, cloak do shell
  e `wm-hidden.txt`/`--wm-restore`, moldura invisível, animação na thread da interface)

- [ ] **Passo 1:** fechar o YASB, `perf.ps1 -Process zefiro` com `[wm] enabled = false` e `true`
  (3 min em repouso cada, `[island] enabled = false`); `frames.ps1` durante 20 trocas de área.
  Metas da spec: memória privada a mais ≤ 5 MB, CPU em repouso ~0 %. Reabrir o YASB.
- [ ] **Passo 2:** registrar os números e os rulings (2× gaps, animação na thread da interface,
  fechar sem imagem) no ADR-0021 e no ledger.
- [ ] **Passo 3:** `cargo test --workspace`, clippy e `check-windows-imports.ps1` → verdes.
- [ ] **Passo 4: Commit** `docs(adr): tiling window manager`.
- [ ] **Passo 5:** revisão da branch inteira contra a spec (AGENTS.md, fluxo, passo 5); entregar ao
  usuário rulings e minors adiados.
