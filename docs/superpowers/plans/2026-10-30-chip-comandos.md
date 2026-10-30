# Chip Comandos Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Um chip `commands` que abre um painel com dropdown de ferramenta, busca por intenção em português e uma lista de comandos que se copia com Enter ou clique.

**Architecture:** Uma crate portável nova (`zefiro-integration-commands`) guarda o modelo, o pacote embutido em TOML, a leitura dos arquivos do usuário e a busca. O painel (estado, teclado e `DisplayList`) fica em `zefiro-bar` (`commands.rs`), sem depender da crate nova; o `app` liga as duas, copia para a área de transferência e grava `[commands] last`. O único trabalho de plataforma é `Family::Mono`.

**Tech Stack:** Rust 2024, `serde` + `toml` (já no workspace), DirectWrite (`platform-windows`), `toml_edit` via `set_value` do core.

**Spec:** `docs/superpowers/specs/2026-10-30-chip-comandos-design.md`

## Global Constraints

- Docs, comentários, logs e textos da interface em **português do Brasil**; identificadores, commits e branches em **inglês** (`feat(bar): …`). Nunca `git push`.
- A crate `windows` só aparece em `platform-windows`, `app` e `integrations/clock/src/windows.rs`; `pwsh -NoProfile -File scripts/check-windows-imports.ps1` precisa passar.
- `cargo clippy --workspace --all-targets -- -D warnings` e `cargo test --workspace` passam ao fim de cada tarefa. `unsafe` só com `// SAFETY:`.
- Painel fechado = zero trabalho: nada lido, nada em thread, nenhuma rede. Nenhum `WM_TIMER` redesenha; a abertura usa a mola do painel como os outros.
- O texto copiado **nunca termina em quebra de linha** (`\n` ou `\r`).
- Arquivos do usuário: `%APPDATA%\zefiro\commands\*.toml`, só leitura (o Zefiro nunca grava ali). Limites: 64 arquivos, 256 KB cada.
- Ids do catálogo: `commands`. Chave de config: `[commands] last` (id da ferramenta; vazio = Todas).
- Busca: sem acento e sem maiúsculas; palavras vazias em português não contam; ordem `keywords` > `cmd` > `what`.
- Copiar fecha o painel na hora, sem aviso e sem temporizador. Sem Ctrl+Tab.

## Review Focus

Entradas que a spec implica e nenhuma tarefa testaria sozinha; cada linha tem o teste na tarefa dona:

1. Busca só com palavras vazias ("como o") deve se comportar como busca vazia (mostra "Mais usados"), não como "Nada encontrado". → Task 4.
2. Arquivo do usuário com TOML quebrado, entrada sem `cmd`, `cmd` com quebra de linha ou arquivo gigante: só aquele arquivo ou entrada cai, com aviso; o resto carrega e nada dá panic. → Task 3.
3. `[commands] last` apontando para uma ferramenta que sumiu volta para Todas. → Task 6.
4. Ferramenta sem nenhum `popular`: a busca vazia mostra todos os comandos dela (não uma lista vazia). → Task 4.
5. Comando mais comprido que o painel, ou mais resultados que linhas visíveis: o texto é cortado com reticências e a linha escolhida fica sempre à vista. → Task 6.

---

### Task 1: Seção `[commands]` na config

**Files:**
- Modify: `crates/core/src/config.rs` (struct `Config` ~linha 85, `same_except_panels` ~linha 296)
- Test: `crates/core/tests/config.rs`

**Interfaces:**
- Produces: `pub struct CommandsConfig { pub last: String }` (`Default` = vazio), campo `Config.commands: CommandsConfig`; `same_except_panels` ignora `commands`. A gravação usa o `set_value` que já existe: `set_value(&path, &["commands", "last"], id)`.

