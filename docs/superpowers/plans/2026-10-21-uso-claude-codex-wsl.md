# Uso do Claude e do Codex no Windows e no WSL — plano

> **Para agentes:** SUB-SKILL OBRIGATÓRIA: use superpowers:subagent-driven-development (recomendado) ou
> superpowers:executing-plans para executar este plano tarefa por tarefa. Os passos usam caixas
> (`- [ ]`) para acompanhar.

**Objetivo:** os chips `claude` e `codex` atualizam sem reiniciar a barra e mostram as contas do
Windows e das distros do WSL que estão rodando, cada uma identificada no painel.

**Arquitetura:** o `http` da plataforma deixa de usar o cache do `HttpClient`. A crate portável
`zefiro-integration-usage` ganha `origins.rs` (origens, contas genéricas `Account<T>`, descoberta das
pastas `home` de uma distro) e, em `claude.rs`/`codex.rs`, identidade, junção por conta e % do chip
com várias contas; o `CodexService` passa a ler várias origens e a acordar no `resets_at`. A
`platform-windows` ganha `wsl.rs` (distros instaladas pelo registro, rodando pelo `wsl.exe` com cache,
caminho `\\wsl.localhost`) e o `ClaudeService` com várias contas. O `zefiro-bar` desenha o painel em
blocos e o `zefiro-app` junta tudo em `usage_loop.rs`.

**Stack:** Rust 2024, `serde_json`, WinRT `Windows.Web.Http(.Filters)`, registro (`HKCU\...\Lxss`),
`wsl.exe --list --running --quiet`.

**Spec:** `docs/superpowers/specs/2026-10-21-uso-claude-codex-wsl-design.md` (leia junto; os § abaixo
são dela).

Ledger: `.superpowers/sdd/2026-10-21-uso-claude-codex-wsl/progress.md`. Verificação de cada tarefa:
`cargo test -p <crate>`; no fim, `cargo test --workspace`,
`cargo clippy --workspace --all-targets -- -D warnings` e
`pwsh -NoProfile -File scripts/check-windows-imports.ps1`.

## Restrições globais

- Crate `windows` só em `crates/platform-windows`, `crates/integrations/clock/src/windows.rs` e
  `crates/app`. `integrations/usage` e `modules/bar` não a usam.
- `clippy::all = deny`; `unsafe` só em blocos pequenos com `// SAFETY:`.
- Textos de interface, comentários e logs em pt-BR; identificadores e commits em inglês
  (`fix(platform-windows): …`, `feat(usage): …`, `feat(bar): …`, `feat(app): …`, `docs(adr): …`).
- **Nunca** ler `\\wsl.localhost\<distro>` de uma distro que não está em `wsl --list --running`.
- **Nunca** gravar, renovar ou logar token. E-mail, `accountUuid`, `creator_account_id` e caminhos com
  nome de usuário não vão para log, teste com dado real, sonda ou diagnóstico. E-mail só no painel.
- Nada de I/O na thread da UI; nada de temporizador na UI para o reinício do Codex.
- Nenhuma chave nova no `config.toml`. Única variável nova: `ZEFIRO_USAGE_WINDOWS_HOME` (só testes).
- Distros cujo nome começa com `docker-desktop` (sem diferenciar maiúsculas) são ignoradas.
- Textos exatos (§6.2, §6.6):
  - sem conta e WSL parado: `"WSL desligado.\nAbra o WSL para ver o uso de lá."` (Claude e Codex);
  - sem conta: `"Faça login no Claude Code"` (Claude) e `"Nenhum uso do Codex encontrado"` (Codex);
  - token vencido: `"Login expirado no {origem}.\nAbra o Claude Code no {origem} para renovar."`, com
    `{origem}` = `"Windows"` ou `"WSL"`;
  - origem parada: sufixo `" · WSL desligado"` no subtítulo do bloco;
  - rótulos de origem: `"Windows"`, `"WSL"`, `"WSL · <distro>"` (só com contas em mais de uma distro);
    origens juntas: `"Windows · WSL"`;
  - log: `"uso do Claude: 2 contas (Windows, WSL)"` / `"uso do Codex: 1 conta (Windows · WSL)"`.

## Pontos de revisão

1. **Nome da distro em minúsculas:** `wsl_running_distros()` devolve minúsculas; o rótulo
   ("WSL · Ubuntu-24.04") e o caminho usam o nome do registro. Uma distro rodando que não está no
   registro mantém o nome como veio. → Task 2 (`proper_names`).
