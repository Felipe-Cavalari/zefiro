# Zefiro — Fase 1a (Barra com a linguagem visual da ilha): Plano de implementação

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Criar do zero o `zefiro.exe`: barra AppBar por monitor nas 2 alturas × 2 formatos, os 8 temas da ilha editáveis, painéis que se transformam a partir das pílulas no compositor, relógio + calendário, e o `zefiro-island.exe` embutido no centro.

**Architecture:** Workspace Rust novo na raiz. As crates portáveis (`core`, `platform`, `resources`, `ui`, `bar`, `island-host`, `integration-clock`) produzem **dados**: configuração, tokens de tema, retângulos de layout, **listas de desenho** e decisões do controlador. Elas são testáveis sem Windows. A `zefiro-platform-windows` desenha as listas com Direct2D sobre um dispositivo **WARP** e anima formas arredondadas no **Windows.UI.Composition** com molas nativas (ADR-0001). O recorte do conteúdo e a opacidade ficam presos à forma por animações de expressão, então a CPU fica parada durante a animação. O `zefiro-app` junta tudo num laço de mensagens de uma thread de UI.

**Tech Stack:** Rust stable (edição 2024, MSVC), crate `windows =0.61.3` (Win32 + WinRT Composition), `serde` + `toml` + `toml_edit`, `notify` (observar arquivos), `tracing`/`tracing-appender`, PowerShell 7 para scripts, MSVC/`island/build.ps1` para o fork C++ da ilha.

**Spec:** [`docs/superpowers/specs/2026-10-15-fase-1a-barra-design.md`](../specs/2026-10-15-fase-1a-barra-design.md). Leia junto com o [ADR-0001](../../adr/0001-pipeline-de-renderizacao.md), o [ADR-0006](../../adr/0006-ilha-de-midia-fork.md) e as [notas de porte da ilha](../../island-port-notes.md).

## Global Constraints

- Branch de trabalho: `fase-1a/barra` (criada a partir de `claude/island-arrow-tabs`, que contém `island/`). Nada de `git push`.
- Sistema de referência: Windows 11 Pro 26200, RTX 5070, `DISPLAY2` primário 1920×1080 a 165 Hz, `DISPLAY1` 1920×1080 a 144 Hz.
- Toolchain: `stable-x86_64-pc-windows-msvc` (hoje rustc 1.99), `clippy` e `rustfmt`; alvo `x86_64-unknown-linux-gnu` só para `cargo check` das crates portáveis.
- Edição Rust **2024** em todas as crates. `windows = "=0.61.3"`, `windows-core = "=0.61.2"`, `windows-numerics = "=0.2.0"` (as do S1). Se uma assinatura do plano não bater com essas versões, ajuste à API real e anote no ADR-0007; não troque a versão.
- **Regra de ouro:** a crate `windows` só pode ser dependência de `zefiro-platform-windows`, `zefiro-integration-clock` (só em `src/windows.rs`, atrás de `cfg(windows)`) e `zefiro-app`. Verificado por `scripts/check-windows-imports.ps1` e no CI.
- Todo bloco `unsafe` tem um comentário `// SAFETY:`.
- Alturas: **completa = 44 DIP**, **compacta = 32 DIP**. Formatos: **faixa** e **pílulas**. Tema padrão: **Obsidian**. komorebi fora até a 1.0.
- Molas: crescer 380/26, encolher 200/28, ajuste 280/24, cutucada −6 DIP com 280/24. Estilos: suave 1,0×1,35 · padrão 1,0×1,0 · elástico 1,1×0,70 · ágil 1,5×1,25. Velocidade v: k×v², c×v.
- Raio: pílula = min(altura/2, `radius_max` = 44); painel = `panel_radius` = 24; estilo `w11` = 8 em tudo.
- Painel: 6 DIP abaixo da faixa, 8 DIP de folga da borda da tela; conteúdo aparece com `alfa = clamp((h − 0,6·H) / (0,4·H))`; conteúdo do chip some em 0,17 s.
- Arquivos do usuário: `%APPDATA%\zefiro\config.toml`, `%APPDATA%\zefiro\themes\*.toml`, `%APPDATA%\zefiro\island.ini`; logs em `%LOCALAPPDATA%\zefiro\logs\zefiro.AAAA-MM-DD.log` (7 arquivos).
- Metas do processo `zefiro` (sem a ilha): memória privada ≤ 15 MB em repouso, CPU ~0 % (≤ 0,05 %) em repouso, primeiro quadro ≤ 250 ms, clique → primeiro quadro do painel ≤ 16 ms, 0 quadros perdidos abrindo e fechando.
- Git: commits pequenos, mensagens em inglês no estilo Conventional Commits, terminando com `Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>`.
- Documentação em pt-BR; identificadores de código em inglês; textos da interface em pt-BR.
- Antes de medir, fechar o YASB e reabrir no fim.

## Review Focus

Entradas e condições que a spec implica e que os testes "normais" não pegariam. Cada linha tem um teste ou passo de verificação na tarefa dona:

1. **Tema do usuário com erro de sintaxe ou cor inválida, salvo com a barra aberta** → a barra mantém o tema anterior e o log mostra arquivo, linha e motivo; nada pisca nem fecha (Task 6, teste `broken_user_theme_keeps_previous_and_reports_line`; Task 24, passo de verificação).
2. **`config.toml` editado à mão com comentários, depois alterado pelo menu** → os comentários e as chaves desconhecidas continuam no arquivo (Task 7, teste `set_value_keeps_comments_and_unknown_keys`).
3. **Abrir outro painel, ou trocar altura/formato/tema, com um painel ainda animando** → o painel antigo começa a fechar na hora e o novo abre sem esperar; nenhum painel fica "preso" aberto ou invisível (Task 16, testes `opening_another_chip_closes_current_first` e `config_change_closes_open_panel`; Task 24, passo 4, item 6).
4. **`zefiro-island.exe` ausente ou morrendo em laço** → a barra abre normalmente, tenta 3 vezes em 60 s, depois para, registra e deixa o centro vazio (Task 17, testes `restart_policy_*`).
5. **Monitor desligado/ligado, Win+P ou reinício do Explorer com a barra aberta** → as barras se recriam no lugar certo, a AppBar é registrada de novo e a ilha volta acima da barra (Task 24, passos de verificação).

---

## Mapa de arquivos

| Arquivo | Responsabilidade | Tarefa |
|---|---|---|
| `Cargo.toml`, `rust-toolchain.toml` | Workspace | 1 |
| `scripts/check-windows-imports.ps1`, `scripts/tests/check-windows-imports.Tests.ps1` | Regra de ouro | 1 |
| `.github/workflows/ci.yml` | CI (Windows: testes + clippy; Linux: check das portáveis) | 1 |
| `crates/core/src/{lib,observability,config,watch}.rs`, `crates/core/tests/*.rs` | Logs, crash, `config.toml`, observador de arquivos | 2, 7, 8 |
| `crates/platform/src/{lib,geometry,color,text}.rs` | Tipos primitivos e trait de medição de texto | 3 |
| `crates/resources/src/{lib,theme,tokens,registry}.rs`, `crates/resources/themes/*.toml` | Temas, tokens, registro | 4, 5, 6 |
| `crates/ui/src/{lib,spring,paint,bar_layout,panel_layout}.rs` | Molas, lista de desenho, layout da barra e do painel | 9, 10, 11 |
| `crates/integrations/clock/src/{lib,grid,format,windows}.rs` | Grade do mês, tempo até o próximo minuto, formatação do Windows | 12 |
| `crates/modules/bar/src/{lib,chips,menu,calendar,controller}.rs` | Conteúdo dos chips, menu do Zefiro, calendário, regras de painel | 13–16 |
| `crates/modules/island-host/src/{lib,ini,policy,process}.rs` | `island.ini`, política de reinício, processo | 17 |
| `island/dynamic-island-for-windows.wh.cpp`, `island/wh_shim.{h,cpp}`, `island/README.md` | Pílula recolhida da ilha com `PillHeight` | 18 |
| `crates/platform-windows/src/{lib,gfx,text,painter,compositor,morph,monitors,window,appbar,system,island_zorder}.rs` | Tudo que fala com o Windows | 19, 20, 21 |
| `crates/app/src/{main,app,theme_state,bar_view,panel_view,event_loop,island,robustness}.rs` | Binário `zefiro.exe` | 22, 23, 24 |
| `docs/adr/0007-barra-fase-1a.md`, `docs/PLANNING.md` | Números e decisões | 25 |

Grafo de dependências (sem ciclos): `platform` ← `resources` ← `ui`; `core` (independente); `ui` → `core` (tipos de altura/formato); `integration-clock` → `platform`; `bar` → `ui`, `resources`, `core`, `integration-clock`; `island-host` → `resources`, `core`; `platform-windows` → `platform`, `ui`, `resources`; `app` → todas.

---

## Task 1: Workspace, toolchain, CI e a regra de ouro

**Files:**
- Create: `Cargo.toml`, `rust-toolchain.toml`, `crates/{core,platform,resources,ui,platform-windows,app}/{Cargo.toml,src/lib.rs|src/main.rs}`, `crates/integrations/clock/{Cargo.toml,src/lib.rs}`, `crates/modules/{bar,island-host}/{Cargo.toml,src/lib.rs}`
- Create: `scripts/check-windows-imports.ps1`, `scripts/tests/check-windows-imports.Tests.ps1`, `.github/workflows/ci.yml`
- Modify: `.gitignore` (acrescentar `/target/` já existe; nada a mudar se já estiver)

**Interfaces:**
- Consumes: nada.
- Produces: workspace com 10 crates compilando (`cargo build --workspace`), lints do workspace, `scripts/check-windows-imports.ps1` (sai com código 1 se uma crate proibida depender de `windows`).

- [ ] **Step 1: Raiz do workspace**

`Cargo.toml`:

```toml
[workspace]
resolver = "3"
members = [
  "crates/core",
  "crates/platform",
  "crates/resources",
  "crates/ui",
  "crates/integrations/clock",
  "crates/modules/bar",
  "crates/modules/island-host",
  "crates/platform-windows",
  "crates/app",
]

[workspace.package]
version = "0.1.0"
edition = "2024"
license = "MIT OR Apache-2.0"
publish = false

[workspace.lints.rust]
unsafe_op_in_unsafe_fn = "deny"

[workspace.lints.clippy]
all = { level = "deny", priority = -1 }

[workspace.dependencies]
zefiro-core = { path = "crates/core" }
zefiro-platform = { path = "crates/platform" }
zefiro-resources = { path = "crates/resources" }
zefiro-ui = { path = "crates/ui" }
zefiro-integration-clock = { path = "crates/integrations/clock" }
zefiro-bar = { path = "crates/modules/bar" }
zefiro-island-host = { path = "crates/modules/island-host" }
zefiro-platform-windows = { path = "crates/platform-windows" }
serde = { version = "1", features = ["derive"] }
toml = "0.8"
toml_edit = "0.22"
notify = "8"
tracing = "0.1"
tracing-appender = "0.2"
tracing-subscriber = { version = "0.3", features = ["fmt", "env-filter"] }
tempfile = "3"
windows-core = "=0.61.2"
windows-numerics = "=0.2.0"
windows = { version = "=0.61.3", features = [
  "Foundation", "Foundation_Collections", "Foundation_Numerics", "Graphics", "Graphics_DirectX", "System", "UI",
  "UI_Composition", "UI_Composition_Desktop",
  "Win32_Foundation", "Win32_Globalization", "Win32_Graphics_Direct2D", "Win32_Graphics_Direct2D_Common",
  "Win32_Graphics_Direct3D", "Win32_Graphics_Direct3D11", "Win32_Graphics_DirectWrite", "Win32_Graphics_Dwm",
  "Win32_Graphics_Dxgi", "Win32_Graphics_Dxgi_Common", "Win32_Graphics_Gdi", "Win32_System_Com",
  "Win32_System_LibraryLoader", "Win32_System_Registry", "Win32_System_SystemInformation", "Win32_System_Threading",
  "Win32_System_WinRT", "Win32_System_WinRT_Composition", "Win32_UI_HiDpi", "Win32_UI_Input_KeyboardAndMouse",
  "Win32_UI_Shell", "Win32_UI_WindowsAndMessaging",
] }
```

`rust-toolchain.toml`:

```toml
[toolchain]
channel = "stable"
components = ["clippy", "rustfmt"]
targets = ["x86_64-unknown-linux-gnu"]
```

- [ ] **Step 2: Crates vazias**

Para cada biblioteca, `Cargo.toml` (troque `<nome>` e `<caminho>`):

```toml
[package]
name = "zefiro-<nome>"
version.workspace = true
edition.workspace = true
license.workspace = true
publish.workspace = true

[lints]
workspace = true
```

| Pasta | `name` | `src/lib.rs` |
|---|---|---|
| `crates/core` | `zefiro-core` | `//! Núcleo portável: logs, configuração e observação de arquivos.` |
| `crates/platform` | `zefiro-platform` | `//! Tipos primitivos (geometria, cor, texto) e contratos que cada sistema implementa.` |
| `crates/resources` | `zefiro-resources` | `//! Temas: arquivos, tokens derivados e registro.` |
| `crates/ui` | `zefiro-ui` | `//! Interface portável: molas, listas de desenho e layout.` |
| `crates/integrations/clock` | `zefiro-integration-clock` | `//! Relógio e calendário.` |
| `crates/modules/bar` | `zefiro-bar` | `//! Barra: chips, menu do Zefiro, calendário e regras dos painéis.` |
| `crates/modules/island-host` | `zefiro-island-host` | `//! Hospeda o zefiro-island.exe: island.ini, política de reinício e processo.` |
| `crates/platform-windows` | `zefiro-platform-windows` | `//! Implementação Windows: Direct2D (WARP), Windows.UI.Composition, janelas e AppBar.` |

`crates/app/Cargo.toml`:

```toml
[package]
name = "zefiro-app"
version.workspace = true
edition.workspace = true
license.workspace = true
publish.workspace = true

[[bin]]
name = "zefiro"
path = "src/main.rs"

[lints]
workspace = true
```

`crates/app/src/main.rs`:

```rust
#![windows_subsystem = "windows"]

fn main() {}
```

- [ ] **Step 3: Compilar**

Run: `cargo build --workspace`
Expected: compila sem avisos; cria `target/debug/zefiro.exe`.

- [ ] **Step 4: Teste do verificador da regra de ouro (falhando)**

`scripts/tests/check-windows-imports.Tests.ps1`:

```powershell
BeforeAll { $script = Join-Path $PSScriptRoot '..\check-windows-imports.ps1' }

Describe 'check-windows-imports' {
    It 'aceita o workspace atual' {
        & pwsh -NoProfile -File $script -Root (Resolve-Path (Join-Path $PSScriptRoot '..\..'))
        $LASTEXITCODE | Should -Be 0
    }
    It 'rejeita uma crate portável que depende de windows' {
        $tmp = Join-Path ([IO.Path]::GetTempPath()) ("zefiro-check-" + [guid]::NewGuid())
        New-Item -ItemType Directory -Force (Join-Path $tmp 'crates\ui') | Out-Null
        Set-Content (Join-Path $tmp 'crates\ui\Cargo.toml') "[package]`nname = `"zefiro-ui`"`n[dependencies]`nwindows.workspace = true`n"
        & pwsh -NoProfile -File $script -Root $tmp
        $LASTEXITCODE | Should -Be 1
        Remove-Item -Recurse -Force $tmp
    }
}
```

Run: `Invoke-Pester scripts/tests/check-windows-imports.Tests.ps1`
Expected: FAIL (script não existe).

- [ ] **Step 5: Implementar o verificador**

`scripts/check-windows-imports.ps1`:

```powershell
<#
.SYNOPSIS
  Regra de ouro do PLANNING §3.1: só estas crates podem depender da crate `windows`.
#>
param([string] $Root = (Resolve-Path (Join-Path $PSScriptRoot '..')))
$allowed = @('zefiro-platform-windows', 'zefiro-integration-clock', 'zefiro-app')
$bad = @()
Get-ChildItem -Path (Join-Path $Root 'crates') -Recurse -Filter Cargo.toml | ForEach-Object {
    $text = Get-Content $_.FullName -Raw
    $name = if ($text -match '(?m)^name\s*=\s*"([^"]+)"') { $Matches[1] } else { $_.Directory.Name }
    if ($text -match '(?m)^\s*windows(-core|-numerics)?\s*(=|\.)' -and $allowed -notcontains $name) { $bad += $name }
}
if ($bad) { Write-Host "Crates portáveis dependendo de windows: $($bad -join ', ')"; exit 1 }
Write-Host 'Regra de ouro ok.'
exit 0
```

Run: `Invoke-Pester scripts/tests/check-windows-imports.Tests.ps1`
Expected: PASS (2 testes).

- [ ] **Step 6: CI**

`.github/workflows/ci.yml`:

```yaml
name: ci
on: [push, pull_request]
jobs:
  windows:
    runs-on: windows-latest
    steps:
      - uses: actions/checkout@v4
      - run: pwsh -NoProfile -File scripts/check-windows-imports.ps1
      - run: cargo clippy --workspace --all-targets -- -D warnings
      - run: cargo test --workspace
  linux-portable:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - run: cargo check -p zefiro-core -p zefiro-platform -p zefiro-resources -p zefiro-ui -p zefiro-integration-clock -p zefiro-bar -p zefiro-island-host
```

- [ ] **Step 7: Commit**

```powershell
git add Cargo.toml rust-toolchain.toml crates scripts/check-windows-imports.ps1 scripts/tests/check-windows-imports.Tests.ps1 .github
git commit -m "chore: Rust workspace for phase 1a with the windows-import rule and CI" -m "Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

---

## Task 2: Logs e relatório de crash (`zefiro-core`)

**Files:**
- Create: `crates/core/src/observability.rs`, `crates/core/tests/logging.rs`, `crates/core/tests/panic_hook.rs`
- Modify: `crates/core/Cargo.toml`, `crates/core/src/lib.rs`

**Interfaces:**
- Consumes: Task 1.
- Produces: `zefiro_core::observability::{log_dir_from(local_app_data: Option<OsString>, xdg_state_home: Option<OsString>, home: Option<OsString>) -> PathBuf, default_log_dir() -> PathBuf, init_logging(dir: &Path, filter: &str) -> io::Result<LogGuard>, install_panic_hook(dir: PathBuf, version: &'static str), LogGuard}`.

- [ ] **Step 1: Dependências e testes (falhando)**

`crates/core/Cargo.toml`, acrescentar:

```toml
[dependencies]
serde.workspace = true
toml.workspace = true
toml_edit.workspace = true
notify.workspace = true
tracing.workspace = true
tracing-appender.workspace = true
tracing-subscriber.workspace = true

[dev-dependencies]
tempfile.workspace = true
```

`crates/core/src/lib.rs`:

```rust
//! Núcleo portável: logs, configuração e observação de arquivos.

pub mod observability;
```

`crates/core/src/observability.rs`:

```rust
//! Logs em arquivo com rotação diária e relatório de crash.

use std::ffi::OsString;
use std::path::PathBuf;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn windows_uses_local_app_data() {
        let p = log_dir_from(Some(OsString::from(r"C:\Users\x\AppData\Local")), None, None);
        assert_eq!(p, PathBuf::from(r"C:\Users\x\AppData\Local").join("zefiro").join("logs"));
    }

    #[test]
    fn linux_prefers_xdg_state_then_home() {
        assert_eq!(log_dir_from(None, Some("/s".into()), Some("/h".into())), PathBuf::from("/s/zefiro/logs"));
        assert_eq!(log_dir_from(None, None, Some("/h".into())), PathBuf::from("/h/.local/state/zefiro/logs"));
        assert_eq!(log_dir_from(None, None, None), PathBuf::from("./.local/state/zefiro/logs"));
    }
}
```

`crates/core/tests/logging.rs`:

```rust
use zefiro_core::observability::init_logging;

#[test]
fn writes_events_to_a_daily_file() {
    let dir = tempfile::tempdir().unwrap();
    let guard = init_logging(dir.path(), "info").unwrap();
    tracing::info!("evento de teste");
    drop(guard); // grava o que estava no buffer

    let mut content = String::new();
    for entry in std::fs::read_dir(dir.path()).unwrap() {
        let path = entry.unwrap().path();
        assert!(path.file_name().unwrap().to_string_lossy().starts_with("zefiro"), "{path:?}");
        content.push_str(&std::fs::read_to_string(path).unwrap());
    }
    assert!(content.contains("evento de teste"));
}
```

`crates/core/tests/panic_hook.rs`:

```rust
use zefiro_core::observability::install_panic_hook;

#[test]
fn panic_writes_a_crash_report() {
    let dir = tempfile::tempdir().unwrap();
    install_panic_hook(dir.path().to_path_buf(), "0.1.0-test");
    let r = std::panic::catch_unwind(|| panic!("falhou de propósito"));
    assert!(r.is_err());

    let reports: Vec<_> = std::fs::read_dir(dir.path()).unwrap().map(|e| e.unwrap().path()).collect();
    assert_eq!(reports.len(), 1, "{reports:?}");
    let name = reports[0].file_name().unwrap().to_string_lossy().to_string();
    assert!(name.starts_with("crash-") && name.ends_with(".txt"), "{name}");
    let body = std::fs::read_to_string(&reports[0]).unwrap();
    assert!(body.contains("0.1.0-test"));
    assert!(body.contains("falhou de propósito"));
    assert!(body.contains("backtrace"));
}
```

- [ ] **Step 2: Rodar para ver falhar**

Run: `cargo test -p zefiro-core`
Expected: FAIL de compilação (`log_dir_from`, `init_logging`, `install_panic_hook` não existem).

- [ ] **Step 3: Implementar** (acima do `#[cfg(test)]` em `observability.rs`)

```rust
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

/// Mantém o escritor de logs vivo; ao ser descartado, grava o que falta.
pub struct LogGuard {
    _guard: tracing_appender::non_blocking::WorkerGuard,
}

/// Pasta de logs a partir das variáveis de ambiente (função pura, testável).
pub fn log_dir_from(local_app_data: Option<OsString>, xdg_state_home: Option<OsString>, home: Option<OsString>) -> PathBuf {
    if let Some(local) = local_app_data {
        return PathBuf::from(local).join("zefiro").join("logs");
    }
    if let Some(state) = xdg_state_home {
        return PathBuf::from(state).join("zefiro").join("logs");
    }
    let home = home.map(PathBuf::from).unwrap_or_else(|| PathBuf::from("."));
    home.join(".local").join("state").join("zefiro").join("logs")
}

/// `%LOCALAPPDATA%\zefiro\logs` no Windows.
pub fn default_log_dir() -> PathBuf {
    log_dir_from(std::env::var_os("LOCALAPPDATA"), std::env::var_os("XDG_STATE_HOME"), std::env::var_os("HOME"))
}

/// Arquivo diário `zefiro.AAAA-MM-DD.log`, mantendo 7 arquivos.
pub fn init_logging(dir: &Path, filter: &str) -> std::io::Result<LogGuard> {
    std::fs::create_dir_all(dir)?;
    let appender = tracing_appender::rolling::Builder::new()
        .rotation(tracing_appender::rolling::Rotation::DAILY)
        .filename_prefix("zefiro")
        .filename_suffix("log")
        .max_log_files(7)
        .build(dir)
        .map_err(std::io::Error::other)?;
    let (writer, guard) = tracing_appender::non_blocking(appender);
    tracing_subscriber::fmt()
        .with_writer(writer)
        .with_ansi(false)
        .with_env_filter(tracing_subscriber::EnvFilter::new(filter))
        .try_init()
        .map_err(std::io::Error::other)?;
    Ok(LogGuard { _guard: guard })
}

/// Em panic, grava `crash-<segundos>-<ms>-<pid>.txt` com versão, mensagem e backtrace.
pub fn install_panic_hook(dir: PathBuf, version: &'static str) {
    let previous = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        let backtrace = std::backtrace::Backtrace::force_capture();
        let now = SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default();
        let name = format!("crash-{}-{:03}-{}.txt", now.as_secs(), now.subsec_millis(), std::process::id());
        let body = format!("zefiro {version}\n{info}\n\nbacktrace:\n{backtrace}\n");
        let _ = std::fs::create_dir_all(&dir);
        let _ = std::fs::write(dir.join(name), body);
        tracing::error!(panic = %info, "panic");
        previous(info);
    }));
}
```

- [ ] **Step 4: Rodar os testes**

Run: `cargo test -p zefiro-core`
Expected: PASS (2 de unidade + `logging` + `panic_hook`; a mensagem padrão do panic aparece no stderr, é esperado).

- [ ] **Step 5: Commit**

```powershell
git add crates/core Cargo.lock
git commit -m "feat(core): daily rolling logs and crash reports" -m "Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

---

## Task 3: Tipos primitivos (`zefiro-platform`)

**Files:**
- Create: `crates/platform/src/{geometry,color,text}.rs`
- Modify: `crates/platform/src/lib.rs`

**Interfaces:**
- Consumes: Task 1.
- Produces:
  - `geometry::{Point { x: f32, y: f32 }, Size { w: f32, h: f32 }, Rect { x: f32, y: f32, w: f32, h: f32 }}` com `Rect::{new, right, bottom, contains(Point) -> bool, union(Rect) -> Rect, inset(dx, dy) -> Rect, center_x() -> f32}`, todos `Copy + PartialEq + Debug + Default`.
  - `color::Rgba { r, g, b, a: f32 }` (0..1) com `Rgba::{rgb(u8,u8,u8), from_hex(&str) -> Option<Rgba>, with_alpha(f32) -> Rgba, mix(Rgba, f32) -> Rgba, relative_luminance() -> f64}` e `Rgba::TRANSPARENT`, `Rgba::WHITE`, `Rgba::BLACK`.
  - `text::{FontWeight { Regular, SemiBold, Bold }, Family { Text, Display }, Align { Leading, Center, Trailing }, TextStyle { size: f32, weight: FontWeight, family: Family }, trait TextMeasurer { fn measure(&self, text: &str, style: &TextStyle) -> Size; }}` (tamanhos em px).

- [ ] **Step 1: Testes (falhando)**

`crates/platform/src/lib.rs`:

```rust
//! Tipos primitivos (geometria, cor, texto) e contratos que cada sistema implementa.

pub mod color;
pub mod geometry;
pub mod text;
```

`crates/platform/src/color.rs` (só os testes por enquanto):

```rust
//! Cor RGBA em ponto flutuante, com o mesmo parser de hex da ilha.

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_3_4_6_and_8_digit_hex_with_or_without_hash() {
        assert_eq!(Rgba::from_hex("#08080A"), Some(Rgba::rgb(8, 8, 10)));
        assert_eq!(Rgba::from_hex("08080A"), Some(Rgba::rgb(8, 8, 10)));
        assert_eq!(Rgba::from_hex("#fff"), Some(Rgba::rgb(255, 255, 255)));
        let half = Rgba::from_hex("#FFFFFF80").unwrap();
        assert!((half.a - 128.0 / 255.0).abs() < 1e-6);
        let short_alpha = Rgba::from_hex("#0008").unwrap();
        assert!((short_alpha.a - 136.0 / 255.0).abs() < 1e-6);
    }

    #[test]
    fn rejects_garbage() {
        for s in ["", "#", "#12", "#12345", "#GGGGGG", "azul", "#1234567"] {
            assert_eq!(Rgba::from_hex(s), None, "{s}");
        }
    }

    #[test]
    fn luminance_matches_wcag() {
        assert!((Rgba::WHITE.relative_luminance() - 1.0).abs() < 1e-9);
        assert!(Rgba::BLACK.relative_luminance().abs() < 1e-9);
        assert!((Rgba::rgb(8, 8, 10).relative_luminance() - 0.0025).abs() < 1e-3);
    }

    #[test]
    fn mix_and_alpha() {
        let m = Rgba::BLACK.mix(Rgba::WHITE, 0.25);
        assert!((m.r - 0.25).abs() < 1e-6 && (m.a - 1.0).abs() < 1e-6);
        assert_eq!(Rgba::WHITE.with_alpha(0.5).a, 0.5);
        assert_eq!(Rgba::WHITE.with_alpha(3.0).a, 1.0);
    }
}
```

`crates/platform/src/geometry.rs` (só os testes):

```rust
//! Pontos, tamanhos e retângulos em pixels (f32).

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rect_helpers() {
        let r = Rect::new(10.0, 20.0, 100.0, 40.0);
        assert_eq!(r.right(), 110.0);
        assert_eq!(r.bottom(), 60.0);
        assert_eq!(r.center_x(), 60.0);
        assert!(r.contains(Point { x: 10.0, y: 20.0 }));
        assert!(!r.contains(Point { x: 110.0, y: 20.0 })); // borda direita é exclusiva
        assert_eq!(r.inset(5.0, 2.0), Rect::new(15.0, 22.0, 90.0, 36.0));
        assert_eq!(r.union(Rect::new(0.0, 0.0, 5.0, 5.0)), Rect::new(0.0, 0.0, 110.0, 60.0));
    }
}
```

`crates/platform/src/text.rs`:

```rust
//! Estilo de texto e medição (implementada pela plataforma com DirectWrite).

use crate::geometry::Size;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FontWeight { Regular, SemiBold, Bold }

/// `Text` = corpo (Segoe UI Variable Text); `Display` = números grandes (Segoe UI Variable Display).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Family { Text, Display }

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Align { Leading, Center, Trailing }

/// Tamanho em pixels (já multiplicado pela escala do monitor e pela escala de texto do tema).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TextStyle { pub size: f32, pub weight: FontWeight, pub family: Family }

pub trait TextMeasurer {
    /// Caixa do texto numa linha, sem quebra.
    fn measure(&self, text: &str, style: &TextStyle) -> Size;
}

/// Medidor aproximado para testes: 0,55 × tamanho por caractere, altura 1,3 × tamanho.
pub struct FixedWidthMeasurer;

impl TextMeasurer for FixedWidthMeasurer {
    fn measure(&self, text: &str, style: &TextStyle) -> Size {
        Size { w: text.chars().count() as f32 * style.size * 0.55, h: style.size * 1.3 }
    }
}
```

- [ ] **Step 2: Rodar para ver falhar**

Run: `cargo test -p zefiro-platform`
Expected: FAIL de compilação (`Rgba`, `Rect` não existem).

- [ ] **Step 3: Implementar**

`color.rs` (acima dos testes):

```rust
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Rgba { pub r: f32, pub g: f32, pub b: f32, pub a: f32 }

impl Rgba {
    pub const TRANSPARENT: Rgba = Rgba { r: 0.0, g: 0.0, b: 0.0, a: 0.0 };
    pub const WHITE: Rgba = Rgba { r: 1.0, g: 1.0, b: 1.0, a: 1.0 };
    pub const BLACK: Rgba = Rgba { r: 0.0, g: 0.0, b: 0.0, a: 1.0 };

    pub fn rgb(r: u8, g: u8, b: u8) -> Rgba {
        Rgba { r: r as f32 / 255.0, g: g as f32 / 255.0, b: b as f32 / 255.0, a: 1.0 }
    }

    /// Aceita 3, 4, 6 ou 8 dígitos (RGB, RGBA, RRGGBB, RRGGBBAA), com ou sem '#'.
    pub fn from_hex(s: &str) -> Option<Rgba> {
        let h = s.trim().trim_start_matches('#');
        if !h.chars().all(|c| c.is_ascii_hexdigit()) {
            return None;
        }
        let nib = |i: usize| u8::from_str_radix(&h[i..i + 1], 16).ok().map(|v| v * 17);
        let byte = |i: usize| u8::from_str_radix(&h[i..i + 2], 16).ok();
        let (r, g, b, a) = match h.len() {
            3 => (nib(0)?, nib(1)?, nib(2)?, 255),
            4 => (nib(0)?, nib(1)?, nib(2)?, nib(3)?),
            6 => (byte(0)?, byte(2)?, byte(4)?, 255),
            8 => (byte(0)?, byte(2)?, byte(4)?, byte(6)?),
            _ => return None,
        };
        Some(Rgba { a: a as f32 / 255.0, ..Rgba::rgb(r, g, b) })
    }

    pub fn with_alpha(self, a: f32) -> Rgba {
        Rgba { a: a.clamp(0.0, 1.0), ..self }
    }

    /// Interpolação linear de cada canal (t = 0 → self, t = 1 → other).
    pub fn mix(self, other: Rgba, t: f32) -> Rgba {
        let l = |a: f32, b: f32| a + (b - a) * t;
        Rgba { r: l(self.r, other.r), g: l(self.g, other.g), b: l(self.b, other.b), a: l(self.a, other.a) }
    }

    /// Luminância relativa (WCAG 2), igual à `RelativeLuminance` da ilha.
    pub fn relative_luminance(&self) -> f64 {
        let lin = |c: f32| {
            let v = c.clamp(0.0, 1.0) as f64;
            if v <= 0.04045 { v / 12.92 } else { ((v + 0.055) / 1.055).powf(2.4) }
        };
        0.2126 * lin(self.r) + 0.7152 * lin(self.g) + 0.0722 * lin(self.b)
    }
}
```

`geometry.rs` (acima dos testes):

```rust
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Point { pub x: f32, pub y: f32 }

#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Size { pub w: f32, pub h: f32 }

#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Rect { pub x: f32, pub y: f32, pub w: f32, pub h: f32 }

impl Rect {
    pub const fn new(x: f32, y: f32, w: f32, h: f32) -> Rect { Rect { x, y, w, h } }
    pub fn right(&self) -> f32 { self.x + self.w }
    pub fn bottom(&self) -> f32 { self.y + self.h }
    pub fn center_x(&self) -> f32 { self.x + self.w / 2.0 }
    /// Borda esquerda/superior inclusiva, direita/inferior exclusiva.
    pub fn contains(&self, p: Point) -> bool {
        p.x >= self.x && p.x < self.right() && p.y >= self.y && p.y < self.bottom()
    }
    pub fn inset(&self, dx: f32, dy: f32) -> Rect {
        Rect::new(self.x + dx, self.y + dy, (self.w - 2.0 * dx).max(0.0), (self.h - 2.0 * dy).max(0.0))
    }
    pub fn union(&self, o: Rect) -> Rect {
        let x = self.x.min(o.x);
        let y = self.y.min(o.y);
        Rect::new(x, y, self.right().max(o.right()) - x, self.bottom().max(o.bottom()) - y)
    }
}
```

- [ ] **Step 4: Rodar os testes**

Run: `cargo test -p zefiro-platform`
Expected: PASS (5 testes).

- [ ] **Step 5: Commit**

```powershell
git add crates/platform
git commit -m "feat(platform): geometry, RGBA color with the island's hex parser, text style and measurer trait" -m "Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

---

## Task 4: Arquivos de tema e leitura (`zefiro-resources`)

**Files:**
- Create: `crates/resources/src/theme.rs`, `crates/resources/themes/{obsidian,graphite,slate,nord,evergreen,espresso,plum,porcelain}.toml`
- Modify: `crates/resources/Cargo.toml`, `crates/resources/src/lib.rs`

**Interfaces:**
- Consumes: Task 3 (`Rgba`).
- Produces: `theme::{Theme, ThemeColors, Accent { Auto, System, Color(Rgba) }, Material, Backdrop { None, Acrylic }, ShapeSpec, ShapeStyle { Pill, W11 }, FontSpec, AnimationSpec, SpringStyle { Smooth, Default, Bouncy, Snappy }, ThemeError { source: String, line: Option<usize>, message: String }, parse_theme(id: &str, source_name: &str, text: &str) -> Result<Theme, ThemeError>, BUILTIN: [(&str, &str); 8]}`. `Theme` tem `id: String`, `name: String`, `colors`, `material`, `shape`, `font`, `animation`.

- [ ] **Step 1: Os 8 arquivos de tema**

`crates/resources/themes/obsidian.toml`:

```toml
# Obsidian — preto verdadeiro (padrão). Valores da ilha (Dynamic Island for Windows, MIT).
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

Os outros 7 são cópias do Obsidian trocando só a primeira linha de comentário, `name` e as 4 cores de `[colors]`:

| Arquivo | `name` | `background` | `text` | `text_secondary` | `border` |
|---|---|---|---|---|---|
| `graphite.toml` | `Graphite` | `#1C1C1E` | `#FFFFFF` | `#A8A8AE` | `#323236` |
| `slate.toml` | `Slate` | `#111721` | `#E9EEF6` | `#92A2B8` | `#232C3A` |
| `nord.toml` | `Nord` | `#2E3440` | `#ECEFF4` | `#A7B0C0` | `#3B4252` |
| `evergreen.toml` | `Evergreen` | `#0C1512` | `#E4F1EA` | `#8CAE9D` | `#1A2A23` |
| `espresso.toml` | `Espresso` | `#1A1512` | `#F6EEE7` | `#B7A395` | `#2E2420` |
| `plum.toml` | `Plum` | `#16111C` | `#F1EAF6` | `#AC9BB9` | `#281F33` |
| `porcelain.toml` | `Porcelain` | `#F5F6F8` | `#14161A` | `#5B6270` | `#D8DBE1` |

- [ ] **Step 2: Testes (falhando)**

`crates/resources/Cargo.toml`, acrescentar:

```toml
[dependencies]
zefiro-platform.workspace = true
serde.workspace = true
toml.workspace = true

[dev-dependencies]
tempfile.workspace = true
```

`crates/resources/src/lib.rs`:

```rust
//! Temas: arquivos, tokens derivados e registro.

pub mod theme;
```

`crates/resources/src/theme.rs` (só os testes):

```rust
//! Arquivo de tema (TOML) e seus tipos.

#[cfg(test)]
mod tests {
    use super::*;
    use zefiro_platform::color::Rgba;

    #[test]
    fn all_builtin_themes_parse() {
        for (id, text) in BUILTIN {
            let t = parse_theme(id, id, text).unwrap_or_else(|e| panic!("{id}: {e:?}"));
            assert_eq!(t.id, id);
            assert_eq!(t.material.opacity, 0.96);
            assert_eq!(t.shape.radius_max, 44.0);
            assert_eq!(t.shape.panel_radius, 24.0);
        }
        assert_eq!(BUILTIN[0].0, "obsidian");
    }

    #[test]
    fn obsidian_matches_the_island_palette() {
        let t = parse_theme("obsidian", "obsidian", BUILTIN[0].1).unwrap();
        assert_eq!(t.name, "Obsidian");
        assert_eq!(t.colors.background, Rgba::rgb(0x08, 0x08, 0x0A));
        assert_eq!(t.colors.text_secondary, Rgba::rgb(0x9B, 0x9B, 0xA5));
        assert_eq!(t.colors.border, Some(Rgba::rgb(0x1E, 0x1E, 0x22)));
        assert_eq!(t.colors.accent, Accent::Auto);
        assert_eq!(t.material.backdrop, Backdrop::None);
        assert_eq!(t.animation.spring, SpringStyle::Default);
    }

    #[test]
    fn missing_sections_take_defaults() {
        let t = parse_theme("mini", "mini.toml", "name = \"Mini\"\n[colors]\nbackground = \"#000\"\ntext = \"#fff\"\ntext_secondary = \"#888\"\n").unwrap();
        assert_eq!(t.colors.border, None);
        assert_eq!(t.material.opacity, 0.96);
        assert_eq!(t.shape.style, ShapeStyle::Pill);
        assert_eq!(t.font.family, "Segoe UI Variable");
        assert_eq!(t.animation.speed, 1.0);
    }

    #[test]
    fn invalid_color_reports_line_and_key() {
        let text = "name = \"X\"\n[colors]\nbackground = \"#000\"\ntext = \"azul\"\ntext_secondary = \"#888\"\n";
        let e = parse_theme("x", "x.toml", text).unwrap_err();
        assert_eq!(e.source, "x.toml");
        assert_eq!(e.line, Some(4));
        assert!(e.message.contains("text"), "{}", e.message);
    }

    #[test]
    fn syntax_error_reports_line() {
        let e = parse_theme("x", "x.toml", "name = \"X\"\n[colors\n").unwrap_err();
        assert_eq!(e.line, Some(2));
    }

    #[test]
    fn values_are_clamped_to_sane_ranges() {
        let text = "name = \"X\"\n[colors]\nbackground = \"#000\"\ntext = \"#fff\"\ntext_secondary = \"#888\"\n[material]\nopacity = 9\n[animation]\nspeed = 0.1\n[font]\nscale = 5\n";
        let t = parse_theme("x", "x.toml", text).unwrap();
        assert_eq!(t.material.opacity, 1.0);
        assert_eq!(t.animation.speed, 0.5);
        assert_eq!(t.font.scale, 1.6);
    }
}
```

- [ ] **Step 3: Rodar para ver falhar**

Run: `cargo test -p zefiro-resources`
Expected: FAIL de compilação.

- [ ] **Step 4: Implementar** (acima dos testes em `theme.rs`)

```rust
use serde::Deserialize;
use zefiro_platform::color::Rgba;

pub const BUILTIN: [(&str, &str); 8] = [
    ("obsidian", include_str!("../themes/obsidian.toml")),
    ("graphite", include_str!("../themes/graphite.toml")),
    ("slate", include_str!("../themes/slate.toml")),
    ("nord", include_str!("../themes/nord.toml")),
    ("evergreen", include_str!("../themes/evergreen.toml")),
    ("espresso", include_str!("../themes/espresso.toml")),
    ("plum", include_str!("../themes/plum.toml")),
    ("porcelain", include_str!("../themes/porcelain.toml")),
];

#[derive(Debug, Clone, PartialEq)]
pub struct Theme {
    pub id: String,
    pub name: String,
    pub colors: ThemeColors,
    pub material: Material,
    pub shape: ShapeSpec,
    pub font: FontSpec,
    pub animation: AnimationSpec,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ThemeColors {
    pub background: Rgba,
    pub text: Rgba,
    pub text_secondary: Rgba,
    pub border: Option<Rgba>,
    pub accent: Accent,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Accent { Auto, System, Color(Rgba) }

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Backdrop { None, Acrylic }

#[derive(Debug, Clone, PartialEq)]
pub struct Material { pub opacity: f32, pub depth: bool, pub shadow: bool, pub backdrop: Backdrop, pub backdrop_fill: f32 }

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ShapeStyle { Pill, W11 }

#[derive(Debug, Clone, PartialEq)]
pub struct ShapeSpec { pub style: ShapeStyle, pub radius_max: f32, pub panel_radius: f32 }

#[derive(Debug, Clone, PartialEq)]
pub struct FontSpec { pub family: String, pub scale: f32 }

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SpringStyle { Smooth, Default, Bouncy, Snappy }

#[derive(Debug, Clone, PartialEq)]
pub struct AnimationSpec { pub spring: SpringStyle, pub speed: f32 }

#[derive(Debug, Clone, PartialEq)]
pub struct ThemeError { pub source: String, pub line: Option<usize>, pub message: String }

// ── Formato do arquivo (serde) ──────────────────────────────────────────────

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct File {
    name: String,
    colors: FileColors,
    #[serde(default)]
    material: FileMaterial,
    #[serde(default)]
    shape: FileShape,
    #[serde(default)]
    font: FileFont,
    #[serde(default)]
    animation: FileAnimation,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct FileColors {
    background: toml::Spanned<String>,
    text: toml::Spanned<String>,
    text_secondary: toml::Spanned<String>,
    #[serde(default)]
    border: Option<toml::Spanned<String>>,
    #[serde(default)]
    accent: Option<toml::Spanned<String>>,
}

#[derive(Deserialize)]
#[serde(default, deny_unknown_fields)]
struct FileMaterial { opacity: f32, depth: bool, shadow: bool, backdrop: String, backdrop_fill: f32 }
impl Default for FileMaterial {
    fn default() -> Self { Self { opacity: 0.96, depth: true, shadow: true, backdrop: "none".into(), backdrop_fill: 0.45 } }
}

#[derive(Deserialize)]
#[serde(default, deny_unknown_fields)]
struct FileShape { style: String, radius_max: f32, panel_radius: f32 }
impl Default for FileShape {
    fn default() -> Self { Self { style: "pill".into(), radius_max: 44.0, panel_radius: 24.0 } }
}

#[derive(Deserialize)]
#[serde(default, deny_unknown_fields)]
struct FileFont { family: String, scale: f32 }
impl Default for FileFont {
    fn default() -> Self { Self { family: "Segoe UI Variable".into(), scale: 1.0 } }
}

#[derive(Deserialize)]
#[serde(default, deny_unknown_fields)]
struct FileAnimation { spring: String, speed: f32 }
impl Default for FileAnimation {
    fn default() -> Self { Self { spring: "default".into(), speed: 1.0 } }
}

fn line_of(text: &str, offset: usize) -> usize {
    text[..offset.min(text.len())].matches('\n').count() + 1
}

/// Lê um tema. `source_name` aparece nas mensagens de erro (nome do arquivo).
pub fn parse_theme(id: &str, source_name: &str, text: &str) -> Result<Theme, ThemeError> {
    let err = |line: Option<usize>, message: String| ThemeError { source: source_name.to_string(), line, message };
    let file: File = toml::from_str(text)
        .map_err(|e| err(e.span().map(|s| line_of(text, s.start)), e.message().to_string()))?;

    let color = |key: &str, v: &toml::Spanned<String>| {
        Rgba::from_hex(v.get_ref())
            .ok_or_else(|| err(Some(line_of(text, v.span().start)), format!("cor inválida em `{key}`: {:?}", v.get_ref())))
    };
    let border = match &file.colors.border {
        Some(v) if v.get_ref().eq_ignore_ascii_case("none") => None,
        Some(v) => Some(color("border", v)?),
        None => None,
    };
    let accent = match &file.colors.accent {
        None => Accent::Auto,
        Some(v) if v.get_ref().eq_ignore_ascii_case("auto") => Accent::Auto,
        Some(v) if v.get_ref().eq_ignore_ascii_case("system") => Accent::System,
        Some(v) => Accent::Color(color("accent", v)?),
    };
    let backdrop = match file.material.backdrop.as_str() {
        "none" => Backdrop::None,
        "acrylic" => Backdrop::Acrylic,
        other => return Err(err(None, format!("backdrop desconhecido: {other:?} (use none ou acrylic)"))),
    };
    let style = match file.shape.style.as_str() {
        "pill" => ShapeStyle::Pill,
        "w11" => ShapeStyle::W11,
        other => return Err(err(None, format!("shape.style desconhecido: {other:?} (use pill ou w11)"))),
    };
    let spring = match file.animation.spring.as_str() {
        "smooth" => SpringStyle::Smooth,
        "default" => SpringStyle::Default,
        "bouncy" => SpringStyle::Bouncy,
        "snappy" => SpringStyle::Snappy,
        other => return Err(err(None, format!("animation.spring desconhecido: {other:?}"))),
    };

    Ok(Theme {
        id: id.to_string(),
        name: file.name,
        colors: ThemeColors {
            background: color("background", &file.colors.background)?,
            text: color("text", &file.colors.text)?,
            text_secondary: color("text_secondary", &file.colors.text_secondary)?,
            border,
            accent,
        },
        material: Material {
            opacity: file.material.opacity.clamp(0.35, 1.0),
            depth: file.material.depth,
            shadow: file.material.shadow,
            backdrop,
            backdrop_fill: file.material.backdrop_fill.clamp(0.0, 1.0),
        },
        shape: ShapeSpec {
            style,
            radius_max: file.shape.radius_max.clamp(0.0, 64.0),
            panel_radius: file.shape.panel_radius.clamp(0.0, 48.0),
        },
        font: FontSpec { family: file.font.family, scale: file.font.scale.clamp(0.7, 1.6) },
        animation: AnimationSpec { spring, speed: file.animation.speed.clamp(0.5, 2.0) },
    })
}
```

- [ ] **Step 5: Rodar os testes**

Run: `cargo test -p zefiro-resources`
Expected: PASS (6 testes).

- [ ] **Step 6: Commit**

```powershell
git add crates/resources Cargo.lock
git commit -m "feat(resources): theme file format and the island's 8 themes" -m "Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

---

## Task 5: Tokens derivados e contraste (`zefiro-resources`)

**Files:**
- Create: `crates/resources/src/tokens.rs`
- Modify: `crates/resources/src/lib.rs` (`pub mod tokens;`), `THIRD_PARTY_NOTICES.md` (acrescentar "regras de tokens e contraste" na linha do Dynamic Island)

**Interfaces:**
- Consumes: Task 4 (`Theme`, `Accent`), Task 3 (`Rgba`).
- Produces: `tokens::{Tokens, derive_tokens(theme: &Theme, system_accent: Rgba, cover_accent: Option<Rgba>) -> Tokens, ensure_contrast(candidate: Rgba, background: Rgba) -> Rgba, contrast_ratio(a: Rgba, b: Rgba) -> f64}`. `Tokens` tem os campos `dark: bool, fill: Rgba, raised: Rgba, raised_strong: Rgba, hairline: Rgba, border: Option<Rgba>, shadow: Rgba, text: Rgba, text_secondary: Rgba, text_tertiary: Rgba, accent: Rgba, accent_soft: Rgba` (todos `Rgba`, alfa já aplicado; `fill` = fundo com `material.opacity`).

- [ ] **Step 1: Testes (falhando)**

`crates/resources/src/tokens.rs` (só os testes):

```rust
//! Tokens derivados do tema, com as regras da ilha (`BuildMaterialTokens`, `EnsureContrastAgainstBackground`).

#[cfg(test)]
mod tests {
    use super::*;
    use crate::theme::{BUILTIN, parse_theme};

    fn theme(id: &str) -> crate::theme::Theme {
        let (_, text) = BUILTIN.iter().find(|(i, _)| *i == id).unwrap();
        parse_theme(id, id, text).unwrap()
    }

    #[test]
    fn dark_theme_uses_white_lifts_with_island_alphas() {
        let t = derive_tokens(&theme("obsidian"), Rgba::rgb(0, 120, 215), None);
        assert!(t.dark);
        assert_eq!(t.raised, Rgba::WHITE.with_alpha(0.085));
        assert_eq!(t.raised_strong, Rgba::WHITE.with_alpha(0.150));
        assert_eq!(t.hairline, Rgba::WHITE.with_alpha(0.100));
        assert_eq!(t.shadow, Rgba::BLACK.with_alpha(0.55));
        assert!((t.text.a - 0.98).abs() < 1e-6);
        assert!((t.text_secondary.a - 0.88).abs() < 1e-6);
        assert!((t.fill.a - 0.96).abs() < 1e-6);
    }

    #[test]
    fn light_theme_uses_black_lifts() {
        let t = derive_tokens(&theme("porcelain"), Rgba::rgb(0, 120, 215), None);
        assert!(!t.dark);
        assert_eq!(t.raised, Rgba::BLACK.with_alpha(0.062));
        assert_eq!(t.raised_strong, Rgba::BLACK.with_alpha(0.105));
        assert_eq!(t.hairline, Rgba::BLACK.with_alpha(0.085));
    }

    #[test]
    fn tertiary_is_secondary_mixed_35_percent_into_background() {
        let th = theme("obsidian");
        let t = derive_tokens(&th, Rgba::BLACK, None);
        let expected = th.colors.text_secondary.mix(th.colors.background, 0.35).with_alpha(0.72);
        assert_eq!(t.text_tertiary, expected);
    }

    #[test]
    fn auto_accent_prefers_cover_then_system() {
        let th = theme("obsidian");
        let cover = Rgba::rgb(230, 80, 60);
        assert_eq!(derive_tokens(&th, Rgba::rgb(0, 120, 215), Some(cover)).accent, ensure_contrast(cover, th.colors.background));
        assert_eq!(
            derive_tokens(&th, Rgba::rgb(0, 120, 215), None).accent,
            ensure_contrast(Rgba::rgb(0, 120, 215), th.colors.background)
        );
    }

    #[test]
    fn every_accent_reaches_3_to_1_on_every_builtin_theme() {
        let accents = [Rgba::rgb(0, 0, 0), Rgba::rgb(255, 255, 255), Rgba::rgb(76, 201, 240), Rgba::rgb(20, 20, 60), Rgba::rgb(240, 240, 150)];
        for (id, _) in BUILTIN {
            let bg = theme(id).colors.background;
            for a in accents {
                let c = ensure_contrast(a, bg);
                assert!(contrast_ratio(c, bg) >= 3.0 - 1e-6 || is_at_limit(c, bg), "{id} {a:?} -> {c:?}");
            }
        }
    }

    // O laço da ilha para no limite de luminosidade; nesse caso o contraste máximo possível é aceito.
    fn is_at_limit(c: Rgba, bg: Rgba) -> bool {
        let (_, _, l) = rgb_to_hsl(c);
        if bg.relative_luminance() < 0.45 { l >= 0.95 - 0.021 } else { l <= 0.06 + 0.021 }
    }

    #[test]
    fn border_none_stays_none() {
        let mut th = theme("obsidian");
        th.colors.border = None;
        assert_eq!(derive_tokens(&th, Rgba::BLACK, None).border, None);
    }
}
```

- [ ] **Step 2: Rodar para ver falhar**

Run: `cargo test -p zefiro-resources tokens`
Expected: FAIL de compilação.

- [ ] **Step 3: Implementar** (acima dos testes)

```rust
use crate::theme::{Accent, Theme};
use zefiro_platform::color::Rgba;

#[derive(Debug, Clone, PartialEq)]
pub struct Tokens {
    pub dark: bool,
    pub fill: Rgba,
    pub raised: Rgba,
    pub raised_strong: Rgba,
    pub hairline: Rgba,
    pub border: Option<Rgba>,
    pub shadow: Rgba,
    pub text: Rgba,
    pub text_secondary: Rgba,
    pub text_tertiary: Rgba,
    pub accent: Rgba,
    pub accent_soft: Rgba,
}

pub fn derive_tokens(theme: &Theme, system_accent: Rgba, cover_accent: Option<Rgba>) -> Tokens {
    let bg = theme.colors.background;
    let dark = bg.relative_luminance() < 0.45;
    let lift = if dark { Rgba::WHITE } else { Rgba::BLACK };
    let raw_accent = match theme.colors.accent {
        Accent::Auto => cover_accent.unwrap_or(system_accent),
        Accent::System => system_accent,
        Accent::Color(c) => c,
    };
    let accent = ensure_contrast(raw_accent, bg);
    Tokens {
        dark,
        fill: bg.with_alpha(bg.a * theme.material.opacity),
        raised: lift.with_alpha(if dark { 0.085 } else { 0.062 }),
        raised_strong: lift.with_alpha(if dark { 0.150 } else { 0.105 }),
        hairline: lift.with_alpha(if dark { 0.100 } else { 0.085 }),
        border: theme.colors.border,
        shadow: Rgba::BLACK.with_alpha(0.55),
        text: theme.colors.text.with_alpha(0.98),
        text_secondary: theme.colors.text_secondary.with_alpha(0.88),
        text_tertiary: theme.colors.text_secondary.mix(bg, 0.35).with_alpha(0.72),
        accent,
        accent_soft: accent.with_alpha(0.18),
    }
}

pub fn contrast_ratio(a: Rgba, b: Rgba) -> f64 {
    let (la, lb) = (a.relative_luminance(), b.relative_luminance());
    (la.max(lb) + 0.05) / (la.min(lb) + 0.05)
}

/// Porte de `EnsureContrastAgainstBackground`: ajusta a luminosidade (HSL) em passos de 0,02
/// até 3:1, clareando em fundo escuro e escurecendo em fundo claro.
pub fn ensure_contrast(candidate: Rgba, background: Rgba) -> Rgba {
    let (h, mut s, mut l) = rgb_to_hsl(candidate);
    if s > 0.01 {
        s = s.max(0.35);
    }
    let on_dark = background.relative_luminance() < 0.45;
    l = if on_dark { l.min(0.85) } else { l.max(0.15) };
    let (step, limit) = if on_dark { (0.02, 0.95) } else { (-0.02, 0.06) };
    let mut c = hsl_to_rgb(h, s, l, candidate.a);
    while contrast_ratio(c, background) < 3.0 && (if on_dark { l < limit } else { l > limit }) {
        l += step;
        c = hsl_to_rgb(h, s, l, candidate.a);
    }
    c
}

pub(crate) fn rgb_to_hsl(c: Rgba) -> (f32, f32, f32) {
    let (r, g, b) = (c.r, c.g, c.b);
    let max = r.max(g).max(b);
    let min = r.min(g).min(b);
    let l = (max + min) / 2.0;
    if (max - min).abs() < f32::EPSILON {
        return (0.0, 0.0, l);
    }
    let d = max - min;
    let s = if l > 0.5 { d / (2.0 - max - min) } else { d / (max + min) };
    let mut h = if max == r {
        (g - b) / d + if g < b { 6.0 } else { 0.0 }
    } else if max == g {
        (b - r) / d + 2.0
    } else {
        (r - g) / d + 4.0
    };
    h *= 60.0;
    (h, s, l)
}

pub(crate) fn hsl_to_rgb(h: f32, s: f32, l: f32, a: f32) -> Rgba {
    let l = l.clamp(0.0, 1.0);
    if s <= 0.0 {
        return Rgba { r: l, g: l, b: l, a };
    }
    let q = if l < 0.5 { l * (1.0 + s) } else { l + s - l * s };
    let p = 2.0 * l - q;
    let hue = |mut t: f32| {
        if t < 0.0 { t += 1.0 }
        if t > 1.0 { t -= 1.0 }
        if t < 1.0 / 6.0 { p + (q - p) * 6.0 * t }
        else if t < 0.5 { q }
        else if t < 2.0 / 3.0 { p + (q - p) * (2.0 / 3.0 - t) * 6.0 }
        else { p }
    };
    let hn = h / 360.0;
    Rgba { r: hue(hn + 1.0 / 3.0), g: hue(hn), b: hue(hn - 1.0 / 3.0), a }
}
```

Em `lib.rs`: `pub mod tokens;`. Em `THIRD_PARTY_NOTICES.md`, na coluna "O que foi portado" da linha do Dynamic Island, acrescentar: "Regras de tokens de material e de contraste do destaque, portadas para Rust em `crates/resources/src/tokens.rs`."

- [ ] **Step 4: Rodar os testes**

Run: `cargo test -p zefiro-resources`
Expected: PASS (12 testes).

- [ ] **Step 5: Commit**

```powershell
git add crates/resources THIRD_PARTY_NOTICES.md
git commit -m "feat(resources): derived material tokens and accent contrast ported from the island" -m "Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

---

## Task 6: Registro de temas (embutidos + pasta do usuário)

**Files:**
- Create: `crates/resources/src/registry.rs`
- Modify: `crates/resources/src/lib.rs` (`pub mod registry;`)

**Interfaces:**
- Consumes: Task 4 (`parse_theme`, `BUILTIN`, `Theme`, `ThemeError`).
- Produces: `registry::{ThemeRegistry, ThemeEntry { theme: Theme, user: bool }}` com `ThemeRegistry::load(user_dir: Option<&Path>) -> (ThemeRegistry, Vec<ThemeError>)`, `get(&self, id: &str) -> Option<&Theme>`, `list(&self) -> &[ThemeEntry]` (embutidos na ordem do `BUILTIN`, depois os do usuário em ordem alfabética), `reload_keeping_previous(&self, user_dir: Option<&Path>) -> (ThemeRegistry, Vec<ThemeError>)` (um tema do usuário com erro mantém a versão anterior dele, se havia), `export_builtin(id: &str, dir: &Path) -> io::Result<PathBuf>` (copia o arquivo embutido para a pasta do usuário, sem sobrescrever).

- [ ] **Step 1: Testes (falhando)**

`crates/resources/src/registry.rs` (só os testes):

```rust
//! Temas embutidos + temas do usuário (`%APPDATA%\zefiro\themes\*.toml`).

#[cfg(test)]
mod tests {
    use super::*;

    const MINI: &str = "name = \"Meu\"\n[colors]\nbackground = \"#101010\"\ntext = \"#fff\"\ntext_secondary = \"#888\"\n";

    #[test]
    fn builtins_only_without_user_dir() {
        let (r, errors) = ThemeRegistry::load(None);
        assert!(errors.is_empty());
        assert_eq!(r.list().len(), 8);
        assert_eq!(r.list()[0].theme.id, "obsidian");
        assert!(r.get("porcelain").is_some());
        assert!(r.get("nao-existe").is_none());
    }

    #[test]
    fn user_theme_is_added_and_can_override_builtin() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("meu.toml"), MINI).unwrap();
        std::fs::write(dir.path().join("obsidian.toml"), MINI.replace("Meu", "Obsidian do usuário")).unwrap();
        std::fs::write(dir.path().join("leia-me.txt"), "ignorado").unwrap();
        let (r, errors) = ThemeRegistry::load(Some(dir.path()));
        assert!(errors.is_empty(), "{errors:?}");
        assert_eq!(r.list().len(), 9);
        assert_eq!(r.get("obsidian").unwrap().name, "Obsidian do usuário");
        assert!(r.list().iter().any(|e| e.user && e.theme.id == "meu"));
    }

    #[test]
    fn broken_user_theme_keeps_previous_and_reports_line() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("meu.toml");
        std::fs::write(&path, MINI).unwrap();
        let (first, _) = ThemeRegistry::load(Some(dir.path()));
        std::fs::write(&path, MINI.replace("#101010", "preto")).unwrap();
        let (second, errors) = first.reload_keeping_previous(Some(dir.path()));
        assert_eq!(errors.len(), 1);
        assert_eq!(errors[0].line, Some(3));
        assert!(errors[0].source.ends_with("meu.toml"), "{}", errors[0].source);
        assert_eq!(second.get("meu").unwrap().colors.background, first.get("meu").unwrap().colors.background);
    }

    #[test]
    fn export_copies_builtin_without_overwriting() {
        let dir = tempfile::tempdir().unwrap();
        let p = export_builtin("nord", dir.path()).unwrap();
        assert!(std::fs::read_to_string(&p).unwrap().contains("name = \"Nord\""));
        std::fs::write(&p, "editado").unwrap();
        export_builtin("nord", dir.path()).unwrap();
        assert_eq!(std::fs::read_to_string(&p).unwrap(), "editado");
        assert!(export_builtin("nao-existe", dir.path()).is_err());
    }
}
```

- [ ] **Step 2: Rodar para ver falhar**

Run: `cargo test -p zefiro-resources registry`
Expected: FAIL de compilação.

- [ ] **Step 3: Implementar** (acima dos testes)

```rust
use std::path::{Path, PathBuf};

use crate::theme::{BUILTIN, Theme, ThemeError, parse_theme};

#[derive(Debug, Clone, PartialEq)]
pub struct ThemeEntry { pub theme: Theme, pub user: bool }

#[derive(Debug, Clone, Default)]
pub struct ThemeRegistry { entries: Vec<ThemeEntry> }

fn builtin_entries() -> Vec<ThemeEntry> {
    BUILTIN
        .iter()
        .map(|(id, text)| ThemeEntry { theme: parse_theme(id, id, text).expect("tema embutido válido"), user: false })
        .collect()
}

fn read_user_dir(dir: &Path) -> Vec<(String, String, Result<Theme, ThemeError>)> {
    let mut files: Vec<PathBuf> = std::fs::read_dir(dir)
        .map(|rd| rd.filter_map(|e| e.ok().map(|e| e.path())).collect())
        .unwrap_or_default();
    files.retain(|p| p.extension().is_some_and(|e| e.eq_ignore_ascii_case("toml")));
    files.sort();
    files
        .into_iter()
        .map(|p| {
            let id = p.file_stem().unwrap_or_default().to_string_lossy().to_lowercase();
            let source = p.display().to_string();
            let parsed = std::fs::read_to_string(&p)
                .map_err(|e| ThemeError { source: source.clone(), line: None, message: e.to_string() })
                .and_then(|text| parse_theme(&id, &source, &text));
            (id, source, parsed)
        })
        .collect()
}

impl ThemeRegistry {
    pub fn load(user_dir: Option<&Path>) -> (ThemeRegistry, Vec<ThemeError>) {
        ThemeRegistry::default().reload_keeping_previous(user_dir)
    }

    /// Relê a pasta do usuário. Um arquivo com erro mantém a versão anterior daquele tema, se havia.
    pub fn reload_keeping_previous(&self, user_dir: Option<&Path>) -> (ThemeRegistry, Vec<ThemeError>) {
        let mut entries = builtin_entries();
        let mut errors = Vec::new();
        let mut user: Vec<ThemeEntry> = Vec::new();
        if let Some(dir) = user_dir {
            for (id, _source, parsed) in read_user_dir(dir) {
                match parsed {
                    Ok(theme) => user.push(ThemeEntry { theme, user: true }),
                    Err(e) => {
                        errors.push(e);
                        if let Some(prev) = self.entries.iter().find(|x| x.user && x.theme.id == id) {
                            user.push(prev.clone());
                        }
                    }
                }
            }
        }
        for u in user {
            if let Some(slot) = entries.iter_mut().find(|e| e.theme.id == u.theme.id) {
                *slot = u;
            } else {
                entries.push(u);
            }
        }
        let builtin_count = BUILTIN.len();
        entries[builtin_count..].sort_by(|a, b| a.theme.id.cmp(&b.theme.id));
        (ThemeRegistry { entries }, errors)
    }

    pub fn get(&self, id: &str) -> Option<&Theme> {
        self.entries.iter().find(|e| e.theme.id == id).map(|e| &e.theme)
    }

    pub fn list(&self) -> &[ThemeEntry] {
        &self.entries
    }
}

/// Copia o arquivo de um tema embutido para `dir` (cria a pasta). Não sobrescreve.
pub fn export_builtin(id: &str, dir: &Path) -> std::io::Result<PathBuf> {
    let (_, text) = BUILTIN
        .iter()
        .find(|(i, _)| *i == id)
        .ok_or_else(|| std::io::Error::new(std::io::ErrorKind::NotFound, format!("tema embutido {id} não existe")))?;
    std::fs::create_dir_all(dir)?;
    let path = dir.join(format!("{id}.toml"));
    if !path.exists() {
        std::fs::write(&path, text)?;
    }
    Ok(path)
}
```

- [ ] **Step 4: Rodar os testes**

Run: `cargo test -p zefiro-resources`
Expected: PASS (16 testes).

- [ ] **Step 5: Commit**

```powershell
git add crates/resources
git commit -m "feat(resources): theme registry with user overrides and error-tolerant reload" -m "Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

---

## Task 7: `config.toml` (`zefiro-core`)

**Files:**
- Create: `crates/core/src/config.rs`, `crates/core/tests/config.rs`
- Modify: `crates/core/src/lib.rs` (`pub mod config;`)

**Interfaces:**
- Consumes: Task 2 (crate e dependências).
- Produces: `config::{Config { theme: String, bar: BarConfig, island: IslandConfig, clock: ClockConfig }, BarConfig { height: BarHeight, format: BarFormat, sections: Sections }, Sections { start: Vec<String>, end: Vec<String> }, BarHeight { Completa, Compacta }, BarFormat { Faixa, Pilulas }, IslandConfig { enabled: bool, exe: String }, ClockConfig { format: String, seconds: bool }, ConfigError { line: Option<usize>, message: String }, DEFAULT_CONFIG: &str, parse_config(text: &str) -> Result<Config, ConfigError>, load_or_create(path: &Path) -> Result<Config, ConfigError>, set_value(path: &Path, keys: &[&str], value: &str) -> io::Result<()>, default_config_dir() -> PathBuf}`. `BarHeight`/`BarFormat` são `Copy + Eq + Debug` e têm `as_str()` (`"completa"`, `"compacta"`, `"faixa"`, `"pilulas"`).

- [ ] **Step 1: Testes (falhando)**

`crates/core/tests/config.rs`:

```rust
use zefiro_core::config::*;

#[test]
fn default_config_parses_to_the_spec_defaults() {
    let c = parse_config(DEFAULT_CONFIG).unwrap();
    assert_eq!(c.theme, "obsidian");
    assert_eq!(c.bar.height, BarHeight::Completa);
    assert_eq!(c.bar.format, BarFormat::Faixa);
    assert_eq!(c.bar.sections.start, vec!["zefiro".to_string()]);
    assert_eq!(c.bar.sections.end, vec!["clock".to_string()]);
    assert!(c.island.enabled);
    assert_eq!(c.island.exe, "");
    assert_eq!(c.clock.format, "");
    assert!(!c.clock.seconds);
}

#[test]
fn missing_keys_take_defaults() {
    let c = parse_config("[bar]\nformat = \"pilulas\"\n").unwrap();
    assert_eq!(c.theme, "obsidian");
    assert_eq!(c.bar.height, BarHeight::Completa);
    assert_eq!(c.bar.format, BarFormat::Pilulas);
}

#[test]
fn invalid_value_reports_line() {
    let e = parse_config("theme = \"nord\"\n[bar]\nheight = \"enorme\"\n").unwrap_err();
    assert_eq!(e.line, Some(3));
}

#[test]
fn load_or_create_writes_default_file_once() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("zefiro").join("config.toml");
    let c = load_or_create(&path).unwrap();
    assert_eq!(c.theme, "obsidian");
    assert_eq!(std::fs::read_to_string(&path).unwrap(), DEFAULT_CONFIG);
    std::fs::write(&path, "theme = \"plum\"\n").unwrap();
    assert_eq!(load_or_create(&path).unwrap().theme, "plum");
}

#[test]
fn set_value_keeps_comments_and_unknown_keys() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("config.toml");
    std::fs::write(&path, "# meu comentário\ntheme = \"obsidian\" # tema\n\n[bar]\nheight = \"completa\"\nextra = 1 # chave do futuro\n").unwrap();
    set_value(&path, &["bar", "height"], "compacta").unwrap();
    set_value(&path, &["theme"], "nord").unwrap();
    set_value(&path, &["bar", "format"], "pilulas").unwrap();
    let text = std::fs::read_to_string(&path).unwrap();
    assert!(text.contains("# meu comentário"));
    assert!(text.contains("extra = 1 # chave do futuro"));
    assert!(text.contains("theme = \"nord\" # tema"), "{text}");
    let c = parse_config(&text).unwrap();
    assert_eq!(c.bar.height, BarHeight::Compacta);
    assert_eq!(c.bar.format, BarFormat::Pilulas);
}
```

- [ ] **Step 2: Rodar para ver falhar**

Run: `cargo test -p zefiro-core --test config`
Expected: FAIL de compilação.

- [ ] **Step 3: Implementar**

`crates/core/src/config.rs`:

```rust
//! `%APPDATA%\zefiro\config.toml`: leitura com padrões, criação e edição sem perder comentários.

use serde::Deserialize;
use std::path::{Path, PathBuf};

pub const DEFAULT_CONFIG: &str = r#"# Zefiro — configuração da barra. Salvar aplica na hora.
theme = "obsidian"

[bar]
height = "completa"   # completa | compacta
format = "faixa"      # faixa | pilulas

[bar.sections]
start = ["zefiro"]
end   = ["clock"]
# o centro é a ilha (monitor principal)

[island]
enabled = true
exe     = ""          # vazio = ao lado do zefiro.exe

[clock]
format  = ""          # vazio = formato do Windows
seconds = false
"#;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum BarHeight { Completa, Compacta }

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum BarFormat { Faixa, Pilulas }

impl BarHeight {
    pub fn as_str(self) -> &'static str {
        match self { BarHeight::Completa => "completa", BarHeight::Compacta => "compacta" }
    }
}

impl BarFormat {
    pub fn as_str(self) -> &'static str {
        match self { BarFormat::Faixa => "faixa", BarFormat::Pilulas => "pilulas" }
    }
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(default)]
pub struct Config { pub theme: String, pub bar: BarConfig, pub island: IslandConfig, pub clock: ClockConfig }

#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(default)]
pub struct BarConfig { pub height: BarHeight, pub format: BarFormat, pub sections: Sections }

#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(default)]
pub struct Sections { pub start: Vec<String>, pub end: Vec<String> }

#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(default)]
pub struct IslandConfig { pub enabled: bool, pub exe: String }

#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(default)]
pub struct ClockConfig { pub format: String, pub seconds: bool }

impl Default for Config {
    fn default() -> Self {
        Config { theme: "obsidian".into(), bar: BarConfig::default(), island: IslandConfig::default(), clock: ClockConfig::default() }
    }
}
impl Default for BarConfig {
    fn default() -> Self { BarConfig { height: BarHeight::Completa, format: BarFormat::Faixa, sections: Sections::default() } }
}
impl Default for Sections {
    fn default() -> Self { Sections { start: vec!["zefiro".into()], end: vec!["clock".into()] } }
}
impl Default for IslandConfig {
    fn default() -> Self { IslandConfig { enabled: true, exe: String::new() } }
}
impl Default for ClockConfig {
    fn default() -> Self { ClockConfig { format: String::new(), seconds: false } }
}

#[derive(Debug, Clone, PartialEq)]
pub struct ConfigError { pub line: Option<usize>, pub message: String }

pub fn parse_config(text: &str) -> Result<Config, ConfigError> {
    toml::from_str(text).map_err(|e| ConfigError {
        line: e.span().map(|s| text[..s.start.min(text.len())].matches('\n').count() + 1),
        message: e.message().to_string(),
    })
}

/// Lê o arquivo; se não existir, cria com `DEFAULT_CONFIG`.
pub fn load_or_create(path: &Path) -> Result<Config, ConfigError> {
    let io = |e: std::io::Error| ConfigError { line: None, message: format!("{}: {e}", path.display()) };
    if !path.exists() {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(io)?;
        }
        std::fs::write(path, DEFAULT_CONFIG).map_err(io)?;
    }
    parse_config(&std::fs::read_to_string(path).map_err(io)?)
}

/// Grava `value` (string) em `keys` (ex.: `["bar", "height"]`), mantendo comentários e chaves desconhecidas.
pub fn set_value(path: &Path, keys: &[&str], value: &str) -> std::io::Result<()> {
    let text = std::fs::read_to_string(path).unwrap_or_default();
    let mut doc: toml_edit::DocumentMut = text.parse().map_err(std::io::Error::other)?;
    let (last, tables) = keys.split_last().expect("ao menos uma chave");
    let mut table = doc.as_table_mut();
    for k in tables {
        let entry = table.entry(k).or_insert(toml_edit::table());
        table = entry.as_table_mut().ok_or_else(|| std::io::Error::other(format!("`{k}` não é uma tabela")))?;
    }
    match table.get_mut(last).and_then(|i| i.as_value_mut()) {
        Some(existing) => {
            let decor = existing.decor().clone();
            *existing = toml_edit::Value::from(value);
            *existing.decor_mut() = decor;
        }
        None => {
            table.insert(last, toml_edit::value(value));
        }
    }
    let tmp = path.with_extension("toml.tmp");
    std::fs::write(&tmp, doc.to_string())?;
    std::fs::rename(&tmp, path)
}

/// `%APPDATA%\zefiro` (ou `$XDG_CONFIG_HOME/zefiro`, ou `~/.config/zefiro`).
pub fn default_config_dir() -> PathBuf {
    if let Some(appdata) = std::env::var_os("APPDATA") {
        return PathBuf::from(appdata).join("zefiro");
    }
    if let Some(xdg) = std::env::var_os("XDG_CONFIG_HOME") {
        return PathBuf::from(xdg).join("zefiro");
    }
    PathBuf::from(std::env::var_os("HOME").unwrap_or_else(|| ".".into())).join(".config").join("zefiro")
}
```

Em `lib.rs`: `pub mod config;`.

- [ ] **Step 4: Rodar os testes**

Run: `cargo test -p zefiro-core`
Expected: PASS (inclui os 5 de `config`).

- [ ] **Step 5: Commit**

```powershell
git add crates/core
git commit -m "feat(core): config.toml with defaults, creation and comment-preserving edits" -m "Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

---

## Task 8: Observador de arquivos com espera (`zefiro-core`)

**Files:**
- Create: `crates/core/src/watch.rs`, `crates/core/tests/watch.rs`
- Modify: `crates/core/src/lib.rs` (`pub mod watch;`)

**Interfaces:**
- Consumes: Task 2 (dependência `notify`).
- Produces: `watch::{FileWatcher, watch_paths(dirs: &[PathBuf], debounce: Duration, on_change: impl Fn(Vec<PathBuf>) + Send + 'static) -> notify::Result<FileWatcher>}`. Observa as **pastas** (não recursivo), junta os eventos que chegam dentro de `debounce` e chama `on_change` uma vez com os caminhos alterados (sem repetição). Parar = descartar o `FileWatcher`.

- [ ] **Step 1: Teste (falhando)**

`crates/core/tests/watch.rs`:

```rust
use std::sync::mpsc;
use std::time::Duration;
use zefiro_core::watch::watch_paths;

#[test]
fn several_quick_writes_become_one_notification() {
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("config.toml");
    std::fs::write(&file, "a").unwrap();
    let (tx, rx) = mpsc::channel();
    let _w = watch_paths(&[dir.path().to_path_buf()], Duration::from_millis(150), move |paths| {
        tx.send(paths).unwrap();
    })
    .unwrap();
    std::thread::sleep(Duration::from_millis(100));
    for i in 0..5 {
        std::fs::write(&file, format!("b{i}")).unwrap();
        std::thread::sleep(Duration::from_millis(20));
    }
    let first = rx.recv_timeout(Duration::from_secs(3)).expect("nenhuma notificação");
    assert!(first.iter().any(|p| p.ends_with("config.toml")), "{first:?}");
    assert!(rx.recv_timeout(Duration::from_millis(400)).is_err(), "notificou mais de uma vez");
}
```

- [ ] **Step 2: Rodar para ver falhar**

Run: `cargo test -p zefiro-core --test watch`
Expected: FAIL de compilação.

- [ ] **Step 3: Implementar**

`crates/core/src/watch.rs`:

```rust
//! Observa pastas e avisa uma vez por rajada de gravações (editores gravam em mais de um passo).

use notify::{RecommendedWatcher, RecursiveMode, Watcher};
use std::path::PathBuf;
use std::sync::mpsc;
use std::time::Duration;

pub struct FileWatcher {
    _watcher: RecommendedWatcher,
}

pub fn watch_paths(
    dirs: &[PathBuf],
    debounce: Duration,
    on_change: impl Fn(Vec<PathBuf>) + Send + 'static,
) -> notify::Result<FileWatcher> {
    let (tx, rx) = mpsc::channel::<PathBuf>();
    let mut watcher = notify::recommended_watcher(move |res: notify::Result<notify::Event>| {
        if let Ok(event) = res {
            if event.kind.is_create() || event.kind.is_modify() || event.kind.is_remove() {
                for p in event.paths {
                    let _ = tx.send(p);
                }
            }
        }
    })?;
    for d in dirs {
        std::fs::create_dir_all(d).ok();
        watcher.watch(d, RecursiveMode::NonRecursive)?;
    }
    std::thread::Builder::new()
        .name("zefiro-watch".into())
        .spawn(move || {
            while let Ok(first) = rx.recv() {
                let mut batch = vec![first];
                while let Ok(p) = rx.recv_timeout(debounce) {
                    batch.push(p);
                }
                batch.sort();
                batch.dedup();
                on_change(batch);
            }
        })
        .map_err(|e| notify::Error::generic(&e.to_string()))?;
    Ok(FileWatcher { _watcher: watcher })
}
```

(Quando o `FileWatcher` é descartado, o `notify` larga o `tx`; o `recv` da thread falha e ela termina.)

Em `lib.rs`: `pub mod watch;`.

- [ ] **Step 4: Rodar os testes**

Run: `cargo test -p zefiro-core`
Expected: PASS.

- [ ] **Step 5: Commit**

```powershell
git add crates/core Cargo.lock
git commit -m "feat(core): debounced folder watcher" -m "Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

---

## Task 9: Molas (`zefiro-ui`)

**Files:**
- Create: `crates/ui/src/spring.rs`
- Modify: `crates/ui/Cargo.toml`, `crates/ui/src/lib.rs`, `THIRD_PARTY_NOTICES.md` (mencionar `crates/ui/src/spring.rs`)

**Interfaces:**
- Consumes: Task 4 (`zefiro_resources::theme::SpringStyle`).
- Produces: `spring::{SpringParams { stiffness: f32, damping: f32 }, CompositorSpring { period_s: f32, damping_ratio: f32 }, Spring, NUDGE_DIP: f32 = -6.0}` com `SpringParams::{GROW, SHRINK, SETTLE, styled(SpringStyle) -> Self, with_speed(f32) -> Self, compositor() -> CompositorSpring, toward(from: f32, to: f32) -> Self}` (`toward` devolve GROW se `to > from`, SHRINK se `to < from`, SETTLE se iguais) e `Spring::{new(f32), step(dt, SpringParams), is_settled()}` (integrador da ilha, usado nos testes e onde não houver compositor).

- [ ] **Step 1: Dependências e testes (falhando)**

`crates/ui/Cargo.toml`:

```toml
[dependencies]
zefiro-platform.workspace = true
zefiro-resources.workspace = true
zefiro-core.workspace = true
```

`crates/ui/src/lib.rs`:

```rust
//! Interface portável: molas, listas de desenho e layout.

pub mod spring;
```

`crates/ui/src/spring.rs` (só os testes):

```rust
//! Molas do Dynamic Island for Windows (MIT): constantes, estilos, velocidade e conversão para o compositor.

#[cfg(test)]
mod tests {
    use super::*;
    use zefiro_resources::theme::SpringStyle;

    fn settle_time(p: SpringParams) -> f32 {
        let mut s = Spring::new(0.0);
        s.target = 100.0;
        let mut t = 0.0;
        while !s.is_settled() && t < 10.0 {
            s.step(1.0 / 240.0, p);
            t += 1.0 / 240.0;
        }
        t
    }

    #[test]
    fn constants_match_the_island() {
        assert_eq!(SpringParams::GROW, SpringParams { stiffness: 380.0, damping: 26.0 });
        assert_eq!(SpringParams::SHRINK, SpringParams { stiffness: 200.0, damping: 28.0 });
        assert_eq!(SpringParams::SETTLE, SpringParams { stiffness: 280.0, damping: 24.0 });
        assert_eq!(NUDGE_DIP, -6.0);
    }

    #[test]
    fn styles_multiply_like_the_island() {
        let p = SpringParams::SETTLE.styled(SpringStyle::Snappy);
        assert!((p.stiffness - 420.0).abs() < 1e-3 && (p.damping - 30.0).abs() < 1e-3);
        let p = SpringParams::SETTLE.styled(SpringStyle::Bouncy);
        assert!((p.stiffness - 308.0).abs() < 1e-3 && (p.damping - 16.8).abs() < 1e-3);
        assert_eq!(SpringParams::SETTLE.styled(SpringStyle::Default), SpringParams::SETTLE);
    }

    #[test]
    fn double_speed_halves_the_settle_time() {
        let normal = settle_time(SpringParams::GROW);
        let fast = settle_time(SpringParams::GROW.with_speed(2.0));
        assert!((fast / normal - 0.5).abs() < 0.05, "normal={normal} fast={fast}");
    }

    #[test]
    fn compositor_conversion() {
        let c = SpringParams::SETTLE.compositor();
        assert!((c.period_s - 0.37549).abs() < 1e-3, "{c:?}");
        assert!((c.damping_ratio - 0.71714).abs() < 1e-3, "{c:?}");
    }

    #[test]
    fn toward_picks_grow_or_shrink() {
        assert_eq!(SpringParams::toward(10.0, 20.0), SpringParams::GROW);
        assert_eq!(SpringParams::toward(20.0, 10.0), SpringParams::SHRINK);
        assert_eq!(SpringParams::toward(10.0, 10.0), SpringParams::SETTLE);
    }
}
```

- [ ] **Step 2: Rodar para ver falhar**

Run: `cargo test -p zefiro-ui`
Expected: FAIL de compilação.

- [ ] **Step 3: Implementar** (acima dos testes)

```rust
use zefiro_resources::theme::SpringStyle;

/// "Cutucada" vertical ao abrir: começa em −6 DIP e volta a 0 com a mola SETTLE.
pub const NUDGE_DIP: f32 = -6.0;
const FIXED_DT: f32 = 0.0005;
const SETTLE_EPS: f32 = 0.01;
const MAX_DT: f32 = 0.050;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SpringParams { pub stiffness: f32, pub damping: f32 }

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CompositorSpring { pub period_s: f32, pub damping_ratio: f32 }

impl SpringParams {
    pub const GROW: Self = Self { stiffness: 380.0, damping: 26.0 };
    pub const SHRINK: Self = Self { stiffness: 200.0, damping: 28.0 };
    pub const SETTLE: Self = Self { stiffness: 280.0, damping: 24.0 };

    pub fn toward(from: f32, to: f32) -> Self {
        if to > from { Self::GROW } else if to < from { Self::SHRINK } else { Self::SETTLE }
    }

    pub fn styled(self, style: SpringStyle) -> Self {
        let (s, d) = match style {
            SpringStyle::Smooth => (1.0, 1.35),
            SpringStyle::Default => (1.0, 1.0),
            SpringStyle::Bouncy => (1.1, 0.70),
            SpringStyle::Snappy => (1.5, 1.25),
        };
        Self { stiffness: self.stiffness * s, damping: self.damping * d }
    }

    /// Velocidade v: equivale a multiplicar o tempo por v (a ilha multiplica o `dt`).
    pub fn with_speed(self, v: f32) -> Self {
        Self { stiffness: self.stiffness * v * v, damping: self.damping * v }
    }

    /// Modelo do compositor: período (2π/ω) e razão de amortecimento (c / 2ω), com ω = √k.
    pub fn compositor(self) -> CompositorSpring {
        let omega = self.stiffness.sqrt();
        CompositorSpring { period_s: 2.0 * std::f32::consts::PI / omega, damping_ratio: self.damping / (2.0 * omega) }
    }
}

/// Integrador da ilha: Euler semi-implícito em passos fixos de 0,5 ms.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Spring { pub value: f32, pub velocity: f32, pub target: f32 }

impl Spring {
    pub fn new(value: f32) -> Self { Self { value, velocity: 0.0, target: value } }

    pub fn is_settled(&self) -> bool {
        (self.value - self.target).abs() < SETTLE_EPS && self.velocity.abs() < SETTLE_EPS
    }

    pub fn step(&mut self, dt: f32, p: SpringParams) {
        let mut remaining = dt.clamp(0.0, MAX_DT);
        while remaining > 0.0 {
            let h = remaining.min(FIXED_DT);
            let accel = -p.stiffness * (self.value - self.target) - p.damping * self.velocity;
            self.velocity += accel * h;
            self.value += self.velocity * h;
            remaining -= h;
        }
        if self.is_settled() {
            self.value = self.target;
            self.velocity = 0.0;
        }
    }
}
```

Em `THIRD_PARTY_NOTICES.md`, acrescentar à linha do Dynamic Island: "Constantes e integrador das molas em `crates/ui/src/spring.rs`."

- [ ] **Step 4: Rodar os testes**

Run: `cargo test -p zefiro-ui`
Expected: PASS (5 testes).

- [ ] **Step 5: Commit**

```powershell
git add crates/ui THIRD_PARTY_NOTICES.md Cargo.lock
git commit -m "feat(ui): island springs with styles, speed and compositor conversion" -m "Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

---

## Task 10: Lista de desenho e layout da barra (`zefiro-ui`)

**Files:**
- Create: `crates/ui/src/paint.rs`, `crates/ui/src/bar_layout.rs`
- Modify: `crates/ui/src/lib.rs` (`pub mod paint; pub mod bar_layout;`)

**Interfaces:**
- Consumes: Task 3 (`Rect`, `Size`, `Point`, `Rgba`, `TextStyle`, `Align`), Task 7 (`BarHeight`, `BarFormat`).
- Produces:
  - `paint::{DrawOp, DisplayList}`: `DrawOp::{RoundRect { rect: Rect, radius: f32, color: Rgba }, RoundRectStroke { rect: Rect, radius: f32, color: Rgba, width: f32 }, Circle { center: Point, radius: f32, color: Rgba }, Text { rect: Rect, text: String, style: TextStyle, color: Rgba, align: Align }}`; `DisplayList { size: Size, ops: Vec<DrawOp> }` com `DisplayList::new(Size)`, `push(DrawOp)`, `translated(dx, dy) -> DisplayList`.
  - `bar_layout::{BarMetrics, metrics(BarHeight, BarFormat) -> BarMetrics, ChipSpec { id: String, content_w: f32 }, ChipBox { id: String, rect: Rect }, BarLayout, layout_bar(monitor_w: f32, scale: f32, height: BarHeight, format: BarFormat, start: &[ChipSpec], end: &[ChipSpec]) -> BarLayout, chip_at(&BarLayout, Point) -> Option<&ChipBox>}`. `BarMetrics` (DIP): `band, pill, chip_h, island_pill, group_pad, edge, chip_pad_x, chip_gap, font`. `BarLayout` (px, origem no canto superior esquerdo do monitor): `scale: f32, band: Rect, strip: Option<Rect>, groups: Vec<Rect>, chips: Vec<ChipBox>, island_pill_h: f32, font_px: f32, chip_h_px: f32`.

Valores (DIP), da spec §5.1:

| | band | pill | chip_h | island_pill | group_pad | font |
|---|---|---|---|---|---|---|
| Faixa completa | 44 | 32 | 32 | 44 | 0 | 13 |
| Faixa compacta | 32 | 24 | 24 | 28 | 0 | 12 |
| Pílulas completa | 44 | 36 | 30 | 36 | 3 | 13 |
| Pílulas compacta | 32 | 24 | 20 | 24 | 2 | 12 |

Comuns: `edge = 8`, `chip_pad_x = 10`, `chip_gap = 4`. Todos os retângulos em px são arredondados para inteiros.

- [ ] **Step 1: Testes (falhando)**

`crates/ui/src/paint.rs`:

```rust
//! Lista de desenho portável. A plataforma a desenha com Direct2D.

use zefiro_platform::color::Rgba;
use zefiro_platform::geometry::{Point, Rect, Size};
use zefiro_platform::text::{Align, TextStyle};

#[derive(Debug, Clone, PartialEq)]
pub enum DrawOp {
    RoundRect { rect: Rect, radius: f32, color: Rgba },
    RoundRectStroke { rect: Rect, radius: f32, color: Rgba, width: f32 },
    Circle { center: Point, radius: f32, color: Rgba },
    Text { rect: Rect, text: String, style: TextStyle, color: Rgba, align: Align },
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct DisplayList { pub size: Size, pub ops: Vec<DrawOp> }

impl DisplayList {
    pub fn new(size: Size) -> Self { Self { size, ops: Vec::new() } }
    pub fn push(&mut self, op: DrawOp) { self.ops.push(op) }

    /// Cópia com todas as operações deslocadas.
    pub fn translated(&self, dx: f32, dy: f32) -> DisplayList {
        let mv = |r: Rect| Rect::new(r.x + dx, r.y + dy, r.w, r.h);
        let ops = self
            .ops
            .iter()
            .map(|op| match op.clone() {
                DrawOp::RoundRect { rect, radius, color } => DrawOp::RoundRect { rect: mv(rect), radius, color },
                DrawOp::RoundRectStroke { rect, radius, color, width } => DrawOp::RoundRectStroke { rect: mv(rect), radius, color, width },
                DrawOp::Circle { center, radius, color } => DrawOp::Circle { center: Point { x: center.x + dx, y: center.y + dy }, radius, color },
                DrawOp::Text { rect, text, style, color, align } => DrawOp::Text { rect: mv(rect), text, style, color, align },
            })
            .collect();
        DisplayList { size: self.size, ops }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn translated_moves_every_op() {
        let mut l = DisplayList::new(Size { w: 10.0, h: 10.0 });
        l.push(DrawOp::Circle { center: Point { x: 1.0, y: 2.0 }, radius: 1.0, color: Rgba::WHITE });
        l.push(DrawOp::RoundRect { rect: Rect::new(0.0, 0.0, 5.0, 5.0), radius: 2.0, color: Rgba::WHITE });
        let t = l.translated(10.0, 20.0);
        assert_eq!(t.ops[0], DrawOp::Circle { center: Point { x: 11.0, y: 22.0 }, radius: 1.0, color: Rgba::WHITE });
        assert_eq!(t.ops[1], DrawOp::RoundRect { rect: Rect::new(10.0, 20.0, 5.0, 5.0), radius: 2.0, color: Rgba::WHITE });
    }
}
```

`crates/ui/src/bar_layout.rs` (só os testes):

```rust
//! Layout da barra nas 2 alturas × 2 formatos (spec §5.1).

#[cfg(test)]
mod tests {
    use super::*;
    use zefiro_core::config::{BarFormat, BarHeight};

    fn chips(ids: &[(&str, f32)]) -> Vec<ChipSpec> {
        ids.iter().map(|(id, w)| ChipSpec { id: id.to_string(), content_w: *w }).collect()
    }

    #[test]
    fn metrics_table_matches_the_spec() {
        let m = metrics(BarHeight::Completa, BarFormat::Faixa);
        assert_eq!((m.band, m.chip_h, m.island_pill, m.font), (44.0, 32.0, 44.0, 13.0));
        let m = metrics(BarHeight::Compacta, BarFormat::Faixa);
        assert_eq!((m.band, m.chip_h, m.island_pill, m.font), (32.0, 24.0, 28.0, 12.0));
        let m = metrics(BarHeight::Completa, BarFormat::Pilulas);
        assert_eq!((m.band, m.pill, m.chip_h, m.island_pill, m.group_pad), (44.0, 36.0, 30.0, 36.0, 3.0));
        let m = metrics(BarHeight::Compacta, BarFormat::Pilulas);
        assert_eq!((m.band, m.pill, m.chip_h, m.island_pill, m.group_pad), (32.0, 24.0, 20.0, 24.0, 2.0));
    }

    #[test]
    fn strip_full_height_has_strip_and_no_groups() {
        let l = layout_bar(1920.0, 1.0, BarHeight::Completa, BarFormat::Faixa, &chips(&[("zefiro", 60.0)]), &chips(&[("clock", 80.0)]));
        assert_eq!(l.band, Rect::new(0.0, 0.0, 1920.0, 44.0));
        assert_eq!(l.strip, Some(l.band));
        assert!(l.groups.is_empty());
        assert_eq!(l.chips[0].rect, Rect::new(8.0, 6.0, 80.0, 32.0)); // 60 + 2×10
        assert_eq!(l.chips[1].rect, Rect::new(1920.0 - 8.0 - 100.0, 6.0, 100.0, 32.0));
        assert_eq!(l.island_pill_h, 44.0);
    }

    #[test]
    fn pills_wrap_each_section_in_a_group() {
        let l = layout_bar(1920.0, 1.0, BarHeight::Completa, BarFormat::Pilulas, &chips(&[("zefiro", 60.0)]), &chips(&[("a", 20.0), ("b", 30.0)]));
        assert_eq!(l.strip, None);
        assert_eq!(l.groups.len(), 2);
        assert_eq!(l.groups[0], Rect::new(8.0, 4.0, 86.0, 36.0)); // 80 + 2×3
        assert_eq!(l.chips[0].rect, Rect::new(11.0, 7.0, 80.0, 30.0));
        // fim: 40 + 4 + 50 + 2×3 = 100, encostado a 8 da direita
        assert_eq!(l.groups[1], Rect::new(1812.0, 4.0, 100.0, 36.0));
        assert_eq!(l.chips[1].rect, Rect::new(1815.0, 7.0, 40.0, 30.0));
        assert_eq!(l.chips[2].rect, Rect::new(1859.0, 7.0, 50.0, 30.0));
    }

    #[test]
    fn empty_section_has_no_group() {
        let l = layout_bar(1920.0, 1.0, BarHeight::Compacta, BarFormat::Pilulas, &[], &chips(&[("clock", 80.0)]));
        assert_eq!(l.groups.len(), 1);
        assert_eq!(l.chips.len(), 1);
    }

    #[test]
    fn scales_and_rounds_at_125_and_150_percent() {
        for scale in [1.25f32, 1.5] {
            let l = layout_bar(2400.0, scale, BarHeight::Compacta, BarFormat::Faixa, &chips(&[("zefiro", 31.0)]), &[]);
            assert_eq!(l.band.h, (32.0 * scale).round());
            let r = l.chips[0].rect;
            for v in [r.x, r.y, r.w, r.h] {
                assert_eq!(v, v.round(), "{scale}: {r:?}");
            }
            assert_eq!(l.font_px, 12.0 * scale);
            assert!(r.bottom() <= l.band.bottom() && r.y >= 0.0);
        }
    }

    #[test]
    fn hit_test_finds_chip() {
        let l = layout_bar(1920.0, 1.0, BarHeight::Completa, BarFormat::Faixa, &chips(&[("zefiro", 60.0)]), &[]);
        assert_eq!(chip_at(&l, Point { x: 10.0, y: 20.0 }).map(|c| c.id.as_str()), Some("zefiro"));
        assert!(chip_at(&l, Point { x: 500.0, y: 20.0 }).is_none());
    }
}
```

- [ ] **Step 2: Rodar para ver falhar**

Run: `cargo test -p zefiro-ui`
Expected: FAIL de compilação (`metrics`, `layout_bar`…).

- [ ] **Step 3: Implementar** (acima dos testes em `bar_layout.rs`)

```rust
use zefiro_core::config::{BarFormat, BarHeight};
use zefiro_platform::geometry::{Point, Rect};

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BarMetrics {
    pub band: f32,
    pub pill: f32,
    pub chip_h: f32,
    pub island_pill: f32,
    pub group_pad: f32,
    pub edge: f32,
    pub chip_pad_x: f32,
    pub chip_gap: f32,
    pub font: f32,
}

pub fn metrics(height: BarHeight, format: BarFormat) -> BarMetrics {
    let (band, pill, chip_h, island_pill, group_pad, font) = match (height, format) {
        (BarHeight::Completa, BarFormat::Faixa) => (44.0, 32.0, 32.0, 44.0, 0.0, 13.0),
        (BarHeight::Compacta, BarFormat::Faixa) => (32.0, 24.0, 24.0, 28.0, 0.0, 12.0),
        (BarHeight::Completa, BarFormat::Pilulas) => (44.0, 36.0, 30.0, 36.0, 3.0, 13.0),
        (BarHeight::Compacta, BarFormat::Pilulas) => (32.0, 24.0, 20.0, 24.0, 2.0, 12.0),
    };
    BarMetrics { band, pill, chip_h, island_pill, group_pad, edge: 8.0, chip_pad_x: 10.0, chip_gap: 4.0, font }
}

#[derive(Debug, Clone, PartialEq)]
pub struct ChipSpec { pub id: String, pub content_w: f32 }

#[derive(Debug, Clone, PartialEq)]
pub struct ChipBox { pub id: String, pub rect: Rect }

#[derive(Debug, Clone, PartialEq)]
pub struct BarLayout {
    pub scale: f32,
    pub band: Rect,
    pub strip: Option<Rect>,
    pub groups: Vec<Rect>,
    pub chips: Vec<ChipBox>,
    pub island_pill_h: f32,
    pub font_px: f32,
    pub chip_h_px: f32,
}

fn rounded(r: Rect) -> Rect {
    Rect::new(r.x.round(), r.y.round(), r.w.round(), r.h.round())
}

/// `content_w` de cada chip já está em px. Início alinhado à esquerda, fim à direita.
pub fn layout_bar(monitor_w: f32, scale: f32, height: BarHeight, format: BarFormat, start: &[ChipSpec], end: &[ChipSpec]) -> BarLayout {
    let m = metrics(height, format);
    let px = |dip: f32| (dip * scale).round();
    let band = Rect::new(0.0, 0.0, monitor_w, px(m.band));
    let chip_h = px(m.chip_h);
    let pad = px(m.group_pad);
    let gap = px(m.chip_gap);
    let chip_w = |c: &ChipSpec| (c.content_w + 2.0 * px(m.chip_pad_x)).round();
    let section_w = |cs: &[ChipSpec]| cs.iter().map(chip_w).sum::<f32>() + gap * cs.len().saturating_sub(1) as f32;

    let mut groups = Vec::new();
    let mut chips = Vec::new();
    let pills = format == BarFormat::Pilulas;
    let group_h = px(m.pill);
    let group_y = ((band.h - group_h) / 2.0).round();
    let chip_y = if pills { group_y + pad } else { ((band.h - chip_h) / 2.0).round() };

    let mut place = |cs: &[ChipSpec], left: f32| {
        if cs.is_empty() {
            return;
        }
        if pills {
            groups.push(rounded(Rect::new(left, group_y, section_w(cs) + 2.0 * pad, group_h)));
        }
        let mut x = left + if pills { pad } else { 0.0 };
        for c in cs {
            let w = chip_w(c);
            chips.push(ChipBox { id: c.id.clone(), rect: rounded(Rect::new(x, chip_y, w, chip_h)) });
            x += w + gap;
        }
    };
    let edge = px(m.edge);
    place(start, edge);
    let end_w = section_w(end) + if pills { 2.0 * pad } else { 0.0 };
    place(end, monitor_w - edge - end_w);

    BarLayout {
        scale,
        band,
        strip: if pills { None } else { Some(band) },
        groups,
        chips,
        island_pill_h: px(m.island_pill),
        font_px: m.font * scale,
        chip_h_px: chip_h,
    }
}

pub fn chip_at(layout: &BarLayout, p: Point) -> Option<&ChipBox> {
    layout.chips.iter().find(|c| c.rect.contains(p))
}
```

Em `lib.rs`: `pub mod paint; pub mod bar_layout;`.

- [ ] **Step 4: Rodar os testes**

Run: `cargo test -p zefiro-ui`
Expected: PASS (11 testes).

- [ ] **Step 5: Commit**

```powershell
git add crates/ui
git commit -m "feat(ui): display list and bar layout for both heights and formats" -m "Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

---

## Task 11: Geometria do painel (`zefiro-ui`)

**Files:**
- Create: `crates/ui/src/panel_layout.rs`
- Modify: `crates/ui/src/lib.rs` (`pub mod panel_layout;`)

**Interfaces:**
- Consumes: Task 3 (`Rect`, `Size`), Task 4 (`ShapeStyle`).
- Produces: `panel_layout::{PANEL_GAP_DIP = 6.0, SCREEN_MARGIN_DIP = 8.0, SHADOW_MARGIN_DIP = 24.0, CHIP_FADE_S = 0.17, Shape { rect: Rect, radius: f32 }, panel_rect(chip: Rect, band_bottom: f32, content: Size, monitor_w: f32, scale: f32) -> Rect, pill_radius(h: f32, radius_max_px: f32, style: ShapeStyle, scale: f32) -> f32, panel_radius(panel_radius_px: f32, style: ShapeStyle, scale: f32) -> f32, content_alpha(h: f32, final_h: f32) -> f32, CONTENT_ALPHA_EXPRESSION: &str, window_bounds(chip: Rect, panel: Rect, scale: f32) -> Rect, to_local(shape: Shape, window: Rect) -> Shape}`.

- [ ] **Step 1: Testes (falhando)**

`crates/ui/src/panel_layout.rs` (só os testes):

```rust
//! Pílula → painel: onde o painel fica, raios, opacidade do conteúdo e a janela que contém a animação.

#[cfg(test)]
mod tests {
    use super::*;
    use zefiro_resources::theme::ShapeStyle;

    #[test]
    fn panel_is_centered_under_chip_and_below_the_band() {
        let chip = Rect::new(900.0, 6.0, 120.0, 32.0);
        let p = panel_rect(chip, 44.0, Size { w: 352.0, h: 186.0 }, 1920.0, 1.0);
        assert_eq!(p, Rect::new(784.0, 50.0, 352.0, 186.0));
    }

    #[test]
    fn panel_stays_8_dip_inside_the_screen() {
        let right = panel_rect(Rect::new(1800.0, 6.0, 110.0, 32.0), 44.0, Size { w: 352.0, h: 100.0 }, 1920.0, 1.0);
        assert_eq!(right.right(), 1912.0);
        let left = panel_rect(Rect::new(8.0, 6.0, 80.0, 32.0), 44.0, Size { w: 300.0, h: 100.0 }, 1920.0, 1.0);
        assert_eq!(left.x, 8.0);
        let scaled = panel_rect(Rect::new(8.0, 6.0, 80.0, 32.0), 55.0, Size { w: 300.0, h: 100.0 }, 2400.0, 1.25);
        assert_eq!((scaled.x, scaled.y), (10.0, 63.0)); // 8 × 1,25 = 10; 55 + 6 × 1,25 = 62,5 → 63
    }

    #[test]
    fn radii_follow_the_island_rules() {
        assert_eq!(pill_radius(32.0, 44.0, ShapeStyle::Pill, 1.0), 16.0);
        assert_eq!(pill_radius(200.0, 44.0, ShapeStyle::Pill, 1.0), 44.0);
        assert_eq!(pill_radius(32.0, 44.0, ShapeStyle::W11, 1.5), 12.0); // 8 DIP
        assert_eq!(panel_radius(24.0, ShapeStyle::Pill, 1.0), 24.0);
        assert_eq!(panel_radius(30.0, ShapeStyle::W11, 1.0), 8.0);
    }

    #[test]
    fn content_alpha_starts_at_60_percent_of_final_height() {
        assert_eq!(content_alpha(0.0, 200.0), 0.0);
        assert_eq!(content_alpha(120.0, 200.0), 0.0);
        assert!((content_alpha(160.0, 200.0) - 0.5).abs() < 1e-6);
        assert_eq!(content_alpha(200.0, 200.0), 1.0);
        assert_eq!(content_alpha(230.0, 200.0), 1.0); // mola passando do alvo
        assert!(CONTENT_ALPHA_EXPRESSION.contains("0.6") && CONTENT_ALPHA_EXPRESSION.contains("0.4"));
    }

    #[test]
    fn window_bounds_cover_chip_panel_and_shadow() {
        let chip = Rect::new(900.0, 6.0, 120.0, 32.0);
        let panel = Rect::new(784.0, 50.0, 352.0, 186.0);
        let w = window_bounds(chip, panel, 1.0);
        assert_eq!(w, Rect::new(760.0, 0.0, 400.0, 260.0)); // ±24 de sombra, sem passar do topo
        let local = to_local(Shape { rect: chip, radius: 16.0 }, w);
        assert_eq!(local.rect, Rect::new(140.0, 6.0, 120.0, 32.0));
    }
}
```

- [ ] **Step 2: Rodar para ver falhar**

Run: `cargo test -p zefiro-ui panel`
Expected: FAIL de compilação.

- [ ] **Step 3: Implementar** (acima dos testes)

```rust
use zefiro_platform::geometry::{Rect, Size};
use zefiro_resources::theme::ShapeStyle;

pub const PANEL_GAP_DIP: f32 = 6.0;
pub const SCREEN_MARGIN_DIP: f32 = 8.0;
pub const SHADOW_MARGIN_DIP: f32 = 24.0;
/// O conteúdo do chip some neste tempo quando o painel abre.
pub const CHIP_FADE_S: f32 = 0.17;

/// Mesma regra de `content_alpha`, como expressão do compositor. `g` = geometria da forma, `H` = altura final.
pub const CONTENT_ALPHA_EXPRESSION: &str = "Clamp((g.Size.Y - 0.6 * H) / (0.4 * H), 0, 1)";

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Shape { pub rect: Rect, pub radius: f32 }

pub fn panel_rect(chip: Rect, band_bottom: f32, content: Size, monitor_w: f32, scale: f32) -> Rect {
    let margin = (SCREEN_MARGIN_DIP * scale).round();
    let x = (chip.center_x() - content.w / 2.0).round().clamp(margin, (monitor_w - margin - content.w).max(margin));
    let y = (band_bottom + PANEL_GAP_DIP * scale).round();
    Rect::new(x, y, content.w, content.h)
}

pub fn pill_radius(h: f32, radius_max_px: f32, style: ShapeStyle, scale: f32) -> f32 {
    match style {
        ShapeStyle::W11 => 8.0 * scale,
        ShapeStyle::Pill => (h / 2.0).min(radius_max_px),
    }
}

pub fn panel_radius(panel_radius_px: f32, style: ShapeStyle, scale: f32) -> f32 {
    match style {
        ShapeStyle::W11 => 8.0 * scale,
        ShapeStyle::Pill => panel_radius_px,
    }
}

pub fn content_alpha(h: f32, final_h: f32) -> f32 {
    if final_h <= 0.0 {
        return 0.0;
    }
    ((h - 0.6 * final_h) / (0.4 * final_h)).clamp(0.0, 1.0)
}

/// Retângulo da janela do painel (coordenadas do monitor): chip + painel + margem de sombra, sem passar do topo.
pub fn window_bounds(chip: Rect, panel: Rect, scale: f32) -> Rect {
    let m = (SHADOW_MARGIN_DIP * scale).round();
    let u = chip.union(panel);
    let top = (u.y - m).max(0.0);
    Rect::new(u.x - m, top, u.w + 2.0 * m, u.bottom() + m - top)
}

pub fn to_local(shape: Shape, window: Rect) -> Shape {
    Shape { rect: Rect::new(shape.rect.x - window.x, shape.rect.y - window.y, shape.rect.w, shape.rect.h), radius: shape.radius }
}
```

Em `lib.rs`: `pub mod panel_layout;`.

- [ ] **Step 4: Rodar os testes**

Run: `cargo test -p zefiro-ui`
Expected: PASS (16 testes).

- [ ] **Step 5: Commit**

```powershell
git add crates/ui
git commit -m "feat(ui): pill-to-panel geometry, radii and content alpha rule" -m "Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

---

## Task 12: Relógio e calendário (`zefiro-integration-clock`)

**Files:**
- Create: `crates/integrations/clock/src/{grid,format,windows}.rs`
- Modify: `crates/integrations/clock/Cargo.toml`, `crates/integrations/clock/src/lib.rs`

**Interfaces:**
- Consumes: Task 1.
- Produces:
  - `grid::{Date { year: i32, month: u32, day: u32 }, days_in_month(i32, u32) -> u32, weekday(Date) -> u32 /* 0 = domingo */, MonthGrid { year, month, first_weekday: u32, weeks: usize, cells: Vec<Option<u32>>, today: Option<u32> }, month_grid(year: i32, month: u32, first_weekday: u32, today: Date) -> MonthGrid, shift_month(year: i32, month: u32, delta: i32) -> (i32, u32), ms_to_next_minute(second: u32, millis: u32) -> u32}`.
  - `format::{LocalTime { date: Date, hour: u32, minute: u32, second: u32, millis: u32 }, ClockText /* trait */, PtBrFallback}`. `trait ClockText { fn chip_text(&self, now: &LocalTime, custom_date: &str, seconds: bool) -> String; fn long_date(&self, d: Date) -> String; fn month_name(&self, year: i32, month: u32) -> String; fn weekday_long(&self, d: Date) -> String; fn weekday_initials(&self) -> [String; 7] /* domingo primeiro */; fn first_weekday(&self) -> u32; }`.
  - Só no Windows: `windows::{WindowsClock /* impl ClockText */, now_local() -> LocalTime}`.

- [ ] **Step 1: Dependências e testes (falhando)**

`crates/integrations/clock/Cargo.toml`, acrescentar:

```toml
[target.'cfg(windows)'.dependencies]
windows.workspace = true
```

`crates/integrations/clock/src/lib.rs`:

```rust
//! Relógio e calendário.

pub mod format;
pub mod grid;
#[cfg(windows)]
pub mod windows;
```

`crates/integrations/clock/src/grid.rs` (só os testes):

```rust
//! Datas e a grade do mês (sem depender do sistema).

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn days_in_month_handles_leap_years() {
        assert_eq!(days_in_month(2026, 2), 28);
        assert_eq!(days_in_month(2028, 2), 29);
        assert_eq!(days_in_month(2100, 2), 28);
        assert_eq!(days_in_month(2000, 2), 29);
        assert_eq!(days_in_month(2026, 10), 31);
        assert_eq!(days_in_month(2026, 11), 30);
    }

    #[test]
    fn weekday_of_known_dates() {
        assert_eq!(weekday(Date { year: 2026, month: 10, day: 15 }), 4); // quinta
        assert_eq!(weekday(Date { year: 2000, month: 1, day: 1 }), 6); // sábado
    }

    #[test]
    fn october_2026_starting_sunday_has_5_weeks() {
        let g = month_grid(2026, 10, 0, Date { year: 2026, month: 10, day: 15 });
        assert_eq!(g.weeks, 5);
        assert_eq!(g.cells.len(), 35);
        assert_eq!(&g.cells[..5], &[None, None, None, None, Some(1)]); // 1º de outubro é quinta
        assert_eq!(g.today, Some(15));
    }

    #[test]
    fn starting_monday_shifts_the_grid() {
        let g = month_grid(2026, 10, 1, Date { year: 2026, month: 9, day: 1 });
        assert_eq!(&g.cells[..4], &[None, None, None, Some(1)]);
        assert_eq!(g.today, None); // hoje está em outro mês
    }

    #[test]
    fn four_and_six_week_months() {
        assert_eq!(month_grid(2026, 2, 0, Date { year: 2026, month: 2, day: 1 }).weeks, 4); // fev/2026 começa no domingo
        assert_eq!(month_grid(2026, 8, 0, Date { year: 2026, month: 8, day: 1 }).weeks, 6); // ago/2026 começa no sábado
    }

    #[test]
    fn shift_month_crosses_years() {
        assert_eq!(shift_month(2026, 12, 1), (2027, 1));
        assert_eq!(shift_month(2026, 1, -1), (2025, 12));
        assert_eq!(shift_month(2026, 10, -22), (2024, 12));
    }

    #[test]
    fn next_minute() {
        assert_eq!(ms_to_next_minute(0, 0), 60_000);
        assert_eq!(ms_to_next_minute(59, 900), 100);
        assert_eq!(ms_to_next_minute(30, 0), 30_000);
    }
}
```

`crates/integrations/clock/src/format.rs` (só os testes):

```rust
//! Textos do relógio. No Windows vêm do sistema (pt-BR ou o idioma do usuário); `PtBrFallback` serve
//! para testes e para sistemas sem implementação.

#[cfg(test)]
mod tests {
    use super::*;

    fn now() -> LocalTime {
        LocalTime { date: Date { year: 2026, month: 10, day: 15 }, hour: 15, minute: 7, second: 9, millis: 0 }
    }

    #[test]
    fn fallback_chip_text() {
        assert_eq!(PtBrFallback.chip_text(&now(), "", false), "qui 15 15:07");
        assert_eq!(PtBrFallback.chip_text(&now(), "", true), "qui 15 15:07:09");
    }

    #[test]
    fn fallback_names() {
        let d = Date { year: 2026, month: 10, day: 15 };
        assert_eq!(PtBrFallback.long_date(d), "quinta-feira, 15 de outubro");
        assert_eq!(PtBrFallback.month_name(2026, 10), "outubro");
        assert_eq!(PtBrFallback.weekday_long(d), "quinta-feira");
        assert_eq!(PtBrFallback.weekday_initials()[0], "D");
        assert_eq!(PtBrFallback.first_weekday(), 0);
    }
}
```

- [ ] **Step 2: Rodar para ver falhar**

Run: `cargo test -p zefiro-integration-clock`
Expected: FAIL de compilação.

- [ ] **Step 3: Implementar a parte portável**

`grid.rs` (acima dos testes):

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Date { pub year: i32, pub month: u32, pub day: u32 }

pub fn days_in_month(year: i32, month: u32) -> u32 {
    match month {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        _ if (year % 4 == 0 && year % 100 != 0) || year % 400 == 0 => 29,
        _ => 28,
    }
}

/// Algoritmo de Sakamoto. 0 = domingo.
pub fn weekday(d: Date) -> u32 {
    const T: [i32; 12] = [0, 3, 2, 5, 0, 3, 5, 1, 4, 6, 2, 4];
    let y = if d.month < 3 { d.year - 1 } else { d.year };
    ((y + y / 4 - y / 100 + y / 400 + T[(d.month - 1) as usize] + d.day as i32).rem_euclid(7)) as u32
}

#[derive(Debug, Clone, PartialEq)]
pub struct MonthGrid {
    pub year: i32,
    pub month: u32,
    pub first_weekday: u32,
    pub weeks: usize,
    pub cells: Vec<Option<u32>>,
    pub today: Option<u32>,
}

pub fn month_grid(year: i32, month: u32, first_weekday: u32, today: Date) -> MonthGrid {
    let lead = ((weekday(Date { year, month, day: 1 }) + 7 - first_weekday % 7) % 7) as usize;
    let days = days_in_month(year, month) as usize;
    let weeks = (lead + days).div_ceil(7);
    let mut cells = vec![None; weeks * 7];
    for d in 0..days {
        cells[lead + d] = Some(d as u32 + 1);
    }
    let today = (today.year == year && today.month == month).then_some(today.day);
    MonthGrid { year, month, first_weekday, weeks, cells, today }
}

pub fn shift_month(year: i32, month: u32, delta: i32) -> (i32, u32) {
    let idx = year * 12 + (month as i32 - 1) + delta;
    (idx.div_euclid(12), (idx.rem_euclid(12) + 1) as u32)
}

pub fn ms_to_next_minute(second: u32, millis: u32) -> u32 {
    60_000 - (second * 1000 + millis).min(59_999)
}
```

`format.rs` (acima dos testes):

```rust
pub use crate::grid::Date;
use crate::grid::weekday;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LocalTime { pub date: Date, pub hour: u32, pub minute: u32, pub second: u32, pub millis: u32 }

pub trait ClockText {
    /// Texto do chip. `custom_date` vazio = formato do sistema; senão, padrão de data do Windows (ex.: "ddd d MMM").
    fn chip_text(&self, now: &LocalTime, custom_date: &str, seconds: bool) -> String;
    fn long_date(&self, d: Date) -> String;
    fn month_name(&self, year: i32, month: u32) -> String;
    fn weekday_long(&self, d: Date) -> String;
    /// Iniciais dos dias, domingo primeiro.
    fn weekday_initials(&self) -> [String; 7];
    /// 0 = domingo, 1 = segunda…
    fn first_weekday(&self) -> u32;
}

pub struct PtBrFallback;

const DAYS: [&str; 7] = ["domingo", "segunda-feira", "terça-feira", "quarta-feira", "quinta-feira", "sexta-feira", "sábado"];
const SHORT: [&str; 7] = ["dom", "seg", "ter", "qua", "qui", "sex", "sáb"];
const MONTHS: [&str; 12] = ["janeiro", "fevereiro", "março", "abril", "maio", "junho", "julho", "agosto", "setembro", "outubro", "novembro", "dezembro"];

impl ClockText for PtBrFallback {
    fn chip_text(&self, now: &LocalTime, _custom_date: &str, seconds: bool) -> String {
        let day = SHORT[weekday(now.date) as usize];
        if seconds {
            format!("{day} {} {:02}:{:02}:{:02}", now.date.day, now.hour, now.minute, now.second)
        } else {
            format!("{day} {} {:02}:{:02}", now.date.day, now.hour, now.minute)
        }
    }
    fn long_date(&self, d: Date) -> String {
        format!("{}, {} de {}", DAYS[weekday(d) as usize], d.day, MONTHS[(d.month - 1) as usize])
    }
    fn month_name(&self, _year: i32, month: u32) -> String { MONTHS[(month - 1) as usize].to_string() }
    fn weekday_long(&self, d: Date) -> String { DAYS[weekday(d) as usize].to_string() }
    fn weekday_initials(&self) -> [String; 7] { ["D", "S", "T", "Q", "Q", "S", "S"].map(String::from) }
    fn first_weekday(&self) -> u32 { 0 }
}
```

- [ ] **Step 4: Rodar os testes portáveis**

Run: `cargo test -p zefiro-integration-clock`
Expected: PASS (9 testes).

- [ ] **Step 5: Implementar a parte Windows**

`crates/integrations/clock/src/windows.rs`:

```rust
//! Textos do relógio pelo Windows (idioma e formatos do usuário).

use crate::format::{ClockText, LocalTime};
use crate::grid::Date;
use windows::Win32::Foundation::SYSTEMTIME;
use windows::Win32::Globalization::*;
use windows::Win32::System::SystemInformation::GetLocalTime;
use windows::core::{HSTRING, PCWSTR};

pub struct WindowsClock;

fn systemtime(d: Date, hour: u32, minute: u32, second: u32) -> SYSTEMTIME {
    SYSTEMTIME {
        wYear: d.year as u16,
        wMonth: d.month as u16,
        wDay: d.day as u16,
        wDayOfWeek: crate::grid::weekday(d) as u16,
        wHour: hour as u16,
        wMinute: minute as u16,
        wSecond: second as u16,
        wMilliseconds: 0,
    }
}

fn date_text(st: &SYSTEMTIME, pattern: &str) -> String {
    let pat = HSTRING::from(pattern);
    let mut buf = [0u16; 128];
    // SAFETY: buffer local com tamanho correto; padrão terminado em zero (HSTRING).
    let n = unsafe { GetDateFormatEx(LOCALE_NAME_USER_DEFAULT, ENUM_DATE_FORMATS_FLAGS(0), Some(st), &pat, Some(&mut buf), None) };
    String::from_utf16_lossy(&buf[..(n.max(1) - 1) as usize])
}

fn time_text(st: &SYSTEMTIME, seconds: bool) -> String {
    let mut buf = [0u16; 64];
    let flags = if seconds { TIME_FORMAT_FLAGS(0) } else { TIME_NOSECONDS };
    // SAFETY: buffer local; formato nulo = formato curto do usuário.
    let n = unsafe { GetTimeFormatEx(LOCALE_NAME_USER_DEFAULT, flags, Some(st), PCWSTR::null(), Some(&mut buf)) };
    String::from_utf16_lossy(&buf[..(n.max(1) - 1) as usize])
}

impl ClockText for WindowsClock {
    fn chip_text(&self, now: &LocalTime, custom_date: &str, seconds: bool) -> String {
        let st = systemtime(now.date, now.hour, now.minute, now.second);
        let date = date_text(&st, if custom_date.is_empty() { "ddd d" } else { custom_date });
        format!("{date} {}", time_text(&st, seconds))
    }
    fn long_date(&self, d: Date) -> String { date_text(&systemtime(d, 0, 0, 0), "dddd, d 'de' MMMM") }
    fn month_name(&self, year: i32, month: u32) -> String { date_text(&systemtime(Date { year, month, day: 1 }, 0, 0, 0), "MMMM") }
    fn weekday_long(&self, d: Date) -> String { date_text(&systemtime(d, 0, 0, 0), "dddd") }
    fn weekday_initials(&self) -> [String; 7] {
        std::array::from_fn(|weekday| {
            // CAL_SSHORTESTDAYNAME1 é segunda; o índice 0 aqui é domingo.
            let cal = CAL_SSHORTESTDAYNAME1 + ((weekday as u32 + 6) % 7);
            let mut buf = [0u16; 16];
            // SAFETY: buffer local com tamanho correto.
            let n = unsafe { GetCalendarInfoEx(LOCALE_NAME_USER_DEFAULT, CAL_GREGORIAN, PCWSTR::null(), cal, Some(&mut buf), None) };
            if n > 1 { String::from_utf16_lossy(&buf[..(n - 1) as usize]) } else { ["D", "S", "T", "Q", "Q", "S", "S"][weekday].to_string() }
        })
    }
    fn first_weekday(&self) -> u32 {
        let mut value = 0u32;
        // SAFETY: LOCALE_RETURN_NUMBER grava um DWORD no buffer (2 u16), que aponta para `value`.
        let n = unsafe {
            GetLocaleInfoEx(
                LOCALE_NAME_USER_DEFAULT,
                LOCALE_IFIRSTDAYOFWEEK | LOCALE_RETURN_NUMBER,
                Some(std::slice::from_raw_parts_mut(&mut value as *mut u32 as *mut u16, 2)),
            )
        };
        // O Windows conta 0 = segunda.
        if n > 0 { (value + 1) % 7 } else { 0 }
    }
}

pub fn now_local() -> LocalTime {
    // SAFETY: sem parâmetros.
    let st = unsafe { GetLocalTime() };
    LocalTime {
        date: Date { year: st.wYear as i32, month: st.wMonth as u32, day: st.wDay as u32 },
        hour: st.wHour as u32,
        minute: st.wMinute as u32,
        second: st.wSecond as u32,
        millis: st.wMilliseconds as u32,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn windows_texts_are_not_empty() {
        let c = WindowsClock;
        let now = now_local();
        assert!(!c.chip_text(&now, "", false).is_empty());
        assert!(!c.long_date(now.date).is_empty());
        assert!(c.weekday_initials().iter().all(|s| !s.is_empty()));
        assert!(c.first_weekday() < 7);
    }
}
```

(Se `LOCALE_NAME_USER_DEFAULT` ou as constantes `CAL_*`/`LOCALE_*` tiverem outro tipo na `windows 0.61.3`, ajuste os casts conforme a API real e anote no ADR-0007.)

- [ ] **Step 6: Rodar e verificar a regra de ouro**

Run: `cargo test -p zefiro-integration-clock; pwsh -File scripts/check-windows-imports.ps1; cargo check -p zefiro-integration-clock --target x86_64-unknown-linux-gnu`
Expected: PASS (10 testes); "Regra de ouro ok."; o check para Linux compila sem o módulo `windows`.

- [ ] **Step 7: Commit**

```powershell
git add crates/integrations Cargo.lock
git commit -m "feat(clock): month grid, minute timing and Windows-locale clock texts" -m "Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

---

## Task 13: Conteúdo dos chips (`zefiro-bar`)

**Files:**
- Create: `crates/modules/bar/src/chips.rs`
- Modify: `crates/modules/bar/Cargo.toml`, `crates/modules/bar/src/lib.rs`

**Interfaces:**
- Consumes: Task 3 (`TextMeasurer`, `TextStyle`), Task 5 (`Tokens`), Task 10 (`DisplayList`, `DrawOp`).
- Produces: `chips::{ChipContent { content_w: f32, list: DisplayList }, zefiro_chip(chip_h: f32, font_px: f32, compact: bool, scale: f32, tokens: &Tokens, m: &dyn TextMeasurer) -> ChipContent, clock_chip(text: &str, chip_h: f32, font_px: f32, tokens: &Tokens, m: &dyn TextMeasurer) -> ChipContent}`. A lista é relativa ao **conteúdo** do chip (origem 0,0; altura `chip_h`; sem o padding lateral de 10 DIP).

- [ ] **Step 1: Dependências e testes (falhando)**

`crates/modules/bar/Cargo.toml`:

```toml
[dependencies]
zefiro-core.workspace = true
zefiro-platform.workspace = true
zefiro-resources.workspace = true
zefiro-ui.workspace = true
zefiro-integration-clock.workspace = true
```

`crates/modules/bar/src/lib.rs`:

```rust
//! Barra: chips, menu do Zefiro, calendário e regras dos painéis.

pub mod chips;
```

`crates/modules/bar/src/chips.rs` (só os testes):

```rust
//! O que cada chip desenha.

#[cfg(test)]
mod tests {
    use super::*;
    use zefiro_platform::color::Rgba;
    use zefiro_platform::text::FixedWidthMeasurer;
    use zefiro_resources::theme::{BUILTIN, parse_theme};
    use zefiro_resources::tokens::derive_tokens;
    use zefiro_ui::paint::DrawOp;

    fn tokens() -> zefiro_resources::tokens::Tokens {
        derive_tokens(&parse_theme("obsidian", "o", BUILTIN[0].1).unwrap(), Rgba::rgb(0, 120, 215), None)
    }

    #[test]
    fn zefiro_chip_has_monogram_and_label_when_full() {
        let c = zefiro_chip(32.0, 13.0, false, 1.0, &tokens(), &FixedWidthMeasurer);
        assert!(matches!(c.list.ops[0], DrawOp::RoundRect { .. }));
        let texts: Vec<&str> = c.list.ops.iter().filter_map(|o| if let DrawOp::Text { text, .. } = o { Some(text.as_str()) } else { None }).collect();
        assert_eq!(texts, vec!["Z", "Zefiro"]);
        assert!(c.content_w > 32.0 * 0.7);
    }

    #[test]
    fn zefiro_chip_is_monogram_only_when_compact() {
        let c = zefiro_chip(24.0, 12.0, true, 1.0, &tokens(), &FixedWidthMeasurer);
        let texts = c.list.ops.iter().filter(|o| matches!(o, DrawOp::Text { .. })).count();
        assert_eq!(texts, 1);
        assert_eq!(c.content_w, (24.0f32 * 0.72).round());
    }

    #[test]
    fn clock_chip_width_is_text_width_rounded_up() {
        let c = clock_chip("qui 15 15:47", 32.0, 13.0, &tokens(), &FixedWidthMeasurer);
        assert_eq!(c.content_w, (12.0f32 * 13.0 * 0.55).ceil());
        assert!(matches!(&c.list.ops[0], DrawOp::Text { text, .. } if text == "qui 15 15:47"));
    }
}
```

- [ ] **Step 2: Rodar para ver falhar**

Run: `cargo test -p zefiro-bar`
Expected: FAIL de compilação.

- [ ] **Step 3: Implementar** (acima dos testes)

```rust
use zefiro_platform::geometry::{Rect, Size};
use zefiro_platform::text::{Align, Family, FontWeight, TextMeasurer, TextStyle};
use zefiro_resources::tokens::Tokens;
use zefiro_ui::paint::{DisplayList, DrawOp};

#[derive(Debug, Clone, PartialEq)]
pub struct ChipContent { pub content_w: f32, pub list: DisplayList }

/// Monograma "Z" num quadrado arredondado (72 % da altura) + "Zefiro" (só na altura completa).
pub fn zefiro_chip(chip_h: f32, font_px: f32, compact: bool, scale: f32, tokens: &Tokens, m: &dyn TextMeasurer) -> ChipContent {
    let side = (chip_h * 0.72).round();
    let top = ((chip_h - side) / 2.0).round();
    let mut ops = vec![
        DrawOp::RoundRect { rect: Rect::new(0.0, top, side, side), radius: 6.0 * scale, color: tokens.raised_strong },
        DrawOp::Text {
            rect: Rect::new(0.0, top, side, side),
            text: "Z".into(),
            style: TextStyle { size: font_px, weight: FontWeight::Bold, family: Family::Text },
            color: tokens.text,
            align: Align::Center,
        },
    ];
    let mut w = side;
    if !compact {
        let style = TextStyle { size: font_px, weight: FontWeight::SemiBold, family: Family::Text };
        let label_w = m.measure("Zefiro", &style).w.ceil();
        let x = side + (8.0 * scale).round();
        ops.push(DrawOp::Text { rect: Rect::new(x, 0.0, label_w, chip_h), text: "Zefiro".into(), style, color: tokens.text_secondary, align: Align::Leading });
        w = x + label_w;
    }
    ChipContent { content_w: w, list: DisplayList { size: Size { w, h: chip_h }, ops } }
}

pub fn clock_chip(text: &str, chip_h: f32, font_px: f32, tokens: &Tokens, m: &dyn TextMeasurer) -> ChipContent {
    let style = TextStyle { size: font_px, weight: FontWeight::SemiBold, family: Family::Text };
    let w = m.measure(text, &style).w.ceil();
    let ops = vec![DrawOp::Text { rect: Rect::new(0.0, 0.0, w, chip_h), text: text.to_string(), style, color: tokens.text, align: Align::Leading }];
    ChipContent { content_w: w, list: DisplayList { size: Size { w, h: chip_h }, ops } }
}
```

- [ ] **Step 4: Rodar os testes**

Run: `cargo test -p zefiro-bar`
Expected: PASS (3 testes).

- [ ] **Step 5: Commit**

```powershell
git add crates/modules/bar Cargo.lock
git commit -m "feat(bar): Zefiro and clock chip contents" -m "Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

---

## Task 14: Menu do Zefiro (`zefiro-bar`)

**Files:**
- Create: `crates/modules/bar/src/menu.rs`
- Modify: `crates/modules/bar/src/lib.rs` (`pub mod menu;`)

**Interfaces:**
- Consumes: Task 7 (`BarHeight`, `BarFormat`), Task 5 (`Tokens`), Task 10 (`DisplayList`), Task 3.
- Produces: `menu::{MenuAction, ThemeSwatch, NavKey, MenuItemBox, MenuModel}`.
  - `MenuAction { SetHeight(BarHeight), SetFormat(BarFormat), SetTheme(String), ReloadTheme, OpenThemesFolder, RestartIsland, Exit }` (`Clone + PartialEq + Debug`).
  - `ThemeSwatch { id: String, name: String, background: Rgba, text: Rgba, border: Option<Rgba> }`.
  - `NavKey { Up, Down, Left, Right, Enter }`.
  - `MenuItemBox { rect: Rect, row: usize, col: usize, action: MenuAction }`.
  - `MenuModel::new(height: BarHeight, format: BarFormat, theme_id: &str, themes: Vec<ThemeSwatch>) -> MenuModel`, `key(&mut self, NavKey) -> Option<MenuAction>`, `focus(&self) -> (usize, usize)`, `layout(&self, scale: f32) -> Vec<MenuItemBox>`, `size(&self, scale: f32) -> Size`, `hit(&self, p: Point, scale: f32) -> Option<MenuAction>`, `paint(&self, scale: f32, tokens: &Tokens) -> DisplayList`.

Layout (DIP): largura 300; padding 16; título de seção com 20 de altura (texto 11, `text_tertiary`); 8 entre seções. Ordem: **Altura** (linha segmentada Completa | Compacta, 32 de altura, fundo `raised`, raio 16, opção marcada `raised_strong`), **Formato** (Faixa | Pílulas, igual), **Tema** (grade de 4 colunas de 67 × 56: círculo de 26 com a cor do tema, contorno de 1, um ponto de 4 com a cor do texto, nome em 11 embaixo; o tema atual ganha um anel `accent`), e as ações **Recarregar tema**, **Abrir pasta de temas**, **Reiniciar a ilha**, **Sair** (linhas de 32, texto 13). O foco do teclado é um contorno `accent` de 1,5 (nas ações, o fundo `raised`).

Navegação: cada linha segmentada é uma linha de navegação; cada linha da grade de temas é uma linha; cada ação é uma linha. ↑/↓ trocam de linha (a coluna é limitada ao tamanho da nova linha); ←/→ andam na linha; Enter devolve a ação da caixa focada. O foco começa na altura atual.

- [ ] **Step 1: Testes (falhando)**

`crates/modules/bar/src/menu.rs` (só os testes):

```rust
//! Painel do Zefiro: altura, formato, tema e ações, com navegação por teclado.

#[cfg(test)]
mod tests {
    use super::*;
    use zefiro_core::config::{BarFormat, BarHeight};
    use zefiro_platform::color::Rgba;

    fn swatches(n: usize) -> Vec<ThemeSwatch> {
        (0..n).map(|i| ThemeSwatch { id: format!("t{i}"), name: format!("T{i}"), background: Rgba::BLACK, text: Rgba::WHITE, border: None }).collect()
    }

    fn model() -> MenuModel {
        MenuModel::new(BarHeight::Completa, BarFormat::Faixa, "t2", swatches(8))
    }

    #[test]
    fn starts_focused_on_the_selected_height() {
        assert_eq!(model().focus(), (0, 0));
        assert_eq!(MenuModel::new(BarHeight::Compacta, BarFormat::Faixa, "t2", swatches(8)).focus(), (0, 1));
    }

    #[test]
    fn right_then_enter_picks_compact() {
        let mut m = model();
        assert_eq!(m.key(NavKey::Right), None);
        assert_eq!(m.key(NavKey::Enter), Some(MenuAction::SetHeight(BarHeight::Compacta)));
    }

    #[test]
    fn down_walks_rows_and_clamps_columns() {
        let mut m = model();
        m.key(NavKey::Down); // formato
        assert_eq!(m.key(NavKey::Enter), Some(MenuAction::SetFormat(BarFormat::Faixa)));
        m.key(NavKey::Down); // primeira linha de temas
        for _ in 0..4 {
            m.key(NavKey::Right); // a quarta passa do fim e fica na coluna 3
        }
        assert_eq!(m.key(NavKey::Enter), Some(MenuAction::SetTheme("t3".into())));
        m.key(NavKey::Down); // segunda linha de temas, coluna 3
        assert_eq!(m.key(NavKey::Enter), Some(MenuAction::SetTheme("t7".into())));
        m.key(NavKey::Down); // primeira ação, coluna volta a 0
        assert_eq!(m.key(NavKey::Enter), Some(MenuAction::ReloadTheme));
        for _ in 0..10 {
            m.key(NavKey::Down);
        }
        assert_eq!(m.key(NavKey::Enter), Some(MenuAction::Exit));
        for _ in 0..20 {
            m.key(NavKey::Up);
        }
        assert_eq!(m.focus().0, 0);
    }

    #[test]
    fn partial_last_theme_row_clamps_column() {
        let mut m = MenuModel::new(BarHeight::Completa, BarFormat::Faixa, "t0", swatches(9));
        m.key(NavKey::Down);
        m.key(NavKey::Down);
        for _ in 0..3 {
            m.key(NavKey::Right);
        }
        m.key(NavKey::Down);
        m.key(NavKey::Down); // terceira linha de temas só tem t8
        assert_eq!(m.key(NavKey::Enter), Some(MenuAction::SetTheme("t8".into())));
    }

    #[test]
    fn hit_test_matches_layout() {
        let m = model();
        for b in m.layout(1.0) {
            let center = Point { x: b.rect.x + b.rect.w / 2.0, y: b.rect.y + b.rect.h / 2.0 };
            assert_eq!(m.hit(center, 1.0), Some(b.action.clone()), "{b:?}");
        }
        assert_eq!(m.hit(Point { x: 1.0, y: 1.0 }, 1.0), None);
    }

    #[test]
    fn size_grows_with_themes_and_scale() {
        let a = MenuModel::new(BarHeight::Completa, BarFormat::Faixa, "t0", swatches(8)).size(1.0);
        let b = MenuModel::new(BarHeight::Completa, BarFormat::Faixa, "t0", swatches(9)).size(1.0);
        assert_eq!(a.w, 300.0);
        assert_eq!(b.h - a.h, 56.0);
        assert_eq!(MenuModel::new(BarHeight::Completa, BarFormat::Faixa, "t0", swatches(8)).size(1.5).w, 450.0);
    }

    #[test]
    fn paint_marks_current_choices() {
        let o = zefiro_resources::theme::parse_theme("obsidian", "o", zefiro_resources::theme::BUILTIN[0].1).unwrap();
        let t = zefiro_resources::tokens::derive_tokens(&o, Rgba::rgb(0, 120, 215), None);
        let list = model().paint(1.0, &t);
        let strong = list.ops.iter().filter(|op| matches!(op, zefiro_ui::paint::DrawOp::RoundRect { color, .. } if *color == t.raised_strong)).count();
        assert_eq!(strong, 2); // altura e formato atuais
        assert_eq!(list.size, model().size(1.0));
    }
}
```

- [ ] **Step 2: Rodar para ver falhar**

Run: `cargo test -p zefiro-bar menu`
Expected: FAIL de compilação.

- [ ] **Step 3: Implementar** (acima dos testes)

```rust
use zefiro_core::config::{BarFormat, BarHeight};
use zefiro_platform::color::Rgba;
use zefiro_platform::geometry::{Point, Rect, Size};
use zefiro_platform::text::{Align, Family, FontWeight, TextStyle};
use zefiro_resources::tokens::Tokens;
use zefiro_ui::paint::{DisplayList, DrawOp};

#[derive(Debug, Clone, PartialEq)]
pub enum MenuAction { SetHeight(BarHeight), SetFormat(BarFormat), SetTheme(String), ReloadTheme, OpenThemesFolder, RestartIsland, Exit }

#[derive(Debug, Clone, PartialEq)]
pub struct ThemeSwatch { pub id: String, pub name: String, pub background: Rgba, pub text: Rgba, pub border: Option<Rgba> }

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NavKey { Up, Down, Left, Right, Enter }

#[derive(Debug, Clone, PartialEq)]
pub struct MenuItemBox { pub rect: Rect, pub row: usize, pub col: usize, pub action: MenuAction }

const W: f32 = 300.0;
const PAD: f32 = 16.0;
const TITLE_H: f32 = 20.0;
const ROW_H: f32 = 32.0;
const SWATCH_W: f32 = 67.0;
const SWATCH_H: f32 = 56.0;
const SECTION_GAP: f32 = 8.0;
const COLS: usize = 4;
fn actions() -> [(&'static str, MenuAction); 4] {
    [
        ("Recarregar tema", MenuAction::ReloadTheme),
        ("Abrir pasta de temas", MenuAction::OpenThemesFolder),
        ("Reiniciar a ilha", MenuAction::RestartIsland),
        ("Sair", MenuAction::Exit),
    ]
}

pub struct MenuModel { height: BarHeight, format: BarFormat, theme_id: String, themes: Vec<ThemeSwatch>, focus: (usize, usize) }

/// Títulos de seção e onde ficam (topo, em px), para o `paint`.
struct Titles { altura: f32, formato: f32, tema: f32 }

impl MenuModel {
    pub fn new(height: BarHeight, format: BarFormat, theme_id: &str, themes: Vec<ThemeSwatch>) -> MenuModel {
        let col = if height == BarHeight::Compacta { 1 } else { 0 };
        MenuModel { height, format, theme_id: theme_id.to_string(), themes, focus: (0, col) }
    }

    pub fn focus(&self) -> (usize, usize) { self.focus }

    fn build(&self, scale: f32) -> (Vec<MenuItemBox>, Titles, f32) {
        let s = |v: f32| v * scale;
        let inner = s(W - 2.0 * PAD);
        let mut out = Vec::new();
        let mut y = s(PAD);
        let mut row = 0;

        let mut segment = |y: &mut f32, row: &mut usize, opts: [(&str, MenuAction); 2]| {
            let w = inner / 2.0;
            for (col, (_, action)) in opts.into_iter().enumerate() {
                out.push(MenuItemBox { rect: Rect::new(s(PAD) + w * col as f32, *y, w, s(ROW_H)), row: *row, col, action });
            }
            *y += s(ROW_H);
            *row += 1;
        };

        let altura = y;
        y += s(TITLE_H);
        segment(&mut y, &mut row, [("Completa", MenuAction::SetHeight(BarHeight::Completa)), ("Compacta", MenuAction::SetHeight(BarHeight::Compacta))]);
        y += s(SECTION_GAP);
        let formato = y;
        y += s(TITLE_H);
        segment(&mut y, &mut row, [("Faixa", MenuAction::SetFormat(BarFormat::Faixa)), ("Pílulas", MenuAction::SetFormat(BarFormat::Pilulas))]);
        y += s(SECTION_GAP);
        let tema = y;
        y += s(TITLE_H);
        for (i, t) in self.themes.iter().enumerate() {
            let (r, c) = (i / COLS, i % COLS);
            let rect = Rect::new(s(PAD) + s(SWATCH_W) * c as f32, y + s(SWATCH_H) * r as f32, s(SWATCH_W), s(SWATCH_H));
            out.push(MenuItemBox { rect, row: row + r, col: c, action: MenuAction::SetTheme(t.id.clone()) });
        }
        let theme_rows = self.themes.len().div_ceil(COLS);
        y += s(SWATCH_H) * theme_rows as f32;
        row += theme_rows;
        y += s(SECTION_GAP);
        for (_, action) in actions() {
            out.push(MenuItemBox { rect: Rect::new(s(PAD), y, inner, s(ROW_H)), row, col: 0, action });
            y += s(ROW_H);
            row += 1;
        }
        (out, Titles { altura, formato, tema }, y + s(PAD))
    }

    pub fn layout(&self, scale: f32) -> Vec<MenuItemBox> { self.build(scale).0 }

    pub fn size(&self, scale: f32) -> Size { Size { w: W * scale, h: self.build(scale).2 } }

    pub fn hit(&self, p: Point, scale: f32) -> Option<MenuAction> {
        self.layout(scale).into_iter().find(|b| b.rect.contains(p)).map(|b| b.action)
    }

    pub fn key(&mut self, k: NavKey) -> Option<MenuAction> {
        let boxes = self.layout(1.0);
        let rows = boxes.iter().map(|b| b.row).max().map_or(0, |r| r + 1);
        let (mut r, mut c) = self.focus;
        match k {
            NavKey::Up => r = r.saturating_sub(1),
            NavKey::Down => r = (r + 1).min(rows.saturating_sub(1)),
            NavKey::Left => c = c.saturating_sub(1),
            NavKey::Right => c += 1,
            NavKey::Enter => return boxes.iter().find(|b| b.row == r && b.col == c).map(|b| b.action.clone()),
        }
        let len = boxes.iter().filter(|b| b.row == r).count();
        self.focus = (r, c.min(len.saturating_sub(1)));
        None
    }

    fn is_selected(&self, a: &MenuAction) -> bool {
        match a {
            MenuAction::SetHeight(h) => *h == self.height,
            MenuAction::SetFormat(f) => *f == self.format,
            MenuAction::SetTheme(id) => *id == self.theme_id,
            _ => false,
        }
    }

    pub fn paint(&self, scale: f32, tokens: &Tokens) -> DisplayList {
        let s = |v: f32| v * scale;
        let (boxes, titles, height) = self.build(scale);
        let mut list = DisplayList::new(Size { w: s(W), h: height });
        let title = TextStyle { size: s(11.0), weight: FontWeight::SemiBold, family: Family::Text };
        let body = TextStyle { size: s(13.0), weight: FontWeight::Regular, family: Family::Text };
        let small = TextStyle { size: s(11.0), weight: FontWeight::Regular, family: Family::Text };

        for (text, y) in [("Altura", titles.altura), ("Formato", titles.formato), ("Tema", titles.tema)] {
            list.push(DrawOp::Text { rect: Rect::new(s(PAD), y, s(W - 2.0 * PAD), s(TITLE_H)), text: text.into(), style: title, color: tokens.text_tertiary, align: Align::Leading });
        }
        for row in [0usize, 1] {
            let cells: Vec<&MenuItemBox> = boxes.iter().filter(|b| b.row == row).collect();
            let r = cells[0].rect.union(cells[cells.len() - 1].rect);
            list.push(DrawOp::RoundRect { rect: r, radius: r.h / 2.0, color: tokens.raised });
        }
        for b in &boxes {
            let focused = (b.row, b.col) == self.focus;
            match &b.action {
                MenuAction::SetHeight(_) | MenuAction::SetFormat(_) => {
                    let inner = b.rect.inset(s(3.0), s(3.0));
                    if self.is_selected(&b.action) {
                        list.push(DrawOp::RoundRect { rect: inner, radius: inner.h / 2.0, color: tokens.raised_strong });
                    }
                    let label = match &b.action {
                        MenuAction::SetHeight(BarHeight::Completa) => "Completa",
                        MenuAction::SetHeight(BarHeight::Compacta) => "Compacta",
                        MenuAction::SetFormat(BarFormat::Faixa) => "Faixa",
                        _ => "Pílulas",
                    };
                    list.push(DrawOp::Text { rect: b.rect, text: label.into(), style: body, color: tokens.text, align: Align::Center });
                    if focused {
                        list.push(DrawOp::RoundRectStroke { rect: inner, radius: inner.h / 2.0, color: tokens.accent, width: s(1.5) });
                    }
                }
                MenuAction::SetTheme(id) => {
                    let t = self.themes.iter().find(|t| &t.id == id).expect("tema da grade");
                    let c = Point { x: b.rect.center_x(), y: b.rect.y + s(18.0) };
                    if self.is_selected(&b.action) {
                        list.push(DrawOp::Circle { center: c, radius: s(16.0), color: tokens.accent });
                    }
                    list.push(DrawOp::Circle { center: c, radius: s(14.0), color: t.border.unwrap_or(tokens.hairline) });
                    list.push(DrawOp::Circle { center: c, radius: s(13.0), color: t.background.with_alpha(1.0) });
                    list.push(DrawOp::Circle { center: c, radius: s(4.0), color: t.text });
                    list.push(DrawOp::Text { rect: Rect::new(b.rect.x, b.rect.y + s(36.0), b.rect.w, s(16.0)), text: t.name.clone(), style: small, color: tokens.text_secondary, align: Align::Center });
                    if focused {
                        list.push(DrawOp::RoundRectStroke { rect: b.rect.inset(s(2.0), s(2.0)), radius: s(10.0), color: tokens.accent, width: s(1.5) });
                    }
                }
                action => {
                    let label = actions().into_iter().find(|(_, a)| a == action).map_or("", |(l, _)| l);
                    if focused {
                        list.push(DrawOp::RoundRect { rect: b.rect, radius: s(10.0), color: tokens.raised });
                    }
                    list.push(DrawOp::Text { rect: b.rect.inset(s(12.0), 0.0), text: label.into(), style: body, color: tokens.text, align: Align::Leading });
                }
            }
        }
        list
    }
}
```

Em `lib.rs`: `pub mod menu;`.

- [ ] **Step 4: Rodar os testes**

Run: `cargo test -p zefiro-bar`
Expected: PASS (10 testes).

- [ ] **Step 5: Commit**

```powershell
git add crates/modules/bar
git commit -m "feat(bar): Zefiro menu panel with height, format, theme grid, actions and keyboard navigation" -m "Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

---

## Task 15: Painel do calendário (`zefiro-bar`)

**Files:**
- Create: `crates/modules/bar/src/calendar.rs`
- Modify: `crates/modules/bar/src/lib.rs` (`pub mod calendar;`), `THIRD_PARTY_NOTICES.md` (layout do calendário da ilha)

**Interfaces:**
- Consumes: Task 12 (`MonthGrid`, `Date`, `ClockText`), Task 5 (`Tokens`), Task 10.
- Produces: `calendar::{CalendarNames { month: String, year: String, weekday_long: String, today_day: String, initials: [String; 7] }, calendar_names(text: &dyn ClockText, shown_year: i32, shown_month: u32, today: Date) -> CalendarNames, calendar_size(weeks: usize, scale: f32) -> Size, calendar_paint(grid: &MonthGrid, names: &CalendarNames, scale: f32, tokens: &Tokens) -> DisplayList}`.

Layout (DIP, como o calendário da ilha): largura 352. Cartão "herói" à esquerda em (22, 16), largura 93, até 20 do fundo, raio 12, fundo `raised`: nome do mês **mostrado** em `accent` negrito 12 no topo, ano em 11 `text_secondary`, o **dia de hoje** em `Display` 34 no meio, o dia da semana de hoje em negrito 11 embaixo. Grade a partir de x = 134, topo 16, colunas de 28, cabeçalho de 18 com as iniciais (11, `text_tertiary`), linhas de 22; hoje com círculo `accent` de raio 11 e o número na cor do fundo; os outros dias em `text`. Altura = 16 + 18 + semanas × 22 + 20.

- [ ] **Step 1: Testes (falhando)**

`crates/modules/bar/src/calendar.rs` (só os testes):

```rust
//! Calendário do mês, no estilo do calendário da ilha.

#[cfg(test)]
mod tests {
    use super::*;
    use zefiro_integration_clock::format::PtBrFallback;
    use zefiro_integration_clock::grid::{Date, month_grid};
    use zefiro_platform::color::Rgba;
    use zefiro_resources::theme::{BUILTIN, parse_theme};
    use zefiro_resources::tokens::derive_tokens;
    use zefiro_ui::paint::DrawOp;

    const TODAY: Date = Date { year: 2026, month: 10, day: 15 };

    fn tokens() -> zefiro_resources::tokens::Tokens {
        derive_tokens(&parse_theme("obsidian", "o", BUILTIN[0].1).unwrap(), Rgba::rgb(0, 120, 215), None)
    }

    #[test]
    fn size_depends_on_weeks() {
        assert_eq!(calendar_size(5, 1.0), Size { w: 352.0, h: 164.0 });
        assert_eq!(calendar_size(6, 1.0), Size { w: 352.0, h: 186.0 });
        assert_eq!(calendar_size(4, 2.0), Size { w: 704.0, h: 284.0 });
    }

    #[test]
    fn names_use_shown_month_but_today_for_the_hero_day() {
        let n = calendar_names(&PtBrFallback, 2026, 11, TODAY);
        assert_eq!(n.month, "novembro");
        assert_eq!(n.year, "2026");
        assert_eq!(n.today_day, "15");
        assert_eq!(n.weekday_long, "quinta-feira");
    }

    #[test]
    fn today_gets_an_accent_circle_and_every_day_is_drawn() {
        let g = month_grid(2026, 10, 0, TODAY);
        let t = tokens();
        let list = calendar_paint(&g, &calendar_names(&PtBrFallback, 2026, 10, TODAY), 1.0, &t);
        let circles = list.ops.iter().filter(|o| matches!(o, DrawOp::Circle { color, .. } if *color == t.accent)).count();
        assert_eq!(circles, 1);
        let numbers = list.ops.iter().filter(|o| matches!(o, DrawOp::Text { text, .. } if text.parse::<u32>().is_ok_and(|d| d <= 31))).count();
        assert_eq!(numbers, 31 + 1); // 31 dias + o número grande do herói
        assert_eq!(list.size, calendar_size(5, 1.0));
    }

    #[test]
    fn other_month_has_no_today_circle() {
        let g = month_grid(2026, 11, 0, TODAY);
        let t = tokens();
        let list = calendar_paint(&g, &calendar_names(&PtBrFallback, 2026, 11, TODAY), 1.0, &t);
        assert!(!list.ops.iter().any(|o| matches!(o, DrawOp::Circle { color, .. } if *color == t.accent)));
    }
}
```

- [ ] **Step 2: Rodar para ver falhar**

Run: `cargo test -p zefiro-bar calendar`
Expected: FAIL de compilação.

- [ ] **Step 3: Implementar** (acima dos testes)

```rust
use zefiro_integration_clock::format::ClockText;
use zefiro_integration_clock::grid::{Date, MonthGrid};
use zefiro_platform::geometry::{Point, Rect, Size};
use zefiro_platform::text::{Align, Family, FontWeight, TextStyle};
use zefiro_resources::tokens::Tokens;
use zefiro_ui::paint::{DisplayList, DrawOp};

#[derive(Debug, Clone, PartialEq)]
pub struct CalendarNames { pub month: String, pub year: String, pub weekday_long: String, pub today_day: String, pub initials: [String; 7] }

pub fn calendar_names(text: &dyn ClockText, shown_year: i32, shown_month: u32, today: Date) -> CalendarNames {
    CalendarNames {
        month: text.month_name(shown_year, shown_month),
        year: shown_year.to_string(),
        weekday_long: text.weekday_long(today),
        today_day: today.day.to_string(),
        initials: text.weekday_initials(),
    }
}

pub fn calendar_size(weeks: usize, scale: f32) -> Size {
    Size { w: 352.0 * scale, h: (16.0 + 18.0 + weeks as f32 * 22.0 + 20.0) * scale }
}

pub fn calendar_paint(grid: &MonthGrid, names: &CalendarNames, scale: f32, tokens: &Tokens) -> DisplayList {
    let s = |v: f32| v * scale;
    let size = calendar_size(grid.weeks, scale);
    let mut l = DisplayList::new(size);
    let st = |px: f32, weight: FontWeight, family: Family| TextStyle { size: s(px), weight, family };

    // Herói.
    let hero = Rect::new(s(22.0), s(16.0), s(93.0), size.h - s(16.0) - s(20.0));
    l.push(DrawOp::RoundRect { rect: hero, radius: s(12.0), color: tokens.raised });
    l.push(DrawOp::Text { rect: Rect::new(hero.x + s(3.0), hero.y + s(6.0), hero.w - s(6.0), s(16.0)), text: names.month.clone(), style: st(12.0, FontWeight::Bold, Family::Text), color: tokens.accent, align: Align::Center });
    l.push(DrawOp::Text { rect: Rect::new(hero.x, hero.y + s(21.0), hero.w, s(15.0)), text: names.year.clone(), style: st(11.0, FontWeight::Regular, Family::Text), color: tokens.text_secondary, align: Align::Center });
    l.push(DrawOp::Text { rect: Rect::new(hero.x, hero.y + s(34.0), hero.w, hero.h - s(60.0)), text: names.today_day.clone(), style: st(34.0, FontWeight::SemiBold, Family::Display), color: tokens.text, align: Align::Center });
    l.push(DrawOp::Text { rect: Rect::new(hero.x + s(4.0), hero.bottom() - s(24.0), hero.w - s(8.0), s(18.0)), text: names.weekday_long.clone(), style: st(11.0, FontWeight::Bold, Family::Text), color: tokens.text_secondary, align: Align::Center });

    // Grade.
    let (x0, y0, col, header, row) = (s(134.0), s(16.0), s(28.0), s(18.0), s(22.0));
    for c in 0..7 {
        let weekday = (grid.first_weekday as usize + c) % 7;
        l.push(DrawOp::Text { rect: Rect::new(x0 + col * c as f32, y0, col, header), text: names.initials[weekday].clone(), style: st(11.0, FontWeight::SemiBold, Family::Text), color: tokens.text_tertiary, align: Align::Center });
    }
    for (i, cell) in grid.cells.iter().enumerate() {
        let Some(day) = cell else { continue };
        let rect = Rect::new(x0 + col * (i % 7) as f32, y0 + header + row * (i / 7) as f32, col, row);
        let is_today = grid.today == Some(*day);
        if is_today {
            l.push(DrawOp::Circle { center: Point { x: rect.center_x(), y: rect.y + row / 2.0 }, radius: s(11.0), color: tokens.accent });
        }
        let color = if is_today { tokens.fill.with_alpha(1.0) } else { tokens.text };
        let weight = if is_today { FontWeight::Bold } else { FontWeight::Regular };
        l.push(DrawOp::Text { rect, text: day.to_string(), style: st(12.0, weight, Family::Text), color, align: Align::Center });
    }
    l
}
```

Em `lib.rs`: `pub mod calendar;`. Em `THIRD_PARTY_NOTICES.md`, acrescentar à linha do Dynamic Island: "Layout do calendário em `crates/modules/bar/src/calendar.rs`."

- [ ] **Step 4: Rodar os testes**

Run: `cargo test -p zefiro-bar`
Expected: PASS (14 testes).

- [ ] **Step 5: Commit**

```powershell
git add crates/modules/bar THIRD_PARTY_NOTICES.md
git commit -m "feat(bar): month calendar panel in the island's calendar style" -m "Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

---

## Task 16: Regras dos painéis (`zefiro-bar`)

**Files:**
- Create: `crates/modules/bar/src/controller.rs`
- Modify: `crates/modules/bar/src/lib.rs` (`pub mod controller;`)

**Interfaces:**
- Consumes: nada além de tipos próprios.
- Produces: `controller::{PanelKind { Menu, Calendar }, OpenPanel { monitor: usize, chip: String, kind: PanelKind }, PanelInput { ChipClick { monitor: usize, chip: String }, ClickOutside, Escape, FocusLost, ConfigChanged }, PanelEffect { Open(OpenPanel), Close { monitor: usize } }, PanelController, panel_kind_for(chip: &str) -> Option<PanelKind>}` com `PanelController::{new(), handle(&mut self, PanelInput) -> Vec<PanelEffect>, open(&self) -> Option<&OpenPanel>}`.

Regras (spec §6.1): clicar num chip com painel abre; clicar no mesmo chip de novo fecha; clicar noutro chip fecha o atual **e** abre o novo no mesmo `handle` (o novo não espera); clique fora, Esc, perda de foco e mudança de configuração fecham. Chip sem painel não faz nada.

- [ ] **Step 1: Testes (falhando)**

`crates/modules/bar/src/controller.rs` (só os testes):

```rust
//! Quem abre e quem fecha: um painel por vez.

#[cfg(test)]
mod tests {
    use super::*;

    fn click(monitor: usize, chip: &str) -> PanelInput {
        PanelInput::ChipClick { monitor, chip: chip.into() }
    }

    #[test]
    fn click_opens_and_second_click_closes() {
        let mut c = PanelController::new();
        assert_eq!(c.handle(click(0, "clock")), vec![PanelEffect::Open(OpenPanel { monitor: 0, chip: "clock".into(), kind: PanelKind::Calendar })]);
        assert_eq!(c.handle(click(0, "clock")), vec![PanelEffect::Close { monitor: 0 }]);
        assert!(c.open().is_none());
    }

    #[test]
    fn opening_another_chip_closes_current_first() {
        let mut c = PanelController::new();
        c.handle(click(0, "clock"));
        assert_eq!(
            c.handle(click(1, "zefiro")),
            vec![PanelEffect::Close { monitor: 0 }, PanelEffect::Open(OpenPanel { monitor: 1, chip: "zefiro".into(), kind: PanelKind::Menu })]
        );
        assert_eq!(c.open().unwrap().chip, "zefiro");
    }

    #[test]
    fn same_chip_on_another_monitor_moves_the_panel() {
        let mut c = PanelController::new();
        c.handle(click(0, "clock"));
        assert_eq!(c.handle(click(1, "clock")).len(), 2);
        assert_eq!(c.open().unwrap().monitor, 1);
    }

    #[test]
    fn outside_escape_focus_lost_close() {
        for input in [PanelInput::ClickOutside, PanelInput::Escape, PanelInput::FocusLost] {
            let mut c = PanelController::new();
            c.handle(click(0, "zefiro"));
            assert_eq!(c.handle(input), vec![PanelEffect::Close { monitor: 0 }]);
        }
    }

    #[test]
    fn config_change_closes_open_panel() {
        let mut c = PanelController::new();
        c.handle(click(0, "zefiro"));
        assert_eq!(c.handle(PanelInput::ConfigChanged), vec![PanelEffect::Close { monitor: 0 }]);
        assert!(c.handle(PanelInput::ConfigChanged).is_empty());
    }

    #[test]
    fn unknown_chip_does_nothing() {
        let mut c = PanelController::new();
        assert!(c.handle(click(0, "desconhecido")).is_empty());
        assert_eq!(panel_kind_for("zefiro"), Some(PanelKind::Menu));
        assert_eq!(panel_kind_for("clock"), Some(PanelKind::Calendar));
    }
}
```

- [ ] **Step 2: Rodar para ver falhar**

Run: `cargo test -p zefiro-bar controller`
Expected: FAIL de compilação.

- [ ] **Step 3: Implementar** (acima dos testes)

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PanelKind { Menu, Calendar }

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OpenPanel { pub monitor: usize, pub chip: String, pub kind: PanelKind }

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PanelInput { ChipClick { monitor: usize, chip: String }, ClickOutside, Escape, FocusLost, ConfigChanged }

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PanelEffect { Open(OpenPanel), Close { monitor: usize } }

pub fn panel_kind_for(chip: &str) -> Option<PanelKind> {
    match chip {
        "zefiro" => Some(PanelKind::Menu),
        "clock" => Some(PanelKind::Calendar),
        _ => None,
    }
}

#[derive(Debug, Default)]
pub struct PanelController { open: Option<OpenPanel> }

impl PanelController {
    pub fn new() -> Self { Self::default() }

    pub fn open(&self) -> Option<&OpenPanel> { self.open.as_ref() }

    pub fn handle(&mut self, input: PanelInput) -> Vec<PanelEffect> {
        let mut fx = Vec::new();
        match input {
            PanelInput::ChipClick { monitor, chip } => {
                let Some(kind) = panel_kind_for(&chip) else { return fx };
                if let Some(current) = self.open.take() {
                    fx.push(PanelEffect::Close { monitor: current.monitor });
                    if current.monitor == monitor && current.chip == chip {
                        return fx;
                    }
                }
                let p = OpenPanel { monitor, chip, kind };
                fx.push(PanelEffect::Open(p.clone()));
                self.open = Some(p);
            }
            PanelInput::ClickOutside | PanelInput::Escape | PanelInput::FocusLost | PanelInput::ConfigChanged => {
                if let Some(current) = self.open.take() {
                    fx.push(PanelEffect::Close { monitor: current.monitor });
                }
            }
        }
        fx
    }
}
```

Em `lib.rs`: `pub mod controller;`.

- [ ] **Step 4: Rodar os testes**

Run: `cargo test -p zefiro-bar`
Expected: PASS (20 testes).

- [ ] **Step 5: Commit**

```powershell
git add crates/modules/bar
git commit -m "feat(bar): one-panel-at-a-time open/close rules" -m "Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

---

## Task 17: Hospedar a ilha (`zefiro-island-host`)

**Files:**
- Create: `crates/modules/island-host/src/{ini,policy,process}.rs`, `crates/modules/island-host/tests/process.rs`
- Modify: `crates/modules/island-host/Cargo.toml`, `crates/modules/island-host/src/lib.rs`

**Interfaces:**
- Consumes: Task 4 (`Theme`, `SpringStyle`, `BUILTIN`, `parse_theme`), Task 7 (`BarHeight`, `BarFormat`).
- Produces:
  - `ini::{IslandSettings { bar_height: u32, pill_height: u32, theme: IslandTheme, spring: SpringStyle, speed: f32 }, IslandTheme { Preset { index: usize, id: String }, Custom { background: Rgba, text: Rgba, text_secondary: Rgba, border: Option<Rgba> } }, island_pill_height(BarHeight, BarFormat) -> u32, settings_for(theme: &Theme, height: BarHeight, format: BarFormat) -> IslandSettings, apply_ini(text: &str, s: &IslandSettings) -> String, apply_state(text: &str, s: &IslandSettings) -> String}`.
  - `policy::{RestartPolicy, Decision { Restart, GiveUp }}` com `RestartPolicy::{new(max: usize, window: Duration), on_exit(&mut self, now: Instant) -> Decision, reset(&mut self)}`.
  - `process::{resolve_exe(configured: &str, beside: &Path) -> Option<PathBuf>, IslandProcess}` com `IslandProcess::{start(exe: &Path) -> io::Result<IslandProcess>, has_exited(&mut self) -> bool, stop(&mut self, timeout: Duration)}`.

Chaves que a barra controla (spec §7.3): `[Zefiro] Anchor=bar`, `BarHeight`, `PillHeight`; `[Themes] ThemePreset` (id do tema embutido, ou `custom`), `PillBgColor`, `TextPrimaryColor`, `TextSecondaryColor`, `ContourBorderHex`, `ContourBorderMode` (`default` ou `borderless`); `[Animations] AnimationStyle` (`smooth|default|bouncy|snappy`) e `AnimationSpeed` (o mais próximo entre `very-slow 0.5`, `slow 0.75`, `normal 1.0`, `fast 1.35`, `very-fast 1.65`, `ultra-fast 2.0`). No `island-state.ini`, `[Values] ColorThemeV2` = índice do tema embutido (0 a 7) ou 8 (custom). **O valor salvo pelo menu da ilha vence o `island.ini`**, por isso a barra grava os dois. Com um tema embutido, os 4 hex voltam aos padrões da ilha (`#08080A`, `#FFFFFF`, `#9B9BA5`, `#1E1E22`); senão a ilha trataria o hex antigo como personalização e ignoraria o tema. Um tema embutido **modificado** pelo usuário (arquivo de mesmo id) é enviado como `custom`.

- [ ] **Step 1: Dependências e testes (falhando)**

`crates/modules/island-host/Cargo.toml`:

```toml
[dependencies]
zefiro-core.workspace = true
zefiro-platform.workspace = true
zefiro-resources.workspace = true
tracing.workspace = true

[dev-dependencies]
tempfile.workspace = true
```

`crates/modules/island-host/src/lib.rs`:

```rust
//! Hospeda o zefiro-island.exe: island.ini, política de reinício e processo.

pub mod ini;
pub mod policy;
pub mod process;
```

`crates/modules/island-host/src/ini.rs` (só os testes):

```rust
//! Grava no island.ini / island-state.ini só as chaves que a barra controla.

#[cfg(test)]
mod tests {
    use super::*;
    use zefiro_core::config::{BarFormat, BarHeight};
    use zefiro_resources::theme::{BUILTIN, parse_theme};

    fn theme(id: &str) -> zefiro_resources::theme::Theme {
        let (_, t) = BUILTIN.iter().find(|(i, _)| *i == id).unwrap();
        parse_theme(id, id, t).unwrap()
    }

    #[test]
    fn pill_heights_follow_the_spec_table() {
        assert_eq!(island_pill_height(BarHeight::Completa, BarFormat::Faixa), 44);
        assert_eq!(island_pill_height(BarHeight::Compacta, BarFormat::Faixa), 28);
        assert_eq!(island_pill_height(BarHeight::Completa, BarFormat::Pilulas), 36);
        assert_eq!(island_pill_height(BarHeight::Compacta, BarFormat::Pilulas), 24);
    }

    #[test]
    fn preset_theme_writes_preset_and_resets_hex_fields() {
        let s = settings_for(&theme("nord"), BarHeight::Compacta, BarFormat::Pilulas);
        let input = "; comentário do usuário\n[Zefiro]\nAnchor=mod\nRenderer=software\n\n[Themes]\nThemePreset=obsidian\nPillBgColor=#123456\nFontFamily=Aptos\n";
        let out = apply_ini(input, &s);
        assert!(out.contains("; comentário do usuário"));
        assert!(out.contains("Renderer=software"));
        assert!(out.contains("FontFamily=Aptos"));
        assert!(out.contains("Anchor=bar") && !out.contains("Anchor=mod"));
        assert!(out.contains("BarHeight=32"));
        assert!(out.contains("PillHeight=24"));
        assert!(out.contains("ThemePreset=nord"));
        assert!(out.contains("PillBgColor=#08080A"));
        assert!(out.contains("[Animations]\r\nAnimationStyle=default\r\nAnimationSpeed=normal"), "{out}");
    }

    #[test]
    fn custom_theme_writes_its_colors() {
        let mut t = theme("obsidian");
        t.id = "meu".into();
        t.colors.background = zefiro_platform::color::Rgba::rgb(0x10, 0x20, 0x30);
        t.colors.border = None;
        let out = apply_ini("", &settings_for(&t, BarHeight::Completa, BarFormat::Faixa));
        assert!(out.contains("ThemePreset=custom"));
        assert!(out.contains("PillBgColor=#102030"));
        assert!(out.contains("ContourBorderMode=borderless"));
    }

    #[test]
    fn modified_builtin_is_sent_as_custom() {
        let mut t = theme("nord");
        t.colors.text = zefiro_platform::color::Rgba::rgb(1, 2, 3);
        assert!(matches!(settings_for(&t, BarHeight::Completa, BarFormat::Faixa).theme, IslandTheme::Custom { .. }));
    }

    #[test]
    fn state_file_gets_the_theme_index_and_keeps_other_values() {
        let s = settings_for(&theme("plum"), BarHeight::Completa, BarFormat::Faixa);
        let out = apply_state("[Values]\r\nColorThemeV2=0\r\nPinnedExpanded=1\r\n", &s);
        assert!(out.contains("ColorThemeV2=6"));
        assert!(out.contains("PinnedExpanded=1"));
        let empty = apply_state("", &s);
        assert!(empty.contains("[Values]") && empty.contains("ColorThemeV2=6"));
    }

    #[test]
    fn speed_maps_to_the_nearest_island_option() {
        let mut t = theme("obsidian");
        t.animation.speed = 1.5;
        let out = apply_ini("", &settings_for(&t, BarHeight::Completa, BarFormat::Faixa));
        assert!(out.contains("AnimationSpeed=very-fast"), "{out}");
    }
}
```

`crates/modules/island-host/src/policy.rs` (só os testes):

```rust
//! Até `max` reinícios dentro de `window`; depois desiste.

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{Duration, Instant};

    #[test]
    fn restart_policy_allows_three_in_a_minute() {
        let mut p = RestartPolicy::new(3, Duration::from_secs(60));
        let t0 = Instant::now();
        for i in 0..3 {
            assert_eq!(p.on_exit(t0 + Duration::from_secs(i)), Decision::Restart);
        }
    }

    #[test]
    fn restart_policy_gives_up_on_fourth() {
        let mut p = RestartPolicy::new(3, Duration::from_secs(60));
        let t0 = Instant::now();
        for i in 0..3 {
            p.on_exit(t0 + Duration::from_secs(i));
        }
        assert_eq!(p.on_exit(t0 + Duration::from_secs(10)), Decision::GiveUp);
        assert_eq!(p.on_exit(t0 + Duration::from_secs(500)), Decision::GiveUp); // desistiu até reset()
    }

    #[test]
    fn restart_policy_window_slides() {
        let mut p = RestartPolicy::new(3, Duration::from_secs(60));
        let t0 = Instant::now();
        for i in 0..3 {
            p.on_exit(t0 + Duration::from_secs(i * 25)); // 0, 25, 50
        }
        assert_eq!(p.on_exit(t0 + Duration::from_secs(61)), Decision::Restart); // o de 0 s saiu da janela
    }

    #[test]
    fn restart_policy_reset_after_manual_restart() {
        let mut p = RestartPolicy::new(3, Duration::from_secs(60));
        let t0 = Instant::now();
        for i in 0..4 {
            p.on_exit(t0 + Duration::from_secs(i));
        }
        p.reset();
        assert_eq!(p.on_exit(t0 + Duration::from_secs(5)), Decision::Restart);
    }
}
```

`crates/modules/island-host/tests/process.rs`:

```rust
use std::path::Path;
use zefiro_island_host::process::resolve_exe;

#[test]
fn resolves_configured_path_or_beside_the_bar() {
    let dir = tempfile::tempdir().unwrap();
    assert_eq!(resolve_exe("", dir.path()), None);
    let beside = dir.path().join("zefiro-island.exe");
    std::fs::write(&beside, b"").unwrap();
    assert_eq!(resolve_exe("", dir.path()), Some(beside.clone()));
    let other = dir.path().join("outra.exe");
    std::fs::write(&other, b"").unwrap();
    assert_eq!(resolve_exe(other.to_str().unwrap(), Path::new("C:\\nao-existe")), Some(other));
    assert_eq!(resolve_exe("C:\\nao-existe\\x.exe", dir.path()), None);
}
```

- [ ] **Step 2: Rodar para ver falhar**

Run: `cargo test -p zefiro-island-host`
Expected: FAIL de compilação.

- [ ] **Step 3: Implementar `ini.rs`** (acima dos testes)

```rust
use zefiro_core::config::{BarFormat, BarHeight};
use zefiro_platform::color::Rgba;
use zefiro_resources::theme::{BUILTIN, SpringStyle, Theme, parse_theme};

#[derive(Debug, Clone, PartialEq)]
pub enum IslandTheme {
    Preset { index: usize, id: String },
    Custom { background: Rgba, text: Rgba, text_secondary: Rgba, border: Option<Rgba> },
}

#[derive(Debug, Clone, PartialEq)]
pub struct IslandSettings { pub bar_height: u32, pub pill_height: u32, pub theme: IslandTheme, pub spring: SpringStyle, pub speed: f32 }

pub fn island_pill_height(height: BarHeight, format: BarFormat) -> u32 {
    match (height, format) {
        (BarHeight::Completa, BarFormat::Faixa) => 44,
        (BarHeight::Compacta, BarFormat::Faixa) => 28,
        (BarHeight::Completa, BarFormat::Pilulas) => 36,
        (BarHeight::Compacta, BarFormat::Pilulas) => 24,
    }
}

pub fn settings_for(theme: &Theme, height: BarHeight, format: BarFormat) -> IslandSettings {
    let builtin = BUILTIN.iter().position(|(id, _)| *id == theme.id);
    let pristine = builtin.is_some_and(|i| parse_theme(BUILTIN[i].0, BUILTIN[i].0, BUILTIN[i].1).ok().is_some_and(|b| b.colors == theme.colors));
    let island_theme = match builtin {
        Some(index) if pristine => IslandTheme::Preset { index, id: theme.id.clone() },
        _ => IslandTheme::Custom {
            background: theme.colors.background,
            text: theme.colors.text,
            text_secondary: theme.colors.text_secondary,
            border: theme.colors.border,
        },
    };
    IslandSettings {
        bar_height: if height == BarHeight::Completa { 44 } else { 32 },
        pill_height: island_pill_height(height, format),
        theme: island_theme,
        spring: theme.animation.spring,
        speed: theme.animation.speed,
    }
}

fn hex(c: Rgba) -> String {
    let b = |v: f32| (v.clamp(0.0, 1.0) * 255.0).round() as u8;
    format!("#{:02X}{:02X}{:02X}", b(c.r), b(c.g), b(c.b))
}

fn speed_name(v: f32) -> &'static str {
    const OPTS: [(f32, &str); 6] = [(0.5, "very-slow"), (0.75, "slow"), (1.0, "normal"), (1.35, "fast"), (1.65, "very-fast"), (2.0, "ultra-fast")];
    OPTS.iter().min_by(|a, b| (a.0 - v).abs().total_cmp(&(b.0 - v).abs())).map_or("normal", |o| o.1)
}

fn spring_name(s: SpringStyle) -> &'static str {
    match s { SpringStyle::Smooth => "smooth", SpringStyle::Default => "default", SpringStyle::Bouncy => "bouncy", SpringStyle::Snappy => "snappy" }
}

/// Troca/insere `key=value` dentro de `[section]`, preservando o resto (comentários, outras chaves, ordem).
fn set(lines: &mut Vec<String>, section: &str, key: &str, value: &str) {
    let header = format!("[{section}]");
    let start = match lines.iter().position(|l| l.trim().eq_ignore_ascii_case(&header)) {
        Some(i) => i,
        None => {
            if lines.last().is_some_and(|l| !l.trim().is_empty()) {
                lines.push(String::new());
            }
            lines.push(header);
            lines.len() - 1
        }
    };
    let end = lines[start + 1..].iter().position(|l| l.trim_start().starts_with('[')).map_or(lines.len(), |p| start + 1 + p);
    let line = format!("{key}={value}");
    for l in &mut lines[start + 1..end] {
        let t = l.trim_start();
        let is_key = !t.starts_with(';') && !t.starts_with('#') && t.split('=').next().is_some_and(|k| k.trim().eq_ignore_ascii_case(key));
        if is_key {
            *l = line;
            return;
        }
    }
    let mut at = end;
    while at > start + 1 && lines[at - 1].trim().is_empty() {
        at -= 1;
    }
    lines.insert(at, line);
}

fn join(lines: Vec<String>) -> String {
    let mut s = lines.join("\r\n");
    s.push_str("\r\n");
    s
}

pub fn apply_ini(text: &str, s: &IslandSettings) -> String {
    let mut l: Vec<String> = text.lines().map(str::to_string).collect();
    set(&mut l, "Zefiro", "Anchor", "bar");
    set(&mut l, "Zefiro", "BarHeight", &s.bar_height.to_string());
    set(&mut l, "Zefiro", "PillHeight", &s.pill_height.to_string());
    match &s.theme {
        IslandTheme::Preset { id, .. } => {
            set(&mut l, "Themes", "ThemePreset", id);
            set(&mut l, "Themes", "PillBgColor", "#08080A");
            set(&mut l, "Themes", "TextPrimaryColor", "#FFFFFF");
            set(&mut l, "Themes", "TextSecondaryColor", "#9B9BA5");
            set(&mut l, "Themes", "ContourBorderHex", "#1E1E22");
            set(&mut l, "Themes", "ContourBorderMode", "default");
        }
        IslandTheme::Custom { background, text, text_secondary, border } => {
            set(&mut l, "Themes", "ThemePreset", "custom");
            set(&mut l, "Themes", "PillBgColor", &hex(*background));
            set(&mut l, "Themes", "TextPrimaryColor", &hex(*text));
            set(&mut l, "Themes", "TextSecondaryColor", &hex(*text_secondary));
            match border {
                Some(b) => {
                    set(&mut l, "Themes", "ContourBorderHex", &hex(*b));
                    set(&mut l, "Themes", "ContourBorderMode", "default");
                }
                None => set(&mut l, "Themes", "ContourBorderMode", "borderless"),
            }
        }
    }
    set(&mut l, "Animations", "AnimationStyle", spring_name(s.spring));
    set(&mut l, "Animations", "AnimationSpeed", speed_name(s.speed));
    join(l)
}

pub fn apply_state(text: &str, s: &IslandSettings) -> String {
    let index = match &s.theme { IslandTheme::Preset { index, .. } => *index, IslandTheme::Custom { .. } => BUILTIN.len() };
    let mut l: Vec<String> = text.lines().map(str::to_string).collect();
    set(&mut l, "Values", "ColorThemeV2", &index.to_string());
    join(l)
}
```

- [ ] **Step 4: Implementar `policy.rs`** (acima dos testes)

```rust
use std::collections::VecDeque;
use std::time::{Duration, Instant};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Decision { Restart, GiveUp }

#[derive(Debug)]
pub struct RestartPolicy { max: usize, window: Duration, restarts: VecDeque<Instant>, gave_up: bool }

impl RestartPolicy {
    pub fn new(max: usize, window: Duration) -> Self {
        Self { max, window, restarts: VecDeque::new(), gave_up: false }
    }

    /// A ilha saiu sem ser pedida. Reinicia se houve menos de `max` reinícios dentro de `window`.
    pub fn on_exit(&mut self, now: Instant) -> Decision {
        if self.gave_up {
            return Decision::GiveUp;
        }
        while self.restarts.front().is_some_and(|t| now.duration_since(*t) > self.window) {
            self.restarts.pop_front();
        }
        if self.restarts.len() >= self.max {
            self.gave_up = true;
            return Decision::GiveUp;
        }
        self.restarts.push_back(now);
        Decision::Restart
    }

    /// "Reiniciar a ilha" no menu: começa a contagem de novo.
    pub fn reset(&mut self) {
        self.restarts.clear();
        self.gave_up = false;
    }
}
```

- [ ] **Step 5: Implementar `process.rs`**

```rust
//! Processo do zefiro-island.exe (std::process; sem a crate windows).

use std::path::{Path, PathBuf};
use std::process::{Child, Command};
use std::time::{Duration, Instant};

pub fn resolve_exe(configured: &str, beside: &Path) -> Option<PathBuf> {
    let candidate = if configured.is_empty() { beside.join("zefiro-island.exe") } else { PathBuf::from(configured) };
    candidate.is_file().then_some(candidate)
}

pub struct IslandProcess { exe: PathBuf, child: Option<Child> }

impl IslandProcess {
    /// Para uma instância anterior (a ilha só aceita uma) e inicia outra.
    pub fn start(exe: &Path) -> std::io::Result<IslandProcess> {
        let _ = Command::new(exe).arg("--stop").status();
        std::thread::sleep(Duration::from_millis(300));
        let child = Command::new(exe).spawn()?;
        tracing::info!(pid = child.id(), exe = %exe.display(), "ilha iniciada");
        Ok(IslandProcess { exe: exe.to_path_buf(), child: Some(child) })
    }

    pub fn has_exited(&mut self) -> bool {
        match self.child.as_mut() {
            None => true,
            Some(c) => !matches!(c.try_wait(), Ok(None)),
        }
    }

    /// Pede para fechar (`--stop`), espera até `timeout` e, se não fechar, encerra o processo.
    pub fn stop(&mut self, timeout: Duration) {
        let Some(mut child) = self.child.take() else { return };
        let _ = Command::new(&self.exe).arg("--stop").status();
        let deadline = Instant::now() + timeout;
        while Instant::now() < deadline {
            if !matches!(child.try_wait(), Ok(None)) {
                return;
            }
            std::thread::sleep(Duration::from_millis(50));
        }
        tracing::warn!("a ilha não fechou em {timeout:?}; encerrando");
        let _ = child.kill();
        let _ = child.wait();
    }
}
```

- [ ] **Step 6: Rodar os testes**

Run: `cargo test -p zefiro-island-host`
Expected: PASS (11 testes).

- [ ] **Step 7: Commit**

```powershell
git add crates/modules/island-host Cargo.lock
git commit -m "feat(island-host): island.ini/state sync, restart policy and process control" -m "Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

---

## Task 18: Pílula recolhida da ilha com `PillHeight` (fork C++)

**Files:**
- Modify: `island/wh_shim.h`, `island/wh_shim.cpp`, `island/dynamic-island-for-windows.wh.cpp`, `island/README.md`

**Interfaces:**
- Consumes: o fork da ilha (ADR-0006). A Task 17 grava `[Zefiro] PillHeight`.
- Produces: `zefiro::CollapsedScale(float designHeight, float sizeScale) -> float` (no shim). A pílula recolhida de mídia (44 px de desenho) e a ociosa (36 px) passam a ter `PillHeight` DIP; o painel expandido continua com `SizeScale`. `zefiro::LoadSettingsFile()` força reler o `island-state.ini`.

**Limite (spec §7.4):** se o diff do `.wh.cpp` desta tarefa passar de ~150 linhas, pare, desfaça as mudanças no `.wh.cpp`, aplique o plano B (só o shim: `PillHeight` centraliza a pílula de 44 px) e registre no ADR-0007.

- [ ] **Step 1: Shim**

Em `island/wh_shim.cpp`, no array `kZefiroDefaults`, acrescentar depois de `PillOffsetY`:

```cpp
    {L"Zefiro", L"PillHeight", L"0",
     L"Altura da pílula recolhida em DIP (0 = a do mod). Gravada pela barra do Zefiro conforme a altura e o formato", L""},
```

Em `island/wh_shim.h`, dentro de `namespace zefiro`, depois de `UseSoftwareRenderer()`:

```cpp
// Escala da pílula recolhida. Com [Zefiro] PillHeight > 0, a pílula de
// `designHeight` px de desenho (44 na mídia, 36 na ociosa) passa a ter
// PillHeight DIP; senão devolve `sizeScale` (comportamento do mod).
float CollapsedScale(float designHeight, float sizeScale);
```

Em `island/wh_shim.cpp`, antes de `bool UseSoftwareRenderer()`:

```cpp
float CollapsedScale(float designHeight, float sizeScale) {
    const int pill = GetZefiroInt(L"PillHeight", 0);
    if (pill <= 0 || designHeight <= 0.0f) return sizeScale;
    UINT dpiX = 96, dpiY = 96;
    GetDpiForMonitor(MonitorFromPoint(POINT{0, 0}, MONITOR_DEFAULTTOPRIMARY), MDT_EFFECTIVE_DPI, &dpiX, &dpiY);
    return static_cast<float>(pill) * (static_cast<float>(dpiY) / 96.0f) / designHeight;
}
```

No `BarAnchorRect`, trocar `const float pill = kCollapsedPillHeight * sizeScale;` por:

```cpp
    const float pill = kCollapsedPillHeight * CollapsedScale(kCollapsedPillHeight, sizeScale);
```

No `LoadValuesLocked`, tornar a próxima checagem forçável: apagar a linha `static ULONGLONG s_nextCheck = 0;` de dentro da função, declarar `ULONGLONG g_valuesNextCheck = 0;` no namespace anônimo, logo depois de `FILETIME g_valuesWriteTime{};`, e trocar os dois usos de `s_nextCheck` por `g_valuesNextCheck`. No fim de `zefiro::LoadSettingsFile()`, antes do `return ok;`:

```cpp
    {
        // A barra grava o tema no island.ini e no island-state.ini juntos; relê o segundo já.
        std::lock_guard valuesLock(g_valuesMutex);
        g_valuesNextCheck = 0;
    }
```

- [ ] **Step 2: Mod — tamanho das pílulas recolhidas**

Logo antes de `Activity ActivityForKind(`, acrescentar:

```cpp
#ifdef ZEFIRO_ISLAND
// ZEFIRO: escala da pílula recolhida ([Zefiro] PillHeight); o resto continua com SizeScale.
#define ZEFIRO_COLLAPSED_SCALE(design, sizeScale) zefiro::CollapsedScale((design), (sizeScale))
#else
#define ZEFIRO_COLLAPSED_SCALE(design, sizeScale) (sizeScale)
#endif
```

No fim de `ActivityForKind`, trocar:

```cpp
    activity.width *= settings.sizeScale;
    activity.height *= settings.sizeScale;
    return activity;
```

por:

```cpp
    // ZEFIRO: pílulas recolhidas de mídia e ociosa usam a escala recolhida.
    const float scale = (kind == IslandKind::Media)  ? ZEFIRO_COLLAPSED_SCALE(44.0f, settings.sizeScale)
                      : (kind == IslandKind::Idle)   ? ZEFIRO_COLLAPSED_SCALE(IdleStripLayout::kHeight, settings.sizeScale)
                                                     : settings.sizeScale;
    activity.width *= scale;
    activity.height *= scale;
    return activity;
```

No laço de desenho, nas três larguras recolhidas calculadas fora do `ActivityForKind`, trocar o `g_settings.sizeScale` final:
- `primary.width = (kCollapsedLyricsWidth + clockSection) * g_settings.sizeScale;` → `* ZEFIRO_COLLAPSED_SCALE(44.0f, g_settings.sizeScale);`
- `primary.width = (MediaPillLayout::kBaseWidth + clockSection) * g_settings.sizeScale;` → `* ZEFIRO_COLLAPSED_SCALE(44.0f, g_settings.sizeScale);`
- `renderer.MeasureIdleStrip(snapshot, g_settings, now).totalWidth *` / `g_settings.sizeScale;` → `ZEFIRO_COLLAPSED_SCALE(IdleStripLayout::kHeight, g_settings.sizeScale);`

Marcar cada uma com `// ZEFIRO:` no fim da linha.

- [ ] **Step 3: Mod — escala do conteúdo durante a transformação**

Em `DrawPill`, trocar:

```cpp
        target_->SetTransform(D2D1::Matrix3x2F::Scale(settings.sizeScale, settings.sizeScale, pillCenter) * oldTransform);
        float invScale = 1.0f / settings.sizeScale;
```

por:

```cpp
        float contentScale = settings.sizeScale;
#ifdef ZEFIRO_ISLAND
        // ZEFIRO: o conteúdo vai da escala recolhida à expandida junto com a altura da pílula.
        if (activity.kind == IslandKind::Media || activity.kind == IslandKind::Idle) {
            const float design = activity.kind == IslandKind::Media ? 44.0f : IdleStripLayout::kHeight;
            const float collapsedScale = zefiro::CollapsedScale(design, settings.sizeScale);
            const float collapsedH = design * collapsedScale;
            const float expandedH = MediaLayout::kExpandedHeight * settings.sizeScale;
            const float t = expandedH > collapsedH
                                ? Clamp(((rect.bottom - rect.top) - collapsedH) / (expandedH - collapsedH), 0.0f, 1.0f)
                                : 1.0f;
            contentScale = collapsedScale + (settings.sizeScale - collapsedScale) * t;
        }
#endif
        target_->SetTransform(D2D1::Matrix3x2F::Scale(contentScale, contentScale, pillCenter) * oldTransform);
        float invScale = 1.0f / contentScale;
```

- [ ] **Step 4: Compilar e conferir o tamanho do diff**

Run: `./island/build.ps1; git diff --stat -- island/dynamic-island-for-windows.wh.cpp`
Expected: compila sem avisos; o diff fica bem abaixo de 150 linhas.

- [ ] **Step 5: Verificação visual**

Com o YASB fechado: no `%APPDATA%\zefiro\island.ini`, seção `[Zefiro]`, gravar `PillHeight=24` e salvar (a ilha relê sozinha). Abrir `island/build/zefiro-island.exe`.
Expected: a pílula recolhida fica com ~24 px de altura, centrada nos 32 DIP do topo; ao clicar, o painel expandido abre com o tamanho normal (380 × 184 a 100 %), sem salto no conteúdo durante a transformação. Com `PillHeight=0`, volta ao tamanho do mod.

- [ ] **Step 6: Documentar e commitar**

Em `island/README.md`, na tabela da seção `[Zefiro]`, acrescentar:

```markdown
| `PillHeight` | `0` | Altura da pílula recolhida em DIP. A barra grava 44/28 (faixa) ou 36/24 (pílulas). `0` = tamanho do mod. O painel expandido não muda. |
```

E em "Acompanhar o upstream", atualizar a lista de blocos `// ZEFIRO:` para incluir a pílula recolhida (`ActivityForKind`, larguras recolhidas no laço de desenho, `DrawPill`), as setas e a saída de som.

```powershell
git add island
git commit -m "feat(island): collapsed pill follows the bar height via [Zefiro] PillHeight" -m "Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

---

## Task 19: Direct2D em WARP, DirectWrite e o desenhista (`zefiro-platform-windows`)

**Files:**
- Create: `crates/platform-windows/src/{gfx,text,painter}.rs`
- Modify: `crates/platform-windows/Cargo.toml`, `crates/platform-windows/src/lib.rs`

**Interfaces:**
- Consumes: Task 3 (`TextMeasurer`, `TextStyle`, `Rgba`), Task 10 (`DisplayList`, `DrawOp`).
- Produces:
  - `gfx::{Gfx { d3d: ID3D11Device, d2d_factory: ID2D1Factory1, d2d_device: ID2D1Device, dwrite: IDWriteFactory }, Gfx::new() -> windows::core::Result<Gfx>}`: dispositivo D3D11 **WARP** sempre (ADR-0001).
  - `text::DWriteText` com `DWriteText::new(dwrite: IDWriteFactory, family: &str)`, `set_family(&self, family: &str)`, `format(&self, style: &TextStyle, align: Align) -> windows::core::Result<IDWriteTextFormat>` (com cache) e `impl TextMeasurer`.
  - `painter::{paint(dc: &ID2D1DeviceContext, list: &DisplayList, text: &DWriteText) -> windows::core::Result<()>, d2d_color(Rgba) -> D2D1_COLOR_F}`.

- [ ] **Step 1: Dependências**

`crates/platform-windows/Cargo.toml`:

```toml
[dependencies]
zefiro-platform.workspace = true
zefiro-resources.workspace = true
zefiro-ui.workspace = true
windows.workspace = true
windows-core.workspace = true
windows-numerics.workspace = true
tracing.workspace = true
```

`crates/platform-windows/src/lib.rs`:

```rust
//! Implementação Windows: Direct2D (WARP), Windows.UI.Composition, janelas e AppBar.
#![cfg(windows)]

pub mod gfx;
pub mod painter;
pub mod text;
```

- [ ] **Step 2: Teste (falhando)**

`crates/platform-windows/src/painter.rs` (só o teste por enquanto):

```rust
//! Desenha uma `DisplayList` num `ID2D1DeviceContext`.

#[cfg(test)]
mod tests {
    use super::*;
    use crate::gfx::Gfx;
    use crate::text::DWriteText;
    use windows::Win32::Graphics::Direct2D::Common::*;
    use windows::Win32::Graphics::Direct2D::*;
    use windows::Win32::Graphics::Dxgi::Common::DXGI_FORMAT_B8G8R8A8_UNORM;
    use zefiro_platform::color::Rgba;
    use zefiro_platform::geometry::{Rect, Size};
    use zefiro_platform::text::{Align, Family, FontWeight, TextMeasurer, TextStyle};
    use zefiro_ui::paint::{DisplayList, DrawOp};

    fn props(options: D2D1_BITMAP_OPTIONS) -> D2D1_BITMAP_PROPERTIES1 {
        D2D1_BITMAP_PROPERTIES1 {
            pixelFormat: D2D1_PIXEL_FORMAT { format: DXGI_FORMAT_B8G8R8A8_UNORM, alphaMode: D2D1_ALPHA_MODE_PREMULTIPLIED },
            dpiX: 96.0,
            dpiY: 96.0,
            bitmapOptions: options,
            ..Default::default()
        }
    }

    #[test]
    fn paints_a_round_rect_and_measures_text_on_warp() {
        let gfx = Gfx::new().unwrap();
        let text = DWriteText::new(gfx.dwrite.clone(), "Segoe UI Variable");
        let style = TextStyle { size: 13.0, weight: FontWeight::SemiBold, family: Family::Text };
        assert!(text.measure("qui 15 15:47", &style).w > 40.0);

        let mut list = DisplayList::new(Size { w: 64.0, h: 32.0 });
        list.push(DrawOp::RoundRect { rect: Rect::new(0.0, 0.0, 64.0, 32.0), radius: 16.0, color: Rgba::rgb(255, 0, 0) });
        list.push(DrawOp::Text { rect: Rect::new(0.0, 0.0, 64.0, 32.0), text: "Z".into(), style, color: Rgba::WHITE, align: Align::Center });

        // SAFETY: chamadas padrão de Direct2D em objetos recém-criados; ponteiros locais válidos.
        unsafe {
            let dc = gfx.d2d_device.CreateDeviceContext(D2D1_DEVICE_CONTEXT_OPTIONS_NONE).unwrap();
            let size = D2D_SIZE_U { width: 64, height: 32 };
            let target = dc.CreateBitmap2(size, None, 0, &props(D2D1_BITMAP_OPTIONS_TARGET | D2D1_BITMAP_OPTIONS_CANNOT_DRAW)).unwrap();
            dc.SetTarget(&target);
            dc.BeginDraw();
            dc.Clear(Some(&D2D1_COLOR_F::default()));
            paint(&dc, &list, &text).unwrap();
            dc.EndDraw(None, None).unwrap();

            let read = dc.CreateBitmap2(size, None, 0, &props(D2D1_BITMAP_OPTIONS_CPU_READ | D2D1_BITMAP_OPTIONS_CANNOT_DRAW)).unwrap();
            read.CopyFromBitmap(None, &target, None).unwrap();
            let map = read.Map(D2D1_MAP_OPTIONS_READ).unwrap();
            let px = |x: usize, y: usize| std::slice::from_raw_parts(map.bits.add(y * map.pitch as usize + x * 4), 4).to_vec();
            assert_eq!(px(8, 16), vec![0, 0, 255, 255]); // BGRA: vermelho opaco dentro da pílula
            assert_eq!(px(0, 0)[3], 0); // canto fora do raio: transparente
            read.Unmap().unwrap();
        }
    }
}
```

Run: `cargo test -p zefiro-platform-windows`
Expected: FAIL de compilação.

- [ ] **Step 3: Implementar `gfx.rs`**

```rust
//! Dispositivo de desenho único: D3D11 em WARP + Direct2D + DirectWrite (ADR-0001).

use windows::Win32::Foundation::HMODULE;
use windows::Win32::Graphics::Direct2D::*;
use windows::Win32::Graphics::Direct3D::D3D_DRIVER_TYPE_WARP;
use windows::Win32::Graphics::Direct3D11::*;
use windows::Win32::Graphics::DirectWrite::*;
use windows::Win32::Graphics::Dxgi::IDXGIDevice;
use windows::core::{Interface, Result};

pub struct Gfx {
    pub d3d: ID3D11Device,
    pub d2d_factory: ID2D1Factory1,
    pub d2d_device: ID2D1Device,
    pub dwrite: IDWriteFactory,
}

impl Gfx {
    pub fn new() -> Result<Gfx> {
        // SAFETY: criação padrão de dispositivos; ponteiros de saída locais.
        unsafe {
            let mut d3d = None;
            D3D11CreateDevice(None, D3D_DRIVER_TYPE_WARP, HMODULE::default(), D3D11_CREATE_DEVICE_BGRA_SUPPORT, None, D3D11_SDK_VERSION, Some(&mut d3d), None, None)?;
            let d3d = d3d.expect("D3D11CreateDevice devolveu sucesso sem dispositivo");
            let dxgi: IDXGIDevice = d3d.cast()?;
            let d2d_factory: ID2D1Factory1 = D2D1CreateFactory(D2D1_FACTORY_TYPE_SINGLE_THREADED, None)?;
            let d2d_device = d2d_factory.CreateDevice(&dxgi)?;
            let dwrite: IDWriteFactory = DWriteCreateFactory(DWRITE_FACTORY_TYPE_SHARED)?;
            Ok(Gfx { d3d, d2d_factory, d2d_device, dwrite })
        }
    }
}
```

- [ ] **Step 4: Implementar `text.rs`**

```rust
//! Formatos de texto do DirectWrite (com cache) e medição.

use std::cell::RefCell;
use windows::Win32::Graphics::DirectWrite::*;
use windows::core::{HSTRING, Result, w};
use zefiro_platform::geometry::Size;
use zefiro_platform::text::{Align, Family, FontWeight, TextMeasurer, TextStyle};

type Key = (String, u32, u8, u8);

pub struct DWriteText {
    dwrite: IDWriteFactory,
    family: RefCell<String>,
    cache: RefCell<HashMap<Key, IDWriteTextFormat>>,
}

impl DWriteText {
    pub fn new(dwrite: IDWriteFactory, family: &str) -> DWriteText {
        DWriteText { dwrite, family: RefCell::new(family.to_string()), cache: RefCell::new(HashMap::new()) }
    }

    /// Troca a família do tema; limpa o cache.
    pub fn set_family(&self, family: &str) {
        *self.family.borrow_mut() = family.to_string();
        self.cache.borrow_mut().clear();
    }

    fn face(&self, family: Family) -> String {
        let base = self.family.borrow().clone();
        if base == "Segoe UI Variable" {
            match family { Family::Text => "Segoe UI Variable Text".into(), Family::Display => "Segoe UI Variable Display".into() }
        } else {
            base
        }
    }

    pub fn format(&self, style: &TextStyle, align: Align) -> Result<IDWriteTextFormat> {
        let face = self.face(style.family);
        let key = (face.clone(), (style.size * 100.0) as u32, style.weight as u8, align as u8);
        if let Some(f) = self.cache.borrow().get(&key) {
            return Ok(f.clone());
        }
        let weight = match style.weight {
            FontWeight::Regular => DWRITE_FONT_WEIGHT_NORMAL,
            FontWeight::SemiBold => DWRITE_FONT_WEIGHT_SEMI_BOLD,
            FontWeight::Bold => DWRITE_FONT_WEIGHT_BOLD,
        };
        // SAFETY: strings válidas durante a chamada; formato criado e configurado antes do uso.
        let f = unsafe {
            let f = self.dwrite.CreateTextFormat(&HSTRING::from(face), None, weight, DWRITE_FONT_STYLE_NORMAL, DWRITE_FONT_STRETCH_NORMAL, style.size, w!("pt-BR"))?;
            f.SetTextAlignment(match align {
                Align::Leading => DWRITE_TEXT_ALIGNMENT_LEADING,
                Align::Center => DWRITE_TEXT_ALIGNMENT_CENTER,
                Align::Trailing => DWRITE_TEXT_ALIGNMENT_TRAILING,
            })?;
            f.SetParagraphAlignment(DWRITE_PARAGRAPH_ALIGNMENT_CENTER)?;
            f.SetWordWrapping(DWRITE_WORD_WRAPPING_NO_WRAP)?;
            f
        };
        self.cache.borrow_mut().insert(key, f.clone());
        Ok(f)
    }
}

impl TextMeasurer for DWriteText {
    fn measure(&self, text: &str, style: &TextStyle) -> Size {
        let utf16: Vec<u16> = text.encode_utf16().collect();
        let Ok(format) = self.format(style, Align::Leading) else { return Size::default() };
        // SAFETY: buffer UTF-16 local; métricas gravadas numa struct local.
        unsafe {
            let Ok(layout) = self.dwrite.CreateTextLayout(&utf16, &format, 100_000.0, 100_000.0) else { return Size::default() };
            let mut m = DWRITE_TEXT_METRICS::default();
            if layout.GetMetrics(&mut m).is_err() {
                return Size::default();
            }
            Size { w: m.widthIncludingTrailingWhitespace, h: m.height }
        }
    }
}
```

- [ ] **Step 5: Implementar `painter.rs`** (acima do teste)

```rust
use windows::Win32::Graphics::Direct2D::Common::*;
use windows::Win32::Graphics::Direct2D::*;
use windows::Win32::Graphics::DirectWrite::DWRITE_MEASURING_MODE_NATURAL;
use windows::core::Result;
use windows_numerics::Vector2;
use zefiro_platform::color::Rgba;
use zefiro_platform::geometry::Rect;
use zefiro_ui::paint::{DisplayList, DrawOp};

use crate::text::DWriteText;

pub fn d2d_color(c: Rgba) -> D2D1_COLOR_F {
    D2D1_COLOR_F { r: c.r, g: c.g, b: c.b, a: c.a }
}

fn d2d_rect(r: Rect) -> D2D_RECT_F {
    D2D_RECT_F { left: r.x, top: r.y, right: r.right(), bottom: r.bottom() }
}

pub fn paint(dc: &ID2D1DeviceContext, list: &DisplayList, text: &DWriteText) -> Result<()> {
    // SAFETY: `dc` está entre BeginDraw/EndDraw (garantido por quem chama); objetos locais.
    unsafe {
        dc.SetTextAntialiasMode(D2D1_TEXT_ANTIALIAS_MODE_GRAYSCALE);
        for op in &list.ops {
            match op {
                DrawOp::RoundRect { rect, radius, color } => {
                    let b = dc.CreateSolidColorBrush(&d2d_color(*color), None)?;
                    dc.FillRoundedRectangle(&D2D1_ROUNDED_RECT { rect: d2d_rect(*rect), radiusX: *radius, radiusY: *radius }, &b);
                }
                DrawOp::RoundRectStroke { rect, radius, color, width } => {
                    let b = dc.CreateSolidColorBrush(&d2d_color(*color), None)?;
                    let r = rect.inset(width / 2.0, width / 2.0);
                    dc.DrawRoundedRectangle(&D2D1_ROUNDED_RECT { rect: d2d_rect(r), radiusX: *radius, radiusY: *radius }, &b, *width, None);
                }
                DrawOp::Circle { center, radius, color } => {
                    let b = dc.CreateSolidColorBrush(&d2d_color(*color), None)?;
                    dc.FillEllipse(&D2D1_ELLIPSE { point: Vector2 { X: center.x, Y: center.y }, radiusX: *radius, radiusY: *radius }, &b);
                }
                DrawOp::Text { rect, text: s, style, color, align } => {
                    let b = dc.CreateSolidColorBrush(&d2d_color(*color), None)?;
                    let format = text.format(style, *align)?;
                    let utf16: Vec<u16> = s.encode_utf16().collect();
                    dc.DrawText(&utf16, &format, &d2d_rect(*rect), &b, D2D1_DRAW_TEXT_OPTIONS_CLIP, DWRITE_MEASURING_MODE_NATURAL);
                }
            }
        }
    }
    Ok(())
}
```

Em `lib.rs`, as três linhas `pub mod` já estão lá.

- [ ] **Step 6: Rodar o teste**

Run: `cargo test -p zefiro-platform-windows`
Expected: PASS. Se o pixel (8, 16) não vier exatamente `[0, 0, 255, 255]` por causa de *antialiasing*, use um ponto mais ao centro (32, 16); o teste existe para provar que o desenho por WARP funciona e cai no lugar certo.

- [ ] **Step 7: Commit**

```powershell
git add crates/platform-windows Cargo.lock
git commit -m "feat(platform-windows): WARP Direct2D device, cached DirectWrite formats and display-list painter" -m "Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

---

## Task 20: Compositor e a forma que se transforma (`zefiro-platform-windows`)

**Files:**
- Create: `crates/platform-windows/src/{compositor,morph}.rs`, `crates/platform-windows/examples/morph.rs`
- Modify: `crates/platform-windows/src/lib.rs` (`pub mod compositor; pub mod morph;`)

**Interfaces:**
- Consumes: Task 19 (`Gfx`, `paint`, `DWriteText`), Task 9 (`SpringParams`), Task 11 (`Shape`, `CONTENT_ALPHA_EXPRESSION`), Task 3.
- Produces:
  - `compositor::{Comp, thread_queue() -> Result<DispatcherQueueController>, ui_color(Rgba) -> windows::UI::Color}`; `Comp::new(gfx: &Gfx) -> Result<Comp>` com `compositor: Compositor`, `target(hwnd: HWND) -> Result<(DesktopWindowTarget, ContainerVisual)>` (raiz sem redimensionar), `surface(w: f32, h: f32) -> Result<CompositionDrawingSurface>`, `draw(surface: &CompositionDrawingSurface, w: f32, h: f32, list: &DisplayList, text: &DWriteText) -> Result<()>` (redimensiona se precisar, limpa e desenha), `sprite_for(surface: &CompositionDrawingSurface, w: f32, h: f32) -> Result<SpriteVisual>`, `scalar_spring(value: f32, p: SpringParams) -> Result<SpringScalarNaturalMotionAnimation>`, `fade(visual: &Visual, to: f32, seconds: f32) -> Result<()>`, `on_settled(f: impl FnOnce() + 'static) -> Result<CompositionScopedBatch>` (lote de animação: chame as animações depois, e `End()` no fim).
  - `morph::{MorphStyle { fill: Rgba, depth: bool, border: Option<Rgba>, border_px: f32, shadow: Option<(Rgba, f32 /* raio de desfoque px */, f32 /* deslocamento y px */)>, backdrop: bool }, MorphShape}` com `MorphShape::new(comp: &Comp, parent: &ContainerVisual, style: &MorphStyle) -> Result<MorphShape>`, `set_style(&self, comp: &Comp, style: &MorphStyle) -> Result<()>`, `set(&self, shape: Shape) -> Result<()>` (imediato), `animate(&self, comp: &Comp, shape: Shape, p: SpringParams) -> Result<()>`, `nudge(&self, comp: &Comp, from_y: f32, p: SpringParams) -> Result<()>`, `set_opacity(&self, v: f32)`, `animate_opacity(&self, comp: &Comp, v: f32, p: SpringParams)`, `clip_child(&self, comp: &Comp, child: &Visual) -> Result<()>` (recorta um visual filho pela forma, ligado por expressão), `bind_alpha(&self, comp: &Comp, child: &Visual, final_h: f32) -> Result<()>` (opacidade do filho = `CONTENT_ALPHA_EXPRESSION`), `root(&self) -> &ContainerVisual`.

**Como funciona (para o revisor):** a forma é um `ShapeVisual` com um `CompositionSpriteShape` de geometria `CompositionRoundedRectangleGeometry`, dentro de um `LayerVisual` com `DropShadow` (a sombra segue o alfa da forma). Os valores x, y, w, h e raio ficam num `CompositionPropertySet`; as **molas animam esses escalares** (`SpringScalarNaturalMotionAnimation`), e `ExpressionAnimation`s ligam `Offset`, `Size` e `CornerRadius` da geometria (e das geometrias de recorte dos filhos) a eles. A opacidade do conteúdo é outra expressão sobre `p.h`. Assim, durante a animação, nada roda na CPU do Zefiro. Uma mola nova sempre parte do valor atual (redirecionar no meio funciona sozinho).

- [ ] **Step 1: Teste (falhando)**

`crates/platform-windows/src/morph.rs` (só o teste):

```rust
//! Forma arredondada com preenchimento, profundidade, contorno, sombra e fundo do Windows,
//! animada no compositor (pílula → painel).

#[cfg(test)]
mod tests {
    use super::*;
    use crate::compositor::Comp;
    use crate::gfx::Gfx;
    use zefiro_platform::geometry::Rect;
    use zefiro_ui::panel_layout::Shape;
    use zefiro_ui::spring::SpringParams;

    #[test]
    fn creates_sets_and_animates_without_a_window() {
        let gfx = Gfx::new().unwrap();
        let comp = Comp::new(&gfx).unwrap();
        let parent = comp.compositor.CreateContainerVisual().unwrap();
        let style = MorphStyle { fill: Rgba::rgb(8, 8, 10), depth: true, border: Some(Rgba::rgb(30, 30, 34)), border_px: 1.0, shadow: Some((Rgba::BLACK.with_alpha(0.55), 18.0, 4.0)), backdrop: false };
        let m = MorphShape::new(&comp, &parent, &style).unwrap();
        m.set(Shape { rect: Rect::new(10.0, 6.0, 120.0, 32.0), radius: 16.0 }).unwrap();
        m.animate(&comp, Shape { rect: Rect::new(0.0, 50.0, 352.0, 186.0), radius: 24.0 }, SpringParams::GROW).unwrap();
        let child = comp.compositor.CreateSpriteVisual().unwrap();
        m.clip_child(&comp, &child.cast().unwrap()).unwrap();
        m.bind_alpha(&comp, &child.cast().unwrap(), 186.0).unwrap();
        assert_eq!(parent.Children().unwrap().Count().unwrap(), 1);
    }
}
```

Run: `cargo test -p zefiro-platform-windows morph`
Expected: FAIL de compilação.

- [ ] **Step 2: Implementar `compositor.rs`**

```rust
//! Windows.UI.Composition: fila de despacho, alvos de janela, superfícies de desenho e animações.

use windows::Foundation::{IReference, PropertyValue, TimeSpan, TypedEventHandler};
use windows::Graphics::DirectX::{DirectXAlphaMode, DirectXPixelFormat};
use windows::Graphics::SizeInt32;
use windows::System::DispatcherQueueController;
use windows::UI::Color;
use windows::UI::Composition::Desktop::DesktopWindowTarget;
use windows::UI::Composition::*;
use windows::Win32::Foundation::{HWND, POINT};
use windows::Win32::Graphics::Direct2D::Common::D2D1_COLOR_F;
use windows::Win32::Graphics::Direct2D::ID2D1DeviceContext;
use windows::Win32::System::WinRT::Composition::{ICompositionDrawingSurfaceInterop, ICompositorDesktopInterop, ICompositorInterop};
use windows::Win32::System::WinRT::{CreateDispatcherQueueController, DQTAT_COM_STA, DQTYPE_THREAD_CURRENT, DispatcherQueueOptions};
use windows::core::{HSTRING, Interface, Result};
use windows_numerics::{Matrix3x2, Vector2};
use zefiro_platform::color::Rgba;
use zefiro_ui::paint::DisplayList;
use zefiro_ui::spring::SpringParams;

use crate::gfx::Gfx;
use crate::painter::paint;
use crate::text::DWriteText;

thread_local! {
    /// O Windows só aceita uma fila de despacho por thread (bug achado no S1).
    static QUEUE: std::cell::RefCell<Option<DispatcherQueueController>> = const { std::cell::RefCell::new(None) };
}

pub fn thread_queue() -> Result<DispatcherQueueController> {
    QUEUE.with(|q| {
        if let Some(existing) = q.borrow().as_ref() {
            return Ok(existing.clone());
        }
        let options = DispatcherQueueOptions { dwSize: size_of::<DispatcherQueueOptions>() as u32, threadType: DQTYPE_THREAD_CURRENT, apartmentType: DQTAT_COM_STA };
        // SAFETY: cria a fila desta thread, exigida pelo Compositor.
        let created = unsafe { CreateDispatcherQueueController(options)? };
        *q.borrow_mut() = Some(created.clone());
        Ok(created)
    })
}

pub fn ui_color(c: Rgba) -> Color {
    let b = |v: f32| (v.clamp(0.0, 1.0) * 255.0).round() as u8;
    Color { A: b(c.a), R: b(c.r), G: b(c.g), B: b(c.b) }
}

pub struct Comp {
    _queue: DispatcherQueueController,
    pub compositor: Compositor,
    gdev: CompositionGraphicsDevice,
}

impl Comp {
    pub fn new(gfx: &Gfx) -> Result<Comp> {
        let queue = thread_queue()?;
        let compositor = Compositor::new()?;
        let interop: ICompositorInterop = compositor.cast()?;
        // SAFETY: dispositivo D2D válido.
        let gdev = unsafe { interop.CreateGraphicsDevice(&gfx.d2d_device)? };
        Ok(Comp { _queue: queue, compositor, gdev })
    }

    /// Alvo da janela (que precisa de WS_EX_NOREDIRECTIONBITMAP) e a raiz.
    pub fn target(&self, hwnd: HWND) -> Result<(DesktopWindowTarget, ContainerVisual)> {
        let desktop: ICompositorDesktopInterop = self.compositor.cast()?;
        // SAFETY: hwnd válido.
        let target = unsafe { desktop.CreateDesktopWindowTarget(hwnd, true)? };
        let root = self.compositor.CreateContainerVisual()?;
        target.SetRoot(&root)?;
        Ok((target, root))
    }

    pub fn surface(&self, w: f32, h: f32) -> Result<CompositionDrawingSurface> {
        self.gdev.CreateDrawingSurface(
            windows::Foundation::Size { Width: w.max(1.0), Height: h.max(1.0) },
            DirectXPixelFormat::B8G8R8A8UIntNormalized,
            DirectXAlphaMode::Premultiplied,
        )
    }

    pub fn draw(&self, surface: &CompositionDrawingSurface, w: f32, h: f32, list: &DisplayList, text: &DWriteText) -> Result<()> {
        let size = SizeInt32 { Width: w.ceil().max(1.0) as i32, Height: h.ceil().max(1.0) as i32 };
        if surface.SizeInt32()? != size {
            surface.Resize(size)?;
        }
        let interop: ICompositionDrawingSurfaceInterop = surface.cast()?;
        // SAFETY: BeginDraw/EndDraw pareados; o contexto só é usado entre eles.
        unsafe {
            let mut offset = POINT::default();
            let dc: ID2D1DeviceContext = interop.BeginDraw(None, &mut offset)?;
            dc.SetTransform(&Matrix3x2::translation(offset.x as f32, offset.y as f32));
            dc.Clear(Some(&D2D1_COLOR_F::default()));
            let result = paint(&dc, list, text);
            interop.EndDraw()?;
            result
        }
    }

    pub fn sprite_for(&self, surface: &CompositionDrawingSurface, w: f32, h: f32) -> Result<SpriteVisual> {
        let v = self.compositor.CreateSpriteVisual()?;
        v.SetBrush(&self.compositor.CreateSurfaceBrushWithSurface(surface)?)?;
        v.SetSize(Vector2 { X: w, Y: h })?;
        Ok(v)
    }

    pub fn scalar_spring(&self, value: f32, p: SpringParams) -> Result<SpringScalarNaturalMotionAnimation> {
        let c = p.compositor();
        let a = self.compositor.CreateSpringScalarAnimation()?;
        a.SetDampingRatio(c.damping_ratio)?;
        a.SetPeriod(TimeSpan { Duration: (c.period_s as f64 * 1e7) as i64 })?;
        a.SetFinalValue(&PropertyValue::CreateSingle(value)?.cast::<IReference<f32>>()?)?;
        Ok(a)
    }

    pub fn fade(&self, visual: &Visual, to: f32, seconds: f32) -> Result<()> {
        let a = self.compositor.CreateScalarKeyFrameAnimation()?;
        a.InsertKeyFrame(1.0, to)?;
        a.SetDuration(TimeSpan { Duration: (seconds as f64 * 1e7) as i64 })?;
        visual.StartAnimation(&HSTRING::from("Opacity"), &a)
    }

    /// Lote que avisa quando as animações iniciadas até `End()` terminarem. O aviso chega na thread de UI.
    pub fn on_settled(&self, f: impl FnOnce() + 'static) -> Result<CompositionScopedBatch> {
        let batch = self.compositor.CreateScopedBatch(CompositionBatchTypes::Animation)?;
        let cell = std::cell::RefCell::new(Some(f));
        batch.Completed(&TypedEventHandler::new(move |_, _| {
            if let Some(f) = cell.borrow_mut().take() {
                f();
            }
            Ok(())
        }))?;
        Ok(batch)
    }
}
```

(Se o `TypedEventHandler` exigir `Send` na `windows 0.61.3`, troque o `RefCell` por um `Mutex` e use um canal para levar o aviso à thread de UI; anote no ADR-0007.)

- [ ] **Step 3: Implementar `morph.rs`** (acima do teste)

```rust
use windows::UI::Composition::*;
use windows::core::{HSTRING, Interface, Result};
use windows_numerics::{Vector2, Vector3};
use zefiro_platform::color::Rgba;
use zefiro_ui::panel_layout::{CONTENT_ALPHA_EXPRESSION, Shape};
use zefiro_ui::spring::SpringParams;

use crate::compositor::{Comp, ui_color};

#[derive(Debug, Clone, PartialEq)]
pub struct MorphStyle {
    pub fill: Rgba,
    pub depth: bool,
    pub border: Option<Rgba>,
    pub border_px: f32,
    /// (cor, raio de desfoque em px, deslocamento vertical em px)
    pub shadow: Option<(Rgba, f32, f32)>,
    pub backdrop: bool,
}

pub struct MorphShape {
    root: ContainerVisual,
    layer: LayerVisual,
    shape: CompositionSpriteShape,
    props: CompositionPropertySet,
    backdrop: SpriteVisual,
}

const KEYS: [&str; 5] = ["x", "y", "w", "h", "r"];

fn bind_geometry(comp: &Comp, props: &CompositionPropertySet, g: &CompositionRoundedRectangleGeometry) -> Result<()> {
    for (prop, expr) in [("Offset", "Vector2(p.x, p.y)"), ("Size", "Vector2(p.w, p.h)"), ("CornerRadius", "Vector2(p.r, p.r)")] {
        let e = comp.compositor.CreateExpressionAnimationWithExpression(&HSTRING::from(expr))?;
        e.SetReferenceParameter(&HSTRING::from("p"), &props.cast::<CompositionObject>()?)?;
        g.StartAnimation(&HSTRING::from(prop), &e)?;
    }
    Ok(())
}

impl MorphShape {
    pub fn new(comp: &Comp, parent: &ContainerVisual, style: &MorphStyle) -> Result<MorphShape> {
        let c = &comp.compositor;
        let props = c.CreatePropertySet()?;
        for k in KEYS {
            props.InsertScalar(&HSTRING::from(k), 0.0)?;
        }
        let geometry = c.CreateRoundedRectangleGeometry()?;
        bind_geometry(comp, &props, &geometry)?;
        let shape = c.CreateSpriteShapeWithGeometry(&geometry)?;
        let shape_visual = c.CreateShapeVisual()?;
        shape_visual.SetRelativeSizeAdjustment(Vector2 { X: 1.0, Y: 1.0 })?;
        shape_visual.Shapes()?.Append(&shape.cast::<CompositionShape>()?)?;

        let layer = c.CreateLayerVisual()?;
        layer.SetRelativeSizeAdjustment(Vector2 { X: 1.0, Y: 1.0 })?;
        layer.Children()?.InsertAtTop(&shape_visual)?;

        // Fundo do Windows: fora do LayerVisual (o desfoque do host não funciona dentro de camadas).
        let backdrop = c.CreateSpriteVisual()?;
        backdrop.SetRelativeSizeAdjustment(Vector2 { X: 1.0, Y: 1.0 })?;
        backdrop.SetBrush(&c.CreateHostBackdropBrush()?)?;

        let root = c.CreateContainerVisual()?;
        root.SetRelativeSizeAdjustment(Vector2 { X: 1.0, Y: 1.0 })?;
        root.Children()?.InsertAtTop(&backdrop)?;
        root.Children()?.InsertAtTop(&layer)?;
        parent.Children()?.InsertAtTop(&root)?;

        let m = MorphShape { root, layer, shape, props, backdrop };
        m.clip_child(comp, &m.backdrop.cast()?)?;
        m.set_style(comp, style)?;
        Ok(m)
    }

    pub fn root(&self) -> &ContainerVisual { &self.root }

    pub fn set_style(&self, comp: &Comp, style: &MorphStyle) -> Result<()> {
        let c = &comp.compositor;
        let fill = style.fill;
        if style.depth {
            // Profundidade: escurece suavemente para baixo (6 % em tema claro, 14 % em escuro).
            let dark = fill.relative_luminance() < 0.45;
            let bottom = fill.mix(Rgba::BLACK.with_alpha(fill.a), if dark { 0.14 } else { 0.06 });
            let g = c.CreateLinearGradientBrush()?;
            g.SetStartPoint(Vector2 { X: 0.0, Y: 0.0 })?;
            g.SetEndPoint(Vector2 { X: 0.0, Y: 1.0 })?;
            g.ColorStops()?.Append(&c.CreateColorGradientStopWithOffsetAndColor(0.0, ui_color(fill))?)?;
            g.ColorStops()?.Append(&c.CreateColorGradientStopWithOffsetAndColor(1.0, ui_color(bottom))?)?;
            self.shape.SetFillBrush(&g)?;
        } else {
            self.shape.SetFillBrush(&c.CreateColorBrushWithColor(ui_color(fill))?)?;
        }
        match style.border {
            Some(b) => {
                self.shape.SetStrokeBrush(&c.CreateColorBrushWithColor(ui_color(b))?)?;
                self.shape.SetStrokeThickness(style.border_px)?;
            }
            None => self.shape.SetStrokeThickness(0.0)?,
        }
        match style.shadow {
            Some((color, blur, dy)) => {
                let s = c.CreateDropShadow()?;
                s.SetColor(ui_color(color.with_alpha(1.0)))?;
                s.SetOpacity(color.a)?;
                s.SetBlurRadius(blur)?;
                s.SetOffset(Vector3 { X: 0.0, Y: dy, Z: 0.0 })?;
                self.layer.SetShadow(&s.cast::<CompositionShadow>()?)?;
            }
            None => self.layer.SetShadow(None::<&CompositionShadow>)?,
        }
        self.backdrop.SetIsVisible(style.backdrop)?;
        Ok(())
    }

    pub fn set(&self, shape: Shape) -> Result<()> {
        let values = [shape.rect.x, shape.rect.y, shape.rect.w, shape.rect.h, shape.radius];
        for (k, v) in KEYS.iter().zip(values) {
            self.props.StopAnimation(&HSTRING::from(*k))?;
            self.props.InsertScalar(&HSTRING::from(*k), v)?;
        }
        Ok(())
    }

    pub fn animate(&self, comp: &Comp, shape: Shape, p: SpringParams) -> Result<()> {
        let values = [shape.rect.x, shape.rect.y, shape.rect.w, shape.rect.h, shape.radius];
        for (k, v) in KEYS.iter().zip(values) {
            self.props.StartAnimation(&HSTRING::from(*k), &comp.scalar_spring(v, p)?)?;
        }
        Ok(())
    }

    /// "Cutucada": desloca a forma inteira `from_y` px e volta a 0 com a mola.
    pub fn nudge(&self, comp: &Comp, from_y: f32, p: SpringParams) -> Result<()> {
        self.root.SetOffset(Vector3 { X: 0.0, Y: from_y, Z: 0.0 })?;
        self.root.StartAnimation(&HSTRING::from("Offset.Y"), &comp.scalar_spring(0.0, p)?)
    }

    pub fn set_opacity(&self, v: f32) -> Result<()> { self.root.SetOpacity(v) }

    pub fn animate_opacity(&self, comp: &Comp, v: f32, p: SpringParams) -> Result<()> {
        self.root.StartAnimation(&HSTRING::from("Opacity"), &comp.scalar_spring(v, p)?)
    }

    /// Recorta `child` pela forma atual (ligado por expressão: acompanha a animação).
    pub fn clip_child(&self, comp: &Comp, child: &Visual) -> Result<()> {
        let g = comp.compositor.CreateRoundedRectangleGeometry()?;
        bind_geometry(comp, &self.props, &g)?;
        child.SetClip(&comp.compositor.CreateGeometricClipWithGeometry(&g)?)
    }

    /// Opacidade de `child` = regra da ilha sobre a altura atual da forma.
    pub fn bind_alpha(&self, comp: &Comp, child: &Visual, final_h: f32) -> Result<()> {
        let expr = CONTENT_ALPHA_EXPRESSION.replace("g.Size.Y", "p.h");
        let e = comp.compositor.CreateExpressionAnimationWithExpression(&HSTRING::from(expr))?;
        e.SetReferenceParameter(&HSTRING::from("p"), &self.props.cast::<CompositionObject>()?)?;
        e.SetScalarParameter(&HSTRING::from("H"), final_h)?;
        child.StartAnimation(&HSTRING::from("Opacity"), &e)
    }
}
```

Em `lib.rs`: `pub mod compositor; pub mod morph;`. (Com fundo do Windows, quem chama já passa `fill` com o alfa `backdrop_fill`; ver Task 22.)

- [ ] **Step 4: Rodar o teste**

Run: `cargo test -p zefiro-platform-windows`
Expected: PASS (2 testes).

- [ ] **Step 5: Exemplo visual (verificação manual da fluidez)**

`crates/platform-windows/examples/morph.rs`: abre uma janela popup sem bitmap de redirecionamento (classe própria, `WS_EX_NOREDIRECTIONBITMAP | WS_EX_TOPMOST | WS_EX_TOOLWINDOW`, 600 × 400 no centro da tela), cria `Comp`, um `MorphShape` com o Obsidian (tokens da Task 5) e, a cada clique (`WM_LBUTTONUP`), alterna entre uma pílula (240, 20, 120, 32, raio 16) e um painel (124, 58, 352, 186, raio 24) com `SpringParams::GROW` ao abrir e `SHRINK` ao fechar, mais `nudge(-6)` ao abrir. O laço de mensagens é `GetMessageW`/`DispatchMessageW`; `Esc` fecha.

```rust
use windows::Win32::Foundation::*;
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::WindowsAndMessaging::*;
use windows::core::w;
use zefiro_platform::color::Rgba;
use zefiro_platform::geometry::Rect;
use zefiro_platform_windows::compositor::Comp;
use zefiro_platform_windows::gfx::Gfx;
use zefiro_platform_windows::morph::{MorphShape, MorphStyle};
use zefiro_ui::panel_layout::Shape;
use zefiro_ui::spring::SpringParams;

static mut CLICKED: bool = false;

extern "system" fn wndproc(hwnd: HWND, msg: u32, wp: WPARAM, lp: LPARAM) -> LRESULT {
    match msg {
        WM_LBUTTONUP => {
            // SAFETY: só a thread de UI toca nesta variável.
            unsafe { CLICKED = true };
            LRESULT(0)
        }
        WM_KEYDOWN if wp.0 == 0x1B => {
            // SAFETY: encerra o laço de mensagens desta thread.
            unsafe { PostQuitMessage(0) };
            LRESULT(0)
        }
        // SAFETY: mensagens não tratadas vão para o Windows.
        _ => unsafe { DefWindowProcW(hwnd, msg, wp, lp) },
    }
}

fn main() -> windows::core::Result<()> {
    // SAFETY: registro de classe e criação de janela com estruturas completas.
    let hwnd = unsafe {
        let instance = GetModuleHandleW(None)?;
        let wc = WNDCLASSEXW { cbSize: size_of::<WNDCLASSEXW>() as u32, lpfnWndProc: Some(wndproc), hInstance: instance.into(), lpszClassName: w!("ZefiroMorphDemo"), ..Default::default() };
        RegisterClassExW(&wc);
        let (sw, sh) = (GetSystemMetrics(SM_CXSCREEN), GetSystemMetrics(SM_CYSCREEN));
        let hwnd = CreateWindowExW(WS_EX_NOREDIRECTIONBITMAP | WS_EX_TOPMOST | WS_EX_TOOLWINDOW, w!("ZefiroMorphDemo"), w!("morph"), WS_POPUP | WS_VISIBLE, (sw - 600) / 2, (sh - 400) / 2, 600, 400, None, None, Some(instance.into()), None)?;
        hwnd
    };
    let gfx = Gfx::new()?;
    let comp = Comp::new(&gfx)?;
    let (_target, root) = comp.target(hwnd)?;
    let style = MorphStyle { fill: Rgba::rgb(8, 8, 10).with_alpha(0.96), depth: true, border: Some(Rgba::rgb(30, 30, 34)), border_px: 1.0, shadow: Some((Rgba::BLACK.with_alpha(0.55), 18.0, 4.0)), backdrop: false };
    let m = MorphShape::new(&comp, &root, &style)?;
    let pill = Shape { rect: Rect::new(240.0, 20.0, 120.0, 32.0), radius: 16.0 };
    let panel = Shape { rect: Rect::new(124.0, 58.0, 352.0, 186.0), radius: 24.0 };
    m.set(pill)?;
    let mut open = false;
    let mut msg = MSG::default();
    // SAFETY: laço de mensagens padrão.
    unsafe {
        while GetMessageW(&mut msg, None, 0, 0).as_bool() {
            let _ = TranslateMessage(&msg);
            DispatchMessageW(&msg);
            if CLICKED {
                CLICKED = false;
                open = !open;
                if open {
                    m.animate(&comp, panel, SpringParams::GROW)?;
                    m.nudge(&comp, -6.0, SpringParams::SETTLE)?;
                } else {
                    m.animate(&comp, pill, SpringParams::SHRINK)?;
                }
            }
        }
    }
    Ok(())
}
```

Run: `cargo run -p zefiro-platform-windows --example morph`
Expected: uma pílula preta com sombra; clicar a transforma num painel com mola (cresce rápido, quase sem balanço), clicar de novo volta mais devagar. Clicar no meio da animação redireciona sem salto. Nenhum engasgo a 165 Hz. Gerenciador de Tarefas: CPU do processo em ~0 % durante a animação.

- [ ] **Step 6: Commit**

```powershell
git add crates/platform-windows
git commit -m "feat(platform-windows): compositor helpers and the spring-driven morph shape" -m "Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

---

## Task 21: Janelas, AppBar, monitores e sistema (`zefiro-platform-windows`)

**Files:**
- Create: `crates/platform-windows/src/{window,appbar,monitors,system,island_zorder}.rs`
- Modify: `crates/platform-windows/src/lib.rs`

**Interfaces:**
- Consumes: Task 3 (`Rect`, `Point`).
- Produces:
  - `window::{WinEvent, WindowKind { Bar, Panel, Message }, register_classes() -> Result<()>, create(kind: WindowKind, rect: Rect) -> Result<HWND>, set_interactive(hwnd: HWND, rects: Vec<Rect>), take_events() -> Vec<WinEvent>, post_user(code: u32), set_bounds(hwnd: HWND, rect: Rect, show: bool), hide(hwnd: HWND), focus(hwnd: HWND), foreground() -> isize, restore_foreground(previous: isize, ours: HWND), set_timer(id: usize, ms: u32), kill_timer(id: usize), pump() -> bool, pump_pending(), WM_USER_BASE: u32}`.
    `WinEvent::{MouseMove { hwnd, x: f32, y: f32 }, MouseLeave { hwnd }, LeftUp { hwnd, x: f32, y: f32 }, RightUp { hwnd, x: f32, y: f32 }, Wheel { hwnd, delta: i32 }, Key { hwnd, vk: u32 }, FocusLost { hwnd }, Timer { id: usize }, DpiChanged { hwnd }, DisplayChanged, AppBar { hwnd, notification: u32 }, TaskbarCreated, SettingChanged, User { code: u32 }, Quit}` (todos `Copy + Debug`; `hwnd` é `isize` para não vazar tipos Win32).
  - `appbar::{register(hwnd: HWND, monitor: Rect, height: i32) -> Rect, set_pos(hwnd: HWND, monitor: Rect, height: i32) -> Rect, unregister(hwnd: HWND)}` (copiado do S1, com `Rect`).
  - `monitors::{Monitor { device: String, rect: Rect, dpi: u32, primary: bool, refresh_hz: u32 }, list() -> Vec<Monitor>}` (primário primeiro).
  - `system::{animations_enabled() -> bool, transparency_enabled() -> bool, system_accent() -> Rgba, open_folder(path: &Path), exe_dir() -> PathBuf, single_instance(name: &str) -> bool}`.
  - `island_zorder::raise_island() -> bool` (põe a janela da ilha, classe `Windhawk.DynamicIslandForWindows`, acima de tudo; `false` se não achar).

**Regras de janela:** barra = `WS_EX_TOOLWINDOW | WS_EX_NOREDIRECTIONBITMAP | WS_EX_TOPMOST | WS_EX_NOACTIVATE` e `WM_MOUSEACTIVATE → MA_NOACTIVATE` (nunca rouba o foco). Painel = `WS_EX_TOOLWINDOW | WS_EX_NOREDIRECTIONBITMAP | WS_EX_TOPMOST` (ativável, para receber teclado). Mensagens = janela `HWND_MESSAGE`, que recebe os `post_user` de outras threads. `WM_NCHITTEST` devolve `HTTRANSPARENT` fora dos retângulos de `set_interactive` (o clique passa para baixo).

- [ ] **Step 1: Teste (falhando)**

`crates/platform-windows/src/window.rs` (só o teste):

```rust
//! Classes de janela, eventos e laço de mensagens.

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hit_test_is_transparent_outside_interactive_rects() {
        register_classes().unwrap();
        create(WindowKind::Message, Rect::default()).unwrap();
        let hwnd = create(WindowKind::Bar, Rect::new(-4000.0, -4000.0, 200.0, 50.0)).unwrap();
        set_interactive(hwnd, vec![Rect::new(10.0, 10.0, 50.0, 20.0)]);
        assert!(is_interactive(hwnd, Point { x: 20.0, y: 15.0 }));
        assert!(!is_interactive(hwnd, Point { x: 100.0, y: 15.0 }));
        post_user(7);
        let mut seen = false;
        for _ in 0..20 {
            pump_pending();
            if take_events().iter().any(|e| matches!(e, WinEvent::User { code: 7 })) {
                seen = true;
                break;
            }
        }
        assert!(seen);
    }
}
```

Run: `cargo test -p zefiro-platform-windows window`
Expected: FAIL de compilação.

- [ ] **Step 2: Implementar `window.rs`** (acima do teste)

```rust
use std::cell::RefCell;
use std::collections::HashMap;
use std::sync::OnceLock;
use std::sync::atomic::{AtomicIsize, Ordering};
use windows::Win32::Foundation::*;
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::Input::KeyboardAndMouse::{SetFocus, TME_LEAVE, TRACKMOUSEEVENT, TrackMouseEvent};
use windows::Win32::UI::WindowsAndMessaging::*;
use windows::core::{PCWSTR, Result, w};
use zefiro_platform::geometry::{Point, Rect};

pub const WM_USER_BASE: u32 = WM_APP + 0x100;
const WM_APPBAR: u32 = WM_APP + 1;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WindowKind { Bar, Panel, Message }

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum WinEvent {
    MouseMove { hwnd: isize, x: f32, y: f32 },
    MouseLeave { hwnd: isize },
    LeftUp { hwnd: isize, x: f32, y: f32 },
    RightUp { hwnd: isize, x: f32, y: f32 },
    Wheel { hwnd: isize, delta: i32 },
    Key { hwnd: isize, vk: u32 },
    FocusLost { hwnd: isize },
    Timer { id: usize },
    DpiChanged { hwnd: isize },
    DisplayChanged,
    AppBar { hwnd: isize, notification: u32 },
    TaskbarCreated,
    SettingChanged,
    User { code: u32 },
    Quit,
}

const BAR: PCWSTR = w!("ZefiroBar");
const PANEL: PCWSTR = w!("ZefiroPanel");
const MESSAGE: PCWSTR = w!("ZefiroMessage");
static TASKBAR_CREATED: OnceLock<u32> = OnceLock::new();
static MESSAGE_HWND: AtomicIsize = AtomicIsize::new(0);

thread_local! {
    static EVENTS: RefCell<Vec<WinEvent>> = const { RefCell::new(Vec::new()) };
    static INTERACTIVE: RefCell<HashMap<isize, Vec<Rect>>> = RefCell::new(HashMap::new());
    static TRACKING: RefCell<HashMap<isize, bool>> = RefCell::new(HashMap::new());
}

fn push(e: WinEvent) { EVENTS.with(|q| q.borrow_mut().push(e)); }

pub fn take_events() -> Vec<WinEvent> { EVENTS.with(|q| std::mem::take(&mut *q.borrow_mut())) }

pub fn set_interactive(hwnd: HWND, rects: Vec<Rect>) {
    INTERACTIVE.with(|m| m.borrow_mut().insert(hwnd.0 as isize, rects));
}

pub(crate) fn is_interactive(hwnd: HWND, p: Point) -> bool {
    INTERACTIVE.with(|m| m.borrow().get(&(hwnd.0 as isize)).is_some_and(|rs| rs.iter().any(|r| r.contains(p))))
}

fn xy(lp: LPARAM) -> (f32, f32) {
    ((lp.0 & 0xFFFF) as i16 as f32, ((lp.0 >> 16) & 0xFFFF) as i16 as f32)
}

extern "system" fn wndproc(hwnd: HWND, msg: u32, wp: WPARAM, lp: LPARAM) -> LRESULT {
    let h = hwnd.0 as isize;
    match msg {
        WM_NCHITTEST => {
            let (sx, sy) = xy(lp);
            let mut pt = POINT { x: sx as i32, y: sy as i32 };
            // SAFETY: converte coordenadas de tela para cliente desta janela.
            let _ = unsafe { ScreenToClient(hwnd, &mut pt) };
            let inside = is_interactive(hwnd, Point { x: pt.x as f32, y: pt.y as f32 });
            return LRESULT(if inside { HTCLIENT as isize } else { HTTRANSPARENT as isize });
        }
        WM_MOUSEMOVE => {
            let tracking = TRACKING.with(|t| t.borrow_mut().insert(h, true).unwrap_or(false));
            if !tracking {
                let mut tme = TRACKMOUSEEVENT { cbSize: size_of::<TRACKMOUSEEVENT>() as u32, dwFlags: TME_LEAVE, hwndTrack: hwnd, dwHoverTime: 0 };
                // SAFETY: struct local completa.
                let _ = unsafe { TrackMouseEvent(&mut tme) };
            }
            let (x, y) = xy(lp);
            push(WinEvent::MouseMove { hwnd: h, x, y });
        }
        WM_MOUSELEAVE => {
            TRACKING.with(|t| t.borrow_mut().insert(h, false));
            push(WinEvent::MouseLeave { hwnd: h });
        }
        WM_LBUTTONUP => {
            let (x, y) = xy(lp);
            push(WinEvent::LeftUp { hwnd: h, x, y });
        }
        WM_RBUTTONUP => {
            let (x, y) = xy(lp);
            push(WinEvent::RightUp { hwnd: h, x, y });
        }
        WM_MOUSEWHEEL => push(WinEvent::Wheel { hwnd: h, delta: ((wp.0 >> 16) & 0xFFFF) as i16 as i32 }),
        WM_KEYDOWN => push(WinEvent::Key { hwnd: h, vk: wp.0 as u32 }),
        WM_KILLFOCUS => push(WinEvent::FocusLost { hwnd: h }),
        WM_ACTIVATE if (wp.0 & 0xFFFF) as u32 == WA_INACTIVE => push(WinEvent::FocusLost { hwnd: h }),
        WM_TIMER => push(WinEvent::Timer { id: wp.0 }),
        WM_DPICHANGED => push(WinEvent::DpiChanged { hwnd: h }),
        WM_DISPLAYCHANGE => push(WinEvent::DisplayChanged),
        WM_SETTINGCHANGE => push(WinEvent::SettingChanged),
        WM_MOUSEACTIVATE => {
            // SAFETY: lê o estilo estendido desta janela.
            let ex = unsafe { GetWindowLongPtrW(hwnd, GWL_EXSTYLE) } as u32;
            if ex & WS_EX_NOACTIVATE.0 != 0 {
                return LRESULT(MA_NOACTIVATE as isize);
            }
            // SAFETY: repassa ao Windows.
            return unsafe { DefWindowProcW(hwnd, msg, wp, lp) };
        }
        WM_CLOSE => push(WinEvent::Quit),
        m if m == WM_APPBAR => push(WinEvent::AppBar { hwnd: h, notification: wp.0 as u32 }),
        m if m >= WM_USER_BASE && m < WM_USER_BASE + 0x100 => push(WinEvent::User { code: m - WM_USER_BASE }),
        m if Some(&m) == TASKBAR_CREATED.get() => push(WinEvent::TaskbarCreated),
        // SAFETY: mensagens não tratadas vão para o Windows.
        _ => return unsafe { DefWindowProcW(hwnd, msg, wp, lp) },
    }
    LRESULT(0)
}

pub fn register_classes() -> Result<()> {
    static DONE: OnceLock<()> = OnceLock::new();
    if DONE.get().is_some() {
        return Ok(());
    }
    // SAFETY: estruturas completas; nomes estáticos.
    unsafe {
        let instance = GetModuleHandleW(None)?;
        for name in [BAR, PANEL, MESSAGE] {
            let wc = WNDCLASSEXW {
                cbSize: size_of::<WNDCLASSEXW>() as u32,
                lpfnWndProc: Some(wndproc),
                hInstance: instance.into(),
                lpszClassName: name,
                hCursor: LoadCursorW(None, IDC_ARROW)?,
                ..Default::default()
            };
            if RegisterClassExW(&wc) == 0 {
                return Err(windows::core::Error::from_win32());
            }
        }
        let _ = TASKBAR_CREATED.set(RegisterWindowMessageW(w!("TaskbarCreated")));
    }
    let _ = DONE.set(());
    Ok(())
}

pub fn create(kind: WindowKind, r: Rect) -> Result<HWND> {
    let (class, ex, parent) = match kind {
        WindowKind::Bar => (BAR, WS_EX_TOOLWINDOW | WS_EX_NOREDIRECTIONBITMAP | WS_EX_TOPMOST | WS_EX_NOACTIVATE, None),
        WindowKind::Panel => (PANEL, WS_EX_TOOLWINDOW | WS_EX_NOREDIRECTIONBITMAP | WS_EX_TOPMOST, None),
        WindowKind::Message => (MESSAGE, WINDOW_EX_STYLE(0), Some(HWND_MESSAGE)),
    };
    // SAFETY: classe registrada; parâmetros simples.
    let hwnd = unsafe {
        let instance = GetModuleHandleW(None)?;
        CreateWindowExW(ex, class, w!("Zefiro"), WS_POPUP, r.x as i32, r.y as i32, r.w as i32, r.h as i32, parent, None, Some(instance.into()), None)?
    };
    if kind == WindowKind::Message {
        MESSAGE_HWND.store(hwnd.0 as isize, Ordering::SeqCst);
    }
    Ok(hwnd)
}

/// Pode ser chamado de qualquer thread: acorda o laço com `WinEvent::User { code }`.
pub fn post_user(code: u32) {
    let h = MESSAGE_HWND.load(Ordering::SeqCst);
    if h != 0 {
        // SAFETY: PostMessage é seguro entre threads; a janela de mensagens vive até o fim do app.
        let _ = unsafe { PostMessageW(Some(HWND(h as *mut _)), WM_USER_BASE + code, WPARAM(0), LPARAM(0)) };
    }
}

pub fn set_bounds(hwnd: HWND, r: Rect, show: bool) {
    let flags = SWP_NOACTIVATE | if show { SWP_SHOWWINDOW } else { SET_WINDOW_POS_FLAGS(0) };
    // SAFETY: hwnd válido.
    let _ = unsafe { SetWindowPos(hwnd, Some(HWND_TOPMOST), r.x as i32, r.y as i32, r.w as i32, r.h as i32, flags) };
}

pub fn hide(hwnd: HWND) {
    // SAFETY: hwnd válido.
    let _ = unsafe { ShowWindow(hwnd, SW_HIDE) };
}

/// Dá o foco do teclado (o clique que abriu o painel dá ao processo o direito de fazer isso).
pub fn focus(hwnd: HWND) {
    // SAFETY: hwnd válido.
    unsafe {
        let _ = SetForegroundWindow(hwnd);
        let _ = SetFocus(Some(hwnd));
    }
}

/// Janela em primeiro plano agora (para devolver o foco depois).
pub fn foreground() -> isize {
    // SAFETY: sem parâmetros.
    unsafe { GetForegroundWindow() }.0 as isize
}

/// Devolve o foco a `previous` se `ours` ainda estiver em primeiro plano (spec §6.1, item 5).
pub fn restore_foreground(previous: isize, ours: HWND) {
    // SAFETY: handles podem ter morrido; IsWindow confere antes de usar.
    unsafe {
        let prev = HWND(previous as *mut _);
        if previous != 0 && GetForegroundWindow() == ours && IsWindow(Some(prev)).as_bool() {
            let _ = SetForegroundWindow(prev);
        }
    }
}

pub fn set_timer(id: usize, ms: u32) {
    let h = MESSAGE_HWND.load(Ordering::SeqCst);
    // SAFETY: janela de mensagens válida; WM_TIMER chega no wndproc.
    unsafe { SetTimer(Some(HWND(h as *mut _)), id, ms, None) };
}

pub fn kill_timer(id: usize) {
    let h = MESSAGE_HWND.load(Ordering::SeqCst);
    // SAFETY: janela de mensagens válida.
    let _ = unsafe { KillTimer(Some(HWND(h as *mut _)), id) };
}

/// Espera e despacha uma mensagem. `false` = WM_QUIT.
pub fn pump() -> bool {
    let mut msg = MSG::default();
    // SAFETY: buffer local.
    unsafe {
        if !GetMessageW(&mut msg, None, 0, 0).as_bool() {
            return false;
        }
        let _ = TranslateMessage(&msg);
        DispatchMessageW(&msg);
    }
    true
}

/// Despacha o que já está na fila, sem esperar.
pub fn pump_pending() {
    let mut msg = MSG::default();
    // SAFETY: buffer local.
    unsafe {
        while PeekMessageW(&mut msg, None, 0, 0, PM_REMOVE).as_bool() {
            let _ = TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }
    }
}
```

- [ ] **Step 3: Implementar `appbar.rs`, `monitors.rs`, `island_zorder.rs`**

`appbar.rs` (do S1, com `Rect`):

```rust
//! AppBar no topo do monitor (SHAppBarMessage).

use windows::Win32::Foundation::{HWND, RECT};
use windows::Win32::UI::Shell::*;
use windows::Win32::UI::WindowsAndMessaging::WM_APP;
use zefiro_platform::geometry::Rect;

const WM_APPBAR: u32 = WM_APP + 1; // a mesma do window.rs

fn data(hwnd: HWND) -> APPBARDATA {
    APPBARDATA { cbSize: size_of::<APPBARDATA>() as u32, hWnd: hwnd, uCallbackMessage: WM_APPBAR, uEdge: ABE_TOP, ..Default::default() }
}

fn to_rect(r: Rect) -> RECT {
    RECT { left: r.x as i32, top: r.y as i32, right: r.right() as i32, bottom: r.bottom() as i32 }
}

fn from_rect(r: RECT) -> Rect {
    Rect::new(r.left as f32, r.top as f32, (r.right - r.left) as f32, (r.bottom - r.top) as f32)
}

/// Registra a janela como AppBar no topo de `monitor` e reserva `height` px.
pub fn register(hwnd: HWND, monitor: Rect, height: i32) -> Rect {
    let mut abd = data(hwnd);
    // SAFETY: `abd` com cbSize correto.
    unsafe { SHAppBarMessage(ABM_NEW, &mut abd) };
    set_pos(hwnd, monitor, height)
}

pub fn set_pos(hwnd: HWND, monitor: Rect, height: i32) -> Rect {
    let mut abd = data(hwnd);
    let mut r = to_rect(monitor);
    r.bottom = r.top + height;
    abd.rc = r;
    // SAFETY: `abd` válido.
    unsafe { SHAppBarMessage(ABM_QUERYPOS, &mut abd) };
    abd.rc.bottom = abd.rc.top + height;
    // SAFETY: `abd` válido.
    unsafe { SHAppBarMessage(ABM_SETPOS, &mut abd) };
    from_rect(abd.rc)
}

pub fn unregister(hwnd: HWND) {
    let mut abd = data(hwnd);
    // SAFETY: `abd` válido.
    unsafe { SHAppBarMessage(ABM_REMOVE, &mut abd) };
}
```

`monitors.rs` (do S1, sem `handle`/`work`):

```rust
//! Monitores, primário primeiro.

use windows::Win32::Foundation::{LPARAM, RECT, TRUE};
use windows::Win32::Graphics::Gdi::*;
use windows::Win32::UI::HiDpi::{GetDpiForMonitor, MDT_EFFECTIVE_DPI};
use windows::Win32::UI::WindowsAndMessaging::MONITORINFOF_PRIMARY;
use windows::core::{BOOL, PCWSTR};
use zefiro_platform::geometry::Rect;

#[derive(Debug, Clone, PartialEq)]
pub struct Monitor { pub device: String, pub rect: Rect, pub dpi: u32, pub primary: bool, pub refresh_hz: u32 }

pub fn list() -> Vec<Monitor> {
    let mut out: Vec<Monitor> = Vec::new();

    unsafe extern "system" fn cb(h: HMONITOR, _: HDC, _: *mut RECT, data: LPARAM) -> BOOL {
        // SAFETY: `data` aponta para o Vec de `list`, vivo durante a enumeração.
        let out = unsafe { &mut *(data.0 as *mut Vec<Monitor>) };
        let mut mi = MONITORINFOEXW::default();
        mi.monitorInfo.cbSize = size_of::<MONITORINFOEXW>() as u32;
        // SAFETY: buffer do tamanho correto.
        if unsafe { GetMonitorInfoW(h, &mut mi as *mut _ as *mut MONITORINFO) }.as_bool() {
            let (mut dx, mut dy) = (96u32, 96u32);
            // SAFETY: ponteiros para u32 locais.
            let _ = unsafe { GetDpiForMonitor(h, MDT_EFFECTIVE_DPI, &mut dx, &mut dy) };
            let mut dm = DEVMODEW { dmSize: size_of::<DEVMODEW>() as u16, ..Default::default() };
            // SAFETY: `szDevice` é uma string terminada em zero dentro de `mi`.
            let hz = if unsafe { EnumDisplaySettingsW(PCWSTR(mi.szDevice.as_ptr()), ENUM_CURRENT_SETTINGS, &mut dm) }.as_bool() { dm.dmDisplayFrequency } else { 60 };
            let r = mi.monitorInfo.rcMonitor;
            out.push(Monitor {
                device: String::from_utf16_lossy(&mi.szDevice).trim_end_matches('\0').to_string(),
                rect: Rect::new(r.left as f32, r.top as f32, (r.right - r.left) as f32, (r.bottom - r.top) as f32),
                dpi: dx,
                primary: mi.monitorInfo.dwFlags & MONITORINFOF_PRIMARY != 0,
                refresh_hz: hz,
            });
        }
        TRUE
    }

    // SAFETY: o callback respeita o contrato de EnumDisplayMonitors.
    unsafe {
        let _ = EnumDisplayMonitors(None, None, Some(cb), LPARAM(&mut out as *mut _ as isize));
    }
    out.sort_by_key(|m| !m.primary);
    out
}
```

`island_zorder.rs`:

```rust
//! Mantém a janela da ilha acima da barra (spec §7.5).

use windows::Win32::UI::WindowsAndMessaging::*;
use windows::core::w;

pub fn raise_island() -> bool {
    // SAFETY: FindWindow/SetWindowPos com parâmetros simples; a janela pode ter sumido, o erro é ignorado.
    unsafe {
        let Ok(hwnd) = FindWindowW(w!("Windhawk.DynamicIslandForWindows"), None) else { return false };
        if hwnd.is_invalid() {
            return false;
        }
        SetWindowPos(hwnd, Some(HWND_TOPMOST), 0, 0, 0, 0, SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE).is_ok()
    }
}
```

- [ ] **Step 4: Implementar `system.rs`**

```rust
//! Preferências do Windows que a barra respeita.

use std::path::{Path, PathBuf};
use windows::Win32::Foundation::{ERROR_ALREADY_EXISTS, GetLastError};
use windows::Win32::System::Registry::*;
use windows::Win32::System::Threading::CreateMutexW;
use windows::Win32::UI::Shell::ShellExecuteW;
use windows::Win32::UI::WindowsAndMessaging::*;
use windows::core::{BOOL, HSTRING, w};
use zefiro_platform::color::Rgba;

fn reg_dword(key: &HSTRING, value: &HSTRING) -> Option<u32> {
    let mut data = 0u32;
    let mut size = 4u32;
    // SAFETY: buffer de 4 bytes para um REG_DWORD.
    let r = unsafe { RegGetValueW(HKEY_CURRENT_USER, key, value, RRF_RT_REG_DWORD, None, Some(&mut data as *mut u32 as *mut _), Some(&mut size)) };
    r.is_ok().then_some(data)
}

/// "Efeitos de animação" do Windows.
pub fn animations_enabled() -> bool {
    let mut on = BOOL(1);
    // SAFETY: SPI_GETCLIENTAREAANIMATION grava um BOOL.
    let _ = unsafe { SystemParametersInfoW(SPI_GETCLIENTAREAANIMATION, 0, Some(&mut on as *mut BOOL as *mut _), SYSTEM_PARAMETERS_INFO_UPDATE_FLAGS(0)) };
    on.as_bool()
}

/// "Efeitos de transparência" do Windows.
pub fn transparency_enabled() -> bool {
    reg_dword(&HSTRING::from(r"Software\Microsoft\Windows\CurrentVersion\Themes\Personalize"), &HSTRING::from("EnableTransparency")).unwrap_or(1) != 0
}

/// Cor de destaque do Windows (DWM\AccentColor, formato ABGR).
pub fn system_accent() -> Rgba {
    let abgr = reg_dword(&HSTRING::from(r"Software\Microsoft\Windows\DWM"), &HSTRING::from("AccentColor")).unwrap_or(0xFFD77800);
    Rgba::rgb((abgr & 0xFF) as u8, ((abgr >> 8) & 0xFF) as u8, ((abgr >> 16) & 0xFF) as u8)
}

pub fn open_folder(path: &Path) {
    // SAFETY: strings válidas durante a chamada.
    unsafe { ShellExecuteW(None, w!("open"), &HSTRING::from(path.as_os_str()), None, None, SW_SHOWNORMAL) };
}

pub fn exe_dir() -> PathBuf {
    std::env::current_exe().ok().and_then(|p| p.parent().map(Path::to_path_buf)).unwrap_or_else(|| PathBuf::from("."))
}

/// `true` se esta é a única instância (o mutex vive até o processo terminar).
pub fn single_instance(name: &str) -> bool {
    // SAFETY: cria um mutex nomeado; o handle é mantido aberto de propósito.
    unsafe {
        let _ = CreateMutexW(None, true, &HSTRING::from(name));
        GetLastError() != ERROR_ALREADY_EXISTS
    }
}
```

Em `lib.rs`, acrescentar `pub mod appbar; pub mod island_zorder; pub mod monitors; pub mod system; pub mod window;`.

- [ ] **Step 5: Rodar os testes**

Run: `cargo test -p zefiro-platform-windows`
Expected: PASS (3 testes).

- [ ] **Step 6: Commit**

```powershell
git add crates/platform-windows
git commit -m "feat(platform-windows): bar/panel/message windows with click-through hit testing, AppBar, monitors and system settings" -m "Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

---

## Task 22: A barra na tela (`zefiro-app`)

**Files:**
- Create: `crates/app/src/{app,bar_view,theme_state}.rs`
- Modify: `crates/app/Cargo.toml`, `crates/app/src/main.rs`

**Interfaces:**
- Consumes: Tasks 2, 5, 6, 7, 9, 10, 12, 13, 19, 20, 21.
- Produces:
  - `theme_state::ThemeState { registry: ThemeRegistry, theme: Theme, tokens: Tokens, animations: bool }` com `ThemeState::load(id: &str, user_dir: &Path) -> (ThemeState, Vec<ThemeError>)`, `reload(&mut self, id: &str, user_dir: &Path) -> Vec<ThemeError>`, `spring(&self, base: SpringParams) -> SpringParams` (aplica estilo e velocidade do tema), `morph_style(&self, scale: f32, kind: SurfaceKind) -> MorphStyle` com `SurfaceKind { Strip, Pill, Panel, Hover }`.
  - `bar_view::BarView` com `BarView::create(app: &AppCtx, monitor: &Monitor) -> Result<BarView>`, `apply(&mut self, app: &AppCtx, animate: bool) -> Result<()>` (refaz layout + desenho dos chips + anima as formas), `chip_at(&self, x: f32, y: f32) -> Option<String>`, `chip_rect(&self, id: &str) -> Option<Rect>` (coordenadas do monitor), `set_hover(&mut self, app: &AppCtx, chip: Option<&str>)`, `set_active(&mut self, app: &AppCtx, chip: Option<&str>)`, `fade_chip(&self, app: &AppCtx, chip: &str, visible: bool)`, `band_bottom(&self) -> f32`, `hwnd(&self) -> HWND`, `register_appbar(&mut self, app: &AppCtx)`, `reposition_appbar(&self)`, `destroy(self)`.
  - `app::AppCtx { gfx: Gfx, comp: Comp, text: DWriteText, theme: ThemeState, config: Config, config_path: PathBuf, themes_dir: PathBuf, clock: WindowsClock }`.

**Composição da janela da barra** (uma por monitor, largura do monitor × (44 DIP + 24 DIP de sombra)): raiz → `strip` (`MorphShape`, estilo `Strip`: preenchimento + profundidade, sem contorno nem sombra) + linha divisória (sprite de 1 DIP na cor `hairline` na base da faixa) + `groups[0..2]` (`MorphShape`, estilo `Pill`: preenchimento, profundidade, contorno, sombra 18/4 DIP) + por chip: um `MorphShape` de fundo (estilo `Hover`: `raised`, opacidade 0) e um `SpriteVisual` com a superfície do conteúdo.

**Transições** (spec §5.3), todas com `theme.spring(SpringParams::SETTLE)`:
- Faixa → pílulas: a `strip` encolhe para um retângulo no centro (largura de 4 × a altura da pílula da ilha, raio de pílula) e some (opacidade 0); os grupos crescem a partir do retângulo da faixa nas pontas até o retângulo de cada seção e aparecem (opacidade 1).
- Pílulas → faixa: o inverso.
- Mudança de altura: todos os retângulos animam até o novo layout.
- `animate = false` ou animações do Windows desligadas: `set` direto (sem mola).
- No fim da transição (`comp.on_settled`), `appbar::set_pos` com a nova altura e `island_zorder::raise_island()`.

- [ ] **Step 1: Dependências e `theme_state.rs`**

`crates/app/Cargo.toml`, acrescentar:

```toml
[dependencies]
zefiro-core.workspace = true
zefiro-platform.workspace = true
zefiro-resources.workspace = true
zefiro-ui.workspace = true
zefiro-integration-clock.workspace = true
zefiro-bar.workspace = true
zefiro-island-host.workspace = true
zefiro-platform-windows.workspace = true
windows.workspace = true
windows-numerics.workspace = true
tracing.workspace = true
```

`crates/app/src/theme_state.rs`:

```rust
//! Tema atual, tokens e as regras de estilo das formas.

use std::path::Path;
use zefiro_platform::color::Rgba;
use zefiro_platform_windows::morph::MorphStyle;
use zefiro_platform_windows::system::{animations_enabled, system_accent, transparency_enabled};
use zefiro_resources::registry::ThemeRegistry;
use zefiro_resources::theme::{Backdrop, Theme, ThemeError};
use zefiro_resources::tokens::{Tokens, derive_tokens};
use zefiro_ui::spring::SpringParams;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SurfaceKind { Strip, Pill, Panel, Hover }

pub struct ThemeState { pub registry: ThemeRegistry, pub theme: Theme, pub tokens: Tokens, pub animations: bool }

impl ThemeState {
    pub fn load(id: &str, user_dir: &Path) -> (ThemeState, Vec<ThemeError>) {
        let (registry, errors) = ThemeRegistry::load(Some(user_dir));
        let theme = registry.get(id).or_else(|| registry.get("obsidian")).cloned().expect("obsidian embutido");
        let tokens = derive_tokens(&theme, system_accent(), None);
        (ThemeState { registry, theme, tokens, animations: animations_enabled() }, errors)
    }

    /// Relê a pasta do usuário e aplica `id`. Erros mantêm a versão anterior do tema com erro.
    pub fn reload(&mut self, id: &str, user_dir: &Path) -> Vec<ThemeError> {
        let (registry, errors) = self.registry.reload_keeping_previous(Some(user_dir));
        if let Some(t) = registry.get(id).or_else(|| registry.get("obsidian")) {
            self.theme = t.clone();
        }
        self.registry = registry;
        self.tokens = derive_tokens(&self.theme, system_accent(), None);
        self.animations = animations_enabled();
        errors
    }

    pub fn spring(&self, base: SpringParams) -> SpringParams {
        base.styled(self.theme.animation.spring).with_speed(self.theme.animation.speed)
    }

    pub fn morph_style(&self, scale: f32, kind: SurfaceKind) -> MorphStyle {
        let backdrop = self.theme.material.backdrop == Backdrop::Acrylic && transparency_enabled();
        let fill = if backdrop { self.tokens.fill.with_alpha(self.tokens.fill.a * self.theme.material.backdrop_fill) } else { self.tokens.fill };
        let shadow = self.theme.material.shadow.then_some((self.tokens.shadow, 18.0 * scale, 4.0 * scale));
        match kind {
            SurfaceKind::Strip => MorphStyle { fill, depth: self.theme.material.depth, border: None, border_px: 0.0, shadow: None, backdrop },
            SurfaceKind::Pill | SurfaceKind::Panel => MorphStyle { fill, depth: self.theme.material.depth, border: self.tokens.border, border_px: scale, shadow, backdrop },
            SurfaceKind::Hover => MorphStyle { fill: self.tokens.raised, depth: false, border: None, border_px: 0.0, shadow: None, backdrop: false },
        }
    }

    /// Cor de fundo do chip quando o painel dele está aberto.
    pub fn active_fill(&self) -> Rgba { self.tokens.raised_strong }
}
```

- [ ] **Step 2: `app.rs` (contexto) e `main.rs` (inicialização)**

`crates/app/src/app.rs`:

```rust
//! Estado compartilhado pelas visões.

use std::path::PathBuf;
use zefiro_core::config::Config;
use zefiro_integration_clock::windows::WindowsClock;
use zefiro_platform_windows::compositor::Comp;
use zefiro_platform_windows::gfx::Gfx;
use zefiro_platform_windows::text::DWriteText;

use crate::theme_state::ThemeState;

pub struct AppCtx {
    pub gfx: Gfx,
    pub comp: Comp,
    pub text: DWriteText,
    pub theme: ThemeState,
    pub config: Config,
    pub config_path: PathBuf,
    pub themes_dir: PathBuf,
    pub clock: WindowsClock,
}
```

`crates/app/src/main.rs`:

```rust
#![windows_subsystem = "windows"]

mod app;
mod bar_view;
mod theme_state;

use std::time::Instant;
use zefiro_core::config::{default_config_dir, load_or_create};
use zefiro_core::observability::{default_log_dir, init_logging, install_panic_hook};
use zefiro_platform::geometry::Rect;
use zefiro_platform_windows::{compositor::Comp, gfx::Gfx, monitors, system, text::DWriteText, window};

const VERSION: &str = env!("CARGO_PKG_VERSION");

fn main() {
    let started = Instant::now();
    let log_dir = default_log_dir();
    install_panic_hook(log_dir.clone(), VERSION);
    let _guard = init_logging(&log_dir, "info").ok();
    if !system::single_instance("Local\\zefiro-bar") {
        tracing::info!("zefiro já está rodando; saindo");
        return;
    }
    if let Err(e) = run(started) {
        tracing::error!(error = %e, "zefiro encerrou com erro");
    }
}

fn run(started: Instant) -> windows::core::Result<()> {
    let config_dir = default_config_dir();
    let config_path = config_dir.join("config.toml");
    let themes_dir = config_dir.join("themes");
    let config = load_or_create(&config_path).unwrap_or_else(|e| {
        tracing::error!(line = ?e.line, "config.toml inválido: {}; usando padrões", e.message);
        zefiro_core::config::Config::default()
    });
    let (theme, errors) = theme_state::ThemeState::load(&config.theme, &themes_dir);
    for e in errors {
        tracing::warn!(source = %e.source, line = ?e.line, "tema com erro: {}", e.message);
    }
    window::register_classes()?;
    window::create(window::WindowKind::Message, Rect::default())?;
    let gfx = Gfx::new()?;
    let comp = Comp::new(&gfx)?;
    let text = DWriteText::new(gfx.dwrite.clone(), &theme.theme.font.family);
    let ctx = app::AppCtx { gfx, comp, text, theme, config, config_path, themes_dir, clock: zefiro_integration_clock::windows::WindowsClock };

    let mut bars = Vec::new();
    for m in monitors::list() {
        bars.push(bar_view::BarView::create(&ctx, &m)?);
    }
    tracing::info!(ms = started.elapsed().as_millis() as u64, monitors = bars.len(), "primeiro quadro da barra");
    for b in &mut bars {
        b.register_appbar(&ctx);
    }
    while window::pump() {
        for e in window::take_events() {
            if matches!(e, window::WinEvent::Quit) {
                return Ok(());
            }
        }
    }
    Ok(())
}
```

(As Tasks 23 e 24 substituem o laço simples acima por `event_loop.rs`.)

- [ ] **Step 3: `bar_view.rs`**

```rust
//! Uma barra por monitor: faixa ou pílulas, chips e transições.

use windows::UI::Composition::{ContainerVisual, SpriteVisual, Visual};
use windows::UI::Composition::Desktop::DesktopWindowTarget;
use windows::Win32::Foundation::HWND;
use windows::core::{Interface, Result};
use windows_numerics::{Vector2, Vector3};
use zefiro_bar::chips::{ChipContent, clock_chip, zefiro_chip};
use zefiro_core::config::{BarFormat, BarHeight};
use zefiro_integration_clock::format::ClockText;
use zefiro_integration_clock::windows::now_local;
use zefiro_platform::geometry::Rect;
use zefiro_platform_windows::morph::MorphShape;
use zefiro_platform_windows::{appbar, island_zorder, window};
use zefiro_ui::bar_layout::{BarLayout, ChipSpec, layout_bar, metrics};
use zefiro_ui::panel_layout::{CHIP_FADE_S, SHADOW_MARGIN_DIP, Shape, pill_radius};
use zefiro_ui::spring::SpringParams;

use crate::app::AppCtx;
use crate::theme_state::SurfaceKind;

struct ChipView { id: String, hover: MorphShape, content: SpriteVisual, surface: windows::UI::Composition::CompositionDrawingSurface, rect: Rect }

pub struct BarView {
    hwnd: HWND,
    monitor: Monitor,
    scale: f32,
    _target: DesktopWindowTarget,
    root: ContainerVisual,
    strip: MorphShape,
    divider: SpriteVisual,
    groups: Vec<MorphShape>,
    chips: Vec<ChipView>,
    layout: Option<BarLayout>,
    appbar: bool,
    hover: Option<String>,
    active: Option<String>,
}

fn chip_content(ctx: &AppCtx, id: &str, layout_h: BarHeight, chip_h: f32, font_px: f32, scale: f32) -> ChipContent {
    let t = &ctx.theme.tokens;
    match id {
        "zefiro" => zefiro_chip(chip_h, font_px, layout_h == BarHeight::Compacta, scale, t, &ctx.text),
        _ => {
            let now = now_local();
            let s = ctx.clock.chip_text(&now, &ctx.config.clock.format, ctx.config.clock.seconds);
            clock_chip(&s, chip_h, font_px, t, &ctx.text)
        }
    }
}

impl BarView {
    pub fn create(ctx: &AppCtx, monitor: &Monitor) -> Result<BarView> {
        let scale = monitor.dpi as f32 / 96.0;
        let h = ((44.0 + SHADOW_MARGIN_DIP) * scale).round();
        let hwnd = window::create(window::WindowKind::Bar, Rect::new(monitor.rect.x, monitor.rect.y, monitor.rect.w, h))?;
        let (target, root) = ctx.comp.target(hwnd)?;
        root.SetSize(Vector2 { X: monitor.rect.w, Y: h })?;
        let strip = MorphShape::new(&ctx.comp, &root, &ctx.theme.morph_style(scale, SurfaceKind::Strip))?;
        let divider = ctx.comp.compositor.CreateSpriteVisual()?;
        divider.SetBrush(&ctx.comp.compositor.CreateColorBrushWithColor(zefiro_platform_windows::compositor::ui_color(ctx.theme.tokens.hairline))?)?;
        root.Children()?.InsertAtTop(&divider)?;
        let mut groups = Vec::new();
        for _ in 0..2 {
            groups.push(MorphShape::new(&ctx.comp, &root, &ctx.theme.morph_style(scale, SurfaceKind::Pill))?);
        }
        let mut view = BarView { hwnd, monitor: monitor.clone(), scale, _target: target, root, strip, divider, groups, chips: Vec::new(), layout: None, appbar: false, hover: None, active: None };
        view.apply(ctx, false)?;
        window::set_bounds(hwnd, Rect::new(monitor.rect.x, monitor.rect.y, monitor.rect.w, h), true);
        Ok(view)
    }

    pub fn hwnd(&self) -> HWND { self.hwnd }
    pub fn monitor(&self) -> &Monitor { &self.monitor }
    pub fn scale(&self) -> f32 { self.scale }
    pub fn band_bottom(&self) -> f32 { self.layout.as_ref().map_or(0.0, |l| l.band.bottom()) + self.monitor.rect.y }

    /// Refaz o layout e anima as formas até ele.
    pub fn apply(&mut self, ctx: &AppCtx, animate: bool) -> Result<()> {
        let cfg = &ctx.config.bar;
        let m = metrics(cfg.height, cfg.format);
        let chip_h = (m.chip_h * self.scale).round();
        let font_px = m.font * self.scale * ctx.theme.theme.font.scale;
        let contents: Vec<(String, ChipContent)> = cfg.sections.start.iter().chain(cfg.sections.end.iter())
            .map(|id| (id.clone(), chip_content(ctx, id, cfg.height, chip_h, font_px, self.scale)))
            .collect();
        let spec = |ids: &[String]| ids.iter().map(|id| ChipSpec { id: id.clone(), content_w: contents.iter().find(|(c, _)| c == id).map_or(0.0, |(_, c)| c.content_w) }).collect::<Vec<_>>();
        let layout = layout_bar(self.monitor.rect.w, self.scale, cfg.height, cfg.format, &spec(&cfg.sections.start), &spec(&cfg.sections.end));

        self.strip.set_style(&ctx.comp, &ctx.theme.morph_style(self.scale, SurfaceKind::Strip))?;
        for g in &self.groups {
            g.set_style(&ctx.comp, &ctx.theme.morph_style(self.scale, SurfaceKind::Pill))?;
        }
        self.divider.SetBrush(&ctx.comp.compositor.CreateColorBrushWithColor(zefiro_platform_windows::compositor::ui_color(ctx.theme.tokens.hairline))?)?;
        let animate = animate && ctx.theme.animations;
        let spring = ctx.theme.spring(SpringParams::SETTLE);
        let place = |shape: &MorphShape, s: Shape| if animate { shape.animate(&ctx.comp, s, spring) } else { shape.set(s) };
        let style = ctx.theme.theme.shape.style;
        let radius_max = ctx.theme.theme.shape.radius_max * self.scale;

        // Faixa ou o "resto" dela encolhido no centro.
        let pill_h = layout.island_pill_h;
        let center = Rect::new(((self.monitor.rect.w - pill_h * 4.0) / 2.0).round(), layout.band.y + ((layout.band.h - pill_h) / 2.0).round(), pill_h * 4.0, pill_h);
        match layout.strip {
            Some(band) => {
                place(&self.strip, Shape { rect: band, radius: 0.0 })?;
                self.strip.set_opacity(1.0)?;
            }
            None => {
                place(&self.strip, Shape { rect: center, radius: pill_h / 2.0 })?;
                if animate { self.strip.animate_opacity(&ctx.comp, 0.0, spring)? } else { self.strip.set_opacity(0.0)? }
            }
        }
        self.divider.SetOffset(Vector3 { X: 0.0, Y: layout.band.bottom() - self.scale, Z: 0.0 })?;
        self.divider.SetSize(Vector2 { X: self.monitor.rect.w, Y: self.scale.max(1.0) })?;
        self.divider.SetOpacity(if layout.strip.is_some() { 1.0 } else { 0.0 })?;

        // Grupos: aparecem do retângulo da faixa nas pontas.
        for (i, g) in self.groups.iter().enumerate() {
            match layout.groups.get(i) {
                Some(r) => {
                    if self.layout.as_ref().is_some_and(|l| l.groups.is_empty()) {
                        g.set(Shape { rect: Rect::new(r.x, layout.band.y, r.w, layout.band.h), radius: 0.0 })?;
                    }
                    place(g, Shape { rect: *r, radius: pill_radius(r.h, radius_max, style, self.scale) })?;
                    if animate { g.animate_opacity(&ctx.comp, 1.0, spring)? } else { g.set_opacity(1.0)? }
                }
                None => {
                    if animate { g.animate_opacity(&ctx.comp, 0.0, spring)? } else { g.set_opacity(0.0)? }
                }
            }
        }

        // Chips: cria os que faltam, redesenha o conteúdo e move.
        for (id, content) in &contents {
            let Some(b) = layout.chips.iter().find(|c| &c.id == id) else { continue };
            if !self.chips.iter().any(|c| &c.id == id) {
                let hover = MorphShape::new(&ctx.comp, &self.root, &ctx.theme.morph_style(self.scale, SurfaceKind::Hover))?;
                hover.set_opacity(0.0)?;
                let surface = ctx.comp.surface(content.list.size.w, content.list.size.h)?;
                let sprite = ctx.comp.sprite_for(&surface, content.list.size.w, content.list.size.h)?;
                self.root.Children()?.InsertAtTop(&sprite)?;
                self.chips.push(ChipView { id: id.clone(), hover, content: sprite, surface, rect: b.rect });
            }
            let chip = self.chips.iter_mut().find(|c| &c.id == id).expect("criado acima");
            ctx.comp.draw(&chip.surface, content.list.size.w, content.list.size.h, &content.list, &ctx.text)?;
            chip.content.SetSize(Vector2 { X: content.list.size.w, Y: content.list.size.h })?;
            let pad = (10.0 * self.scale).round();
            chip.content.SetOffset(Vector3 { X: b.rect.x + pad, Y: b.rect.y, Z: 0.0 })?;
            let r = pill_radius(b.rect.h, radius_max, style, self.scale);
            if animate { chip.hover.animate(&ctx.comp, Shape { rect: b.rect, radius: r }, spring)? } else { chip.hover.set(Shape { rect: b.rect, radius: r })? }
            chip.rect = b.rect;
        }

        // Clique: chips + faixa (ou grupos) recebem clique; o resto passa para baixo.
        let mut interactive: Vec<Rect> = layout.chips.iter().map(|c| c.rect).collect();
        interactive.extend(layout.strip);
        interactive.extend(layout.groups.iter().copied());
        window::set_interactive(self.hwnd, interactive);

        self.refresh_chip_fills(ctx)?;
        let height_px = layout.band.h as i32;
        let reserve = self.appbar;
        let (hwnd, mon) = (self.hwnd, self.monitor.rect);
        self.layout = Some(layout);
        if reserve {
            let batch = ctx.comp.on_settled(move || {
                appbar::set_pos(hwnd, mon, height_px);
                island_zorder::raise_island();
            })?;
            batch.End()?;
        }
        Ok(())
    }

    pub fn register_appbar(&mut self, _ctx: &AppCtx) {
        if let Some(l) = &self.layout {
            appbar::register(self.hwnd, self.monitor.rect, l.band.h as i32);
            self.appbar = true;
            island_zorder::raise_island();
        }
    }

    /// Outra AppBar mudou de lugar: só reposiciona (sem registrar de novo).
    pub fn reposition_appbar(&self) {
        if let (true, Some(l)) = (self.appbar, &self.layout) {
            appbar::set_pos(self.hwnd, self.monitor.rect, l.band.h as i32);
        }
    }

    pub fn chip_at(&self, x: f32, y: f32) -> Option<String> {
        self.chips.iter().find(|c| c.rect.contains(zefiro_platform::geometry::Point { x, y })).map(|c| c.id.clone())
    }

    /// Retângulo do chip em coordenadas do monitor (para o painel).
    pub fn chip_rect(&self, id: &str) -> Option<Rect> {
        self.chips.iter().find(|c| c.id == id).map(|c| Rect::new(c.rect.x + self.monitor.rect.x, c.rect.y + self.monitor.rect.y, c.rect.w, c.rect.h))
    }

    pub fn set_hover(&mut self, ctx: &AppCtx, chip: Option<&str>) -> Result<()> {
        if self.hover.as_deref() == chip {
            return Ok(());
        }
        self.hover = chip.map(str::to_string);
        self.refresh_chip_fills(ctx)
    }

    pub fn set_active(&mut self, ctx: &AppCtx, chip: Option<&str>) -> Result<()> {
        self.active = chip.map(str::to_string);
        self.refresh_chip_fills(ctx)
    }

    fn refresh_chip_fills(&self, ctx: &AppCtx) -> Result<()> {
        let spring = ctx.theme.spring(SpringParams::SETTLE);
        for c in &self.chips {
            let active = self.active.as_deref() == Some(&c.id);
            let hovered = self.hover.as_deref() == Some(&c.id);
            let mut style = ctx.theme.morph_style(self.scale, SurfaceKind::Hover);
            if active {
                style.fill = ctx.theme.active_fill();
            }
            c.hover.set_style(&ctx.comp, &style)?;
            c.hover.animate_opacity(&ctx.comp, if active || hovered { 1.0 } else { 0.0 }, spring)?;
        }
        Ok(())
    }

    /// Esconde (ao abrir o painel) ou mostra (ao fechar) o conteúdo do chip em 0,17 s.
    pub fn fade_chip(&self, ctx: &AppCtx, chip: &str, visible: bool) -> Result<()> {
        if let Some(c) = self.chips.iter().find(|c| c.id == chip) {
            ctx.comp.fade(&c.content.cast::<Visual>()?, if visible { 1.0 } else { 0.0 }, CHIP_FADE_S)?;
        }
        Ok(())
    }

    pub fn destroy(self) {
        if self.appbar {
            appbar::unregister(self.hwnd);
        }
        window::hide(self.hwnd);
    }
}
```

Acrescentar `mod app; mod bar_view; mod theme_state;` no `main.rs` (já está no passo 2).

- [ ] **Step 4: Rodar e ver**

Run: `cargo build -p zefiro-app; Get-Process yasb -ErrorAction SilentlyContinue | Stop-Process; ./target/debug/zefiro.exe`
Expected: uma faixa Obsidian de 44 DIP no topo dos dois monitores, com "Z Zefiro" à esquerda e a data e a hora à direita; janelas maximizadas começam abaixo dela; o log mostra "primeiro quadro da barra" com o tempo. Feche pelo Gerenciador de Tarefas (o menu Sair vem na Task 23).

Troque à mão no `%APPDATA%\zefiro\config.toml` `format = "pilulas"` e reabra: duas pílulas flutuantes com sombra, desktop aparecendo entre elas. Troque `height = "compacta"` e reabra: 32 DIP.

- [ ] **Step 5: Commit**

```powershell
git add crates/app
git commit -m "feat(app): per-monitor bar with strip/pill formats, chips and animated layout changes" -m "Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

---

## Task 23: Painéis, menu, calendário e relógio (`zefiro-app`)

**Files:**
- Create: `crates/app/src/{panel_view,event_loop}.rs`
- Modify: `crates/app/src/main.rs`

**Interfaces:**
- Consumes: Task 22 (`BarView`, `AppCtx`, `ThemeState`), Tasks 14–16 (`MenuModel`, `calendar_*`, `PanelController`), Task 20 (`MorphShape`), Task 21 (`window`, `system`).
- Produces:
  - `panel_view::PanelView` (uma por monitor, reaproveitada) com `PanelView::create(ctx: &AppCtx, monitor: &Monitor) -> Result<PanelView>`, `open(&mut self, ctx: &AppCtx, chip: Rect, band_bottom: f32, list: &DisplayList) -> Result<()>`, `resize(&mut self, ctx: &AppCtx, list: &DisplayList) -> Result<()>` (conteúdo mudou de tamanho: mola SETTLE), `close(&mut self, ctx: &AppCtx) -> Result<()>`, `hwnd(&self) -> HWND`, `content_origin(&self) -> (f32, f32)` (canto do painel na janela, para converter cliques).
  - `event_loop::run(ctx: AppCtx, bars: Vec<BarView>) -> Result<()>`.

**Fluxo de abrir** (spec §6.1): `PanelController` decide → `PanelView::open`: calcula `panel_rect` e `window_bounds`, posiciona a janela, desenha o conteúdo **uma vez** na superfície (no lugar final dentro da janela), põe a forma no retângulo do chip (`set`), liga o recorte e a opacidade do conteúdo à forma (`clip_child`, `bind_alpha` com a altura final), anima até o painel com `GROW` + `nudge(-6 DIP)`, `window::focus`. A barra chama `fade_chip(false)` e `set_active(Some)`. **Fechar:** anima até o retângulo do chip com `SHRINK`; no `on_settled`, esconde a janela e limpa os retângulos clicáveis; a barra faz `fade_chip(true)` e `set_active(None)`. Abrir com outro painel fechando no mesmo monitor apenas redireciona a mesma forma (a mola parte do valor atual).

**Entradas:** clique num chip → `ChipClick`; botão direito na barra → `ChipClick { chip: "zefiro" }` (se já não estiver aberto); `FocusLost` do painel → `FocusLost`; `Esc` → `Escape`; setas/Enter no menu → `MenuModel::key`; clique no menu → `MenuModel::hit`; ←/→ ou a roda no calendário → mês anterior/próximo (redesenha e, se mudar o número de semanas, `resize`). Ações do menu: altura/formato/tema aplicam na hora (barras animam, `set_value` grava o arquivo) e o menu **continua aberto** e é redesenhado; Recarregar tema → `theme.reload`; Abrir pasta → `export_builtin` + `open_folder`; Reiniciar a ilha → Task 24; Sair → `PostQuitMessage`.

**Relógio:** timer 1 com `ms_to_next_minute` (ou 1 s, se `seconds = true`); a cada disparo, `apply(ctx, true)` em todas as barras (o chip do relógio é redesenhado; se a largura mudar, ele anima) e o calendário aberto é redesenhado à meia-noite.

- [ ] **Step 1: `panel_view.rs`**

```rust
//! Janela de painel de um monitor: a forma que nasce do chip e o conteúdo revelado por ela.

use windows::UI::Composition::Desktop::DesktopWindowTarget;
use windows::UI::Composition::{CompositionDrawingSurface, ContainerVisual, SpriteVisual, Visual};
use windows::Win32::Foundation::HWND;
use windows::core::{Interface, Result};
use windows_numerics::{Vector2, Vector3};
use zefiro_platform::geometry::{Rect, Size};
use zefiro_platform_windows::monitors::Monitor;
use zefiro_platform_windows::morph::MorphShape;
use zefiro_platform_windows::window;
use zefiro_ui::paint::DisplayList;
use zefiro_ui::panel_layout::{Shape, panel_radius, panel_rect, pill_radius, to_local, window_bounds};
use zefiro_ui::spring::{NUDGE_DIP, SpringParams};

use crate::app::AppCtx;
use crate::theme_state::SurfaceKind;

pub struct PanelView {
    hwnd: HWND,
    monitor: Monitor,
    scale: f32,
    _target: DesktopWindowTarget,
    root: ContainerVisual,
    shape: MorphShape,
    content: SpriteVisual,
    surface: CompositionDrawingSurface,
    window: Rect,
    chip: Rect,
    panel: Rect,
    previous_foreground: isize,
    pub open: bool,
}

impl PanelView {
    pub fn create(ctx: &AppCtx, monitor: &Monitor) -> Result<PanelView> {
        let scale = monitor.dpi as f32 / 96.0;
        let hwnd = window::create(window::WindowKind::Panel, Rect::new(monitor.rect.x, monitor.rect.y, 10.0, 10.0))?;
        let (target, root) = ctx.comp.target(hwnd)?;
        let shape = MorphShape::new(&ctx.comp, &root, &ctx.theme.morph_style(scale, SurfaceKind::Panel))?;
        let surface = ctx.comp.surface(1.0, 1.0)?;
        let content = ctx.comp.sprite_for(&surface, 1.0, 1.0)?;
        // Dentro da raiz da forma: a "cutucada" move forma e conteúdo juntos.
        shape.root().Children()?.InsertAtTop(&content)?;
        shape.clip_child(&ctx.comp, &content.cast::<Visual>()?)?;
        Ok(PanelView { hwnd, monitor: monitor.clone(), scale, _target: target, root, shape, content, surface, window: Rect::default(), chip: Rect::default(), panel: Rect::default(), previous_foreground: 0, open: false })
    }

    pub fn hwnd(&self) -> HWND { self.hwnd }

    /// Canto superior esquerdo do painel dentro da janela (para converter cliques).
    pub fn content_origin(&self) -> (f32, f32) { (self.panel.x - self.window.x, self.panel.y - self.window.y) }

    fn local(&self, r: Rect) -> Rect { to_local(Shape { rect: r, radius: 0.0 }, self.window).rect }

    fn draw(&mut self, ctx: &AppCtx, list: &DisplayList) -> Result<()> {
        ctx.comp.draw(&self.surface, list.size.w, list.size.h, list, &ctx.text)?;
        let p = self.local(self.panel);
        self.content.SetSize(Vector2 { X: list.size.w, Y: list.size.h })?;
        self.content.SetOffset(Vector3 { X: p.x, Y: p.y, Z: 0.0 })
    }

    pub fn open(&mut self, ctx: &AppCtx, chip: Rect, band_bottom: f32, list: &DisplayList) -> Result<()> {
        self.place(ctx, chip, band_bottom, list, true)
    }

    fn place(&mut self, ctx: &AppCtx, chip: Rect, band_bottom: f32, list: &DisplayList, opening: bool) -> Result<()> {
        let mon = self.monitor.rect;
        let chip_local_mon = Rect::new(chip.x - mon.x, chip.y - mon.y, chip.w, chip.h);
        let panel_mon = panel_rect(chip_local_mon, band_bottom - mon.y, list.size, mon.w, self.scale);
        let was_open = self.open;
        self.chip = Rect::new(chip.x, chip.y, chip.w, chip.h);
        self.panel = Rect::new(panel_mon.x + mon.x, panel_mon.y + mon.y, panel_mon.w, panel_mon.h);
        self.window = window_bounds(self.chip, self.panel, self.scale);
        window::set_bounds(self.hwnd, self.window, true);
        self.root.SetSize(Vector2 { X: self.window.w, Y: self.window.h })?;
        self.shape.set_style(&ctx.comp, &ctx.theme.morph_style(self.scale, SurfaceKind::Panel))?;
        self.draw(ctx, list)?;

        let style = ctx.theme.theme.shape.style;
        let chip_shape = Shape { rect: self.local(self.chip), radius: pill_radius(self.chip.h, ctx.theme.theme.shape.radius_max * self.scale, style, self.scale) };
        let panel_shape = Shape { rect: self.local(self.panel), radius: panel_radius(ctx.theme.theme.shape.panel_radius * self.scale, style, self.scale) };
        if !was_open {
            self.shape.set(chip_shape)?;
        }
        self.shape.bind_alpha(&ctx.comp, &self.content.cast::<Visual>()?, panel_shape.rect.h)?;
        if ctx.theme.animations {
            // Abrir: mola de crescer + cutucada. Conteúdo mudando de tamanho: mola de ajuste.
            let p = if opening && !was_open { SpringParams::GROW } else { SpringParams::SETTLE };
            self.shape.animate(&ctx.comp, panel_shape, ctx.theme.spring(p))?;
            if opening {
                self.shape.nudge(&ctx.comp, NUDGE_DIP * self.scale, ctx.theme.spring(SpringParams::SETTLE))?;
            }
        } else {
            self.shape.set(panel_shape)?;
        }
        window::set_interactive(self.hwnd, vec![panel_shape.rect]);
        if !was_open {
            self.previous_foreground = window::foreground();
        }
        window::focus(self.hwnd);
        self.open = true;
        Ok(())
    }

    /// O conteúdo mudou (menu redesenhado, outro mês no calendário).
    pub fn resize(&mut self, ctx: &AppCtx, list: &DisplayList) -> Result<()> {
        if (list.size.h - self.panel.h).abs() < 0.5 && (list.size.w - self.panel.w).abs() < 0.5 {
            return self.draw(ctx, list);
        }
        let band_bottom = self.panel.y - (6.0 * self.scale).round();
        self.place(ctx, self.chip, band_bottom, list, false)
    }

    pub fn close(&mut self, ctx: &AppCtx) -> Result<()> {
        if !self.open {
            return Ok(());
        }
        self.open = false;
        window::set_interactive(self.hwnd, Vec::new());
        // Esc / clique no chip: o foco volta para a janela de antes (spec §6.1, item 5).
        window::restore_foreground(self.previous_foreground, self.hwnd);
        let style = ctx.theme.theme.shape.style;
        let chip_shape = Shape { rect: self.local(self.chip), radius: pill_radius(self.chip.h, ctx.theme.theme.shape.radius_max * self.scale, style, self.scale) };
        if ctx.theme.animations {
            let batch = ctx.comp.on_settled(|| window::post_user(crate::event_loop::USER_PANEL_SETTLED))?;
            self.shape.animate(&ctx.comp, chip_shape, ctx.theme.spring(SpringParams::SHRINK))?;
            batch.End()?;
        } else {
            self.shape.set(chip_shape)?;
            window::hide(self.hwnd);
        }
        Ok(())
    }

    /// Chamado quando a animação de fechar terminou (se nada reabriu o painel nesse meio-tempo).
    pub fn hide_if_closed(&self) {
        if !self.open {
            window::hide(self.hwnd);
        }
    }

    pub fn size(&self) -> Size { Size { w: self.panel.w, h: self.panel.h } }
}
```

- [ ] **Step 2: `event_loop.rs`**

```rust
//! O laço de eventos: entradas → controlador → visões.

use std::collections::HashMap;
use windows::Win32::UI::WindowsAndMessaging::PostQuitMessage;
use windows::core::Result;
use zefiro_bar::calendar::{calendar_names, calendar_paint};
use zefiro_bar::controller::{PanelController, PanelEffect, PanelInput, PanelKind};
use zefiro_bar::menu::{MenuAction, MenuModel, NavKey, ThemeSwatch};
use zefiro_core::config::set_value;
use zefiro_integration_clock::format::ClockText;
use zefiro_integration_clock::grid::{month_grid, ms_to_next_minute, shift_month};
use zefiro_integration_clock::windows::now_local;
use zefiro_platform::geometry::Point;
use zefiro_platform_windows::monitors::Monitor;
use zefiro_platform_windows::window::{self, WinEvent};
use zefiro_platform_windows::{island_zorder, system};
use zefiro_resources::registry::export_builtin;
use zefiro_ui::paint::DisplayList;

use crate::app::AppCtx;
use crate::bar_view::BarView;
use crate::panel_view::PanelView;

pub const USER_PANEL_SETTLED: u32 = 1;
pub const USER_CONFIG_CHANGED: u32 = 2;
pub const USER_THEMES_CHANGED: u32 = 3;
pub const USER_ISLAND: u32 = 4;
pub const TIMER_CLOCK: usize = 1;
pub const TIMER_DISPLAY: usize = 2;
pub const TIMER_RAISE: usize = 3;

const VK_ESCAPE: u32 = 0x1B;
const VK_RETURN: u32 = 0x0D;
const VK_LEFT: u32 = 0x25;
const VK_UP: u32 = 0x26;
const VK_RIGHT: u32 = 0x27;
const VK_DOWN: u32 = 0x28;

pub struct Loop {
    pub ctx: AppCtx,
    pub bars: Vec<BarView>,
    pub panels: Vec<PanelView>,
    pub controller: PanelController,
    menu: Option<MenuModel>,
    shown_month: (i32, u32),
}

fn swatches(ctx: &AppCtx) -> Vec<ThemeSwatch> {
    ctx.theme.registry.list().iter().map(|e| ThemeSwatch {
        id: e.theme.id.clone(),
        name: e.theme.name.clone(),
        background: e.theme.colors.background,
        text: e.theme.colors.text,
        border: e.theme.colors.border,
    }).collect()
}

impl Loop {
    pub fn new(ctx: AppCtx, bars: Vec<BarView>) -> Result<Loop> {
        let mut panels = Vec::new();
        for b in &bars {
            panels.push(PanelView::create(&ctx, b.monitor())?);
        }
        let today = now_local().date;
        Ok(Loop { ctx, bars, panels, controller: PanelController::new(), menu: None, shown_month: (today.year, today.month) })
    }

    fn bar_index(&self, hwnd: isize) -> Option<usize> { self.bars.iter().position(|b| b.hwnd().0 as isize == hwnd) }
    fn panel_index(&self, hwnd: isize) -> Option<usize> { self.panels.iter().position(|p| p.hwnd().0 as isize == hwnd) }

    fn panel_list(&mut self, kind: PanelKind, scale: f32) -> DisplayList {
        match kind {
            PanelKind::Menu => {
                let m = self.menu.get_or_insert_with(|| MenuModel::new(self.ctx.config.bar.height, self.ctx.config.bar.format, &self.ctx.config.theme, swatches(&self.ctx)));
                m.paint(scale, &self.ctx.theme.tokens)
            }
            PanelKind::Calendar => {
                let today = now_local().date;
                let (y, mo) = self.shown_month;
                let grid = month_grid(y, mo, self.ctx.clock.first_weekday(), today);
                calendar_paint(&grid, &calendar_names(&self.ctx.clock, y, mo, today), scale, &self.ctx.theme.tokens)
            }
        }
    }

    fn run_effects(&mut self, fx: Vec<PanelEffect>) -> Result<()> {
        for e in fx {
            match e {
                PanelEffect::Close { monitor } => {
                    self.panels[monitor].close(&self.ctx)?;
                    for b in &mut self.bars {
                        b.set_active(&self.ctx, None)?;
                    }
                    for b in &self.bars {
                        for id in ["zefiro", "clock"] {
                            b.fade_chip(&self.ctx, id, true)?;
                        }
                    }
                    self.menu = None;
                }
                PanelEffect::Open(p) => {
                    if p.kind == PanelKind::Calendar {
                        let today = now_local().date;
                        self.shown_month = (today.year, today.month);
                    }
                    let scale = self.bars[p.monitor].scale();
                    let list = self.panel_list(p.kind, scale);
                    let Some(chip) = self.bars[p.monitor].chip_rect(&p.chip) else { continue };
                    let band_bottom = self.bars[p.monitor].band_bottom();
                    self.panels[p.monitor].open(&self.ctx, chip, band_bottom, &list)?;
                    self.bars[p.monitor].set_active(&self.ctx, Some(&p.chip))?;
                    self.bars[p.monitor].fade_chip(&self.ctx, &p.chip, false)?;
                }
            }
        }
        Ok(())
    }

    fn redraw_open_panel(&mut self) -> Result<()> {
        let Some(open) = self.controller.open().cloned() else { return Ok(()) };
        let scale = self.bars[open.monitor].scale();
        let list = self.panel_list(open.kind, scale);
        self.panels[open.monitor].resize(&self.ctx, &list)
    }

    pub fn apply_bars(&mut self, animate: bool) -> Result<()> {
        for b in &mut self.bars {
            b.apply(&self.ctx, animate)?;
        }
        island_zorder::raise_island();
        Ok(())
    }

    fn menu_action(&mut self, a: MenuAction) -> Result<()> {
        let path = self.ctx.config_path.clone();
        match a {
            MenuAction::SetHeight(h) => {
                self.ctx.config.bar.height = h;
                let _ = set_value(&path, &["bar", "height"], h.as_str());
            }
            MenuAction::SetFormat(f) => {
                self.ctx.config.bar.format = f;
                let _ = set_value(&path, &["bar", "format"], f.as_str());
            }
            MenuAction::SetTheme(id) => {
                self.ctx.config.theme = id.clone();
                let _ = set_value(&path, &["theme"], &id);
                let dir = self.ctx.themes_dir.clone();
                self.ctx.theme.reload(&id, &dir);
                self.ctx.text.set_family(&self.ctx.theme.theme.font.family);
            }
            MenuAction::ReloadTheme => {
                let (id, dir) = (self.ctx.config.theme.clone(), self.ctx.themes_dir.clone());
                for e in self.ctx.theme.reload(&id, &dir) {
                    tracing::warn!(source = %e.source, line = ?e.line, "tema com erro: {}", e.message);
                }
            }
            MenuAction::OpenThemesFolder => {
                let dir = self.ctx.themes_dir.clone();
                if export_builtin(&self.ctx.config.theme, &dir).is_err() {
                    let _ = std::fs::create_dir_all(&dir);
                }
                system::open_folder(&dir);
                return Ok(());
            }
            MenuAction::RestartIsland => {
                window::post_user(USER_ISLAND);
                return Ok(());
            }
            MenuAction::Exit => {
                // SAFETY: encerra o laço de mensagens desta thread.
                unsafe { PostQuitMessage(0) };
                return Ok(());
            }
        }
        // Altura/formato/tema: barras animam e o menu continua aberto, redesenhado com a nova escolha.
        self.menu = None;
        self.apply_bars(true)?;
        crate::island::sync(&self.ctx);
        self.redraw_open_panel()
    }

    pub fn schedule_clock(&self) {
        let now = now_local();
        let ms = if self.ctx.config.clock.seconds { 1000 - now.millis } else { ms_to_next_minute(now.second, now.millis) };
        window::set_timer(TIMER_CLOCK, ms.max(16));
    }

    pub fn handle(&mut self, e: WinEvent) -> Result<bool> {
        match e {
            WinEvent::Quit => return Ok(false),
            WinEvent::LeftUp { hwnd, x, y } => {
                if let Some(i) = self.bar_index(hwnd) {
                    if let Some(chip) = self.bars[i].chip_at(x, y) {
                        let fx = self.controller.handle(PanelInput::ChipClick { monitor: i, chip });
                        self.run_effects(fx)?;
                    }
                } else if let Some(i) = self.panel_index(hwnd) {
                    let (ox, oy) = self.panels[i].content_origin();
                    let scale = self.bars[i].scale();
                    let hit = self.menu.as_ref().and_then(|m| m.hit(Point { x: x - ox, y: y - oy }, scale));
                    if let Some(a) = hit {
                        self.menu_action(a)?;
                    }
                }
            }
            WinEvent::RightUp { hwnd, .. } => {
                if let Some(i) = self.bar_index(hwnd) {
                    let already = self.controller.open().is_some_and(|o| o.chip == "zefiro" && o.monitor == i);
                    if !already {
                        let fx = self.controller.handle(PanelInput::ChipClick { monitor: i, chip: "zefiro".into() });
                        self.run_effects(fx)?;
                    }
                }
            }
            WinEvent::MouseMove { hwnd, x, y } => {
                if let Some(i) = self.bar_index(hwnd) {
                    let chip = self.bars[i].chip_at(x, y);
                    self.bars[i].set_hover(&self.ctx, chip.as_deref())?;
                }
            }
            WinEvent::MouseLeave { hwnd } => {
                if let Some(i) = self.bar_index(hwnd) {
                    self.bars[i].set_hover(&self.ctx, None)?;
                }
            }
            WinEvent::FocusLost { hwnd } => {
                if self.panel_index(hwnd).is_some() && self.controller.open().is_some() {
                    let fx = self.controller.handle(PanelInput::FocusLost);
                    self.run_effects(fx)?;
                }
            }
            WinEvent::Key { hwnd, vk } => {
                if self.panel_index(hwnd).is_none() {
                    return Ok(true);
                }
                if vk == VK_ESCAPE {
                    let fx = self.controller.handle(PanelInput::Escape);
                    return self.run_effects(fx).map(|_| true);
                }
                match self.controller.open().map(|o| o.kind) {
                    Some(PanelKind::Menu) => {
                        let key = match vk { VK_UP => Some(NavKey::Up), VK_DOWN => Some(NavKey::Down), VK_LEFT => Some(NavKey::Left), VK_RIGHT => Some(NavKey::Right), VK_RETURN => Some(NavKey::Enter), _ => None };
                        if let (Some(k), Some(m)) = (key, self.menu.as_mut()) {
                            match m.key(k) {
                                Some(a) => self.menu_action(a)?,
                                None => self.redraw_open_panel()?,
                            }
                        }
                    }
                    Some(PanelKind::Calendar) if vk == VK_LEFT || vk == VK_RIGHT => {
                        let (y, m) = self.shown_month;
                        self.shown_month = shift_month(y, m, if vk == VK_LEFT { -1 } else { 1 });
                        self.redraw_open_panel()?;
                    }
                    _ => {}
                }
            }
            WinEvent::Wheel { hwnd, delta } => {
                if self.panel_index(hwnd).is_some() && self.controller.open().is_some_and(|o| o.kind == PanelKind::Calendar) {
                    let (y, m) = self.shown_month;
                    self.shown_month = shift_month(y, m, if delta > 0 { -1 } else { 1 });
                    self.redraw_open_panel()?;
                }
            }
            WinEvent::Timer { id: TIMER_CLOCK } => {
                self.apply_bars(true)?;
                if self.controller.open().is_some_and(|o| o.kind == PanelKind::Calendar) {
                    self.redraw_open_panel()?;
                }
                self.schedule_clock();
            }
            WinEvent::User { code: USER_PANEL_SETTLED } => {
                for p in &self.panels {
                    p.hide_if_closed();
                }
            }
            _ => return crate::robustness::handle(self, e),
        }
        Ok(true)
    }
}

pub fn run(ctx: AppCtx, bars: Vec<BarView>) -> Result<()> {
    let mut l = Loop::new(ctx, bars)?;
    l.schedule_clock();
    crate::island::start(&l.ctx);
    while window::pump() {
        for e in window::take_events() {
            match l.handle(e) {
                Ok(true) => {}
                Ok(false) => {
                    crate::island::stop();
                    return Ok(());
                }
                Err(err) => tracing::error!(error = %err, ?e, "erro tratando evento"),
            }
        }
    }
    crate::island::stop();
    for b in l.bars.drain(..) {
        b.destroy();
    }
    Ok(())
}

```

**Para compilar nesta tarefa**, crie `crates/app/src/island.rs` com `pub fn start(_: &crate::app::AppCtx) {}`, `pub fn stop() {}`, `pub fn sync(_: &crate::app::AppCtx) {}` e `crates/app/src/robustness.rs` com `pub fn handle(_: &mut crate::event_loop::Loop, _: zefiro_platform_windows::window::WinEvent) -> windows::core::Result<bool> { Ok(true) }`; a Task 24 os preenche.)

No `main.rs`, trocar o laço simples por `event_loop::run(ctx, bars)` e acrescentar `mod event_loop; mod island; mod panel_view; mod robustness;`.

- [ ] **Step 3: Rodar e ver**

Run: `cargo build -p zefiro-app; ./target/debug/zefiro.exe`
Expected (YASB fechado):
- Clicar no relógio: o chip "derrete" num calendário de outubro com o dia 15 destacado, com mola, sombra e conteúdo aparecendo no fim; ←/→ e a roda trocam o mês (a altura anima quando muda o número de semanas); Esc, clicar fora ou clicar no relógio de novo fecham, voltando para o chip.
- Clicar em "Z Zefiro" (ou botão direito na barra): o menu abre; ↑/↓/←/→/Enter funcionam; escolher **Compacta**, **Pílulas** ou outro tema anima a barra na hora e o menu continua aberto com a nova marcação; **Sair** fecha o app.
- Abrir o menu e clicar no relógio: o menu fecha e o calendário abre sem esperar.
- O relógio vira na troca do minuto.

- [ ] **Step 4: Commit**

```powershell
git add crates/app
git commit -m "feat(app): pill-to-panel morphing panels, Zefiro menu, calendar and minute clock" -m "Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

---

## Task 24: Ilha, recarga ao vivo e robustez (`zefiro-app`)

**Files:**
- Modify: `crates/app/src/{island,robustness,main}.rs`, `crates/app/src/event_loop.rs` (só o `USER_CONFIG_CHANGED`/`USER_THEMES_CHANGED`/`USER_ISLAND`)

**Interfaces:**
- Consumes: Task 17 (`settings_for`, `apply_ini`, `apply_state`, `RestartPolicy`, `IslandProcess`, `resolve_exe`), Task 8 (`watch_paths`), Task 21.
- Produces: `island::{start(ctx: &AppCtx), stop(), sync(ctx: &AppCtx), restart(ctx: &AppCtx)}` e `robustness::handle(l: &mut Loop, e: WinEvent) -> Result<bool>`.

**Ilha** (spec §7): uma thread `zefiro-island` com um `Mutex<Option<IslandProcess>>` e a `RestartPolicy::new(3, 60 s)`. `start` só é chamado **depois** da primeira barra aparecer (Task 23 já chama no início do laço). A thread verifica `has_exited` a cada 1 s; saída inesperada → `on_exit` → `Restart` (inicia de novo e loga) ou `GiveUp` (loga "a ilha caiu 3 vezes em 60 s; centro vazio até Reiniciar a ilha"). `sync` grava `island.ini` e `island-state.ini` (`%APPDATA%\zefiro\`) com `apply_ini`/`apply_state` a partir do tema e da configuração atuais — **antes** de iniciar e a cada mudança. `restart` = `policy.reset()` + `stop` + `start`. `stop` = `IslandProcess::stop(3 s)` e encerra a thread. `[island] enabled = false` → nada disso roda. Depois de iniciar a ilha e a cada `sync`, `island_zorder::raise_island()` com 500 ms de atraso (timer `TIMER_RAISE`), para dar tempo da janela dela existir.

**Recarga ao vivo:** `watch_paths(&[config_dir, themes_dir], 150 ms, ...)` → `window::post_user(USER_CONFIG_CHANGED)` se o caminho for o `config.toml`, `USER_THEMES_CHANGED` se for um `.toml` de `themes`. No laço: `CONFIG_CHANGED` → relê (`parse_config`; erro → log com linha, mantém) → se mudou: `PanelInput::ConfigChanged` (fecha painel), `theme.reload` se o tema mudou, `apply_bars(true)`, `island::sync`, reagenda o relógio; `THEMES_CHANGED` → `theme.reload` (erros no log) → `apply_bars(true)` + `sync` + redesenha o painel aberto.

**Robustez** (`robustness::handle`):
- `TaskbarCreated` → `register_appbar` de novo em todas as barras e `raise_island()`.
- `DisplayChanged` ou `DpiChanged` → `set_timer(TIMER_DISPLAY, 400)` (debounce: chegam até 3 por evento, ADR-0001); no `Timer { id: TIMER_DISPLAY }`: fecha o painel, destrói barras e painéis, recria a partir de `monitors::list()`, `register_appbar`, `raise_island()`.
- `SettingChanged` → `theme.animations = animations_enabled()` e `apply_bars(false)` (transparência pode ter mudado).
- `AppBar { notification: ABN_POSCHANGED (1) }` → `set_pos` com a altura atual.
- Qualquer outro evento → `Ok(true)`.

- [ ] **Step 1: `island.rs`**

```rust
//! Ciclo de vida do zefiro-island.exe (spec §7).

use std::path::PathBuf;
use std::sync::{Arc, Mutex, OnceLock, mpsc};
use std::time::{Duration, Instant};
use zefiro_island_host::ini::{apply_ini, apply_state, settings_for};
use zefiro_island_host::policy::{Decision, RestartPolicy};
use zefiro_island_host::process::{IslandProcess, resolve_exe};
use zefiro_platform_windows::{system, window};

use crate::app::AppCtx;

enum Cmd { Restart, Stop }

struct Host { tx: mpsc::Sender<Cmd>, thread: Option<std::thread::JoinHandle<()>> }

static HOST: OnceLock<Mutex<Option<Host>>> = OnceLock::new();

fn host() -> &'static Mutex<Option<Host>> { HOST.get_or_init(|| Mutex::new(None)) }

pub fn sync(ctx: &AppCtx) {
    if !ctx.config.island.enabled {
        return;
    }
    let dir = ctx.config_path.parent().map(PathBuf::from).unwrap_or_default();
    let s = settings_for(&ctx.theme.theme, ctx.config.bar.height, ctx.config.bar.format);
    let writers: [(&str, fn(&str, &zefiro_island_host::ini::IslandSettings) -> String); 2] = [("island-state.ini", apply_state), ("island.ini", apply_ini)];
    for (name, apply) in writers {
        let path = dir.join(name);
        let old = std::fs::read_to_string(&path).unwrap_or_default();
        let new = apply(&old, &s);
        if new != old {
            if let Err(e) = std::fs::write(&path, new) {
                tracing::warn!(path = %path.display(), "não consegui gravar: {e}");
            }
        }
    }
    window::set_timer(crate::event_loop::TIMER_RAISE, 500);
}

pub fn start(ctx: &AppCtx) {
    if !ctx.config.island.enabled {
        return;
    }
    let Some(exe) = resolve_exe(&ctx.config.island.exe, &system::exe_dir()) else {
        tracing::warn!("zefiro-island.exe não encontrado ao lado do zefiro.exe nem em [island] exe; centro vazio");
        return;
    };
    sync(ctx);
    let (tx, rx) = mpsc::channel::<Cmd>();
    let thread = std::thread::Builder::new().name("zefiro-island".into()).spawn(move || {
        let mut policy = RestartPolicy::new(3, Duration::from_secs(60));
        let proc: Arc<Mutex<Option<IslandProcess>>> = Arc::new(Mutex::new(IslandProcess::start(&exe).ok()));
        let mut gave_up = false;
        loop {
            match rx.recv_timeout(Duration::from_secs(1)) {
                Ok(Cmd::Stop) | Err(mpsc::RecvTimeoutError::Disconnected) => {
                    if let Some(mut p) = proc.lock().unwrap().take() {
                        p.stop(Duration::from_secs(3));
                    }
                    return;
                }
                Ok(Cmd::Restart) => {
                    policy.reset();
                    gave_up = false;
                    if let Some(mut p) = proc.lock().unwrap().take() {
                        p.stop(Duration::from_secs(3));
                    }
                    *proc.lock().unwrap() = IslandProcess::start(&exe).ok();
                    window::post_user(crate::event_loop::USER_ISLAND + 100);
                }
                Err(mpsc::RecvTimeoutError::Timeout) => {
                    let exited = proc.lock().unwrap().as_mut().is_some_and(|p| p.has_exited());
                    if exited && !gave_up {
                        match policy.on_exit(Instant::now()) {
                            Decision::Restart => {
                                tracing::warn!("a ilha saiu sozinha; reiniciando");
                                *proc.lock().unwrap() = IslandProcess::start(&exe).ok();
                                window::post_user(crate::event_loop::USER_ISLAND + 100);
                            }
                            Decision::GiveUp => {
                                tracing::error!("a ilha caiu 3 vezes em 60 s; centro vazio até \"Reiniciar a ilha\"");
                                gave_up = true;
                                *proc.lock().unwrap() = None;
                            }
                        }
                    }
                }
            }
        }
    });
    match thread {
        Ok(t) => *host().lock().unwrap() = Some(Host { tx, thread: Some(t) }),
        Err(e) => tracing::error!("não consegui criar a thread da ilha: {e}"),
    }
}

pub fn restart(ctx: &AppCtx) {
    let running = host().lock().unwrap().as_ref().map(|h| h.tx.send(Cmd::Restart).is_ok()).unwrap_or(false);
    if !running {
        start(ctx);
    }
}

pub fn stop() {
    if let Some(mut h) = host().lock().unwrap().take() {
        let _ = h.tx.send(Cmd::Stop);
        if let Some(t) = h.thread.take() {
            let _ = t.join();
        }
    }
}
```

No `event_loop.rs`, dentro do `match` do `handle`, antes do braço `_ =>`:

```rust
            WinEvent::User { code: USER_ISLAND } => crate::island::restart(&self.ctx),
            WinEvent::User { code } if code == USER_ISLAND + 100 => window::set_timer(TIMER_RAISE, 500),
            WinEvent::Timer { id: TIMER_RAISE } => {
                window::kill_timer(TIMER_RAISE);
                island_zorder::raise_island();
            }
```

- [ ] **Step 2: Observador de arquivos no `main.rs`**

Depois de criar as barras e antes de `event_loop::run`:

```rust
    let cfg_path = ctx.config_path.clone();
    let _watcher = zefiro_core::watch::watch_paths(&[config_dir.clone(), themes_dir.clone()], std::time::Duration::from_millis(150), move |paths| {
        for p in paths {
            if p == cfg_path {
                window::post_user(event_loop::USER_CONFIG_CHANGED);
            } else if p.extension().is_some_and(|e| e.eq_ignore_ascii_case("toml")) && p.parent().is_some_and(|d| d.ends_with("themes")) {
                window::post_user(event_loop::USER_THEMES_CHANGED);
            }
        }
    })
    .map_err(|e| tracing::warn!("observador de arquivos indisponível: {e}"))
    .ok();
```

E no `event_loop.rs`, os dois códigos:

```rust
            WinEvent::User { code: USER_CONFIG_CHANGED } => {
                let text = std::fs::read_to_string(&self.ctx.config_path).unwrap_or_default();
                match zefiro_core::config::parse_config(&text) {
                    Ok(new) if new != self.ctx.config => {
                        let fx = self.controller.handle(PanelInput::ConfigChanged);
                        self.run_effects(fx)?;
                        let theme_changed = new.theme != self.ctx.config.theme;
                        self.ctx.config = new;
                        if theme_changed {
                            let (id, dir) = (self.ctx.config.theme.clone(), self.ctx.themes_dir.clone());
                            self.ctx.theme.reload(&id, &dir);
                            self.ctx.text.set_family(&self.ctx.theme.theme.font.family);
                        }
                        self.apply_bars(true)?;
                        crate::island::sync(&self.ctx);
                        self.schedule_clock();
                    }
                    Ok(_) => {}
                    Err(e) => tracing::warn!(line = ?e.line, "config.toml inválido: {}; mantendo a configuração anterior", e.message),
                }
            }
            WinEvent::User { code: USER_THEMES_CHANGED } => {
                let (id, dir) = (self.ctx.config.theme.clone(), self.ctx.themes_dir.clone());
                for e in self.ctx.theme.reload(&id, &dir) {
                    tracing::warn!(source = %e.source, line = ?e.line, "tema com erro: {}; mantendo a versão anterior", e.message);
                }
                self.ctx.text.set_family(&self.ctx.theme.theme.font.family);
                self.apply_bars(true)?;
                crate::island::sync(&self.ctx);
                self.menu = None;
                self.redraw_open_panel()?;
            }
```

- [ ] **Step 3: `robustness.rs`**

```rust
//! Explorer reiniciado, monitores e DPI mudando, preferências do Windows.

use windows::core::Result;
use zefiro_platform_windows::window::{self, WinEvent};
use zefiro_platform_windows::{island_zorder, monitors, system};

use crate::bar_view::BarView;
use crate::event_loop::{Loop, TIMER_DISPLAY};
use crate::panel_view::PanelView;

const ABN_POSCHANGED: u32 = 1;

pub fn handle(l: &mut Loop, e: WinEvent) -> Result<bool> {
    match e {
        WinEvent::TaskbarCreated => {
            tracing::info!("Explorer reiniciado: registrando as AppBars de novo");
            for b in &mut l.bars {
                b.register_appbar(&l.ctx);
            }
            island_zorder::raise_island();
        }
        WinEvent::DisplayChanged | WinEvent::DpiChanged { .. } => window::set_timer(TIMER_DISPLAY, 400),
        WinEvent::Timer { id: TIMER_DISPLAY } => {
            window::kill_timer(TIMER_DISPLAY);
            tracing::info!("monitores mudaram: recriando as barras");
            // Os painéis são recriados abaixo; o controlador só precisa saber que nada está aberto.
            let _ = l.controller.handle(zefiro_bar::controller::PanelInput::ConfigChanged);
            for b in l.bars.drain(..) {
                b.destroy();
            }
            for p in l.panels.drain(..) {
                window::hide(p.hwnd());
            }
            for m in monitors::list() {
                let mut b = BarView::create(&l.ctx, &m)?;
                b.register_appbar(&l.ctx);
                l.panels.push(PanelView::create(&l.ctx, &m)?);
                l.bars.push(b);
            }
            island_zorder::raise_island();
        }
        WinEvent::SettingChanged => {
            l.ctx.theme.animations = system::animations_enabled();
            l.apply_bars(false)?;
        }
        WinEvent::AppBar { notification: ABN_POSCHANGED, .. } => {
            for b in &l.bars {
                b.reposition_appbar();
            }
        }
        _ => {}
    }
    Ok(true)
}
```


- [ ] **Step 4: Rodar e verificar (Review Focus 1, 4 e 5)**

Run: `cargo build --release -p zefiro-app; ./island/build.ps1; Copy-Item island/build/zefiro-island.exe target/release/; ./target/release/zefiro.exe`
Expected, com o YASB fechado:
1. A ilha aparece no centro do monitor principal, combinando com a altura e o formato (44 na faixa completa; 28 na compacta; 36/24 nas pílulas) e com o tema; trocar o tema pelo menu do Zefiro muda a cor da ilha também.
2. **Review Focus 4:** fechar a ilha pelo Gerenciador de Tarefas 4 vezes seguidas → 3 reinícios, depois o log diz "a ilha caiu 3 vezes em 60 s" e o centro fica vazio; o menu **Reiniciar a ilha** a traz de volta. Renomear `zefiro-island.exe` → a barra abre normalmente e loga "não encontrado".
3. **Review Focus 1:** copiar `obsidian.toml` para `%APPDATA%\zefiro\themes\`, trocar `background = "#300000"` e salvar → a barra muda na hora. Trocar para `background = "preto"` e salvar → nada pisca, a barra mantém o vermelho escuro e o log mostra o arquivo, a linha 7 e "cor inválida".
4. Editar `config.toml` à mão (`format = "pilulas"`) e salvar → a barra anima para pílulas; se um painel estava aberto, ele fecha.
5. **Review Focus 5:** com a barra aberta, reiniciar o Explorer (`Stop-Process -Name explorer`; ele volta sozinho) → a barra continua reservando o topo e a ilha fica acima dela. Win+P "Somente tela do PC" e depois "Estender" → as barras somem e voltam no monitor certo. Desligar os "Efeitos de animação" do Windows → painéis abrem e fecham sem mola.
6. **Review Focus 3:** abrir o calendário e, durante a animação, clicar em "Z Zefiro"; depois, com o menu aberto, trocar o formato → nada fica preso aberto nem invisível.

- [ ] **Step 5: Commit**

```powershell
git add crates/app
git commit -m "feat(app): embedded island lifecycle, live config/theme reload and display/explorer robustness" -m "Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

---

## Task 25: Medições, ADR-0007, PLANNING e validação

**Files:**
- Create: `docs/adr/0007-barra-fase-1a.md`
- Modify: `docs/PLANNING.md` (§2, §3.5, §4.1, §6, §12 — spec §14), `README.md` (como rodar)

**Interfaces:**
- Consumes: tudo acima; `scripts/perf.ps1`, `scripts/frames.ps1`, `island/first-frame.ps1`.
- Produces: números medidos, decisões registradas e a validação do usuário.

- [ ] **Step 1: Contador de quadros do painel**

Acrescentar ao `event_loop.rs` um modo de ciclos para medir: com a variável de ambiente `ZEFIRO_CYCLES=50`, depois de 2 s o app abre e fecha o calendário 50 vezes (um ciclo a cada 900 ms, por `set_timer`) e grava no log `ciclo N: clique→primeiro quadro X ms` (tempo do `handle` até o retorno do `open`, que inclui o primeiro `Commit` do compositor) e sai. Rodar com o PresentMon por baixo:

Run (PowerShell elevado, YASB fechado): `$env:ZEFIRO_CYCLES=50; ./scripts/frames.ps1 -Process zefiro -Seconds 60` (ver o cabeçalho do `frames.ps1` para os parâmetros exatos)
Expected: intervalos > 1,5 período ≈ 0 % no monitor de 165 Hz durante as rajadas; clique→quadro ≤ 16 ms no log.

- [ ] **Step 2: RAM, CPU e primeiro quadro**

Run (YASB fechado; ilha rodando, mas medida à parte):
```powershell
./scripts/perf.ps1 -Process zefiro -WarmupSeconds 60 -SampleSeconds 180 -Out ./scripts/out/zefiro-repouso.json
```
E de novo com o calendário sendo aberto e fechado à mão a cada ~5 s, para `zefiro-paineis.json`. Primeiro quadro: abrir o `zefiro.exe` 5 vezes e ler "primeiro quadro da barra … ms" no log.
Expected: memória privada ≤ 15 MB; CPU em repouso ≤ 0,05 %; primeiro quadro ≤ 250 ms. Reabrir o YASB no fim.

- [ ] **Step 3: ADR-0007**

`docs/adr/0007-barra-fase-1a.md`, seguindo `docs/adr/0000-modelo.md`: contexto (spec da 1a), as decisões tomadas na implementação (listas de desenho portáveis + desenho na plataforma; molas animando escalares de um `CompositionPropertySet` e expressões ligando geometria, recorte e opacidade; uma janela de painel por monitor reaproveitada; tema da ilha sincronizado pelo `island.ini` **e** `island-state.ini`; o menu continua aberto ao trocar altura/formato/tema), a tabela com os números dos Steps 1–2 contra as metas, e o que ficou de fora ou divergiu (ex.: cor de destaque "auto" ainda não vem da capa da ilha — a barra usa a do sistema até o porte da 1c; ajustes de API da `windows 0.61.3` feitos durante a implementação; plano B da Task 18, se usado).

- [ ] **Step 4: PLANNING e README**

`docs/PLANNING.md` (spec §14):
- §2 e §12: komorebi fora até a 1.0; 1a = esta spec (link); 1c = "porte da ilha para Rust dentro da barra"; tirar komorebi da 1d.
- §3.5: o modelo pílula → painel (molas no compositor via `CompositionPropertySet` + expressões).
- §4.1: tema padrão Obsidian + os 7 temas da ilha; o formato de arquivo de tema da spec §8.1; `bar.height = completa | compacta` e `bar.format = faixa | pilulas`.
- §6: komorebi sai da Fase 1.

`README.md`: seção "Rodar" com `cargo build --release -p zefiro-app`, `./island/build.ps1`, copiar o `zefiro-island.exe` para `target/release/` e abrir `zefiro.exe`.

- [ ] **Step 5: Validação com o usuário**

Mostrar ao usuário (capturas ou ao vivo) as 4 combinações — completa/faixa, completa/pílulas, compacta/faixa, compacta/pílulas — com o Obsidian e com o Porcelain (tema claro), o calendário aberto e o menu aberto. Registrar no ADR-0007 o que ele aprovou e o que pediu para mudar.

- [ ] **Step 6: Commit**

```powershell
git add docs README.md crates/app
git commit -m "docs(adr): 0007 phase 1a bar — measurements, decisions and PLANNING updates" -m "Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```