- [ ] **Step 1: Escrever os testes que falham** em `crates/core/tests/config.rs`: `commands_last_defaults_to_empty` (`CommandsConfig::default().last == ""`), `commands_last_is_read` (`parse_config("[commands]\nlast = \"docker\"\n")` dá `"docker"`), `only_commands_changed_is_same_except_panels` (duas configs que só diferem em `commands.last` → `same_except_panels` é `true`; se diferem também em `theme` → `false`), `set_value_writes_commands_last_keeping_comments` (arquivo com comentário e `[bar]`; depois de `set_value(.., &["commands","last"], "git")` o comentário e o `[bar]` continuam e `parse_config` lê `git`).
- [ ] **Step 2: Rodar e ver falhar** — `cargo test -p zefiro-core --test config commands` → erro de compilação (`CommandsConfig` não existe).
- [ ] **Step 3: Implementar** `CommandsConfig` (mesmo estilo de `SpotifyConfig`: `#[serde(default)]`), o campo em `Config` e o `commands: other.commands.clone()` em `same_except_panels`; atualizar o comentário do método ("`[terminal]`, `[chat]`, `[spotify]`, `[ia]` ou `[commands]`").
- [ ] **Step 4: Rodar** `cargo test -p zefiro-core` → tudo passa.
- [ ] **Step 5: Commit** — `feat(core): [commands] config section`.

---

### Task 2: `Family::Mono`

**Files:**
- Modify: `crates/platform/src/text.rs` (enum `Family` e o doc acima dele), `crates/platform-windows/src/text.rs` (`face`, campo novo, teste)

**Interfaces:**
- Produces: `Family::Mono` (fonte monoespaçada). Em `platform-windows`: `fn mono_face(installed: impl Fn(&str) -> bool) -> &'static str` devolve `"Cascadia Mono"` se instalada, senão `"Consolas"`.

- [ ] **Step 1: Escrever o teste que falha** no `mod tests` de `crates/platform-windows/src/text.rs`: `mono_prefers_cascadia_then_consolas` — com `installed = |n| n == "Cascadia Mono"` devolve `"Cascadia Mono"`; com `installed = |_| false` devolve `"Consolas"`.
- [ ] **Step 2: Rodar e ver falhar** — `cargo test -p zefiro-platform-windows mono_prefers` → não compila.
- [ ] **Step 3: Implementar** `Family::Mono` (doc: "`Mono` = comandos e código"), `mono_face`, e em `DWriteText` um `mono: RefCell<Option<&'static str>>` preenchido na primeira vez que `face(Family::Mono)` roda, com `installed` feito por `GetSystemFontCollection` + `FindFamilyName` (um `// SAFETY:` por bloco `unsafe`). Se a coleção falhar, use `"Consolas"`. `set_family` não limpa `mono` (ele não depende do tema).
- [ ] **Step 4: Rodar** `cargo test -p zefiro-platform-windows` e `cargo build --workspace` (qualquer `match` exaustivo sobre `Family` que quebrar ganha o braço `Mono`).
- [ ] **Step 5: Commit** — `feat(platform-windows): monospace font family`.

---

### Task 3: Crate `zefiro-integration-commands`: modelo, TOML, pasta do usuário e mesclagem

**Files:**
- Create: `crates/integrations/commands/Cargo.toml` (como a do `vscode`; dependências `serde.workspace` e `toml.workspace`, `tempfile` em dev), `crates/integrations/commands/src/lib.rs`, `crates/integrations/commands/src/model.rs`, `crates/integrations/commands/data/` (vazio até a Task 5, com um `docker.toml` mínimo de 1 comando para compilar)
- Modify: `Cargo.toml` (membro `crates/integrations/commands` e `zefiro-integration-commands = { path = … }` em `[workspace.dependencies]`)
- Test: `crates/integrations/commands/src/model.rs` (`mod tests`)

