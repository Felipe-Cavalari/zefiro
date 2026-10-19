# Fase 1e — Widgets do YASB: plano de implementação

> **Para agentes:** SUB-SKILL OBRIGATÓRIA: use superpowers:subagent-driven-development (recomendado) ou superpowers:executing-plans para executar este plano tarefa por tarefa. Os passos usam caixas (`- [ ]`) para acompanhamento.

**Objetivo:** seis chips novos no catálogo (`codex`, `claude`, `vscode`, `power`, `notifications`, `dnd`; o `apps` do plano original foi removido a pedido do usuário, ver Tarefa 9) que trazem para o Zefiro os widgets do YASB que faltam, sem mudar nada da barra atual.

**Arquitetura:** a lógica que dá para testar (parsers, formatação, agrupamento, máquinas de estado) fica em crates portáveis: `integrations/usage`, `integrations/vscode` e `modules/bar`. A `platform-windows` traz as chamadas do sistema: HTTP com cabeçalhos, janelas, energia, processos e Não perturbe. A `app` liga tudo em `*_loop.rs`, no padrão do `devices_loop.rs` da 1d. Cada fonte de dados tem sua thread e avisa a UI por `window::post_user`.

**Tecnologia:** Rust (workspace atual), crate `windows` só onde a regra de ouro permite, `serde_json` (já no workspace), `toml_edit` (já no core). Nenhuma dependência nova.

**Spec:** [`docs/superpowers/specs/2026-10-19-fase-1e-widgets-do-yasb-design.md`](../specs/2026-10-19-fase-1e-widgets-do-yasb-design.md). Leia a spec inteira antes da primeira tarefa.

## Restrições globais