2. **Sessão do Codex que passou da meia-noite:** o arquivo mais modificado pode estar na pasta do dia
   anterior; o percurso pelas pastas de data não pode perdê-lo. → Task 4 (`recent_sessions`).
3. **Mesma conta do Claude com o token do Windows vencido e o do WSL válido:** a consulta usa o
   válido; o painel não pode dizer "Login expirado". → Task 3 (`first_valid_token`).
4. **Pasta `home` de outro usuário Linux sem `.claude`/`.codex`, ou sem permissão:** é ignorada sem
   erro e sem log a cada ciclo. → Task 2.
5. **Chip do Codex com a janela reiniciando com a barra aberta e nenhum arquivo novo:** cai para 0 %
   em até ~1 s depois do `resets_at`. → Task 5.

---

### Task 1: HTTP sem cache

**Files:**
- Modify: `Cargo.toml` (a feature `"Web_Http_Filters"` já foi acrescentada na investigação; conferir
  que está na lista da crate `windows`)
- Modify: `crates/platform-windows/src/http.rs` (`client()`)
- Create: `crates/platform-windows/examples/http_cache_probe.rs`
- Delete: `crates/platform-windows/examples/claude_cache_probe.rs` (sonda temporária da investigação)

**Interfaces — produz:**
```rust
// http.rs
pub fn get_source(url: &str) -> windows::core::Result<(u16, bool /* veio do cache */)>;
```

- [ ] **Step 1: Escrever a sonda `http_cache_probe`.** Chama `http::get_source` duas vezes na URL do
  clima (`weather::url` ou a URL fixa do Open-Meteo para São Paulo) e imprime
  `"1ª: status=200 origem=rede"` / `"2ª: status=200 origem=cache|rede"`. Sai com código 1 se a 2ª vier
  do cache.
- [ ] **Step 2: Implementar `get_source`** em `http.rs` com o `client()` atual e
  `response.Source()? == HttpResponseMessageSource::Cache`.
- [ ] **Step 3: Rodar e ver falhar.** `cargo run -q -p zefiro-platform-windows --example http_cache_probe`
  → a 2ª linha diz `origem=cache`, código 1.
- [ ] **Step 4: `client()` com filtro sem cache.** `HttpBaseProtocolFilter::new()`,
  `CacheControl()?.SetReadBehavior(HttpCacheReadBehavior::NoCache)`,
  `SetWriteBehavior(HttpCacheWriteBehavior::NoCache)`, `HttpClient::Create(&filter)`. Comentário de
  uma linha com o porquê (o cache devolvia a 1ª resposta do processo; medido em 2026-10-21).
- [ ] **Step 5: Rodar e ver passar.** A mesma sonda → as duas linhas `origem=rede`, código 0.
- [ ] **Step 6: Apagar `claude_cache_probe.rs`;** `cargo clippy -p zefiro-platform-windows --all-targets -- -D warnings`.
- [ ] **Step 7: Commit** `fix(platform-windows): bypass the HttpClient cache so usage refreshes`.

### Task 2: Origens e contas (portável)

**Files:**
- Create: `crates/integrations/usage/src/origins.rs`
- Modify: `crates/integrations/usage/src/lib.rs` (`pub mod origins;`)

**Interfaces — produz:**
```rust
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum OriginKind { Windows, Wsl(String) } // Windows < Wsl; Wsl em ordem do nome

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Origin { pub kind: OriginKind, pub home: std::path::PathBuf }

/// O que a plataforma descobriu num ciclo.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Discovery { pub origins: Vec<Origin>, pub running: Vec<String>, pub wsl_stopped: bool }
pub type Discover = std::sync::Arc<dyn Fn() -> Discovery + Send + Sync>;

#[derive(Debug, Clone, PartialEq)]
pub struct Account<T> { pub key: String, pub origins: Vec<OriginKind>, pub stopped: bool, pub data: T }

pub fn is_ignored_distro(name: &str) -> bool;
pub fn proper_names(running_lower: &[String], installed: &[String]) -> Vec<String>;
pub fn wsl_origins(distros: &[String], root: &dyn Fn(&str) -> std::path::PathBuf) -> Vec<Origin>;
pub fn origins_label(origins: &[OriginKind], many_distros: bool) -> String;
pub fn many_distros<T>(accounts: &[Account<T>]) -> bool;
pub fn sort_accounts<T>(accounts: &mut [Account<T>]);
pub fn carry_stopped<T: Clone>(previous: &[Account<T>], current: &mut Vec<Account<T>>, running: &[String]);
pub fn accounts_summary<T>(service: &str, accounts: &[Account<T>]) -> String;
```