**Interfaces:**
- Produces:
  - `pub struct Command { pub cmd: String, pub what: String, pub keywords: Vec<String>, pub popular: bool }`
  - `pub struct Tool { pub id: String, pub name: String, pub icon: Option<char>, pub commands: Vec<Command> }`
  - `pub struct Library { pub tools: Vec<Tool> }`
  - `pub fn parse_tool(text: &str) -> Result<(Tool, Vec<String>), String>` (a ferramenta e os avisos das entradas descartadas; `Err` se o TOML é inválido ou falta `[tool] id`/`name`)
  - `pub fn builtin() -> &'static Library` (`OnceLock`; ordem fixa: `docker`, `linux`, `powershell`, `python`, `git`, lida de `data/*.toml` com `include_str!`)
  - `pub fn read_user_dir(dir: &Path) -> (Vec<Tool>, Vec<String>)` (ferramentas e avisos)
  - `pub fn merge(builtin: &Library, user: Vec<Tool>) -> Library`
- Formato do arquivo: `[tool]` com `id`, `name` e `icon` opcional (um caractere); `[[command]]` com `cmd`, `what`, `keywords` (lista, opcional) e `popular` (opcional). `id` é aparado e passado a minúsculas.

- [ ] **Step 1: Escrever os testes que falham** em `model.rs`:
  - `parses_a_tool_file`: um TOML com `[tool]` e dois `[[command]]` vira `Tool` com `id`, `name`, `keywords` e `popular` certos.
  - `drops_bad_entries_but_keeps_the_rest`: `cmd` vazio, `cmd` com `"a\nb"` e `what` vazio saem, cada uma com um aviso que cita o arquivo; a entrada boa fica.
  - `bad_toml_or_missing_id_is_an_error`: `parse_tool("[tool\n")` e um arquivo sem `id` dão `Err`.
  - `cmd_is_trimmed_and_never_ends_with_a_newline`: `cmd = "ls -la  \n"` (string multilinha TOML) vira `"ls -la"`.
  - `user_dir_skips_broken_files_and_limits`: pasta temporária com um `.toml` bom, um quebrado, um `.txt` e um arquivo de 300 KB → só o bom entra, com 2 avisos (quebrado e grande); pasta inexistente → `(vazio, vazio)` sem aviso; 70 arquivos → lê só 64, em ordem de nome.
  - `merge_same_cmd_in_same_tool_overrides`: o `cmd` igual na mesma ferramenta troca a entrada do pacote (mesma posição); `cmd` igual em outra ferramenta não troca.
  - `merge_keeps_builtin_name_and_orders_user_tools_last`: usuário com `id = "docker"` e `name = "Meu Docker"` → o nome continua "Docker", os comandos novos entram no fim; ferramentas novas vêm depois das embutidas, em ordem alfabética do `name`.
  - `builtin_is_valid`: `builtin().tools` tem os 5 ids na ordem acima, sem `cmd`/`what` vazios e sem `cmd` com quebra de linha.
- [ ] **Step 2: Rodar e ver falhar** — `cargo test -p zefiro-integration-commands` → não compila.
- [ ] **Step 3: Implementar** as assinaturas acima. `parse_tool` usa `toml::from_str` numa struct `serde` privada e valida em seguida; os limites de arquivo e quantidade são constantes (`MAX_FILES = 64`, `MAX_FILE_BYTES = 256 * 1024`). Ler a pasta em ordem de nome de arquivo.
- [ ] **Step 4: Rodar** `cargo test -p zefiro-integration-commands` → passa; `cargo clippy -p zefiro-integration-commands --all-targets -- -D warnings`.
- [ ] **Step 5: Commit** — `feat(commands): tool model, TOML parsing, user dir and merge`.

---

### Task 4: Busca e "Mais usados"

**Files:**
- Create: `crates/integrations/commands/src/search.rs`
- Modify: `crates/integrations/commands/src/lib.rs` (`pub mod search;` e re-exportar `fold`, `terms`, `Hit`)
- Test: `crates/integrations/commands/src/search.rs` (`mod tests`)