- Não mudar a barra atual: chips existentes, layout, ilha, menus e cliques ficam iguais. O `config.toml` do usuário (`%APPDATA%\zefiro\`) não é tocado. Testes de tela usam um perfil isolado com `ZEFIRO_CONFIG_DIR`.
- Crate `windows` só em `crates/platform-windows`, `crates/integrations/clock/src/windows.rs` e `crates/app`. Conferir com `pwsh -NoProfile -File scripts/check-windows-imports.ps1`.
- `cargo clippy --workspace --all-targets -- -D warnings` passa em toda tarefa. `unsafe` só em blocos pequenos, cada um com `// SAFETY:`.
- Textos, comentários e logs em pt-BR; identificadores, commits e branches em inglês. Commits no estilo `feat(bar): …`, terminando com `Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>`. Trabalhar direto na `main`. Nunca `git push`.
- Tokens e conteúdo de credenciais nunca vão para log, teste, fixture ou saída de sonda.
- Nenhuma consulta por quadro. Ritmos: Claude e Codex a cada 120 s; Apps, notificações e Não perturbe por evento; VS Code, foto da conta e tempo ligado ao abrir o painel.
- HTTP com tempo limite de 15 s. Erro de rede repete no próximo ciclo de 120 s, sem rajadas.
- Repouso com os chips novos: CPU 0,0 % e no máximo +4 MB privados em relação à barra sem eles (medido no fim).
- Antes de testes que movem o mouse ou mandam teclas, pedir um momento ao usuário. Nunca fechar apps do usuário: o app de teste do Apps é o Bloco de notas aberto pelo próprio teste. Suspender, Sair, Reiniciar e Desligar nunca são executados de verdade.
- Ledger em `.superpowers/sdd/2026-10-19-fase-1e-widgets-do-yasb/progress.md`: uma linha `Task N: complete (commits …, tests: … → …)` por tarefa e `Task N: Ruling: <decisão> — <por quê> — <custo se errado>` para cada desvio.

## Pontos de atenção na revisão

Situações que a spec implica e que nenhum teste da tarefa dona cobriria sem esta lista. Cada uma já tem teste na tarefa indicada.

1. **Usuário sem login no Claude Code:** o arquivo de credenciais não existe ou não tem `claudeAiOauth`. O chip mostra "—", o painel diz "Faça login no Claude Code" e nada entra em pânico (Tarefa 4: `token_state_missing_file_and_missing_key`).
2. **Sessão do Codex recém-aberta, ainda sem `rate_limits`:** o `.jsonl` mais novo não tem o evento. Os números não somem: a leitura passa para os próximos arquivos, até 5 (Tarefa 2: `falls_back_to_older_session_without_rate_limits`).
3. **Caminhos do VS Code com espaço e acento** (`%20`, `%C3%A7`): o nome aparece decodificado e o projeto abre (Tarefa 5: `decodes_spaces_and_accents`).
4. **Apps da Store** (Configurações, Calculadora), cujas janelas são do `ApplicationFrameHost.exe`: agrupam pelo AUMID, não viram um grupo "ApplicationFrameHost" (Tarefa 9: `groups_store_apps_by_aumid_not_frame_host`).
5. **Energia com outro botão destrutivo armado:** clicar em Desligar com Reiniciar armado arma o Desligar e não executa nada (Tarefa 6: `arming_another_action_never_runs_the_first`).

---

### Tarefa 1: Catálogo, tipos de painel e botão do meio

**Arquivos:**
- Modificar: `crates/modules/bar/src/catalog.rs`, `crates/modules/bar/src/controller.rs`
- Modificar: `crates/platform-windows/src/window.rs` (mensagem `WM_MBUTTONUP`)
- Modificar: `crates/app/src/event_loop.rs` (lista fixa de chips em `PanelEffect::Close`; roteamento de `MiddleUp`)
- Modificar: `crates/app/src/bar_view.rs` (`chip_content` para os ids novos)

**Interfaces:**
- Produz: `ITEMS` com os ids `claude`, `codex`, `vscode`, `apps`, `notifications`, `dnd` e `power`.
- Produz: `PanelKind::{Claude, Codex, VsCode, Power}`. `panel_kind_for` devolve `None` para `apps`, `notifications` e `dnd`, que agem direto.
- Produz: `pub fn action_chip(id: &str) -> bool` em `controller.rs`: `true` para `apps`, `notifications` e `dnd`.
- Produz: `WinEvent::MiddleUp { hwnd: isize, x: f32, y: f32 }`.
- Produz: `Loop::chip_action(&mut self, bar: usize, chip: &str, button: MouseButton, x: f32, y: f32) -> Result<()>` em `event_loop.rs`, com `pub enum MouseButton { Left, Middle, Right }`. Nesta tarefa só registra no log; cada widget preenche o seu ramo depois.

- [ ] **Passo 1: testes que falham.** Em `catalog.rs`, `new_items_are_in_the_catalog`: os sete ids existem e todos os `name` são não vazios e únicos. Em `controller.rs`, `new_chips_map_to_panels_or_actions`: `claude` → `Claude`, `codex` → `Codex`, `vscode` → `VsCode`, `power` → `Power`; `apps`, `notifications` e `dnd` → `None` com `action_chip` = `true`; `clock` continua `Calendar` e `action_chip("clock")` = `false`.
- [ ] **Passo 2:** `cargo test -p zefiro-bar` → FALHA (ids e variantes ausentes).
- [ ] **Passo 3: implementar.**
  - Catálogo, em pt-BR: Claude "Uso do Claude (5 h e 7 dias)", Codex "Uso do Codex (janelas de uso)", VS Code "Projetos recentes do VS Code", Apps "Apps fixados e abertos", Notificações "Contador; clique abre a central", Não perturbe "Liga e desliga o Não perturbe", Energia "Bloquear, suspender, sair, reiniciar e desligar".
  - Em `PanelEffect::Close`, a lista fixa `["zefiro", "clock", "audio", "mic", "brightness", ISLAND_CHIP]` vira os ids do catálogo mais o `ISLAND_CHIP`.
  - No `LeftUp` da barra, um chip com `action_chip` = `true` chama `chip_action(..., Left, ...)` em vez de `controller.handle`. No `RightUp`, só o `apps` vai para `chip_action(..., Right, ...)`; os outros chips continuam abrindo o menu do Zefiro. `MiddleUp` sobre qualquer chip chama `chip_action(..., Middle, ...)`.
  - `chip_content` dos ids novos: `level_chip` com ícone e texto vazio, apagado, até as tarefas seguintes trazerem os dados.
- [ ] **Passo 4:** `cargo test -p zefiro-bar` → PASSA. `cargo clippy --workspace --all-targets -- -D warnings` → limpo.
- [ ] **Passo 5: sonda de tela.** Perfil isolado com `end = ["claude", "codex", "vscode", "notifications", "dnd", "power", "clock"]` e `start = ["zefiro", "apps"]`. Os chips aparecem apagados; clicar em `claude` abre um painel vazio; o botão do meio sobre `notifications` registra `chip_action` no log. Com o perfil do usuário, a captura da faixa fica igual à de antes desta tarefa.
- [ ] **Passo 6: commit** `feat(bar): catalog entries, panel kinds and middle click for the YASB widgets`.

### Tarefa 2: Crate `integrations/usage` — Codex e formatação

**Arquivos:**
- Criar: `crates/integrations/usage/{Cargo.toml, src/lib.rs, src/codex.rs, src/format.rs}` (`zefiro-integration-usage`, sem dependências além de `serde_json`)
- Modificar: `Cargo.toml` (membro do workspace), `.github/workflows/ci.yml` (`-p zefiro-integration-usage` no check do Linux)
- Teste: módulos `#[cfg(test)]` nos próprios arquivos e fixtures em `crates/integrations/usage/tests/fixtures/`

**Interfaces:**
- Produz: `pub struct Window { pub used_percent: f32, pub window_minutes: u32, pub resets_at: i64 }`.
- Produz: `pub struct CodexUsage { pub primary: Option<Window>, pub secondary: Option<Window>, pub plan: Option<String>, pub event_at: Option<String> }`. `event_at` é o `timestamp` do evento.
- Produz: `pub fn parse_codex_tail(text: &str) -> Option<CodexUsage>`: última linha com `payload.rate_limits`, lida de trás para frente; ignora linhas inválidas e uma primeira linha cortada.
- Produz: `pub fn read_codex(sessions_dir: &Path) -> Option<(PathBuf, SystemTime, CodexUsage)>`: arquivos `.jsonl` de `sessions_dir` (recursivo) por data de modificação, do mais novo para o mais velho, até 5; lê os últimos 256 KB de cada um.
- Produz: `pub fn effective_percent(w: &Window, now_unix: i64) -> f32`: 0 se `resets_at <= now_unix`, senão `used_percent`.
- Produz: `pub fn window_name(minutes: u32) -> String`: 300 → "5 horas", 10080 → "7 dias", múltiplo de 1440 → "N dias", senão "N horas".
- Produz: `pub fn format_relative(seconds: i64) -> String` e `pub fn format_absolute(unix: i64, utc_offset_s: i32) -> String`.
- Produz: `pub fn is_high(percent: f32) -> bool`: `percent >= 80.0`.

- [ ] **Passo 1: testes que falham.**
  - `parses_rate_limits_from_real_event`: fixture com o evento da spec §4.2 → primário 80,0 % com janela de 300 min, secundário 19,0 % com 10080 min, `plan` = "plus".
  - `skips_cut_first_line_and_non_json`.
  - `none_without_rate_limits`.
  - `falls_back_to_older_session_without_rate_limits`: diretório temporário com dois `.jsonl`; o mais novo sem `rate_limits` → usa o mais velho.
  - `reset_window_counts_as_zero`: `effective_percent` com `resets_at` no passado = 0.
  - `window_names`: 300, 10080, 2880 → "2 dias", 120 → "2 horas".
  - `relative_and_absolute_formats`: `format_relative(8040)` = "2 h 14 min", `format_relative(840)` = "14 min", `format_relative(30)` = "menos de 1 min"; `format_absolute(1792678081, -10800)` = "qui 22 out, 11:08" (conferido com Python). Dias e meses em pt-BR minúsculos e abreviados (`dom seg ter qua qui sex sáb`; `jan fev mar abr mai jun jul ago set out nov dez`).
  - `high_band_starts_at_80`.
- [ ] **Passo 2:** `cargo test -p zefiro-integration-usage` → FALHA (crate vazia).
- [ ] **Passo 3: implementar.** Conversão de data civil sem crates: dias desde a época (algoritmo de Howard Hinnant), com `utc_offset_s` aplicado antes.
- [ ] **Passo 4:** `cargo test -p zefiro-integration-usage` → PASSA. `cargo check --target x86_64-unknown-linux-gnu -p zefiro-integration-usage` → ok.
- [ ] **Passo 5: commit** `feat(usage): portable Codex rate limit reader and usage formatting`.

### Tarefa 3: Chip e painel do Codex

**Arquivos:**
- Criar: `crates/modules/bar/src/usage.rs` (chip e painel de uso, comuns a Claude e Codex)
- Criar: `crates/app/src/usage_loop.rs`
- Modificar: `crates/integrations/clock/src/windows.rs` (`pub fn utc_offset_seconds() -> i32`), `crates/app/src/app.rs` (`AppCtx.usage: UsageState`), `crates/app/src/event_loop.rs` (`USER_CODEX = 19`, `panel_list`), `crates/app/src/bar_view.rs` (chip `codex`)

**Interfaces:**
- Consome: Tarefa 2 (`read_codex`, `effective_percent`, `window_name`, `format_*`, `is_high`), Tarefa 1 (`PanelKind::Codex`).
- Produz: em `modules/bar/src/usage.rs`:
  - `pub struct UsageRow { pub label: String, pub percent: Option<f32>, pub detail: String }`;
  - `pub fn usage_chip(glyph: &str, percent: Option<f32>, chip_h: f32, font_px: f32, scale: f32, tokens: &Tokens, m: &dyn TextMeasurer) -> ChipContent`: usa `level_chip` com texto `"{p:.0}%"` (ou vazio e apagado quando `None`) e cor `tokens.accent` quando `is_high`. Para isso, `Level` ganha o campo `color: Option<Rgba>` (`None` = regra atual), sem mudar os chips da 1d;
  - `pub fn usage_panel(title: &str, rows: &[UsageRow], message: Option<&str>, footer: &str, scale: f32, tokens: &Tokens) -> DisplayList`: largura 340, uma linha por item com rótulo, % à direita, barra de 5 px (`accent` quando alto, senão `text_secondary`) e o detalhe embaixo em texto terciário. `message` substitui as linhas.
- Produz: `pub struct CodexService` em `crates/integrations/usage/src/codex.rs`, com `start(dir: PathBuf, notify: Arc<dyn Fn() + Send + Sync>) -> io::Result<Self>`, `latest() -> Option<(SystemTime, CodexUsage)>` e `refresh()`. Thread `zefiro-codex`: a cada 120 s, `stat` do mais novo, relendo só se a data mudou; `refresh()` acorda a thread.
- Produz: `AppCtx.usage.codex: Option<(SystemTime, CodexUsage)>`.

- [ ] **Passo 1: testes que falham** em `usage.rs`:
  - `chip_turns_accent_at_80`;
  - `chip_without_data_is_dim_and_empty`;
  - `chip_width_does_not_follow_percent` (como o da 1d);
  - `panel_lists_rows_with_details`: textos "5 horas", "80%" e "reinicia em 2 h 14 min";
  - `panel_message_replaces_rows`.
- [ ] **Passo 2:** `cargo test -p zefiro-bar usage` → FALHA.
- [ ] **Passo 3: implementar** `usage.rs`, `CodexService` e `usage_loop.rs`.
  - `usage_start()` liga o serviço com o diretório `~/.codex/sessions`.
  - O painel "Codex" tem título "Codex · {plan}" e, por janela, rótulo `window_name`, % efetivo e detalhe "reinicia em …" (ou "reiniciada" se já passou).
  - Rodapé "Último uso do Codex: dd/MM HH:mm", a partir de `event_at`.
  - Sem dado: mensagem "Nenhum uso do Codex encontrado".
  - Abrir o painel chama `refresh()`.
- [ ] **Passo 4:** `cargo test --workspace` → PASSA; clippy limpo.
- [ ] **Passo 5: sonda de tela.** No perfil de teste, o chip mostra o mesmo primário que `read_codex` devolve (conferir com um `examples/codex_probe.rs` em `integrations/usage` que imprime só os números). Hoje a janela de 5 h já reiniciou, então o esperado é 0 % e "reiniciada"; o painel abre com as duas linhas.
- [ ] **Passo 6: commit** `feat(app): Codex usage chip and panel`.

### Tarefa 4: Claude — sonda, parser, credenciais, HTTP e chip

**Arquivos:**
- Criar: `crates/integrations/usage/src/claude.rs`
- Criar: `crates/platform-windows/examples/claude_usage_probe.rs`
- Modificar: `crates/platform-windows/src/http.rs` (`get_with`), `crates/app/src/usage_loop.rs` (`USER_CLAUDE = 20`), `crates/app/src/bar_view.rs`

**Interfaces:**
- Produz:
  - `pub fn get_with(url: &str, headers: &[(&str, &str)], timeout: Duration) -> windows::core::Result<(u16, String)>` em `http.rs`, com tempo limite por cancelamento da operação;
  - `pub enum TokenState { Valid(String), Expired, Missing }`;
  - `pub fn token_state(credentials_json: &str, now_ms: i64) -> TokenState`: lê `claudeAiOauth.accessToken` e `expiresAt` (ms); `Debug` de `Valid` não mostra o token;
  - `pub struct ClaudeUsage { pub five_hour: Option<Limit>, pub seven_day: Option<Limit>, pub extra: Vec<(String, Limit)> }` e `pub struct Limit { pub percent: f32, pub resets_at: Option<i64> }`;
  - `pub fn parse_claude(json: &str) -> Result<ClaudeUsage, UsageError>`, com `UsageError::{UnknownFormat, Http(u16), Expired, Missing, Network}`.
- Produz: `ClaudeService` em `crates/platform-windows/src/claude.rs`, com `start(credentials: PathBuf, notify) -> io::Result<Self>`, `latest() -> Option<(Instant, Result<ClaudeUsage, UsageError>)>` e `refresh()`. Ele relê o arquivo de credenciais a cada consulta e consulta ao iniciar, a cada 120 s e no `refresh()` se o dado tiver mais de 60 s.

- [ ] **Passo 1: sonda primeiro.** `claude_usage_probe.rs` lê o token sem imprimir, chama `GET https://api.anthropic.com/api/oauth/usage` com `Authorization: Bearer …` e `anthropic-beta: oauth-2025-04-20`, e imprime só o status e a árvore de **nomes** de campos. Rodar com `cargo run -p zefiro-platform-windows --example claude_usage_probe`. Anotar no ledger os nomes reais dos campos e montar a fixture `claude_usage.json` com valores inventados nesses nomes. Se o status não for 200, registrar um Ruling e ajustar o cabeçalho antes de seguir.
- [ ] **Passo 2: testes que falham** em `claude.rs`:
  - `parses_five_hour_and_seven_day` (fixture);
  - `utilization_as_fraction_or_percent`: aceitar 0–1 e 0–100, decidido pela sonda e anotado;
  - `unknown_format_is_error_not_panic`;
  - `extra_weekly_limits_keep_api_names`;
  - `token_state_valid_and_expired`;
  - `token_state_missing_file_and_missing_key` (Ponto de atenção 1);
  - `valid_token_debug_hides_secret`: o `format!("{:?}")` não contém o token.
- [ ] **Passo 3:** `cargo test -p zefiro-integration-usage claude` → FALHA.
- [ ] **Passo 4: implementar** parser, `get_with`, `ClaudeService` e a ligação no `usage_loop`.
  - Chip `claude`: `usage_chip` com `five_hour`.
  - Painel "Claude": linhas "5 horas" (detalhe relativo) e "7 dias" (detalhe absoluto com `utc_offset_seconds()`), mais os `extra`, e rodapé "Atualizado há N min".
  - Mensagens: `Expired`/401 → "Login do Claude Code expirado; abra o Claude Code para renovar"; `Missing` → "Faça login no Claude Code"; `Network` → "Sem conexão; tento de novo em 2 min"; `UnknownFormat` → "Formato de resposta desconhecido".
  - O Zefiro nunca grava no arquivo de credenciais.
- [ ] **Passo 5:** `cargo test --workspace` → PASSA; clippy e `check-windows-imports.ps1` limpos.
- [ ] **Passo 6: sonda de tela.**
  - O chip e o painel batem com a sonda.
  - Credenciais ausentes: com `USERPROFILE` apontando para uma pasta vazia, numa cópia do exe no perfil de teste, aparece "Faça login no Claude Code".
  - Token expirado: um arquivo de credenciais **inventado** (token falso, `expiresAt` no passado) num `USERPROFILE` de teste mostra a mensagem de login expirado. O arquivo real nunca é copiado nem alterado.
- [ ] **Passo 7: commit** `feat(app): Claude usage chip and panel from the OAuth usage endpoint`.

### Tarefa 5: VS Code

**Arquivos:**
- Criar: `crates/integrations/vscode/{Cargo.toml, src/lib.rs}` (`zefiro-integration-vscode`, depende de `serde_json`; entra no CI do Linux)
- Criar: `crates/modules/bar/src/vscode.rs` (painel e rolagem), `crates/app/src/vscode_loop.rs`
- Modificar: `crates/platform-windows/src/system.rs` (`spawn_detached`, `find_code_exe`)

**Interfaces:**
- Produz: `pub enum Kind { Local, Wsl, Remote, Workspace }` e `pub struct Recent { pub uri: String, pub name: String, pub location: String, pub kind: Kind, pub modified: SystemTime }`.
- Produz: `pub fn parse_workspace_json(text: &str) -> Option<(String, bool)>`: devolve a uri e `true` se for workspace.
- Produz: `pub fn describe(uri: &str, is_workspace: bool) -> Option<(String, String, Kind)>`: nome, local e tipo.
- Produz: `pub fn read_recents(storage_dir: &Path, local_exists: &dyn Fn(&Path) -> bool, max: usize) -> Vec<Recent>`, com `max` = 20 e ordenado por data de modificação da pasta.
- Produz: `pub fn open_args(r: &Recent) -> Vec<String>`: `["--folder-uri", uri]` ou `["--file-uri", uri]`.
- Produz: `pub fn vscode_panel(items: &[RecentRow], scroll: usize, scale: f32, tokens: &Tokens) -> (DisplayList, Vec<(Rect, usize)>)`, com no máximo 12 linhas visíveis de 44 px e largura 360. `pub struct RecentRow { pub name: String, pub location: String, pub badge: Option<&'static str>, pub date: String }`.
- Produz: `pub fn spawn_detached(exe: &Path, args: &[String]) -> io::Result<()>` (`CREATE_NO_WINDOW` e `DETACHED_PROCESS`) e `pub fn find_code_exe() -> Option<PathBuf>` (`code.cmd` no PATH, senão `%LOCALAPPDATA%\Programs\Microsoft VS Code\Code.exe`; com o `code.cmd`, usar o `Code.exe` ao lado de `bin\`).

- [ ] **Passo 1: testes que falham:**
  - `local_folder_name_and_location`: `file:///c%3A/Users/felip/Projects/Gbpa/lumi` → "lumi", "C:\Users\felip\Projects\Gbpa", `Local`;
  - `wsl_folder`: `vscode-remote://wsl%2Bubuntu-24.04/home/felip/Projects/garimpo-bot/garimpo-bot-frontend` → "garimpo-bot-frontend", "WSL: Ubuntu-24.04 · /home/felip/Projects/garimpo-bot", `Wsl`;
  - `workspace_file` → nome do arquivo sem `.code-workspace`, `Workspace`;
  - `decodes_spaces_and_accents` (Ponto de atenção 3): `file:///c%3A/Users/felip/Meus%20Projetos/Constru%C3%A7%C3%A3o` → "Construção";
  - `deleted_local_folder_is_skipped`;
  - `orders_by_modified_and_caps_at_max` (diretório temporário);
  - `open_args_for_folder_and_workspace`;
  - `panel_scrolls_after_twelve_rows`.
- [ ] **Passo 2:** `cargo test -p zefiro-integration-vscode -p zefiro-bar vscode` → FALHA.
- [ ] **Passo 3: implementar.**
  - Ao abrir o painel, `vscode_loop` lê `%APPDATA%\Code\User\workspaceStorage` numa thread (`USER_VSCODE = 21`).
  - Enquanto lê, o painel mostra "Carregando…"; a roda rola; o clique abre o projeto e fecha o painel.
  - Sem `Code.exe`: mensagem "VS Code não encontrado".
  - Selos: `Wsl` → "WSL", `Remote` → "Remoto", `Workspace` → "Workspace".
- [ ] **Passo 4:** `cargo test --workspace` → PASSA; clippy, imports e o check do Linux limpos.
- [ ] **Passo 5: sonda de tela.** A lista bate com os seis mais recentes medidos na spec (Portal-Mizuho no topo). Abrir o `lumi` (local) e o `garimpo-bot-frontend` (WSL) pelo painel; os dois abrem no VS Code. Pedir um momento ao usuário antes.
- [ ] **Passo 6: commit** `feat(app): VS Code recent projects panel`.

### Tarefa 6: Energia

**Arquivos:**
- Criar: `crates/modules/bar/src/power.rs`, `crates/platform-windows/src/power.rs`, `crates/app/src/power_loop.rs`

**Interfaces:**
- Produz: em `modules/bar/src/power.rs`:
  - `pub enum PowerAction { Lock, Sleep, SignOut, Restart, Shutdown }`;
  - `pub struct ConfirmState { armed: Option<(PowerAction, f64)> }`, com `click(&mut self, a: PowerAction, now: f64) -> Option<PowerAction>`: Lock e Sleep agem na hora; os outros armam no primeiro clique e agem no segundo, se for dentro de 3,0 s. `expire(&mut self, now: f64) -> bool` e `cancel(&mut self)`;
  - `pub fn format_uptime(secs: u64) -> String`: "Ligado há 3 d 4 h", "Ligado há 5 h 12 min", "Ligado há 7 min";
  - `pub fn power_panel(user: &str, initials: &str, picture: Option<&Image>, uptime: &str, armed: Option<PowerAction>, scale: f32, tokens: &Tokens) -> (DisplayList, Vec<(Rect, PowerAction)>)`: largura 320; grade 3+2 de botões com ícone e rótulo ("Bloquear", "Suspender", "Sair", "Reiniciar", "Desligar"); o botão armado diz "Clique de novo para sair" (ou reiniciar, desligar), com o fundo `accent_soft`.
- Produz: em `platform-windows/src/power.rs`, `pub fn run(a: PowerAction) -> windows::core::Result<()>` (`LockWorkStation`, `SetSuspendState(false, false, false)`, `ExitWindowsEx(EWX_LOGOFF | EWX_REBOOT | EWX_SHUTDOWN)` com `SE_SHUTDOWN_NAME` ajustado, sem `EWX_FORCE`), `pub fn uptime() -> Duration` (`GetTickCount64`) e `pub fn user_info() -> (String, Option<Image>)`. O `user_info` usa `Windows.System.User` (`DisplayName`, `GetPictureAsync`), cai para `GetUserNameExW(NameDisplay)` e depois para `GetUserNameW`, e roda numa thread.
- Produz: `ZEFIRO_POWER_DRY_RUN=1` faz o `power_loop` só registrar `ação de energia (simulada): {a:?}` no log em vez de chamar `run`.

- [ ] **Passo 1: testes que falham:**
  - `lock_and_sleep_act_on_first_click`;
  - `destructive_actions_need_second_click_within_3s`;
  - `second_click_after_3s_rearms`;
  - `arming_another_action_never_runs_the_first` (Ponto de atenção 5);
  - `cancel_disarms`;
  - `uptime_formats`;
  - `panel_shows_armed_label`.
- [ ] **Passo 2:** `cargo test -p zefiro-bar power` → FALHA.
- [ ] **Passo 3: implementar.**
  - A tecla Esc e o fechamento do painel chamam `cancel`; um timer de 3 s chama `expire` e redesenha.
  - Antes de `SignOut`, `Restart` e `Shutdown`, a barra libera a AppBar e fecha como no "Sair" do menu, e só então chama `run`.
- [ ] **Passo 4:** `cargo test --workspace` → PASSA; clippy limpo.
- [ ] **Passo 5: sonda de tela.**
  - Com `ZEFIRO_POWER_DRY_RUN=1`: Reiniciar arma, o segundo clique registra a ação simulada e a barra fecha e libera a AppBar.
  - Bloquear de verdade só com o usuário presente e de acordo.
  - O painel mostra nome, foto (ou iniciais) e o tempo ligado.
- [ ] **Passo 6: commit** `feat(app): power panel with two-click confirmation`.

### Tarefa 7: Notificações

**Arquivos:**
- Modificar: `crates/platform-windows/src/notifications.rs`, `crates/app/src/island_loop.rs`
- Criar: `crates/app/src/system_loop.rs`

**Interfaces:**
- Produz: em `NotificationWatcher`, `pub fn count(&self) -> usize` e `pub fn clear(&self)`. `clear` chama `ClearNotifications` na thread do serviço. O aviso `notify` também dispara quando a contagem muda, não só com notificação nova.
- Produz: `pub fn open_notification_center() -> windows::core::Result<()>`, que abre `ms-actioncenter:` e, se falhar, manda Win+N por `SendInput`.
- Produz: o watcher passa da ilha para `Loop.system.notifications`. Ele liga quando a ilha embutida está ativa **ou** algum layout tem `notifications`, e é reavaliado na mudança de configuração. A ilha lê dele.
- Produz: `pub fn bell_chip(count: usize, …) -> ChipContent` em `modules/bar/src/chips.rs`: sino apagado com 0; com 1 ou mais, sino com ponto `accent` e o número (reserva a largura de "99+"; acima de 99 mostra "99+").

- [ ] **Passo 1: testes que falham:** `bell_without_notifications_is_dim`, `bell_shows_count_and_caps_at_99`, `bell_width_is_stable`.
- [ ] **Passo 2:** `cargo test -p zefiro-bar bell` → FALHA.
- [ ] **Passo 3: implementar.**
  - Clique esquerdo: `open_notification_center`.
  - Meio: `clear`.
  - A ilha continua mostrando os avisos como hoje. Conferir que `island_notifications` funciona com o watcher novo.
- [ ] **Passo 4:** `cargo test --workspace` → PASSA; clippy limpo.
- [ ] **Passo 5: sonda de tela.**
  - Uma notificação de teste (`New-BurntToastNotification`, ou um toast pelo PowerShell com `Windows.UI.Notifications`) sobe o contador em até 2 s.
  - O clique abre a central.
  - Limpar só com o usuário de acordo.
  - Com `[island] enabled = false` no perfil de teste, o contador continua funcionando.
- [ ] **Passo 6: commit** `feat(app): notifications chip with count, center and clear`.

### Tarefa 8: Não perturbe

**Arquivos:**
- Criar: `crates/platform-windows/examples/dnd_toggle_probe.rs`
- Modificar: `crates/platform-windows/src/dnd.rs`, `crates/app/src/system_loop.rs`, `crates/app/src/island_loop.rs`, `crates/app/src/event_loop.rs`

**Interfaces:**
- Produz: `pub fn set_active(on: bool) -> windows::core::Result<()>` em `dnd.rs`. Só existe se a sonda passar; se não passar, o chip usa `open_settings()`, que abre `ms-settings:notifications`.
- Produz: `dnd::watch` passa a ser chamado no início do `Loop`, sem depender da ilha. O `USER_DND` atualiza o chip (`apply_bars(false)`) e a ilha (`island_dnd_changed`, que continua saindo cedo sem ilha).

- [ ] **Passo 1: sonda.** `dnd_toggle_probe` lê `dnd::active()`, tenta ligar o perfil "somente prioridade" (`NtUpdateWnfStateData` no `WNF_QUIET_HOURS_ACTIVE_PROFILE_CHANGED` e, se não funcionar, o serviço Quiet Hours como o `dnd_api` do YASB), espera até 1 s pela leitura mudar, imprime o resultado e **sempre restaura** o estado original no fim. Pedir um momento ao usuário antes de rodar.
- [ ] **Passo 2: decidir pela sonda e registrar.** Ruling no ledger: "alternar" (a leitura mudou em ≤ 1 s nas 5 tentativas) ou "abrir configurações" (qualquer falha). Se for "abrir configurações", registrar também o risco para o ADR.
- [ ] **Passo 3: implementar o chip.** Ícone de lua; `accent` quando ativo. O clique chama `set_active(!active())` ou `open_settings()`, conforme o Passo 2.
- [ ] **Passo 4:** `cargo test --workspace` → PASSA; clippy limpo.
- [ ] **Passo 5: sonda de tela.** O clique alterna (ou abre as configurações); ligar o Não perturbe pela central do Windows acende o chip em ≤ 1 s, com e sem a ilha.
- [ ] **Passo 6: commit** `feat(app): do not disturb chip`.

### Tarefa 9: Apps

> **Removida a pedido do usuário (2026-10-19).** O chip foi implementado e, ao ver na tela, o usuário pediu para tirar: misturar atalhos com apps abertos não ficou bom, e a barra de tarefas do Windows e o Raycast já cobrem. O código foi descartado e o id `apps` saiu do catálogo. O texto abaixo fica só como registro.

**Arquivos:**
- Criar: `crates/modules/bar/src/apps.rs` (modelo portável e desenho)
- Criar: `crates/platform-windows/src/taskbar.rs` (janelas, ganchos, ações e atalhos `.lnk`)
- Criar: `crates/app/src/apps_loop.rs`
- Modificar: `crates/core/src/config.rs` (`[apps] pinned`, `set_array`), `crates/app/src/bar_view.rs` (clique e realce por ícone), `crates/modules/bar/src/chips.rs` (`ChipContent.parts`)

**Interfaces:**
- Produz: em `zefiro-core`, `#[serde(default)] pub struct AppsConfig { pub pinned: Option<Vec<String>> }` no `Config` (`apps`), e `pub fn set_array(path: &Path, keys: &[&str], values: &[String]) -> io::Result<()>`, que preserva comentários como `set_value`.
- Produz: em `modules/bar/src/apps.rs`:
  - `pub struct AppWindow { pub hwnd: isize, pub app_key: String, pub title: String, pub minimized: bool }`;
  - `pub struct PinnedApp { pub key: String, pub launch: String, pub name: String }`;
  - `pub struct AppSlot { pub key: String, pub name: String, pub pinned: bool, pub windows: Vec<isize>, pub foreground: bool }`;
  - `pub fn slots(pinned: &[PinnedApp], windows: &[AppWindow], foreground: Option<isize>, max: usize) -> (Vec<AppSlot>, Vec<AppSlot>)`: fixados primeiro e depois abertos; `max` = 12; o segundo vetor é o excedente;
  - `pub enum ClickPlan { Launch, Minimize(isize), Activate(isize), NewInstance }`;
  - `pub fn left_click_plan(slot: &AppSlot, foreground: Option<isize>, last_cycled: Option<isize>) -> ClickPlan`, que segue as regras da spec §4.4;
  - `pub fn apps_chip(slots: &[AppSlot], icons: &dyn Fn(&str) -> Option<Image>, chip_h: f32, scale: f32, tokens: &Tokens) -> ChipContent`, com ícone de 18 px × `scale`, espaço de 6 px e ponto de 3 px embaixo dos apps abertos (`accent` no app em primeiro plano, `text_secondary` nos outros).
- Produz: `ChipContent` ganha `pub parts: Vec<(Rect, String)>` (vazio nos chips atuais), e `BarView::part_at(x, y) -> Option<(String, String)>` devolve o chip e a parte. O realce usa o retângulo da parte quando há partes.
- Produz: em `platform-windows/src/taskbar.rs`:
  - `pub struct TaskbarWatcher`, com `start(notify) -> io::Result<Self>`, `windows() -> Vec<AppWindow>` e `foreground() -> Option<isize>`. Thread com `SetWinEventHook` (`EVENT_OBJECT_CREATE`/`DESTROY`/`SHOW`/`HIDE`/`NAMECHANGE`, `EVENT_SYSTEM_FOREGROUND`, `EVENT_SYSTEM_MINIMIZESTART`/`END`) e o laço de mensagens próprio dela; avisa no máximo uma vez a cada 50 ms e só quando o conjunto muda;
  - `pub fn app_key(hwnd) -> String`: AUMID pela `PropertyStore` da janela (e, no `ApplicationFrameHost`, pela janela filha do app), senão o caminho do exe em minúsculas;
  - `pub fn windows_pins() -> Vec<PinnedApp>`, a partir de `%APPDATA%\Microsoft\Internet Explorer\Quick Launch\User Pinned\TaskBar\*.lnk` (alvo resolvido por `IShellLinkW`);
  - `pub fn activate(hwnd)`, `pub fn minimize(hwnd)`, `pub fn close(hwnd)` (`WM_CLOSE`) e `pub fn launch(launch: &str)` (`ShellExecuteW` do `.lnk` ou exe; AUMID por `shell:AppsFolder\<aumid>`).
- Produz: o critério de janela da barra de tarefas, como `pub fn is_taskbar_window(hwnd) -> bool`: visível, sem dono, sem `WS_EX_TOOLWINDOW`, não "cloaked" (`DWMWA_CLOAKED`), com título, e não da classe `ZefiroBar`, `ZefiroPanel` ou `ZefiroInput`.

- [ ] **Passo 1: testes que falham.**
  - Em `apps.rs`:
    - `pinned_come_first_then_open_unpinned`;
    - `windows_of_pinned_app_attach_to_pin`;
    - `groups_store_apps_by_aumid_not_frame_host` (Ponto de atenção 4: duas janelas com `app_key` de AUMID diferentes e o mesmo exe `applicationframehost.exe` → dois slots);
    - `overflow_after_twelve`;
    - `click_plan_launch_minimize_activate_cycle` (fechado → `Launch`; uma janela em primeiro plano → `Minimize`; uma janela atrás → `Activate`; três janelas → alterna pela ordem a partir de `last_cycled`);
    - `chip_parts_match_icons`.
  - Em `config.rs`: `apps_pinned_absent_means_windows_pins` e `set_array_keeps_comments`.
- [ ] **Passo 2:** `cargo test -p zefiro-bar apps -p zefiro-core` → FALHA.
- [ ] **Passo 3: implementar o modelo, `set_array` e `parts`/`part_at`.** Com isso, os testes do Passo 1 passam.
- [ ] **Passo 4: implementar `taskbar.rs`.** Sonda `examples/taskbar_probe.rs` que lista os slots e as janelas atuais (só nome e `app_key`, sem títulos de janela no log).
- [ ] **Passo 5: ligar o `apps_loop.rs`** (`USER_APPS = 22`).
  - Ícones por `app_icon` com cache por `app_key`, carregados na thread do watcher.
  - Clique esquerdo segue `left_click_plan`; meio → `launch` (nova instância).
  - Direito → `popup_menu::show` com "Nova janela", "Fixar na barra"/"Desafixar da barra" e "Fechar janela"/"Fechar todas as janelas".
  - Fixar e desafixar gravam `[apps] pinned` com `set_array` (na primeira vez, a lista parte dos fixados do Windows).
- [ ] **Passo 6:** `cargo test --workspace` → PASSA; clippy e imports limpos.
- [ ] **Passo 7: sonda de tela** com o Bloco de notas aberto pelo teste. Pedir um momento ao usuário antes.
  - O ícone aparece com o ponto.
  - Clique minimiza e restaura.
  - Botão do meio abre uma segunda janela; o clique passa a alternar entre as duas.
  - "Fechar todas as janelas" fecha só as do Bloco de notas.
  - Fixar grava no `config.toml` do perfil de teste e o ícone fica depois de fechar.
  - Os fixados do Windows (Brave, Explorador, VS Code) aparecem na ordem.
  - Conferir `WindowFromPoint` sobre um ícone: a janela de entrada da barra recebe o clique.
- [ ] **Passo 8: commit** `feat(app): apps chip with pinned and open apps`.

### Tarefa 10: Medição, revisão e documentação

**Arquivos:**
- Criar: `docs/adr/0011-widgets-do-yasb-fase-1e.md` (modelo `0000-modelo.md`)
- Modificar: `docs/PLANNING.md` (§12: a 1e passa a ser os widgets do YASB; a 1f, itens declarativos + Modo noturno; tabela de widgets da Fase 1 com VS Code pelo `workspaceStorage`), `AGENTS.md` (estrutura: `integrations/usage` e `integrations/vscode`; coisas não óbvias: Claude nunca renova token, Apps por ganchos, Não perturbe conforme a sonda), `README.md` (status)

- [ ] **Passo 1: medir com o YASB fechado.** `scripts/perf.ps1 -Process zefiro -WarmupSeconds 20 -SampleSeconds 60` com o perfil do usuário e com o perfil de teste dos sete chips, os dois recém-abertos. Meta: CPU 0,0 % e diferença ≤ 4 MB privados. Medir também depois de abrir cada painel uma vez, só para registrar.
- [ ] **Passo 2: barra do usuário igual.** Captura da faixa com o perfil do usuário antes da Tarefa 1 (guardada no ledger) e agora; tem de ser visualmente igual.
- [ ] **Passo 3: revisão da branch inteira contra a spec.** Corrigir o que for Critical ou Important, cada correção com um teste que falhe antes. Registrar os Minor no ledger.
- [ ] **Passo 4: suíte final.** `cargo test --workspace`, `cargo clippy --workspace --all-targets -- -D warnings`, `scripts/check-windows-imports.ps1` e `cargo check --target x86_64-unknown-linux-gnu` das crates portáveis (incluindo `zefiro-integration-usage` e `zefiro-integration-vscode`).
- [ ] **Passo 5: escrever o ADR-0011 e atualizar PLANNING, AGENTS e README.** O ADR traz decisões, números, validações feitas, pendentes e os Minor adiados.
- [ ] **Passo 6: commit** `docs(adr): record phase 1e YASB widgets delivery and validation`.
- [ ] **Passo 7: oferecer ao usuário,** sem fazer antes do "sim", colocar os itens na ordem do YASB no `config.toml` dele (spec §3). Entregar a lista de Rulings e de Minors adiados.