Regras: `wsl_origins` lista as subpastas de `root(distro).join("home")` (ordem alfabética); falha ao
listar = nenhuma origem daquela distro, sem log. `carry_stopped` repõe, com `stopped = true`, as contas
de `previous` cuja `key` não está em `current` e cujas origens são todas `Wsl(d)` com `d` fora de
`running` (comparação sem maiúsculas); contas com `Windows` nunca são repostas; depois `sort_accounts`
(pela menor `OriginKind` de cada conta). `accounts_summary("Claude", …)` → `"uso do Claude: 2 contas (Windows, WSL)"`.

- [ ] **Step 1: Testes que falham** (em `origins.rs`, pastas em `std::env::temp_dir()` com o pid, como
  os testes do `codex.rs`):
  - `ignores_docker_distros`: `is_ignored_distro("docker-desktop")`, `("Docker-Desktop-Data")` → true;
    `("Ubuntu-24.04")` → false.
  - `proper_names_use_registry_case`: `proper_names(&["ubuntu-24.04", "docker-desktop", "arch"], &["Ubuntu-24.04", "docker-desktop"])`
    == `["arch", "Ubuntu-24.04"]` (sem `docker-desktop`, nome do registro, ordem sem diferenciar maiúsculas).
  - `wsl_origins_lists_home_folders`: raiz temporária com `home/ana` e `home/bia` → duas `Origin` com
    `kind = Wsl("Ubuntu")` e `home` = cada pasta; raiz sem `home` → vazio.
  - `labels`: `origins_label(&[Windows], false)` == `"Windows"`;
    `(&[Wsl("Ubuntu-24.04".into())], false)` == `"WSL"`; `(…, true)` == `"WSL · Ubuntu-24.04"`;
    `(&[Windows, Wsl("U".into())], false)` == `"Windows · WSL"`.
  - `carry_keeps_stopped_wsl_accounts_only`: previous = [conta "a" em Windows, conta "b" em
    Wsl("Ubuntu")], current = [], running = [] → current = [conta "b" com `stopped == true`]; com
    `running = ["ubuntu"]` → current fica vazio (a distro roda, a conta sumiu de verdade).
  - `summary_has_no_personal_data`: com `key = "uuid-x"` e duas contas → `"uso do Claude: 2 contas (Windows, WSL)"`;
    uma conta → `"… 1 conta (…)"`; o texto não contém `"uuid-x"`.
- [ ] **Step 2: Rodar e ver falhar:** `cargo test -p zefiro-integration-usage origins` → erros de compilação.
- [ ] **Step 3: Implementar** os tipos e funções acima.
- [ ] **Step 4: Rodar e ver passar:** `cargo test -p zefiro-integration-usage origins`.
- [ ] **Step 5: Commit** `feat(usage): usage origins and accounts across Windows and WSL`.

### Task 3: Contas do Claude (portável)

**Files:**
- Modify: `crates/integrations/usage/src/claude.rs`

**Interfaces — consome:** `Origin`, `OriginKind`, `Account<T>`, `sort_accounts` (Task 2). **Produz:**
```rust
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ClaudeIdentity { pub account_uuid: Option<String>, pub email: Option<String> }
pub fn parse_identity(claude_json: &str) -> ClaudeIdentity;        // oauthAccount.accountUuid / emailAddress
pub fn plan_name(credentials_json: &str) -> Option<String>;        // claudeAiOauth.subscriptionType "pro" → "Pro"

#[derive(Debug, Clone, PartialEq)]
pub struct ClaudeEntry {
    pub email: Option<String>,
    pub plan: Option<String>,
    pub credentials: Vec<std::path::PathBuf>, // ordem de preferência: Windows, depois WSL por distro
    pub result: Option<Result<ClaudeUsage, UsageError>>,
    pub last_ok: Option<ClaudeUsage>,
}
#[derive(Debug, Clone, PartialEq)]
pub struct ClaudeFound { pub origin: Origin, pub identity: ClaudeIdentity, pub plan: Option<String> }
#[derive(Debug, Clone, Default, PartialEq)]
pub struct ClaudeSnapshot { pub accounts: Vec<Account<ClaudeEntry>>, pub wsl_stopped: bool, pub at: Option<std::time::Instant> }

pub fn group_claude(found: Vec<ClaudeFound>) -> Vec<Account<ClaudeEntry>>;
pub fn inherit_claude(previous: &[Account<ClaudeEntry>], current: &mut [Account<ClaudeEntry>]);
pub fn first_valid_token(credentials: &[String], now_ms: i64) -> TokenState;
pub fn chip_percent_accounts(accounts: &[Account<ClaudeEntry>], now_unix: i64) -> Option<f32>;
```