**Interfaces:**
- Consumes: `Library`, `Tool`, `Command` (Task 3).
- Produces:
  - `pub fn fold(s: &str) -> String` (minúsculas, sem acento: á à â ã ä → a, é è ê ë → e, í ì î ï → i, ó ò ô õ ö → o, ú ù û ü → u, ç → c, ñ → n)
  - `pub fn terms(query: &str) -> Vec<String>` (`fold`, separa por espaço, tira as palavras vazias `como eu o a os as de do da dos das para um uma com que no na em e`)
  - `pub struct Hit<'a> { pub tool: &'a Tool, pub command: &'a Command }`
  - `impl Library { pub fn tool(&self, id: &str) -> Option<&Tool>; pub fn search(&self, tool: Option<&str>, query: &str) -> Vec<Hit<'_>>; pub fn popular(&self, tool: Option<&str>) -> Vec<Hit<'_>> }`
- Regras: cada termo precisa casar. Um termo casa quando o texto dobrado do campo o contém, ou, com 5 ou mais letras, quando alguma palavra do campo começa com as 4 primeiras letras do termo. A posição do resultado é a do primeiro campo que sozinho casa todos os termos: `keywords` (0), `cmd` (1), `what` (2); se nenhum sozinho casa, mas o conjunto casa, 3. Empate mantém a ordem da biblioteca. `search` com `terms` vazio devolve `popular(tool)`. `popular(Some(id))` devolve os marcados e, se não houver nenhum, todos os comandos da ferramenta; `popular(None)` devolve só os marcados, ferramenta por ferramenta. Id desconhecido devolve vazio.

- [ ] **Step 1: Escrever os testes que falham:**
  - `fold_strips_accents_and_case` ("AÇÃO ü" → "acao u").
  - `terms_drop_stopwords`: `terms("como apago o cache")` == `["apago", "cache"]`.
  - `intent_search_finds_docker_cache`: biblioteca de teste com `docker builder prune` (keywords `["apagar cache", "limpar cache"]`) → `search(Some("docker"), "como apago o cache")` o acha (prefixo `apag`).
  - `every_term_must_match`: "limpar zzz" não acha.
  - `short_terms_are_not_prefix_matched`: "apag" (4 letras) só casa por substring, e "ap" acha por substring mas não por prefixo de palavra.
  - `ranking_prefers_keywords_over_cmd_over_what`: três comandos que casam "cache" em campos diferentes saem nessa ordem; empates seguem a ordem do arquivo.
  - `tool_filter_and_all`: `Some("git")` não devolve comandos do Docker; `None` devolve de todas, com `hit.tool` certo.
  - `only_stopwords_behaves_like_empty` (Review Focus 1): `search(Some("docker"), "como o")` == `popular(Some("docker"))`, não vazio.
  - `popular_falls_back_to_all_for_a_tool_without_flags` (Review Focus 4) e `popular_all_tools_only_lists_flagged`.
  - `unknown_tool_is_empty`.
- [ ] **Step 2: Rodar e ver falhar** — `cargo test -p zefiro-integration-commands search` → não compila.
- [ ] **Step 3: Implementar** as assinaturas e regras acima (uma passada por comando, dobrando os campos uma vez).
- [ ] **Step 4: Rodar** `cargo test -p zefiro-integration-commands` e clippy da crate.
- [ ] **Step 5: Commit** — `feat(commands): intent search and popular listing`.

---

### Task 5: O pacote embutido (conteúdo)

**Files:**
- Create/overwrite: `crates/integrations/commands/data/docker.toml`, `linux.toml`, `powershell.toml`, `python.toml`, `git.toml`
- Modify: `crates/integrations/commands/src/model.rs` (o teste de lint)
- Test: `crates/integrations/commands/src/model.rs` (`mod tests`)

**Interfaces:**
- Consumes: `builtin()` (Task 3), `search` (Task 4).

