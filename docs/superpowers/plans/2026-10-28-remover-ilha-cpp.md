# Plano: remover a ilha C++ e o motor externo

- **Spec:** [2026-10-28-remover-ilha-cpp-design](../specs/2026-10-28-remover-ilha-cpp-design.md)
- **Ledger:** `.superpowers/sdd/2026-10-28-remover-ilha-cpp/progress.md`
- **Branch:** `main` (commits locais, sem push)

Cada tarefa termina com `cargo test --workspace` e `cargo clippy --workspace --all-targets -- -D warnings` passando e um commit próprio.

### Task 1: config sem `engine` e `exe`

- **Teste primeiro** (`crates/core/tests/config.rs`): `legacy_island_keys_are_ignored`. `[island]\nengine = "externa"\nexe = "x"` abre sem erro, com `island.embedded()` e `shows_embedded_island()` como numa config sem essas chaves. `removed_island_keys(texto)` diz `true` para essa config e `false` para `[island]\nenabled = true` e para uma config sem `[island]`. Falha antes porque `removed_island_keys` não existe e o `engine = "externa"` vira `external()`.
- **Implementar:**
  - sai `IslandEngine`, `IslandConfig::{exe, engine}` e `external()`; `embedded()` passa a ser só `enabled`;
  - entra `pub fn removed_island_keys(text: &str) -> bool` (lê a tabela `[island]` com `toml`);
  - saem as linhas `engine` e `exe` do modelo do `config.toml`;
  - `autohide_active()` deixa de olhar a ilha;
  - os testes que usavam `engine = 'externa'` (`config.rs`, `layout_config.rs`) saem ou passam a esperar a ilha embutida.
- **Verificação:** `cargo test -p zefiro-core`.

### Task 2: modo de edição e Configurações sem a ilha externa

- **Testes primeiro** (`crates/modules/bar`):
  - `settings.rs`: os botões são `Back, ToggleAutostart, ToggleHud, ExportDiagnostics`, sem `RestartIsland`;
  - `tests/editor_panel.rs`: o rótulo é sempre "Ocultar automaticamente", e o centro nunca vem travado (saem o `island_locked` do `EditorContext` e o `center_locked` do `EditorLayout`).
- **Implementar:** sai `SettingsAction::RestartIsland` e o "Reiniciar a ilha"; saem `island_locked`/`center_locked` e o ramo que travava o centro; o rótulo do ocultar automaticamente fica fixo.
- **Verificação:** `cargo test -p zefiro-bar`.

### Task 3: o app sem o processo da ilha

Sem teste automático novo (é ligação da `crates/app`); a verificação é compilar, os testes de sempre e a Task 6 na tela.

- Sai `crates/app/src/island.rs` (`mod island`) e as chamadas `crate::island::{start, stop, sync, restart}` no `event_loop`.
- Saem `USER_ISLAND`, `USER_ISLAND + 100`, `TIMER_RAISE`, `USER_ISLAND_ACCENT` e `refresh_island_accent`, com o observador do `island-state.ini` no `main.rs`. A troca de engine no recarregamento da config (`event_loop` ~838) sai.
- Sai "Reiniciar a ilha" do menu do ícone da bandeja (`tray_loop.rs`).
- `bar_view.rs`: sai a reserva do centro para a externa e as chamadas `island_zorder::raise_island()` (também no `robustness.rs`, `tray_loop.rs` e `event_loop.rs`).
- `editor_view.rs`: sai o `island_locked` e o bloqueio do centro no principal.
- `autohide_loop.rs`: sai o aviso "indisponível com a ilha externa".
- No início, se `removed_island_keys` der `true`, o log avisa uma vez: "a ilha externa foi removida; usando a ilha embutida".
- `platform-windows`: sai `island_zorder.rs`, `window::island_reservation`, a mensagem registrada `ISLAND_GEOMETRY` e o `WinEvent::IslandGeometryChanged`.
- **Verificação:** `cargo build -p zefiro-app`, `cargo test --workspace`, clippy, `scripts/check-windows-imports.ps1`.

### Task 4: tirar o crate `island-host` e a pasta `island/`

- `git rm -r island crates/modules/island-host`; sai do `members` e do `[workspace.dependencies]` do `Cargo.toml` e do `crates/app/Cargo.toml`.
- `.github/workflows/ci.yml`: sai o `-p zefiro-island-host`.
- **Verificação:** `cargo test --workspace`, clippy, e `git grep -n "island_host\|island-host\|zefiro-island.exe"` só acha docs antigos.

### Task 5: instalador e scripts

- `installer/zefiro.iss`: sai o `Source` do `zefiro-island.exe` e a espera de 3 s pela ilha ao fechar; entra `[InstallDelete] Type: files; Name: "{app}\zefiro-island.exe"`.
- `scripts/package.ps1`: sai o `-BuildIsland` e a cópia do exe.
- `scripts/stability.ps1`: `$apps` fica só com o `zefiro.exe`. O caso do `zefiro-island.exe` em `scripts/tests/stability.Tests.ps1` passa a esperar que o evento dele **não** conte.
- **Teste primeiro:** mudar o `stability.Tests.ps1` e ver falhar; depois o script.
- **Verificação:** `pwsh -NoProfile -File scripts/tests/stability.Tests.ps1` e `pwsh -NoProfile -File scripts/package.ps1` gerando o instalador.

### Task 6: docs, créditos e conferência na tela

- **ADR-0026** (`docs/adr/0026-remocao-da-ilha-cpp.md`): a remoção e o motivo. A ADR-0006 passa a "substituída por ADR-0026", e a ADR-0009 ganha uma nota de que a externa saiu.
- **README:** a linha do Dynamic Island for Windows vira uma linha curta de inspiração, mais abaixo na tabela; saem `island/`, o build da ilha em "Rodar" e a frase do `zefiro-island.exe`. A linha do Windhawk sai junto, porque só existia por causa do fork.
- **`THIRD_PARTY_NOTICES.md`:** fica "Código portado" para a ilha em Rust, com o texto da licença MIT do projeto original (que estava em `island/LICENSE`); sai o que falava do fork em `island/`.
- **AGENTS.md:** a primeira frase diz que o Zefiro é todo em Rust; saem a linha `island/` e o `island-host` da tabela de estrutura, os comandos da ilha e as notas "Fork da ilha", "Ilha externa e temas novos" e "Barra ↔ ilha externa"; a nota da barra limpa perde "Com a ilha externa a opção é ignorada".
- **PLANNING.md:** uma linha no roteiro com a remoção (ADR-0026).
- **Na tela (com o ok do usuário para reiniciar a barra):** `cargo build --release -p zefiro-app`, fechar a barra por `WM_CLOSE` e abrir a nova; conferir que a ilha aparece no chip, que o modo de edição deixa mexer no centro, que as Configurações não têm "Reiniciar a ilha" e que Sair fecha na hora.
- **Revisão final** da branch contra a spec; Critical e Important corrigidos com teste; Minor no ledger.