Regras: `group_claude` junta por `account_uuid`; sem ele, `key = format!("origem:{:?}", kind)`. O
caminho de credenciais de cada origem é `home/.claude/.credentials.json`. `inherit_claude` copia
`result` e `last_ok` da conta de mesma `key`. `first_valid_token` devolve o 1º `Valid`; senão `Expired`
se algum venceu; senão `Missing`. `chip_percent_accounts` é o máximo, por conta, de `chip_percent`
(resultado ou `last_ok` em falha passageira, incluindo contas `stopped` pelo `last_ok`), contando 0 %
quando `five_hour.resets_at <= now_unix`; nenhuma conta com número → `None`.

- [ ] **Step 1: Testes que falham** (dados inventados, nenhum token real):
  - `identity_from_claude_json`: `{"oauthAccount": {"accountUuid": "u1", "emailAddress": "a@b.c"}}` →
    `Some("u1")`, `Some("a@b.c")`; `"{}"` e texto inválido → `ClaudeIdentity::default()`.
  - `plan_names`: `subscriptionType` `"pro"` → `Some("Pro")`, `"max"` → `Some("Max")`, ausente → `None`.
  - `same_uuid_in_two_origins_is_one_account`: Windows e Wsl("Ubuntu") com `u1` → uma conta,
    `origins == [Windows, Wsl("Ubuntu")]`, `credentials[0]` termina em Windows; `u1` e `u2` → duas.
  - `first_valid_token_skips_expired`: `[vencido, válido]` → `Valid`; `[vencido]` → `Expired`; `[""]` → `Missing`.
  - `chip_is_max_across_accounts`: contas com 5 h = 9 e 17 → `Some(17.0)`; a de 17 com
    `resets_at = now - 1` → `Some(9.0)`; conta `stopped` com `last_ok` de 30 e `resets_at` futuro →
    `Some(30.0)`; todas `Expired` → `None`.
  - `inherit_keeps_last_ok`: previous com `last_ok` na key `u1` → current `u1` recebe o `last_ok`.
- [ ] **Step 2: Rodar e ver falhar:** `cargo test -p zefiro-integration-usage claude`.
- [ ] **Step 3: Implementar.** `chip_percent` e `token_state` de hoje continuam e são reaproveitados.
- [ ] **Step 4: Rodar e ver passar.**
- [ ] **Step 5: Commit** `feat(usage): Claude accounts grouped by account across origins`.

### Task 4: Contas do Codex (portável)

**Files:**
- Modify: `crates/integrations/usage/src/codex.rs`

**Interfaces — consome:** Task 2. **Produz:**
```rust
pub fn session_account(first_line: &str) -> Option<String>; // payload.creator_account_id
pub fn recent_sessions(sessions_dir: &Path, limit: usize) -> Vec<(SystemTime, PathBuf)>; // mais novo primeiro
#[derive(Debug, Clone, PartialEq)]
pub struct CodexRead { pub account: Option<String>, pub modified: SystemTime, pub usage: CodexUsage }
pub fn read_codex_accounts(sessions_dir: &Path) -> Vec<CodexRead>;
#[derive(Debug, Clone, PartialEq)]
pub struct CodexEntry { pub modified: SystemTime, pub usage: CodexUsage }
#[derive(Debug, Clone, Default, PartialEq)]
pub struct CodexSnapshot { pub accounts: Vec<Account<CodexEntry>>, pub wsl_stopped: bool }
pub fn merge_codex(per_origin: Vec<(OriginKind, Vec<CodexRead>)>) -> Vec<Account<CodexEntry>>;
pub fn codex_chip_percent(accounts: &[Account<CodexEntry>], now_unix: i64) -> Option<f32>;
pub fn next_reset(accounts: &[Account<CodexEntry>], now_unix: i64) -> Option<i64>; // menor resets_at > now
```

Regras:
- `recent_sessions`: em cada nível, as subpastas com nome só de dígitos são visitadas da maior para a
  menor; ao juntar `limit` arquivos, visita **mais uma** pasta de dia e para (pega a sessão que passou
  da meia-noite). Pastas com outros nomes e arquivos soltos são percorridos sempre, como hoje. Resultado
  ordenado por `modified`, mais novo primeiro.