Escreva cerca de 40 comandos por ferramenta, em português do Brasil, cada um com `what` de uma linha e `keywords` de intenção ("limpar cache", "liberar espaço", "subir api"). Marque de 8 a 10 como `popular`. Todo comando que apaga, sobrescreve ou descarta dados tem `what` começando com `"⚠ "`. O Python inclui as receitas: ambiente virtual, `pip`, `pytest` e FastAPI (`uvicorn main:app --reload`, com `--host`/`--port`). Docker inclui `docker builder prune` e `docker system prune -a`. Confira cada comando contra a documentação oficial antes de gravar; nada de comando inventado.

- [ ] **Step 1: Escrever os testes que falham** em `model.rs`:
  - `pack_has_enough_commands`: cada ferramenta tem 35 ou mais comandos e 8 ou mais `popular`.
  - `spec_examples_are_findable`: `search(Some("docker"), "limpar cache")` tem `docker builder prune`; `search(None, "subir fastapi uvicorn")` tem `uvicorn main:app --reload` na ferramenta `python`; `search(Some("docker"), "como apago o cache")` também acha `docker builder prune`.
  - `destructive_commands_carry_a_warning`: para todo comando do pacote cujo `cmd` contém algum de `["prune", "rm -rf", "rm -fr", "reset --hard", "clean -f", "push --force", "push -f", "Remove-Item", " rmi", "docker rm", "volume rm", "branch -D", "kill -9", "stash drop", "mkfs", "dd if=", "git checkout -- ", "git restore "]` (sem diferenciar maiúsculas), o `what` começa com `"⚠ "`.
  - `no_duplicate_commands_per_tool`.
- [ ] **Step 2: Rodar e ver falhar** — `cargo test -p zefiro-integration-commands pack` → falha (poucos comandos).
- [ ] **Step 3: Escrever os cinco TOML** conforme o parágrafo acima.
- [ ] **Step 4: Rodar** `cargo test -p zefiro-integration-commands` → passa.
- [ ] **Step 5: Commit** — `feat(commands): built-in pt-BR command pack`.

---

### Task 6: Estado e desenho do painel (`zefiro-bar`)

**Files:**
- Create: `crates/modules/bar/src/commands.rs`
- Modify: `crates/modules/bar/src/lib.rs` (`pub mod commands;`)
- Test: `crates/modules/bar/src/commands.rs` (`mod tests`, mesmo estilo de `vscode.rs`: `derive_tokens` do Obsidian, `FixedWidthMeasurer`)

**Interfaces:**
- Consumes: `crate::vscode::edit_query` (digitação, colar, apagar) e `Rect`/`Point`/`DisplayList`/`DrawOp`/`TextStyle`/`Family` como em `vscode.rs`. Esta crate **não** depende da crate de comandos.
- Produces:
  - `pub struct ToolEntry { pub id: String, pub name: String, pub icon: char }` (a lista do dropdown **sem** "Todas"; o índice 0 do dropdown é sempre "Todas")
  - `pub struct CommandRow { pub tool: String, pub cmd: String, pub what: String }`
  - `pub struct PanelState { pub tool: Option<String>, pub query: String, pub selected: usize, pub scroll: usize, pub navigated: bool, pub dropdown: Option<usize> }` (`Default`)
  - `pub enum Action { None, Redraw, Copy(usize), SaveTool(Option<String>), Close }`
  - `impl PanelState { pub fn open(last: &str, tools: &[ToolEntry]) -> PanelState` (ferramenta inexistente vira `None`; Review Focus 3)`; pub fn on_char(&mut self, code: u32, paste: Option<&str>) -> bool; pub fn on_key(&mut self, vk: u32, rows: usize, tools: &[ToolEntry]) -> Option<Action>` (`None` = não tratou); `pub fn click_header(&mut self, tools: &[ToolEntry]); pub fn click_dropdown_item(&mut self, index: usize, tools: &[ToolEntry]) -> Action; pub fn click_outside_dropdown(&mut self) -> bool }`
  - `pub struct PanelView<'a> { pub tools: &'a [ToolEntry], pub state: &'a PanelState, pub rows: &'a [CommandRow], pub heading: Option<&'a str>, pub measure: &'a dyn TextMeasurer }`
  - `pub struct PanelLayout { pub list: DisplayList, pub rows: Vec<(Rect, usize)>, pub header: Rect, pub dropdown_items: Vec<(Rect, usize)>, pub search: Rect }`
  - `pub fn commands_panel(view: &PanelView, scale: f32, tokens: &Tokens) -> PanelLayout`
  - `pub const VISIBLE: usize = 10; pub fn max_scroll(count: usize) -> usize`
- Comportamento: largura 400 DIP, linhas de 44 DIP, dropdown com no máximo 8 itens visíveis (janela que segue o destaque), corpo com altura mínima para o dropdown caber (a altura do painel não muda ao abrir). Comando em `Family::Mono`, explicação em cinza, "copiar" à direita; selo da ferramenta na linha só quando `state.tool` é `None`; `heading` ("Mais usados" ou "Comandos") aparece só quando a busca (por `terms`, decidido no app) está vazia; rodapé fixo "↵ copiar · ↑↓ navegar · Esc fechar". Sem linhas: "Nada encontrado em <ferramenta>" e, com ferramenta específica, a dica "Tente em Todas". Teclas: ↑ ↓ movem a linha (ou o destaque do dropdown se aberto), Enter copia a linha (ou escolhe o item do dropdown), Esc fecha só o dropdown se aberto, senão limpa a busca se houver, senão `Action::Close`; a digitação sempre vai para a busca. Escolher uma ferramenta devolve `Action::SaveTool` e zera busca, seleção e rolagem. Digitar volta ao primeiro resultado.

- [ ] **Step 1: Escrever os testes que falham:**
  - Estado: `open_with_unknown_tool_falls_back_to_all` (`open("sumiu", ..)` → `tool == None`) e `open_with_known_tool`; `typing_resets_selection_and_scroll`; `enter_copies_the_selected_row` (`Action::Copy(i)`); `esc_closes_dropdown_then_clears_query_then_closes_panel` (três Esc em sequência); `arrows_move_the_dropdown_highlight_while_open` e `typing_still_edits_the_query_while_dropdown_is_open`; `picking_a_tool_saves_it_and_resets` (`SaveTool(Some("git"))`; escolher o índice 0 → `SaveTool(None)`); `click_outside_closes_only_the_dropdown`; `selection_stays_visible_when_moving_past_visible_rows` (Review Focus 5: 25 linhas, `selected` vai a 12, `scroll` acompanha).
  - Desenho: `header_shows_tool_name_and_search_hint` ("buscar em Docker…", "buscar em todas…" sem ferramenta); `rows_show_cmd_what_and_copy_hint`; `command_uses_the_mono_family` (o `DrawOp::Text` do comando tem `Family::Mono`); `badge_only_in_all` (selo presente com `tool == None`, ausente com ferramenta); `heading_shown_only_when_given`; `footer_is_always_drawn`; `empty_state_has_tool_hint` ("Nada encontrado em Docker" + "Tente em Todas"; sem a dica em Todas); `dropdown_overlay_does_not_change_panel_height` (mesmo `list.size` com `dropdown` aberto e fechado); `dropdown_lists_all_then_tools_and_marks_the_current`; `long_command_is_clipped_to_the_row` (o retângulo do texto do comando não passa de `row.right() - espaço do "copiar"`); `hits_match_the_visible_rows`.
- [ ] **Step 2: Rodar e ver falhar** — `cargo test -p zefiro-bar commands` → não compila.
- [ ] **Step 3: Implementar** o módulo (desenho no padrão de `vscode_panel`: `px`, `DrawOp::RoundRect`/`Text`, cores dos `Tokens`; ícones do dropdown em `Family::Icons`).
- [ ] **Step 4: Rodar** `cargo test -p zefiro-bar` e `cargo clippy -p zefiro-bar --all-targets -- -D warnings`.
- [ ] **Step 5: Commit** — `feat(bar): commands panel state and drawing`.