- `read_codex_accounts`: dos `MAX_FILES` mais novos, o `rate_limits` mais recente de **cada** conta; a
  conta vem de `session_account` da 1ª linha do arquivo (ler só até o 1º `\n`). Arquivo sem
  `creator_account_id` entra na conta do arquivo mais novo que tem; se nenhum tem, `account = None`.
- `merge_codex`: `key` = conta ou `format!("origem:{:?}", kind)` se `None`; mesma `key` em várias
  origens → uma conta com todas as origens e o `CodexEntry` cujo `usage.event_at` (por `parse_iso_utc`)
  é o mais recente (empate ou sem data: o `modified` mais novo). `sort_accounts` no fim.
- `read_codex` e `newest_session` passam a usar `recent_sessions`.

- [ ] **Step 1: Testes que falham** (a fixture `codex_session.jsonl` existente; a 1ª linha dela não tem
  `session_meta`, então os testes escrevem uma linha `{"type":"session_meta","payload":{"creator_account_id":"c1"}}` antes):
  - `session_account_from_meta`: `"c1"`; linha sem o campo → `None`.
  - `recent_sessions_walks_newest_days_first`: `2026/10/20/a.jsonl`, `2026/10/21/b.jsonl`,
    `2025/01/01/velho.jsonl`; `limit = 1` → contém `b` e `a` (a pasta extra), não contém `velho`.
  - `session_from_yesterday_modified_today_is_found`: `a` (pasta 20) gravado depois de `b` (pasta 21);
    `limit = 1` → o 1º do resultado é `a`.
  - `two_accounts_in_one_origin`: `c1` (velho, 80 %) e `c2` (novo, 42 %) → dois `CodexRead`.
  - `file_without_account_joins_newest_known`: arquivo novo sem campo + arquivo com `c1` → um `CodexRead` com `Some("c1")`.
  - `same_account_across_origins_keeps_newest_event`: Windows `c1` em `2026-10-16T02:56:30Z` (80 %) e
    Wsl `c1` em `2026-10-21T10:00:00Z` (12 %) → uma conta, 12 %, `origins == [Windows, Wsl(..)]`.
  - `chip_and_next_reset`: contas com primária 80 % (`resets_at = now + 100`) e 30 % → chip `Some(80.0)`,
    `next_reset == Some(now + 100)`; com `resets_at = now` → chip `Some(30.0)`.
  - Os testes antigos de `read_codex`/`falls_back_to_older_session…` continuam passando.
- [ ] **Step 2: Rodar e ver falhar:** `cargo test -p zefiro-integration-usage codex`.
- [ ] **Step 3: Implementar.**
- [ ] **Step 4: Rodar e ver passar.**
- [ ] **Step 5: Commit** `feat(usage): Codex accounts per origin with newest-day session walk`.

### Task 5: `CodexService` com várias origens e aviso no reinício

**Files:**
- Modify: `crates/integrations/usage/src/codex.rs` (`CodexService`)

**Interfaces — consome:** `Discover`, `Discovery`, `carry_stopped` (Task 2); Task 4. **Produz:**
```rust
impl CodexService {
    pub fn start(discover: Discover, notify: Arc<dyn Fn() + Send + Sync>) -> std::io::Result<CodexService>;
    pub fn latest(&self) -> Option<CodexSnapshot>;
    pub fn refresh(&self);
}
```

Regras: a cada volta, `discover()`; para cada origem com `home/.codex/sessions`, o `newest_session`;
se a lista (origem, mais novo) mudou ou é a 1ª volta, relê tudo (`read_codex_accounts` +
`merge_codex` + `carry_stopped` com o snapshot anterior) e avisa se o snapshot mudou. A espera é
`min(120 s, next_reset - agora + 1 s)`; ao acordar por tempo depois de um `resets_at`, avisa mesmo sem
mudança de arquivo (a UI recalcula o `effective_percent`).

- [ ] **Step 1: Testes que falham:**
  - `service_reads_on_start_and_rereads_on_refresh` (o de hoje), adaptado: `discover` devolve uma
    origem Windows com `home` temporário (`home/.codex/sessions/a.jsonl`).
  - `service_notifies_at_reset_without_new_file`: sessão com primária `resets_at = agora + 1` →
    depois do aviso inicial, um 2º aviso chega em até 3 s sem gravar arquivo.
  - `service_keeps_stopped_wsl_account`: 1ª volta com origem Wsl("U") e `running = ["u"]`; `refresh`
    com `Discovery { origins: [], running: [], wsl_stopped: true }` → `latest()` tem a conta com
    `stopped == true` e `wsl_stopped == true`.