---

### Task 7: O chip no catálogo, no controlador e na barra

**Files:**
- Modify: `crates/modules/bar/src/catalog.rs` (item + teste), `crates/modules/bar/src/controller.rs` (`PanelKind::Commands`, `panel_kind_for`, teste), `crates/app/src/bar_view.rs` (`icon_glyph` e o braço `"commands"` do `chip_content`, na lista de `level_chip` sem texto), `crates/app/src/event_loop.rs` (braço `PanelKind::Commands` no `match` do painel — provisório, devolve uma lista vazia até a Task 8)

**Interfaces:**
- Produces: item `commands` ("Comandos", "Cola de comandos: Docker, Linux, PowerShell, Python e mais"); `PanelKind::Commands`; `panel_kind_for("commands") == Some(PanelKind::Commands)`.

- [ ] **Step 1: Escrever os testes que falham:** em `catalog.rs`, acrescentar `"commands"` à lista de `new_items_are_in_the_catalog` e um teste `commands_item_text` com nome e descrição exatos; em `controller.rs`, `assert_eq!(panel_kind_for("commands"), Some(PanelKind::Commands))`.
- [ ] **Step 2: Rodar e ver falhar** — `cargo test -p zefiro-bar` (os testes novos de `catalog` e `controller` falham).
- [ ] **Step 3: Implementar.** Glifo do chip: `"\u{E943}"` (Código, `</>`), **não** `\u{E756}`, que já é o chip Terminal; confirme na tela na Task 9 e registre o desvio da spec (`>_`) no ledger.
- [ ] **Step 4: Rodar** `cargo test --workspace` e `cargo build -p zefiro-app`.
- [ ] **Step 5: Commit** — `feat(bar): commands chip in the catalog`.

---

### Task 8: Ligação no `app` (`commands_loop.rs`)

**Files:**
- Create: `crates/app/src/commands_loop.rs`
- Modify: `crates/app/Cargo.toml` (`zefiro-integration-commands.workspace = true`), `crates/app/src/main.rs` (`mod commands_loop;`), `crates/app/src/event_loop.rs` (campo `commands: CommandsRuntime` ao lado de `vscode`, a inicialização em `Loop::new`, o braço do painel, `commands_prepare` junto de `vscode_prepare`, e os ganchos de clique, mouse, tecla, roda e `WM_CHAR` em cada lugar onde hoje há `vscode_*`)
- Test: o que for puro vai para a Task 6/4; esta tarefa tem a verificação por sonda (Task 9)

**Interfaces:**
- Consumes: `zefiro_bar::commands::*` (Task 6), `zefiro_integration_commands::{builtin, read_user_dir, merge, Library}` (Task 3), `search`/`terms`/`popular` (Task 4), `zefiro_core::config::set_value`, `system_events::set_clipboard_text(hwnd, &str) -> bool` e `system_events::clipboard_text()`.
- Produces: `pub struct CommandsRuntime` (`Default`) e, em `impl Loop`: `commands_prepare(kind)`, `commands_panel_list(scale) -> DisplayList`, `commands_move(Point)`, `commands_char(u32)`, `commands_key(u32) -> Result<bool>`, `commands_wheel(i32) -> Result<bool>`, `commands_click(Point) -> Result<bool>`.

- [ ] **Step 1: Mapear** com `grep -n "vscode_" crates/app/src/event_loop.rs`: cada chamada tem um par `commands_*` com a mesma forma (`controller.open().is_some_and(|o| o.kind == PanelKind::Commands)` como guarda).
- [ ] **Step 2: Implementar `commands_prepare`:** ao abrir, ler `%APPDATA%\zefiro\commands` (pasta de config do app, que respeita `ZEFIRO_CONFIG_DIR`) com `read_user_dir`, logar cada aviso com `tracing::warn!`, montar `Library` com `merge(builtin(), user)`, montar `Vec<ToolEntry>` (`icon` = o da ferramenta ou o glifo genérico `\u{E8A5}`) e `PanelState::open(&config.commands.last, &tools)`. A leitura é síncrona (poucos arquivos pequenos, com os limites da Task 3).
- [ ] **Step 3: Implementar `commands_panel_list`:** `terms(&state.query)` decide `heading` (`Some("Mais usados")` ou, se nenhum resultado é `popular`, `Some("Comandos")`; `None` com busca); linhas = `library.search(state.tool.as_deref(), &state.query)` mapeadas para `CommandRow` (guardar os `cmd` num `Vec<String>` do runtime para o Enter/clique); chamar `commands_panel`, guardar `rows`, `header` e `dropdown_items` para o mouse.
- [ ] **Step 4: Implementar os eventos:** `on_key` devolve `Action`; `Copy(i)` copia `cmd` (apare qualquer `\n`/`\r` final antes de copiar) com `set_clipboard_text(self.panels[open.monitor].hwnd(), text)`, loga `tracing::info!` sem o texto do comando e chama `close_open_panel()`; `SaveTool(id)` grava com `set_value(&self.ctx.config_path, &["commands","last"], id.unwrap_or_default())` (o recarregamento não fecha o painel por causa de `same_except_panels`); `Close` chama `close_open_panel()`. Clique no cabeçalho abre/fecha o dropdown; clique num item escolhe; clique fora do dropdown aberto só o fecha; clique numa linha copia; mouse sobre uma linha ou item move o destaque (só redesenha quando muda); a roda rola a lista (ou o dropdown aberto).
- [ ] **Step 5: Rodar** `cargo build -p zefiro-app`, `cargo test --workspace`, `cargo clippy --workspace --all-targets -- -D warnings` e `pwsh -NoProfile -File scripts/check-windows-imports.ps1`.
- [ ] **Step 6: Commit** — `feat(app): wire the commands panel`.

---

### Task 9: Verificação na tela, docs e ADR

**Files:**
- Create: `docs/adr/0029-chip-comandos.md` (modelo em `docs/adr/0000-modelo.md`: pacote embutido em pt-BR em vez de tldr, busca por intenção, `Family::Mono`, sem tldr nesta fase)
- Modify: `AGENTS.md` (tabela de estrutura: linha de `crates/integrations/commands`; chaves de config: `[commands] last`; ids do catálogo: `commands`; um item em "Coisas não óbvias" sobre a pasta do usuário e o pacote), `docs/PLANNING.md` se o roteiro citar chips de dev

- [ ] **Step 1: Pedir um momento ao usuário** antes de abrir a barra: o teste mexe com o mouse e com a barra (regra 6 do `AGENTS.md`). Usar um perfil isolado: `ZEFIRO_CONFIG_DIR=<pasta temporária>` com `commands` em `[bar.sections]`.
- [ ] **Step 2: Verificar na tela:** abrir o painel; buscar "limpar cache" em Docker (acha `docker builder prune`); trocar para Python e buscar "subir fastapi"; Enter copia e fecha o painel; colar no Bloco de Notas e conferir que não há quebra de linha no fim; reabrir e ver que a última ferramenta voltou; criar `commands\meu.toml` com um comando e ver que aparece; confirmar o glifo do chip. Capturar uma imagem do painel.
- [ ] **Step 3: Custo:** `ZEFIRO_CYCLES=50` e `./scripts/perf.ps1` com o painel fechado, para conferir que não houve mudança de RAM/CPU (feche o YASB antes e reabra no fim).
- [ ] **Step 4: Escrever o ADR-0029 e atualizar o `AGENTS.md`.**
- [ ] **Step 5: Rodar tudo:** `cargo test --workspace`, `cargo clippy --workspace --all-targets -- -D warnings`, `pwsh -NoProfile -File scripts/check-windows-imports.ps1`.
- [ ] **Step 6: Commit** — `docs(adr): commands chip` (e `docs: commands chip in AGENTS.md`).