- [ ] **Step 2: Rodar e ver falhar:** `cargo test -p zefiro-integration-usage service`.
- [ ] **Step 3: Implementar.**
- [ ] **Step 4: Rodar e ver passar.**
- [ ] **Step 5: Commit** `feat(usage): Codex service over many origins, waking at window reset`.

### Task 6: Descoberta do WSL na plataforma

**Files:**
- Create: `crates/platform-windows/src/wsl.rs`
- Modify: `crates/platform-windows/src/lib.rs` (`pub mod wsl;`)
- Create: `crates/platform-windows/examples/usage_origins_probe.rs`

**Interfaces — consome:** `system::wsl_running_distros()`, `Origin`, `Discovery`, `proper_names`,
`wsl_origins`, `is_ignored_distro` (Task 2). **Produz:**
```rust
pub fn installed_distros() -> Vec<String>;              // HKCU\Software\Microsoft\Windows\CurrentVersion\Lxss\*\DistributionName
pub fn running_distros() -> Vec<String>;                // proper_names(wsl_running_distros(), installed); cache de 100 s num static Mutex
pub fn root(distro: &str) -> std::path::PathBuf;        // \\wsl.localhost\<distro>
pub fn discover(windows_home: &std::path::Path) -> Discovery;
```

Regras: `discover` = origem Windows (`windows_home`) + `wsl_origins(running_distros(), root)`;
`running` = `running_distros()`; `wsl_stopped` = existe distro instalada não ignorada fora de
`running`. O cache faz o Claude e o Codex dividirem um `wsl.exe` por ciclo (§6.2).

- [ ] **Step 1: Sonda `usage_origins_probe`** que imprime, para cada origem, `kind` e se existem
  `.claude/.credentials.json` e `.codex/sessions` (sim/não), e `wsl_stopped`. **Não** imprime `home`.
- [ ] **Step 2: Implementar** `wsl.rs` (registro com `RegOpenKeyExW`/`RegEnumKeyExW`/`RegGetValueW`
  como `system::reg_dword`; sem WSL → listas vazias).
- [ ] **Step 3: Rodar a sonda:** `cargo run -q -p zefiro-platform-windows --example usage_origins_probe`
  → `Windows: claude=sim codex=sim`, `Wsl("Ubuntu-24.04"): claude=sim codex=sim`, nenhuma linha com
  `docker-desktop`. Rodar duas vezes seguidas e conferir com `Measure-Command` que a 2ª chamada a
  `running_distros` não roda o `wsl.exe` (< 1 ms).
- [ ] **Step 4:** `pwsh -NoProfile -File scripts/check-windows-imports.ps1` e clippy da crate.
- [ ] **Step 5: Commit** `feat(platform-windows): discover running WSL distros for usage`.

### Task 7: `ClaudeService` com várias contas

**Files:**
- Modify: `crates/platform-windows/src/claude.rs`
- Modify: `crates/platform-windows/examples/claude_usage_probe.rs` (passa a listar contas)

**Interfaces — consome:** `Discover` (Task 2), Task 3. **Produz:**
```rust
impl ClaudeService {
    pub fn start(discover: Discover, notify: Arc<dyn Fn() + Send + Sync>) -> std::io::Result<ClaudeService>;
    pub fn latest(&self) -> Option<ClaudeSnapshot>;
    pub fn refresh(&self); // como hoje: só consulta se o dado tiver mais de 60 s
}
```

Regras: a cada consulta, `discover()`; das origens com `.claude/.credentials.json`, lê `.claude.json`
(`parse_identity`) e as credenciais (`plan_name`); `group_claude`; `inherit_claude` com o snapshot
anterior; para cada conta, lê os textos de `credentials` e usa `first_valid_token`; `Valid` → `GET`
como hoje; atualiza `result` e, se `Ok`, `last_ok`; `carry_stopped`; `at = Instant::now()`; avisa.
O log de erro por conta só quando muda (com `origins_label`, nunca e-mail). `accounts_summary` no log
só quando o texto muda. A sonda imprime, por conta, `origins_label`, plano, "tem e-mail: sim/não" e o
status (`Ok 5h=17` / `Expired` …), nunca token, e-mail ou uuid.

- [ ] **Step 1: Atualizar a sonda** para usar `ClaudeService::start(Arc::new(|| wsl::discover(&home)), …)`,
  esperar o 1º aviso e imprimir o snapshot.
- [ ] **Step 2: Implementar.**
- [ ] **Step 3: Rodar a sonda:** duas contas, uma `Windows` e uma `WSL`, cada uma com o status real
  (a do WSL pode estar `Expired`; anotar no ledger).
- [ ] **Step 4:** clippy da crate.
- [ ] **Step 5: Commit** `feat(platform-windows): query Claude usage for every account found`.

### Task 8: Painel em blocos

**Files:**
- Modify: `crates/modules/bar/src/usage.rs`

**Interfaces — produz:**
```rust
#[derive(Debug, Clone, PartialEq)]
pub struct UsageBlock { pub title: String, pub subtitle: String, pub rows: Vec<UsageRow>, pub message: Option<String> }
pub fn usage_panel(blocks: &[UsageBlock], footer: &str, scale: f32, tokens: &Tokens, m: &dyn TextMeasurer) -> DisplayList;
fn fit(text: &str, style: &TextStyle, max_w: f32, m: &dyn TextMeasurer) -> String; // corta com "…"
```

Layout: cada bloco = título (13, SemiBold, `text_secondary`, como hoje) em `top + 14`; subtítulo (11,
Regular, `text_tertiary`, cortado por `fit` na largura `size.w - 40`) em `top + 36` se não vazio; as
linhas ou a mensagem começam em `top + 44` (sem subtítulo, igual a hoje) ou `top + 62`. Entre blocos,
12 DIP e uma linha `hairline`. Largura 340 DIP. Rodapé como hoje.

- [ ] **Step 1: Testes que falham:**
  - `one_block_without_subtitle_matches_old_layout`: um bloco com as duas linhas da fixture → mesma
    altura e mesmos `rect` dos textos que o painel de hoje (guardar os valores atuais antes de mudar).
  - `subtitle_is_cut_with_ellipsis`: subtítulo de 200 caracteres → o texto desenhado termina em `"…"`
    e cabe em `size.w - 40` pelo `FixedWidthMeasurer`.
  - `two_blocks_stack_with_hairline`: dois blocos → altura = soma + 12 e uma `RoundRect` com `hairline`
    entre eles; o título do 2º vem depois do último texto do 1º.
  - `message_block`: bloco com `message = Some("WSL desligado.\nAbra o WSL para ver o uso de lá.")` →
    duas linhas de texto na ordem.
- [ ] **Step 2: Rodar e ver falhar:** `cargo test -p zefiro-bar usage`.
- [ ] **Step 3: Implementar.**
- [ ] **Step 4: Rodar e ver passar.**
- [ ] **Step 5: Commit** `feat(bar): usage panel with one block per account`.

### Task 9: Juntar na barra

**Files:**
- Modify: `crates/app/src/app.rs` (`UsageState`)
- Modify: `crates/app/src/usage_loop.rs`
- Modify: `crates/app/src/bar_view.rs:145-146`

**Interfaces — consome:** Tasks 3–8. **Produz:**
```rust
pub struct UsageState { pub claude: Option<ClaudeSnapshot>, pub codex: Option<CodexSnapshot> } // sai claude_last_ok
impl UsageState { pub fn claude_chip(&self) -> Option<f32>; pub fn codex_chip(&self) -> Option<f32>; }
fn windows_home() -> PathBuf; // ZEFIRO_USAGE_WINDOWS_HOME, senão USERPROFILE
fn claude_blocks(s: &ClaudeSnapshot, now: i64, offset: i32) -> Vec<UsageBlock>;
fn codex_blocks(s: &CodexSnapshot, now: i64, offset: i32) -> Vec<UsageBlock>;
```

Regras (§6.5, §6.6 e os textos exatos das restrições globais):
- `usage_services` liga os serviços com `Arc::new(move || wsl::discover(&home))`.
- `usage_claude_changed`/`usage_codex_changed`: refazem a barra só se o número do chip mudou; o painel
  aberto é sempre redesenhado.
- Claude: um bloco por conta; título `"Claude · {plano}"` (sem plano, `"Claude"`); subtítulo
  `"{e-mail} · {origens}"` (sem e-mail, só as origens) + `" · WSL desligado"` se `stopped`; linhas como
  hoje; mensagem do erro da conta (o "Login expirado no {origem}…" usa `"Windows"` se a conta tem
  origem Windows, senão `"WSL"`). Sem contas: um bloco `"Claude"` com a mensagem de WSL desligado (se
  `wsl_stopped`) ou "Faça login no Claude Code". Rodapé `updated_ago(at)`.
- Codex: título `"Codex · {Plano}"`, mais `" · {origens}"` quando há mais de uma conta; subtítulo
  `"Último uso do Codex: {data}"` + `" · WSL desligado"` se `stopped`; sem rodapé. Sem contas: bloco
  `"Codex"` com a mensagem de WSL desligado ou "Nenhum uso do Codex encontrado".
- Com uma conta e sem `stopped`, o painel é o de hoje mais o subtítulo.

- [ ] **Step 1: Testes que falham** (em `usage_loop.rs`, `#[cfg(test)]`, dados inventados):
  - `claude_single_account_block`: conta Windows, `email = "a@b.c"`, plano Pro, 5 h 9 % e 7 d 18 % →
    um bloco `title == "Claude · Pro"`, `subtitle == "a@b.c · Windows"`, 2 linhas.
  - `claude_expired_wsl_message`: conta só WSL com `Err(Expired)` → `message == "Login expirado no WSL.\nAbra o Claude Code no WSL para renovar."`.
  - `claude_no_accounts_with_wsl_stopped`: snapshot vazio, `wsl_stopped = true` → mensagem de WSL desligado; `false` → "Faça login no Claude Code".
  - `codex_two_accounts_titles`: duas contas Plus (Windows, WSL) → títulos `"Codex · Plus · Windows"` e `"Codex · Plus · WSL"`.
  - `stopped_suffix`: conta `stopped` → subtítulo termina em `" · WSL desligado"`.
- [ ] **Step 2: Rodar e ver falhar:** `cargo test -p zefiro-app usage_loop`.
- [ ] **Step 3: Implementar** (e `bar_view.rs` usa `ctx.usage.claude_chip()` / `codex_chip()`).
- [ ] **Step 4:** `cargo test --workspace`, `cargo clippy --workspace --all-targets -- -D warnings`,
  `pwsh -NoProfile -File scripts/check-windows-imports.ps1`.
- [ ] **Step 5: Commit** `feat(app): usage chips and panels over Windows and WSL accounts`.

### Task 10: Na tela, medições e documentação

**Files:**
- Create: `docs/adr/0019-uso-em-varias-origens.md` (modelo `docs/adr/0000-modelo.md`)
- Modify: `AGENTS.md` ("Coisas não óbvias": cache do `HttpClient`; não ler distro parada; chips de uso
  com várias contas), `docs/adr/0011-widgets-do-yasb-fase-1e.md` (nota "ver ADR-0019" nas decisões do
  Claude e do Codex)

Antes de qualquer passo na tela: **pedir um momento ao usuário** (AGENTS.md, regra 6) e fechar o YASB
para as medições (regra 4), reabrindo no fim.

- [ ] **Step 1: Build:** `cargo build --release -p zefiro-app`; fechar a barra pelo `WM_CLOSE` e abrir a nova.
- [ ] **Step 2: Claude acompanha o Claude Code** por 10 min sem reiniciar a barra (comparar com o
  "Account & Usage" do Claude Code); anotar os pares de números.
- [ ] **Step 3: Duas contas do Claude** com e-mail e origem; um bloco do Codex com o evento mais recente.
- [ ] **Step 4: Tester só WSL:** perfil isolado com `ZEFIRO_CONFIG_DIR` e `ZEFIRO_USAGE_WINDOWS_HOME`
  numa pasta vazia → só as contas do WSL.
- [ ] **Step 5: WSL parado** (só com o "sim" do usuário: `wsl --shutdown` derruba o Docker e os
  terminais do WSL) → blocos "WSL desligado"; `wsl -l --running` continua vazio por 5 min.
- [ ] **Step 6: Ociosidade:** com uma distro rodando sem processos do usuário, conferir se ela desliga
  sozinha com o Zefiro lendo a cada 120 s; anotar o resultado (decisão do usuário: aceito, §10).
- [ ] **Step 7: Medição:** `./scripts/perf.ps1 -Process zefiro -WarmupSeconds 20 -SampleSeconds 60`,
  antes (1.1.1) e depois: CPU 0,0 % e memória privada ≤ +1 MB.
- [ ] **Step 8: ADR-0019 e AGENTS.md**, com os números medidos (cache: 2ª consulta do cache antes,
  rede depois; ociosidade; perf).
- [ ] **Step 9: Commit** `docs(adr): usage across Windows and WSL origins`.
- [ ] **Step 10: Revisão da branch inteira contra a spec** (AGENTS.md, fluxo, passo 5): Critical e
  Important corrigidos com teste; Minor no ledger; entregar ao usuário os rulings e os minors adiados.
