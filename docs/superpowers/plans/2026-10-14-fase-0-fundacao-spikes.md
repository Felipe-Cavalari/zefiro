# Zefiro — Fase 0 (Fundação e spikes): Plano de implementação

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Instalar o ambiente, criar o repositório do Zefiro, medir o YASB, executar os 6 spikes (com ADRs e números) e deixar a fundação Rust (workspace, logs, contratos, CI) pronta para a Fase 1.

**Architecture:** O repositório `~/Projects/zefiro` tem o workspace principal (`crates/*`) no `main`. Cada spike é um projeto Cargo **independente** em `spikes/<nome>/`, numa branch e worktree próprias, e **nunca é mesclado**: no `main` entram só os ADRs e os números. A fundação (F2–F4) é escrita depois dos spikes, usando o que eles decidiram.

**Tech Stack:** Rust stable (edição 2024) com MSVC; crate `windows` (Win32, COM, WinRT); `tracing`/`tracing-appender`; PowerShell 7 para scripts; PresentMon para medir quadros; GitHub Actions.

**Spec:** `docs/superpowers/specs/2026-10-14-fase-0-fundacao-spikes-design.md` (leia junto com `docs/PLANNING.md`).

## Global Constraints

- Sistema de referência: Windows 11 Pro build 26200, RTX 5070, 2 monitores 1920×1080 (`DISPLAY2` primário em x=0; `DISPLAY1` em x=−1920), 144 Hz no principal.
- Toolchain: `stable-x86_64-pc-windows-msvc`, componentes `clippy` e `rustfmt`; alvo extra `x86_64-unknown-linux-gnu` só para `cargo check`.
- Edição Rust **2024** em todos os projetos (spikes e workspace).
- Crate `windows` com **uma única versão fixada** em todos os spikes e no workspace: `windows = "=0.61.3"` e `windows-core = "=0.61.2"` (se `cargo` não resolver essas versões, use a mais recente `0.61.x` e anote no ADR-0001). Se uma assinatura do plano não bater com a versão fixada, ajuste à API real e registre a diferença no ADR do spike; não troque a versão no meio da fase.
- Nada de WebView, Electron ou Python no produto.
- Licença do projeto: **MIT OR Apache-2.0**. Código do Seelen-UI (AGPL-3.0) e do Zebar (GPL-3.0) **não pode** ser copiado. Lógica do Dynamic Island (MIT) pode ser portada, sempre registrada em `THIRD_PARTY_NOTICES.md`.
- Spikes: código **descartável**, em `spikes/<nome>/` na branch `spike/<nome>`, cada um com seu próprio `[workspace]` vazio no `Cargo.toml` (não entra no workspace principal). Timebox por spike: S1 5 dias, S2 1, S3 1, S4 1, S5 2, S6 3. Estourou → parar, registrar no ADR e decidir.
- ADRs em `docs/adr/NNNN-<titulo>.md`, seguindo `docs/adr/0000-modelo.md`, sempre com números medidos.
- Metas de performance da fase (spec §8): S1 RAM ≤ 25 MB, CPU em repouso ≤ 0,2 %, primeiro quadro ≤ 250 ms, clique→quadro ≤ 16 ms, ≤ 1 quadro perdido em 50 ciclos a 144 Hz; S6 CPU em transição ≤ 1 %, com visualizador ≤ 1,5 %, GPU ≤ 2 %, RAM +≤ 1 MB em 30 min, 20/20 cliques atravessando.
- Logs em `%LOCALAPPDATA%\zefiro\logs` (rotação diária, 7 arquivos). Tokens só dentro de `Secret<T>`; nunca formatados.
- Todo bloco `unsafe` tem um comentário `// SAFETY:` explicando por que é seguro (spikes e workspace).
- Git: commits locais pequenos, mensagens no estilo Conventional Commits em inglês, terminando com a linha `Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>`. **Nunca** `git push`, criar repositório remoto ou instalar certificado/software sem confirmação explícita do usuário no chat.
- Instalações de software (Build Tools, rustup, PresentMon) e mudanças de sistema (certificado de teste no S2, taxa de atualização do monitor no S6) são feitas **pelo usuário** ou com confirmação explícita dele.
- Textos de documentação em português (pt-BR); identificadores de código em inglês.

## Review Focus

Entradas e condições que a spec implica, mas que nenhum teste automatizado cobre por padrão. Cada linha tem um passo de verificação na tarefa dona:

1. **Monitor conectado/desconectado ou escala (DPI) alterada com o app rodando** → as barras se recriam no monitor certo, no tamanho certo, e a AppBar é registrada de novo (Task 6, Passo 7).
2. **Explorer reiniciado** (barra de tarefas recriada, registros de AppBar perdidos) → a barra se registra de novo ao receber `TaskbarCreated` (Task 6, Passo 8).
3. **Dispositivo gráfico perdido** (driver reiniciado com `Win+Ctrl+Shift+B`, ou suspender/retomar) → o app recria D3D/D2D/compositor e redesenha sem fechar (Task 9, Passo 6).
4. **Dispositivo de áudio padrão trocado** (fone conectado) durante a captura do visualizador → a captura reinicia no novo dispositivo, sem travar (Task 12, Passo 6).
5. **Segredo dentro de uma mensagem de panic ou de log** → o crash report e os logs nunca contêm o valor de um `Secret` (Task 23, Passo 1: testes `logging.rs` e `panic_hook.rs`).

---

## Mapa de arquivos

**No `main` (permanente):**

| Arquivo | Responsabilidade | Tarefa |
|---|---|---|
| `.gitignore`, `README.md`, `LICENSE-MIT`, `LICENSE-APACHE` | Básico do repositório | 1 |
| `THIRD_PARTY_NOTICES.md` | Créditos e licenças | 1, 16 |
| `docs/adr/0000-modelo.md` | Modelo de ADR | 1 |
| `scripts/perf.ps1`, `scripts/tests/perf.Tests.ps1` | Medir RAM/CPU de um processo e filhos | 2 |
| `scripts/frames.ps1`, `scripts/tests/frames.Tests.ps1`, `scripts/tests/fixtures/presentmon-sample.csv` | Contar quadros perdidos num CSV do PresentMon | 3 |
| `docs/baseline.md` | Números do YASB | 4 |
| `docs/adr/0001…0006-*.md` | Decisões dos spikes | 10, 16, 17, 18, 19, 20 |
| `scripts/gpu.ps1`, `scripts/tests/gpu.Tests.ps1` | Uso de GPU de um processo | 16 |
| `Cargo.toml`, `rust-toolchain.toml`, `crates/*/Cargo.toml` | Workspace | 21 |
| `crates/core/src/{lib,secret,observability,state,integration,scheduler,supervisor}.rs`, `crates/core/tests/*.rs` | Núcleo portável | 22–25 |
| `crates/ui/src/{lib,spring}.rs` | Modelo de mola | 26 |
| `crates/resources/src/lib.rs` | Vazia (Fase 1) | 21 |
| `crates/platform/src/{lib,geometry}.rs` | Traits de plataforma | 27 |
| `crates/platform-windows/src/lib.rs` | Vazia por enquanto | 21 |
| `crates/app/src/main.rs` | Binário: logs + panic hook | 21, 23 |
| `.github/workflows/ci.yml` | CI Windows + Linux | 28 |
| `docs/PLANNING.md` | Fechamento da fase | 29 |

**Em branches de spike (descartáveis, nunca mescladas):**

| Pasta | Branch | Tarefas |
|---|---|---|
| `spikes/s1-render/` | `spike/s1-render` | 5–10 |
| `spikes/s6-island/` | `spike/s6-island` (criada a partir de `spike/s1-render`) | 11–16 |
| `spikes/s2-notifications/` | `spike/s2-notifications` | 17 |
| `spikes/s3-ddc/` | `spike/s3-ddc` | 18 |
| `spikes/s4-smtc/` | `spike/s4-smtc` | 19 |
| `spikes/s5-focus-night/` | `spike/s5-focus-night` | 20 |

Os ADRs e números são escritos **no `main`** (worktree principal), mesmo quando o código está numa branch de spike.

---

## Task 0: Pré-requisitos do ambiente

**Files:**
- Create (temporário, fora do repo): `%TEMP%\zefiro-hello\` (projeto de teste)

**Interfaces:**
- Consumes: nada.
- Produces: `cargo`, `rustc`, MSVC e Windows SDK funcionando; PresentMon disponível no PATH.

- [ ] **Step 1: Pedir ao usuário para instalar as ferramentas**

Mostrar ao usuário os comandos abaixo e esperar a confirmação de que rodaram (são instalações de software; o usuário executa ou autoriza explicitamente):

```powershell
winget install --id Microsoft.VisualStudio.2022.BuildTools -e --override "--quiet --wait --add Microsoft.VisualStudio.Workload.VCTools --add Microsoft.VisualStudio.Component.Windows11SDK.26100 --includeRecommended"
```

```powershell
winget install --id Rustlang.Rustup -e
```

```powershell
winget install --id Intel.PresentMon.Console -e
```

Se o ID do PresentMon não existir no winget, baixar o executável de linha de comando em `https://github.com/GameTechDev/PresentMon/releases` e colocar em `%LOCALAPPDATA%\Programs\PresentMon\PresentMon.exe`, adicionando a pasta ao PATH do usuário.

- [ ] **Step 2: Configurar o toolchain (num terminal novo)**

```powershell
rustup default stable-x86_64-pc-windows-msvc
rustup component add clippy rustfmt
rustup target add x86_64-unknown-linux-gnu
rustc --version
cargo --version
```

Expected: versões impressas, sem erro.

- [ ] **Step 3: Criar o "hello world" com a crate `windows`**

```powershell
cargo new "$env:TEMP\zefiro-hello" --edition 2024
cd "$env:TEMP\zefiro-hello"
cargo add windows@=0.61.3 --features Win32_UI_WindowsAndMessaging,Win32_Foundation
```

Substituir `src/main.rs` por:

```rust
use windows::Win32::UI::WindowsAndMessaging::{MessageBoxW, MB_OK};
use windows::core::w;

fn main() {
    // SAFETY: chamada Win32 simples com strings estáticas válidas.
    unsafe {
        MessageBoxW(None, w!("Zefiro: toolchain OK"), w!("Zefiro"), MB_OK);
    }
}
```

- [ ] **Step 4: Rodar e confirmar**

Run: `cargo run`
Expected: aparece uma caixa "Zefiro: toolchain OK". Fechar a caixa encerra o programa.

- [ ] **Step 5: Confirmar o PresentMon**

Run: `PresentMon --help` (ou `PresentMon.exe --help`)
Expected: texto de ajuda impresso.

Sem commit (nada no repositório ainda).

---

## Task 1: Repositório, licenças e documentação base (F1)

**Files:**
- Create: `.gitignore`, `README.md`, `LICENSE-MIT`, `LICENSE-APACHE`, `THIRD_PARTY_NOTICES.md`, `docs/adr/0000-modelo.md`
- Já existem: `docs/PLANNING.md`, `docs/superpowers/specs/2026-10-14-fase-0-fundacao-spikes-design.md`, este plano

**Interfaces:**
- Consumes: Task 0 (git já instalado).
- Produces: repositório git em `~/Projects/zefiro` com branch `main` e o modelo de ADR usado por todos os spikes.

- [ ] **Step 1: Inicializar o git**

```powershell
cd ~/Projects/zefiro
git init -b main
```

- [ ] **Step 2: Criar `.gitignore`**

```gitignore
/target/
**/target/
*.pdb
*.dmp
/scripts/out/
/docs/adr/media/*.mp4
```

- [ ] **Step 3: Criar `README.md`**

```markdown
# Zefiro

Um ambiente de desktop nativo para Windows (barra, ilha de mídia, launcher, dock), sem WebView: leve, fluido e personalizável.

> **Status:** em desenvolvimento (Fase 0: fundação e spikes). Ainda não há versão utilizável.

- Planejamento: [`docs/PLANNING.md`](docs/PLANNING.md)
- Decisões de arquitetura: [`docs/adr/`](docs/adr/)

Licença: MIT OR Apache-2.0.
```

- [ ] **Step 4: Criar as licenças**

`LICENSE-MIT`:

```text
MIT License

Copyright (c) 2026 Felipe Cavalari

Permission is hereby granted, free of charge, to any person obtaining a copy
of this software and associated documentation files (the "Software"), to deal
in the Software without restriction, including without limitation the rights
to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
copies of the Software, and to permit persons to whom the Software is
furnished to do so, subject to the following conditions:

The above copyright notice and this permission notice shall be included in all
copies or substantial portions of the Software.

THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
SOFTWARE.
```

`LICENSE-APACHE`: baixar o texto oficial (não editar):

```powershell
Invoke-WebRequest https://www.apache.org/licenses/LICENSE-2.0.txt -OutFile LICENSE-APACHE
```

- [ ] **Step 5: Criar `THIRD_PARTY_NOTICES.md`**

```markdown
# Avisos de terceiros

## Inspiração (nenhum código copiado)

| Projeto | Licença | O que inspirou |
|---|---|---|
| [Seelen-UI](https://github.com/eythaann/Seelen-UI) | AGPL-3.0 | Módulos (toolbar, dock, WM, launcher), recursos `@autor/nome`, app de configurações. **Só ideias.** |
| [Zebar](https://github.com/glzr-io/zebar) | GPL-3.0 | Modelo de providers reativos, pacotes de widgets. **Só ideias.** |
| PowerToys (Dock da Paleta de Comandos) | MIT | Modo de edição e fluidez. **Só ideias.** |

## Código portado

_(Nenhum ainda. O Dynamic Island for Windows (MIT) entra aqui no S6.)_
```

- [ ] **Step 6: Criar `docs/adr/0000-modelo.md`**

```markdown
# ADR-NNNN: <título curto>

- **Status:** proposto | aceito | substituído por ADR-XXXX
- **Data:** AAAA-MM-DD
- **Spike/branch:** spike/<nome> (commit <hash>)

## Contexto

Qual pergunta precisávamos responder e por quê (link para a seção da spec).

## Opções consideradas

1. Opção A — como funciona, prós, contras.
2. Opção B — ...

## Números medidos

| Métrica | Opção A | Opção B | Meta |
|---|---|---|---|

Ambiente: (sistema, GPU, monitores, taxa de atualização, data).

## Decisão

O que escolhemos, em uma frase, e por quê.

## Consequências

O que muda no PLANNING/fases seguintes; riscos que sobram; plano B.
```

- [ ] **Step 7: Commit**

```powershell
git add .gitignore README.md LICENSE-MIT LICENSE-APACHE THIRD_PARTY_NOTICES.md docs
git commit -m "docs: bootstrap repository with planning, spec, plan and ADR template" -m "Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

---

## Task 2: Script de medição de RAM/CPU (`perf.ps1`)

**Files:**
- Create: `scripts/perf.ps1`
- Test: `scripts/tests/perf.Tests.ps1`

**Interfaces:**
- Consumes: Task 1.
- Produces: `scripts/perf.ps1 -Process <nome> [-WarmupSeconds 60] [-SampleSeconds 300] [-IntervalSeconds 5] [-Out <arquivo.json>]` → JSON com `process`, `samples`, `working_set_mb_median`, `private_mb_median`, `cpu_percent_avg`, `started_at`. Usado no B0, S1 e S6.

- [ ] **Step 1: Escrever o teste que falha**

`scripts/tests/perf.Tests.ps1`:

```powershell
$ErrorActionPreference = 'Stop'
$root = Split-Path -Parent (Split-Path -Parent $PSCommandPath)
$perf = Join-Path $root 'perf.ps1'
$out = Join-Path $env:TEMP "perf-test-$PID.json"

# Processo de teste: PING roda por ~20 s e é fácil de achar pelo nome.
$ping = Start-Process -FilePath 'ping.exe' -ArgumentList '-n','20','127.0.0.1' -WindowStyle Hidden -PassThru
try {
    & $perf -Process 'PING' -WarmupSeconds 1 -SampleSeconds 3 -IntervalSeconds 1 -Out $out
    $r = Get-Content $out -Raw | ConvertFrom-Json
    foreach ($k in 'process','samples','working_set_mb_median','private_mb_median','cpu_percent_avg','started_at') {
        if (-not ($r.PSObject.Properties.Name -contains $k)) { throw "faltou a chave $k" }
    }
    if ($r.samples -lt 3) { throw "esperava >= 3 amostras, veio $($r.samples)" }
    if ($r.working_set_mb_median -le 0) { throw "working set deveria ser > 0" }
    if ($r.cpu_percent_avg -lt 0) { throw "CPU não pode ser negativa" }

    # Processo inexistente precisa falhar com mensagem clara.
    $failed = $false
    try { & $perf -Process 'nao-existe-zefiro' -WarmupSeconds 0 -SampleSeconds 1 -IntervalSeconds 1 -Out $out } catch { $failed = $true }
    if (-not $failed) { throw "processo inexistente deveria falhar" }

    Write-Host 'perf.Tests: OK'
} finally {
    Stop-Process -Id $ping.Id -ErrorAction SilentlyContinue
    Remove-Item $out -ErrorAction SilentlyContinue
}
```

- [ ] **Step 2: Rodar para ver falhar**

Run: `pwsh -NoProfile -File scripts/tests/perf.Tests.ps1`
Expected: FAIL (o arquivo `scripts/perf.ps1` não existe).

- [ ] **Step 3: Implementar `scripts/perf.ps1`**

```powershell
<#
.SYNOPSIS
  Mede RAM e CPU de um processo (somando processos filhos) e grava JSON.
#>
param(
    [Parameter(Mandatory)] [string] $Process,
    [int] $WarmupSeconds = 60,
    [int] $SampleSeconds = 300,
    [int] $IntervalSeconds = 5,
    [string] $Out = (Join-Path (Join-Path $PSScriptRoot 'out') "perf-$Process-$(Get-Date -Format yyyyMMdd-HHmmss).json")
)
$ErrorActionPreference = 'Stop'

function Get-Tree([string] $name) {
    $all = Get-CimInstance Win32_Process
    $roots = $all | Where-Object { $_.Name -ieq "$name.exe" -or $_.Name -ieq $name }
    if (-not $roots) { throw "Processo '$name' não encontrado." }
    $ids = [System.Collections.Generic.HashSet[int]]::new()
    $queue = [System.Collections.Generic.Queue[int]]::new()
    foreach ($r in $roots) { [void]$ids.Add([int]$r.ProcessId); $queue.Enqueue([int]$r.ProcessId) }
    while ($queue.Count -gt 0) {
        $p = $queue.Dequeue()
        foreach ($c in ($all | Where-Object { $_.ParentProcessId -eq $p })) {
            if ($ids.Add([int]$c.ProcessId)) { $queue.Enqueue([int]$c.ProcessId) }
        }
    }
    return @($ids)
}

function Get-Snapshot([int[]] $ids) {
    $ws = 0L; $priv = 0L; $cpu = 0.0
    foreach ($id in $ids) {
        $p = Get-Process -Id $id -ErrorAction SilentlyContinue
        if ($p) { $ws += $p.WorkingSet64; $priv += $p.PrivateMemorySize64; $cpu += $p.TotalProcessorTime.TotalSeconds }
    }
    [pscustomobject]@{ Ws = $ws; Priv = $priv; Cpu = $cpu; At = [DateTime]::UtcNow }
}

function Get-Median([double[]] $v) {
    $s = $v | Sort-Object
    $n = $s.Count
    if ($n % 2) { return $s[[int][math]::Floor($n / 2)] }
    return ($s[$n / 2 - 1] + $s[$n / 2]) / 2
}

$ids = Get-Tree $Process
$started = (Get-Date).ToString('o')
Start-Sleep -Seconds $WarmupSeconds

$cores = [Environment]::ProcessorCount
$prev = Get-Snapshot $ids
$wsList = @(); $privList = @(); $cpuList = @()
$deadline = (Get-Date).AddSeconds($SampleSeconds)
while ((Get-Date) -lt $deadline) {
    Start-Sleep -Seconds $IntervalSeconds
    $ids = Get-Tree $Process
    $cur = Get-Snapshot $ids
    $elapsed = ($cur.At - $prev.At).TotalSeconds
    $wsList += $cur.Ws / 1MB
    $privList += $cur.Priv / 1MB
    $cpuList += [math]::Max(0, ($cur.Cpu - $prev.Cpu) / ($elapsed * $cores) * 100)
    $prev = $cur
}

$result = [ordered]@{
    process               = $Process
    samples               = $wsList.Count
    working_set_mb_median = [math]::Round((Get-Median $wsList), 2)
    private_mb_median     = [math]::Round((Get-Median $privList), 2)
    cpu_percent_avg       = [math]::Round((($cpuList | Measure-Object -Average).Average), 3)
    started_at            = $started
}
New-Item -ItemType Directory -Force -Path (Split-Path -Parent $Out) | Out-Null
$result | ConvertTo-Json | Set-Content -Path $Out -Encoding utf8
$result | ConvertTo-Json
```

- [ ] **Step 4: Rodar o teste**

Run: `pwsh -NoProfile -File scripts/tests/perf.Tests.ps1`
Expected: `perf.Tests: OK`

- [ ] **Step 5: Commit**

```powershell
git add scripts/perf.ps1 scripts/tests/perf.Tests.ps1
git commit -m "chore: add perf.ps1 to measure RAM and CPU of a process tree" -m "Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

---

## Task 3: Script de quadros perdidos (`frames.ps1`)

**Files:**
- Create: `scripts/frames.ps1`
- Test: `scripts/tests/frames.Tests.ps1`, `scripts/tests/fixtures/presentmon-sample.csv`

**Interfaces:**
- Consumes: Task 1.
- Produces: `scripts/frames.ps1 -Csv <arquivo> -RefreshHz <n> [-Column MsBetweenDisplayChange] [-Process dwm.exe]` → objeto/JSON com `frames`, `dropped`, `p50_ms`, `p99_ms`. Um quadro conta como perdido quando o intervalo entre exibições passa de **1,5 ×** o período do monitor. Usado no S1 e no S6.

- [ ] **Step 1: Criar a fixture**

`scripts/tests/fixtures/presentmon-sample.csv` (144 Hz → período 6,944 ms; dois quadros longos):

```csv
Application,ProcessID,MsBetweenDisplayChange
dwm.exe,1000,6.94
dwm.exe,1000,6.95
dwm.exe,1000,13.89
dwm.exe,1000,6.94
dwm.exe,1000,20.83
dwm.exe,1000,6.94
other.exe,2000,50.00
```

- [ ] **Step 2: Escrever o teste que falha**

`scripts/tests/frames.Tests.ps1`:

```powershell
$ErrorActionPreference = 'Stop'
$root = Split-Path -Parent (Split-Path -Parent $PSCommandPath)
$frames = Join-Path $root 'frames.ps1'
$csv = Join-Path $PSScriptRoot 'fixtures/presentmon-sample.csv'

$r = & $frames -Csv $csv -RefreshHz 144 | ConvertFrom-Json
if ($r.frames -ne 6) { throw "esperava 6 quadros do dwm.exe, veio $($r.frames)" }
if ($r.dropped -ne 2) { throw "esperava 2 quadros perdidos, veio $($r.dropped)" }
if ([math]::Abs($r.p50_ms - 6.945) -gt 0.01) { throw "p50 errado: $($r.p50_ms)" }

$failed = $false
try { & $frames -Csv $csv -RefreshHz 144 -Column 'NaoExiste' | Out-Null } catch { $failed = $true }
if (-not $failed) { throw 'coluna inexistente deveria falhar' }
Write-Host 'frames.Tests: OK'
```

- [ ] **Step 3: Rodar para ver falhar**

Run: `pwsh -NoProfile -File scripts/tests/frames.Tests.ps1`
Expected: FAIL (`frames.ps1` não existe).

- [ ] **Step 4: Implementar `scripts/frames.ps1`**

```powershell
<#
.SYNOPSIS
  Conta quadros perdidos num CSV do PresentMon.
.EXAMPLE
  PresentMon --process_name dwm.exe --output_file out.csv --timed 30 --terminate_after_timed
  ./frames.ps1 -Csv out.csv -RefreshHz 144
#>
param(
    [Parameter(Mandatory)] [string] $Csv,
    [Parameter(Mandatory)] [double] $RefreshHz,
    [string] $Column = 'MsBetweenDisplayChange',
    [string] $Process = 'dwm.exe'
)
$ErrorActionPreference = 'Stop'
$rows = Import-Csv $Csv
if (-not $rows -or -not ($rows[0].PSObject.Properties.Name -contains $Column)) {
    throw "Coluna '$Column' não encontrada em $Csv."
}
$values = $rows | Where-Object { $_.Application -ieq $Process } |
    ForEach-Object { [double]::Parse($_.$Column, [Globalization.CultureInfo]::InvariantCulture) } |
    Where-Object { $_ -gt 0 }
$period = 1000.0 / $RefreshHz
$sorted = $values | Sort-Object
function Pct([double[]] $s, [double] $p) {
    if ($s.Count -eq 0) { return 0 }
    $i = [math]::Min($s.Count - 1, [math]::Max(0, [int][math]::Ceiling($p * $s.Count) - 1))
    return $s[$i]
}
$p50 = if ($sorted.Count % 2) { $sorted[[int][math]::Floor($sorted.Count / 2)] } else { ($sorted[$sorted.Count / 2 - 1] + $sorted[$sorted.Count / 2]) / 2 }
[ordered]@{
    frames  = $values.Count
    dropped = @($values | Where-Object { $_ -gt 1.5 * $period }).Count
    p50_ms  = [math]::Round($p50, 3)
    p99_ms  = [math]::Round((Pct $sorted 0.99), 3)
} | ConvertTo-Json
```

- [ ] **Step 5: Rodar o teste**

Run: `pwsh -NoProfile -File scripts/tests/frames.Tests.ps1`
Expected: `frames.Tests: OK`

- [ ] **Step 6: Commit**

```powershell
git add scripts/frames.ps1 scripts/tests/frames.Tests.ps1 scripts/tests/fixtures/presentmon-sample.csv
git commit -m "chore: add frames.ps1 to count dropped frames from PresentMon CSV" -m "Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

---

## Task 4: B0 — Medição de referência do YASB

**Files:**
- Create: `docs/baseline.md`

**Interfaces:**
- Consumes: Task 2 (`perf.ps1`).
- Produces: `docs/baseline.md` com os números do YASB (referência das metas).

- [ ] **Step 1: Subir YASB e komorebi como no dia a dia**

```powershell
komorebic start
Start-Process "C:\Program Files\YASB\yasb.exe"
```

Esperar a barra aparecer nos dois monitores. Medir com cronômetro o tempo do comando até a barra aparecer (3 vezes, fechando o YASB entre elas: `Stop-Process -Name yasb`).

- [ ] **Step 2: Medir 3 vezes**

```powershell
1..3 | ForEach-Object { pwsh -NoProfile -File scripts/perf.ps1 -Process yasb -WarmupSeconds 60 -SampleSeconds 300 -Out "scripts/out/yasb-$_.json" }
```

Expected: três arquivos JSON em `scripts/out/`.

- [ ] **Step 3: Escrever `docs/baseline.md`**

Preencher com a **mediana** das três execuções (valores reais lidos dos JSONs):

```markdown
# Medição de referência (B0)

- **Data:** AAAA-MM-DD
- **Ambiente:** Windows 11 Pro 26200, RTX 5070, 2× 1920×1080, 144 Hz
- **Configuração:** YASB v2.0.7, tema CavalariDev, 2 barras, komorebi ligado

| Métrica | Execução 1 | Execução 2 | Execução 3 | Mediana |
|---|---|---|---|---|
| Working set (MB) | | | | |
| Private bytes (MB) | | | | |
| CPU média em repouso (%) | | | | |
| Tempo até a barra aparecer (s) | | | | |

Metas do Zefiro (PLANNING §1): RAM ≤ 35 MB, CPU ≤ 0,3 %, primeiro quadro ≤ 0,25 s.
```

Se o Seelen-UI estiver instalado, repetir a medição para ele (`-Process seelen-ui`) e adicionar uma coluna. Se não estiver, escrever "Seelen-UI: não medido".

- [ ] **Step 4: Commit**

```powershell
git add docs/baseline.md
git commit -m "docs: record YASB baseline measurements (B0)" -m "Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

---

## Task 5: S1 — Projeto do spike, argumentos e modelo de mola

**Files:**
- Create: `spikes/s1-render/Cargo.toml`, `spikes/s1-render/src/main.rs`, `spikes/s1-render/src/args.rs`, `spikes/s1-render/src/spring.rs`

**Interfaces:**
- Consumes: Task 1 (repositório).
- Produces:
  - `args::parse(&[&str]) -> Result<Args, String>`; `Args { path: PathKind, acrylic: AcrylicKind, anim: AnimMode, cycles: u32 }`; `PathKind::{A, B}`, `AcrylicKind::{A1, A2, Host}`, `AnimMode::{Compositor, Frame}`.
  - `spring::{Spring, SpringParams, SpringStyle, FIXED_DT, SETTLE_EPS, sample}`; `Spring { value, velocity, target }` com `new(f32)`, `step(dt: f32, p: SpringParams)`, `is_settled()`; `SpringParams { stiffness, damping }` com `GROW`, `SHRINK`, `DEFAULT`, `styled(SpringStyle)`, `to_period_and_ratio() -> (f32, f32)`; `sample(from, to, p, step, max_secs) -> Vec<(f32, f32)>`.

- [ ] **Step 1: Criar a branch e o worktree do spike**

```powershell
cd ~/Projects/zefiro
git worktree add ../zefiro-s1 -b spike/s1-render
cd ../zefiro-s1
cargo new spikes/s1-render --edition 2024 --vcs none
```

- [ ] **Step 2: Escrever `spikes/s1-render/Cargo.toml`**

```toml
[package]
name = "s1-render"
version = "0.0.0"
edition = "2024"
publish = false

# Projeto independente: não entra no workspace principal.
[workspace]

[dependencies]
windows-core = "=0.61.2"
windows = { version = "=0.61.3", features = [
  "Foundation", "Foundation_Numerics", "Graphics", "Graphics_DirectX", "System", "UI",
  "UI_Composition", "UI_Composition_Desktop",
  "Win32_Foundation", "Win32_Graphics_Direct2D", "Win32_Graphics_Direct2D_Common",
  "Win32_Graphics_Direct3D", "Win32_Graphics_Direct3D11", "Win32_Graphics_DirectComposition",
  "Win32_Graphics_DirectWrite", "Win32_Graphics_Dwm", "Win32_Graphics_Dxgi",
  "Win32_Graphics_Dxgi_Common", "Win32_Graphics_Gdi", "Win32_System_Com",
  "Win32_System_LibraryLoader", "Win32_System_WinRT", "Win32_System_WinRT_Composition",
  "Win32_UI_HiDpi", "Win32_UI_Shell", "Win32_UI_WindowsAndMessaging",
] }
```

- [ ] **Step 3: Escrever os testes de argumentos (falhando)**

`spikes/s1-render/src/args.rs`:

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PathKind { A, B }

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AcrylicKind { A1, A2, Host }

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AnimMode { Compositor, Frame }

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Args {
    pub path: PathKind,
    pub acrylic: AcrylicKind,
    pub anim: AnimMode,
    pub cycles: u32,
}

pub fn parse(_args: &[&str]) -> Result<Args, String> {
    Err("não implementado".into())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_to_path_b_with_host_backdrop() {
        assert_eq!(
            parse(&[]).unwrap(),
            Args { path: PathKind::B, acrylic: AcrylicKind::Host, anim: AnimMode::Compositor, cycles: 0 }
        );
    }

    #[test]
    fn path_a_defaults_to_a1() {
        assert_eq!(parse(&["--path", "a"]).unwrap().acrylic, AcrylicKind::A1);
    }

    #[test]
    fn parses_all_flags() {
        let a = parse(&["--path", "a", "--acrylic", "a2", "--anim", "frame", "--cycles", "50"]).unwrap();
        assert_eq!(a, Args { path: PathKind::A, acrylic: AcrylicKind::A2, anim: AnimMode::Frame, cycles: 50 });
    }

    #[test]
    fn rejects_host_backdrop_on_path_a() {
        assert!(parse(&["--path", "a", "--acrylic", "host"]).is_err());
    }

    #[test]
    fn rejects_unknown_flag_and_missing_value() {
        assert!(parse(&["--banana"]).is_err());
        assert!(parse(&["--cycles"]).is_err());
        assert!(parse(&["--cycles", "muitos"]).is_err());
    }
}
```

`spikes/s1-render/src/main.rs` (provisório, só para compilar os testes):

```rust
mod args;
mod spring;

fn main() {}
```

`spikes/s1-render/src/spring.rs`: criar o arquivo **vazio** por enquanto (o conteúdo vem no Passo 7).

- [ ] **Step 4: Rodar para ver falhar**

Run: `cargo test --manifest-path spikes/s1-render/Cargo.toml args`
Expected: FAIL (`parse` devolve erro).

- [ ] **Step 5: Implementar `parse`**

Substituir a função `parse` em `args.rs`:

```rust
pub fn parse(args: &[&str]) -> Result<Args, String> {
    let mut path = PathKind::B;
    let mut acrylic: Option<AcrylicKind> = None;
    let mut anim = AnimMode::Compositor;
    let mut cycles = 0u32;
    let mut it = args.iter();
    while let Some(flag) = it.next() {
        let mut value = || it.next().copied().ok_or_else(|| format!("faltou o valor de {flag}"));
        match *flag {
            "--path" => path = match value()? { "a" => PathKind::A, "b" => PathKind::B, v => return Err(format!("--path inválido: {v}")) },
            "--acrylic" => acrylic = Some(match value()? { "a1" => AcrylicKind::A1, "a2" => AcrylicKind::A2, "host" => AcrylicKind::Host, v => return Err(format!("--acrylic inválido: {v}")) }),
            "--anim" => anim = match value()? { "compositor" => AnimMode::Compositor, "frame" => AnimMode::Frame, v => return Err(format!("--anim inválido: {v}")) },
            "--cycles" => cycles = value()?.parse().map_err(|_| "--cycles precisa ser um número".to_string())?,
            other => return Err(format!("argumento desconhecido: {other}")),
        }
    }
    let acrylic = acrylic.unwrap_or(match path { PathKind::A => AcrylicKind::A1, PathKind::B => AcrylicKind::Host });
    if path == PathKind::A && acrylic == AcrylicKind::Host {
        return Err("o acrílico 'host' só existe no caminho B".into());
    }
    Ok(Args { path, acrylic, anim, cycles })
}
```

- [ ] **Step 6: Rodar os testes de argumentos**

Run: `cargo test --manifest-path spikes/s1-render/Cargo.toml args`
Expected: PASS (5 testes).

- [ ] **Step 7: Escrever os testes da mola (falhando)**

`spikes/s1-render/src/spring.rs`:

```rust
//! Modelo de mola portado do Dynamic Island for Windows (MIT):
//! Euler semi-implícito com passos fixos de 0,5 ms, massa 1.

pub const FIXED_DT: f32 = 0.0005;
pub const SETTLE_EPS: f32 = 0.01;
pub const MAX_DT: f32 = 0.050;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SpringStyle { Smooth, Default, Bouncy, Snappy }

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SpringParams { pub stiffness: f32, pub damping: f32 }

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Spring { pub value: f32, pub velocity: f32, pub target: f32 }

#[cfg(test)]
mod tests {
    use super::*;

    fn run(mut s: Spring, p: SpringParams, secs: f32) -> (Spring, f32) {
        let mut max = s.value;
        let mut t = 0.0;
        while t < secs {
            s.step(1.0 / 144.0, p);
            max = max.max(s.value);
            t += 1.0 / 144.0;
        }
        (s, max)
    }

    #[test]
    fn every_style_settles_on_target_within_two_seconds() {
        for style in [SpringStyle::Smooth, SpringStyle::Default, SpringStyle::Bouncy, SpringStyle::Snappy] {
            for base in [SpringParams::GROW, SpringParams::SHRINK, SpringParams::DEFAULT] {
                let mut s = Spring::new(0.0);
                s.target = 100.0;
                let (s, _) = run(s, base.styled(style), 2.0);
                assert_eq!(s.value, 100.0, "{style:?} {base:?}");
                assert!(s.is_settled());
            }
        }
    }

    #[test]
    fn bouncy_overshoots_and_smooth_does_not() {
        let mut s = Spring::new(0.0);
        s.target = 100.0;
        let (_, bouncy_max) = run(s, SpringParams::DEFAULT.styled(SpringStyle::Bouncy), 2.0);
        let (_, smooth_max) = run(s, SpringParams::DEFAULT.styled(SpringStyle::Smooth), 2.0);
        assert!(bouncy_max > 101.0, "elástico deveria passar do alvo: {bouncy_max}");
        assert!(smooth_max < 100.1, "suave não deveria passar do alvo: {smooth_max}");
    }

    #[test]
    fn style_multipliers_match_dynamic_island() {
        let p = SpringParams::DEFAULT.styled(SpringStyle::Snappy);
        assert!((p.stiffness - 420.0).abs() < 1e-3);
        assert!((p.damping - 30.0).abs() < 1e-3);
    }

    #[test]
    fn huge_dt_is_clamped_to_50ms() {
        let mut a = Spring::new(0.0);
        a.target = 100.0;
        let mut b = a;
        a.step(10.0, SpringParams::DEFAULT);
        b.step(MAX_DT, SpringParams::DEFAULT);
        assert_eq!(a, b);
    }

    #[test]
    fn converts_to_compositor_period_and_ratio() {
        let (period, ratio) = SpringParams::DEFAULT.to_period_and_ratio();
        assert!((period - 0.37549).abs() < 1e-3, "{period}");
        assert!((ratio - 0.71714).abs() < 1e-3, "{ratio}");
    }

    #[test]
    fn sample_starts_at_from_and_ends_at_target() {
        let s = sample(0.0, 1.0, SpringParams::GROW, 1.0 / 240.0, 2.0);
        assert_eq!(s.first().copied(), Some((0.0, 0.0)));
        assert_eq!(s.last().unwrap().1, 1.0);
        assert!(s.windows(2).all(|w| w[1].0 > w[0].0));
    }
}
```

- [ ] **Step 8: Rodar para ver falhar**

Run: `cargo test --manifest-path spikes/s1-render/Cargo.toml spring`
Expected: FAIL de compilação (`new`, `step`, `styled`, `sample`… não existem).

- [ ] **Step 9: Implementar a mola**

Adicionar em `spring.rs`, acima do `#[cfg(test)]`:

```rust
impl SpringStyle {
    /// (multiplicador de rigidez, multiplicador de amortecimento)
    pub fn multipliers(self) -> (f32, f32) {
        match self {
            SpringStyle::Smooth => (1.0, 1.35),
            SpringStyle::Default => (1.0, 1.0),
            SpringStyle::Bouncy => (1.1, 0.70),
            SpringStyle::Snappy => (1.5, 1.25),
        }
    }
}

impl SpringParams {
    pub const GROW: Self = Self { stiffness: 380.0, damping: 26.0 };
    pub const SHRINK: Self = Self { stiffness: 200.0, damping: 28.0 };
    pub const DEFAULT: Self = Self { stiffness: 280.0, damping: 24.0 };

    pub fn styled(self, style: SpringStyle) -> Self {
        let (s, d) = style.multipliers();
        Self { stiffness: self.stiffness * s, damping: self.damping * d }
    }

    /// Converte para o modelo do compositor: (período em segundos, razão de amortecimento).
    pub fn to_period_and_ratio(self) -> (f32, f32) {
        let omega = self.stiffness.sqrt();
        (2.0 * std::f32::consts::PI / omega, self.damping / (2.0 * omega))
    }
}

impl Spring {
    pub fn new(value: f32) -> Self {
        Self { value, velocity: 0.0, target: value }
    }

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

/// Amostra a mola de `from` até `to` em passos de `step` segundos (para curvas do DirectComposition).
pub fn sample(from: f32, to: f32, p: SpringParams, step: f32, max_secs: f32) -> Vec<(f32, f32)> {
    let mut s = Spring::new(from);
    s.target = to;
    let mut out = vec![(0.0, from)];
    let mut t = 0.0;
    while t < max_secs {
        s.step(step, p);
        t += step;
        out.push((t, s.value));
        if s.is_settled() {
            break;
        }
    }
    if let Some(last) = out.last_mut() {
        last.1 = to;
    }
    out
}
```

- [ ] **Step 10: Rodar todos os testes**

Run: `cargo test --manifest-path spikes/s1-render/Cargo.toml`
Expected: PASS (11 testes).

- [ ] **Step 11: Commit (na branch do spike)**

```powershell
git add spikes/s1-render
git commit -m "spike(s1): add args parsing and spring model" -m "Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

---

## Task 6: S1 — Janelas por monitor e AppBar

**Files:**
- Create: `spikes/s1-render/src/events.rs`, `spikes/s1-render/src/window.rs`, `spikes/s1-render/src/monitors.rs`, `spikes/s1-render/src/appbar.rs`, `spikes/s1-render/src/timing.rs`
- Modify: `spikes/s1-render/src/main.rs`

**Interfaces:**
- Consumes: Task 5 (`args`).
- Produces:
  - `events::{Event, push, drain}`; `Event::{Click { hwnd, x, y, at: Instant }, Timer { id: usize }, AppBar { hwnd, notification: u32, flag: isize }, DpiChanged { hwnd, dpi: u32 }, DisplayChanged, TaskbarCreated, Close}`.
  - `window::{register_class() -> Result<()>, create_popup(RECT) -> Result<HWND>, pump_blocking() -> bool, pump_nonblocking() -> bool}`.
  - `monitors::{MonitorInfo { handle, rect, work, primary, dpi, refresh_hz, device }, list() -> Vec<MonitorInfo>}`.
  - `appbar::{WM_APPBAR_CALLBACK, register(hwnd, monitor_rect, height) -> RECT, set_pos(hwnd, monitor_rect, height) -> RECT, unregister(hwnd)}`.
  - `timing::{mark_start(), since_start_ms() -> f64, log(label, ms)}`.

- [ ] **Step 1: Escrever `events.rs`**

```rust
use std::cell::RefCell;
use std::collections::VecDeque;
use std::time::Instant;
use windows::Win32::Foundation::HWND;

/// Eventos vindos do wndproc. O wndproc só enfileira; o loop principal trata.
/// Assim nenhum handler roda dentro de outro (sem reentrância).
#[derive(Debug, Clone, Copy)]
pub enum Event {
    Click { hwnd: HWND, x: i32, y: i32, at: Instant },
    Timer { id: usize },
    AppBar { hwnd: HWND, notification: u32, flag: isize },
    DpiChanged { hwnd: HWND, dpi: u32 },
    DisplayChanged,
    TaskbarCreated,
    Close,
}

thread_local! {
    static QUEUE: RefCell<VecDeque<Event>> = const { RefCell::new(VecDeque::new()) };
}

pub fn push(e: Event) {
    QUEUE.with(|q| q.borrow_mut().push_back(e));
}

pub fn drain() -> Vec<Event> {
    QUEUE.with(|q| q.borrow_mut().drain(..).collect())
}
```

- [ ] **Step 2: Escrever `timing.rs`**

```rust
use std::sync::OnceLock;
use std::time::{Duration, Instant};

static START: OnceLock<Instant> = OnceLock::new();

/// Chamar como primeira linha do `main`.
pub fn mark_start() {
    let _ = START.set(Instant::now());
}

pub fn since_start_ms() -> f64 {
    START.get().map(|s| s.elapsed().as_secs_f64() * 1000.0).unwrap_or(0.0)
}

pub fn log(label: &str, d: Duration) {
    eprintln!("[timing] {label}: {:.2} ms", d.as_secs_f64() * 1000.0);
}
```

- [ ] **Step 3: Escrever `window.rs`**

```rust
use std::sync::OnceLock;
use std::time::Instant;
use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, RECT, WPARAM};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::WindowsAndMessaging::*;
use windows::core::{PCWSTR, Result, w};

use crate::appbar::WM_APPBAR_CALLBACK;
use crate::events::{Event, push};

const CLASS: PCWSTR = w!("ZefiroSpikeS1");
static TASKBAR_CREATED: OnceLock<u32> = OnceLock::new();

pub fn register_class() -> Result<()> {
    // SAFETY: estruturas inicializadas por completo; nome de classe estático.
    unsafe {
        let instance = GetModuleHandleW(None)?;
        let wc = WNDCLASSEXW {
            cbSize: size_of::<WNDCLASSEXW>() as u32,
            lpfnWndProc: Some(wndproc),
            hInstance: instance.into(),
            lpszClassName: CLASS,
            hCursor: LoadCursorW(None, IDC_ARROW)?,
            ..Default::default()
        };
        if RegisterClassExW(&wc) == 0 {
            return Err(windows::core::Error::from_win32());
        }
        let _ = TASKBAR_CREATED.set(RegisterWindowMessageW(w!("TaskbarCreated")));
    }
    Ok(())
}

pub fn create_popup(rect: RECT) -> Result<HWND> {
    // SAFETY: classe registrada em `register_class`; sem parâmetros de criação.
    unsafe {
        let instance = GetModuleHandleW(None)?;
        CreateWindowExW(
            WS_EX_TOOLWINDOW | WS_EX_NOREDIRECTIONBITMAP | WS_EX_TOPMOST | WS_EX_NOACTIVATE,
            CLASS,
            w!("Zefiro S1"),
            WS_POPUP,
            rect.left,
            rect.top,
            rect.right - rect.left,
            rect.bottom - rect.top,
            None,
            None,
            Some(instance.into()),
            None,
        )
    }
}

extern "system" fn wndproc(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    let x = (lparam.0 & 0xFFFF) as i16 as i32;
    let y = ((lparam.0 >> 16) & 0xFFFF) as i16 as i32;
    match msg {
        WM_LBUTTONDOWN => push(Event::Click { hwnd, x, y, at: Instant::now() }),
        // Botão direito fecha o spike.
        WM_RBUTTONUP | WM_CLOSE => push(Event::Close),
        WM_TIMER => push(Event::Timer { id: wparam.0 }),
        WM_DPICHANGED => push(Event::DpiChanged { hwnd, dpi: (wparam.0 & 0xFFFF) as u32 }),
        WM_DISPLAYCHANGE => push(Event::DisplayChanged),
        WM_MOUSEACTIVATE => return LRESULT(MA_NOACTIVATE as isize),
        m if m == WM_APPBAR_CALLBACK => {
            push(Event::AppBar { hwnd, notification: wparam.0 as u32, flag: lparam.0 })
        }
        m if Some(&m) == TASKBAR_CREATED.get() => push(Event::TaskbarCreated),
        // SAFETY: repassa mensagens não tratadas ao Windows.
        _ => return unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) },
    }
    LRESULT(0)
}

/// Espera a próxima mensagem. Devolve `false` em WM_QUIT.
pub fn pump_blocking() -> bool {
    let mut msg = MSG::default();
    // SAFETY: `msg` é um buffer válido.
    unsafe {
        if !GetMessageW(&mut msg, None, 0, 0).as_bool() {
            return false;
        }
        let _ = TranslateMessage(&msg);
        DispatchMessageW(&msg);
    }
    true
}

/// Processa as mensagens pendentes sem bloquear. Devolve `false` em WM_QUIT.
pub fn pump_nonblocking() -> bool {
    let mut msg = MSG::default();
    // SAFETY: `msg` é um buffer válido.
    unsafe {
        while PeekMessageW(&mut msg, None, 0, 0, PM_REMOVE).as_bool() {
            if msg.message == WM_QUIT {
                return false;
            }
            let _ = TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }
    }
    true
}
```

- [ ] **Step 4: Escrever `monitors.rs`**

```rust
use windows::Win32::Foundation::{BOOL, LPARAM, RECT, TRUE};
use windows::Win32::Graphics::Gdi::*;
use windows::Win32::UI::HiDpi::{GetDpiForMonitor, MDT_EFFECTIVE_DPI};
use windows::Win32::UI::WindowsAndMessaging::MONITORINFOF_PRIMARY;
use windows::core::PCWSTR;

#[derive(Debug, Clone)]
pub struct MonitorInfo {
    pub handle: HMONITOR,
    pub rect: RECT,
    pub work: RECT,
    pub primary: bool,
    pub dpi: u32,
    pub refresh_hz: u32,
    pub device: String,
}

pub fn list() -> Vec<MonitorInfo> {
    let mut out: Vec<MonitorInfo> = Vec::new();

    unsafe extern "system" fn cb(h: HMONITOR, _: HDC, _: *mut RECT, data: LPARAM) -> BOOL {
        // SAFETY: `data` aponta para o Vec criado em `list`, vivo durante a enumeração.
        let out = unsafe { &mut *(data.0 as *mut Vec<MonitorInfo>) };
        let mut mi = MONITORINFOEXW::default();
        mi.monitorInfo.cbSize = size_of::<MONITORINFOEXW>() as u32;
        // SAFETY: buffer do tamanho correto.
        if unsafe { GetMonitorInfoW(h, &mut mi as *mut _ as *mut MONITORINFO) }.as_bool() {
            let (mut dx, mut dy) = (96u32, 96u32);
            // SAFETY: ponteiros válidos para u32.
            let _ = unsafe { GetDpiForMonitor(h, MDT_EFFECTIVE_DPI, &mut dx, &mut dy) };
            let mut dm = DEVMODEW { dmSize: size_of::<DEVMODEW>() as u16, ..Default::default() };
            // SAFETY: `szDevice` é uma string terminada em zero dentro de `mi`.
            let hz = if unsafe { EnumDisplaySettingsW(PCWSTR(mi.szDevice.as_ptr()), ENUM_CURRENT_SETTINGS, &mut dm) }.as_bool() {
                dm.dmDisplayFrequency
            } else {
                60
            };
            out.push(MonitorInfo {
                handle: h,
                rect: mi.monitorInfo.rcMonitor,
                work: mi.monitorInfo.rcWork,
                primary: mi.monitorInfo.dwFlags & MONITORINFOF_PRIMARY != 0,
                dpi: dx,
                refresh_hz: hz,
                device: String::from_utf16_lossy(&mi.szDevice).trim_end_matches('\0').to_string(),
            });
        }
        TRUE
    }

    // SAFETY: callback acima respeita o contrato de EnumDisplayMonitors.
    unsafe {
        let _ = EnumDisplayMonitors(None, None, Some(cb), LPARAM(&mut out as *mut _ as isize));
    }
    out.sort_by_key(|m| !m.primary); // primário primeiro
    out
}
```

- [ ] **Step 5: Escrever `appbar.rs`**

```rust
use windows::Win32::Foundation::{HWND, RECT};
use windows::Win32::UI::Shell::*;
use windows::Win32::UI::WindowsAndMessaging::WM_APP;

pub const WM_APPBAR_CALLBACK: u32 = WM_APP + 1;

fn data(hwnd: HWND) -> APPBARDATA {
    APPBARDATA {
        cbSize: size_of::<APPBARDATA>() as u32,
        hWnd: hwnd,
        uCallbackMessage: WM_APPBAR_CALLBACK,
        uEdge: ABE_TOP,
        ..Default::default()
    }
}

/// Registra a janela como AppBar no topo do monitor e devolve o retângulo final.
pub fn register(hwnd: HWND, monitor_rect: RECT, height: i32) -> RECT {
    let mut abd = data(hwnd);
    // SAFETY: `abd` inicializado com cbSize correto.
    unsafe { SHAppBarMessage(ABM_NEW, &mut abd) };
    set_pos(hwnd, monitor_rect, height)
}

pub fn set_pos(hwnd: HWND, monitor_rect: RECT, height: i32) -> RECT {
    let mut abd = data(hwnd);
    abd.rc = RECT { left: monitor_rect.left, top: monitor_rect.top, right: monitor_rect.right, bottom: monitor_rect.top + height };
    // SAFETY: `abd` válido.
    unsafe { SHAppBarMessage(ABM_QUERYPOS, &mut abd) };
    abd.rc.bottom = abd.rc.top + height;
    // SAFETY: `abd` válido.
    unsafe { SHAppBarMessage(ABM_SETPOS, &mut abd) };
    abd.rc
}

pub fn unregister(hwnd: HWND) {
    let mut abd = data(hwnd);
    // SAFETY: `abd` válido.
    unsafe { SHAppBarMessage(ABM_REMOVE, &mut abd) };
}
```

- [ ] **Step 6: Escrever o `main.rs` desta tarefa (barras vazias)**

```rust
mod appbar;
mod args;
mod events;
mod monitors;
mod spring;
mod timing;
mod window;

use events::Event;
use windows::Win32::Foundation::HWND;
use windows::Win32::UI::HiDpi::{DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2, SetProcessDpiAwarenessContext};
use windows::Win32::UI::Shell::ABN_FULLSCREENAPP;
use windows::Win32::UI::WindowsAndMessaging::*;

const BAR_HEIGHT_DIP: i32 = 32;

struct Bar { hwnd: HWND, monitor_rect: windows::Win32::Foundation::RECT, height: i32 }

fn create_bars() -> windows::core::Result<Vec<Bar>> {
    let mut bars = Vec::new();
    for m in monitors::list() {
        let height = BAR_HEIGHT_DIP * m.dpi as i32 / 96;
        let hwnd = window::create_popup(m.rect)?;
        let rect = appbar::register(hwnd, m.rect, height);
        // SAFETY: hwnd recém-criado.
        unsafe {
            SetWindowPos(hwnd, Some(HWND_TOPMOST), rect.left, rect.top, rect.right - rect.left, rect.bottom - rect.top, SWP_NOACTIVATE)?;
            let _ = ShowWindow(hwnd, SW_SHOWNOACTIVATE);
        }
        eprintln!("[bar] {} rect={rect:?} dpi={} hz={}", m.device, m.dpi, m.refresh_hz);
        bars.push(Bar { hwnd, monitor_rect: m.rect, height });
    }
    Ok(bars)
}

fn destroy_bars(bars: &mut Vec<Bar>) {
    for b in bars.drain(..) {
        appbar::unregister(b.hwnd);
        // SAFETY: janela criada por nós.
        unsafe { let _ = DestroyWindow(b.hwnd); }
    }
}

fn main() -> windows::core::Result<()> {
    timing::mark_start();
    // SAFETY: chamada única no início do processo.
    unsafe { SetProcessDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2)? };
    window::register_class()?;
    let mut bars = create_bars()?;
    while window::pump_blocking() {
        for e in events::drain() {
            match e {
                Event::AppBar { hwnd, notification, flag } if notification == ABN_FULLSCREENAPP => {
                    let show = if flag != 0 { SW_HIDE } else { SW_SHOWNOACTIVATE };
                    // SAFETY: hwnd é uma das nossas barras.
                    unsafe { let _ = ShowWindow(hwnd, show); }
                    eprintln!("[appbar] tela cheia={}", flag != 0);
                }
                Event::DpiChanged { .. } | Event::DisplayChanged => {
                    eprintln!("[monitor] mudou: recriando barras");
                    destroy_bars(&mut bars);
                    bars = create_bars()?;
                }
                Event::TaskbarCreated => {
                    eprintln!("[explorer] TaskbarCreated: registrando AppBars de novo");
                    for b in &bars { appbar::register(b.hwnd, b.monitor_rect, b.height); }
                }
                Event::Close => {
                    destroy_bars(&mut bars);
                    // SAFETY: encerra o loop de mensagens.
                    unsafe { PostQuitMessage(0) };
                }
                _ => {}
            }
        }
    }
    Ok(())
}
```

- [ ] **Step 7: Verificar o espaço reservado e mudanças de monitor (Review Focus 1)**

Run: `cargo run --manifest-path spikes/s1-render/Cargo.toml`

As barras não têm conteúdo ainda (ficam invisíveis), mas:
1. O log mostra uma linha `[bar] …` por monitor, com `rect` de altura 32 px a 100 % de escala.
2. Maximizar uma janela em cada monitor: o topo dela fica **32 px abaixo** do topo da tela.
3. Mudar a escala de um monitor para 125 % em Configurações → Tela: o log mostra `[monitor] mudou: recriando barras` e a nova altura é 40 px. Voltar para 100 %.
4. `Win+P` → "Somente tela do computador" e depois "Estender": o log mostra recriação, e a contagem de barras acompanha (1 → 2).

Expected: todos os 4 itens acontecem. Fechar com `Stop-Process -Name s1-render` (ainda não há onde clicar com o botão direito).

- [ ] **Step 8: Verificar o reinício do Explorer (Review Focus 2)**

Com o spike rodando:

```powershell
Stop-Process -Name explorer -Force; Start-Sleep 2; Start-Process explorer
```

Expected: o log mostra `[explorer] TaskbarCreated: registrando AppBars de novo`, e janelas maximizadas continuam começando 32 px abaixo do topo.

- [ ] **Step 9: Commit**

```powershell
git add spikes/s1-render
git commit -m "spike(s1): per-monitor AppBar windows with DPI, display and explorer restart handling" -m "Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

---

## Task 7: S1 — Caminho A (DirectComposition) com conteúdo Direct2D e acrílico do DWM

**Files:**
- Create: `spikes/s1-render/src/gfx.rs`, `spikes/s1-render/src/backdrop.rs`, `spikes/s1-render/src/backend.rs`, `spikes/s1-render/src/path_a.rs`, `spikes/s1-render/src/app.rs`
- Modify: `spikes/s1-render/src/main.rs` (substituir por inteiro)

**Interfaces:**
- Consumes: Tasks 5–6.
- Produces:
  - `gfx::{Gfx { d3d, dxgi, d2d_device, dwrite, text, icons }, Gfx::new(), draw_test_content(dc, gfx, w, h, accent_hex), draw_flyout_content(dc, gfx, w, h), rgba(u32)}`.
  - `backdrop::apply(hwnd, AcrylicKind) -> Result<()>`.
  - `backend::{Backend, LayerKind::{Bar, Flyout}, Pose { opacity, scale, offset_y }, Pose::OPEN, Pose::CLOSED, create(PathKind, &Gfx) -> Result<Box<dyn Backend>>}`; métodos do trait: `add_layer(hwnd, w, h, kind) -> Result<usize>`, `draw(layer, &Gfx, &dyn Fn(&ID2D1DeviceContext) -> Result<()>) -> Result<()>`, `set_pose(layer, Pose)`, `animate_to(layer, Pose, SpringParams)`, `commit()`.
  - `app::{App, App::new(Args), App::handle(Event) -> Result<bool>, App::toggle_flyout(Option<Instant>)}`.

- [ ] **Step 1: Escrever `gfx.rs`**

```rust
use windows::Foundation::Numerics::Matrix3x2;
use windows::Win32::Foundation::HMODULE;
use windows::Win32::Graphics::Direct2D::Common::*;
use windows::Win32::Graphics::Direct2D::*;
use windows::Win32::Graphics::Direct3D::D3D_DRIVER_TYPE_HARDWARE;
use windows::Win32::Graphics::Direct3D11::*;
use windows::Win32::Graphics::DirectWrite::*;
use windows::Win32::Graphics::Dxgi::IDXGIDevice;
use windows::Win32::UI::Shell::SHCreateMemStream;
use windows::core::{Interface, Result, w};

pub struct Gfx {
    pub d3d: ID3D11Device,
    pub dxgi: IDXGIDevice,
    pub d2d_device: ID2D1Device,
    pub dwrite: IDWriteFactory,
    pub text: IDWriteTextFormat,
    pub icons: IDWriteTextFormat,
}

pub fn rgba(hex: u32) -> D2D1_COLOR_F {
    D2D1_COLOR_F {
        r: ((hex >> 24) & 0xFF) as f32 / 255.0,
        g: ((hex >> 16) & 0xFF) as f32 / 255.0,
        b: ((hex >> 8) & 0xFF) as f32 / 255.0,
        a: (hex & 0xFF) as f32 / 255.0,
    }
}

impl Gfx {
    pub fn new() -> Result<Self> {
        // SAFETY: criação padrão de dispositivos D3D11/D2D/DWrite; ponteiros de saída válidos.
        unsafe {
            let mut d3d = None;
            D3D11CreateDevice(
                None,
                D3D_DRIVER_TYPE_HARDWARE,
                HMODULE::default(),
                D3D11_CREATE_DEVICE_BGRA_SUPPORT,
                None,
                D3D11_SDK_VERSION,
                Some(&mut d3d),
                None,
                None,
            )?;
            let d3d = d3d.expect("D3D11CreateDevice devolveu sucesso sem dispositivo");
            let dxgi: IDXGIDevice = d3d.cast()?;
            let factory: ID2D1Factory1 = D2D1CreateFactory(D2D1_FACTORY_TYPE_SINGLE_THREADED, None)?;
            let d2d_device = factory.CreateDevice(&dxgi)?;
            let dwrite: IDWriteFactory = DWriteCreateFactory(DWRITE_FACTORY_TYPE_SHARED)?;
            let text = dwrite.CreateTextFormat(
                w!("Segoe UI Variable Text"), None, DWRITE_FONT_WEIGHT_SEMI_BOLD,
                DWRITE_FONT_STYLE_NORMAL, DWRITE_FONT_STRETCH_NORMAL, 13.0, w!("pt-BR"),
            )?;
            let icons = dwrite.CreateTextFormat(
                w!("Segoe Fluent Icons"), None, DWRITE_FONT_WEIGHT_NORMAL,
                DWRITE_FONT_STYLE_NORMAL, DWRITE_FONT_STRETCH_NORMAL, 14.0, w!("pt-BR"),
            )?;
            for f in [&text, &icons] {
                f.SetParagraphAlignment(DWRITE_PARAGRAPH_ALIGNMENT_CENTER)?;
            }
            Ok(Self { d3d, dxgi, d2d_device, dwrite, text, icons })
        }
    }
}

const SVG_ICON: &str = r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24"><path fill="#FFFFFF" d="M12 2a10 10 0 1 0 0 20a10 10 0 0 0 0-20zm1 5v5.4l3.6 2.1-.8 1.3L11 13V7z"/></svg>"##;

fn text(dc: &ID2D1DeviceContext, s: &str, fmt: &IDWriteTextFormat, rect: D2D_RECT_F, brush: &ID2D1SolidColorBrush) {
    let utf16: Vec<u16> = s.encode_utf16().collect();
    // SAFETY: buffers válidos durante a chamada.
    unsafe { dc.DrawText(&utf16, fmt, &rect, brush, D2D1_DRAW_TEXT_OPTIONS_NONE, DWRITE_MEASURING_MODE_NATURAL) };
}

/// Conteúdo de teste da barra: pílula, glifo Fluent, texto e SVG recolorido.
pub fn draw_test_content(dc: &ID2D1DeviceContext, gfx: &Gfx, _w: f32, h: f32, accent_hex: &str) -> Result<()> {
    // SAFETY: `dc` está entre BeginDraw/EndDraw (garantido pelo backend).
    unsafe {
        let fg = dc.CreateSolidColorBrush(&rgba(0xE9E9EDFF), None)?;
        let pill = dc.CreateSolidColorBrush(&rgba(0x9184D9FF), None)?;
        let r = (h - 8.0) / 2.0;
        dc.FillRoundedRectangle(&D2D1_ROUNDED_RECT { rect: D2D_RECT_F { left: 8.0, top: 4.0, right: 140.0, bottom: h - 4.0 }, radiusX: r, radiusY: r }, &pill);
        text(dc, "\u{E767}", &gfx.icons, D2D_RECT_F { left: 152.0, top: 0.0, right: 176.0, bottom: h }, &fg);
        text(dc, "qua 14  22:53", &gfx.text, D2D_RECT_F { left: 182.0, top: 0.0, right: 340.0, bottom: h }, &fg);

        let dc5: ID2D1DeviceContext5 = dc.cast()?;
        let src = SVG_ICON.replace("#FFFFFF", accent_hex);
        let stream = SHCreateMemStream(Some(src.as_bytes())).expect("SHCreateMemStream");
        let doc = dc5.CreateSvgDocument(&stream, D2D_SIZE_F { width: 18.0, height: 18.0 })?;
        let mut base = Matrix3x2::default();
        dc.GetTransform(&mut base);
        dc.SetTransform(&(Matrix3x2::translation(350.0, (h - 18.0) / 2.0) * base));
        dc5.DrawSvgDocument(&doc);
        dc.SetTransform(&base);
    }
    Ok(())
}

/// Conteúdo de teste do painel.
pub fn draw_flyout_content(dc: &ID2D1DeviceContext, gfx: &Gfx, w: f32, _h: f32) -> Result<()> {
    // SAFETY: `dc` está entre BeginDraw/EndDraw.
    unsafe {
        let fg = dc.CreateSolidColorBrush(&rgba(0xE9E9EDFF), None)?;
        let track = dc.CreateSolidColorBrush(&rgba(0xFFFFFF33), None)?;
        let fill = dc.CreateSolidColorBrush(&rgba(0x9184D9FF), None)?;
        text(dc, "Painel de teste", &gfx.text, D2D_RECT_F { left: 16.0, top: 8.0, right: w - 16.0, bottom: 40.0 }, &fg);
        let bar = D2D_RECT_F { left: 16.0, top: 60.0, right: w - 16.0, bottom: 66.0 };
        dc.FillRoundedRectangle(&D2D1_ROUNDED_RECT { rect: bar, radiusX: 3.0, radiusY: 3.0 }, &track);
        let half = D2D_RECT_F { right: bar.left + (bar.right - bar.left) * 0.6, ..bar };
        dc.FillRoundedRectangle(&D2D1_ROUNDED_RECT { rect: half, radiusX: 3.0, radiusY: 3.0 }, &fill);
    }
    Ok(())
}
```

- [ ] **Step 2: Escrever `backdrop.rs`**

```rust
use std::ffi::c_void;
use windows::Win32::Foundation::{BOOL, E_NOTIMPL, HWND};
use windows::Win32::Graphics::Dwm::*;
use windows::Win32::System::LibraryLoader::{GetModuleHandleW, GetProcAddress};
use windows::Win32::UI::Controls::MARGINS;
use windows::core::{Error, Result, s, w};

use crate::args::AcrylicKind;

#[repr(C)]
struct AccentPolicy { state: u32, flags: u32, gradient_color: u32, animation_id: u32 }

#[repr(C)]
struct WinCompAttrData { attrib: u32, data: *mut c_void, size: usize }

type SetWindowCompositionAttribute = unsafe extern "system" fn(HWND, *mut WinCompAttrData) -> BOOL;

const ACCENT_ENABLE_ACRYLICBLURBEHIND: u32 = 4;
const WCA_ACCENT_POLICY: u32 = 19;
/// #161826 com 60 % de opacidade, no formato AABBGGRR.
const TINT_AABBGGRR: u32 = 0x9926_1816;

fn set_attr<T>(hwnd: HWND, attr: DWMWINDOWATTRIBUTE, value: &T) -> Result<()> {
    // SAFETY: ponteiro e tamanho do mesmo valor.
    unsafe { DwmSetWindowAttribute(hwnd, attr, value as *const T as *const c_void, size_of::<T>() as u32) }
}

pub fn apply(hwnd: HWND, kind: AcrylicKind) -> Result<()> {
    set_attr(hwnd, DWMWA_WINDOW_CORNER_PREFERENCE, &DWMWCP_ROUND)?;
    set_attr(hwnd, DWMWA_USE_IMMERSIVE_DARK_MODE, &BOOL(1))?;
    match kind {
        AcrylicKind::A1 => {
            set_attr(hwnd, DWMWA_SYSTEMBACKDROP_TYPE, &DWMSBT_TRANSIENTWINDOW)?;
            let m = MARGINS { cxLeftWidth: -1, cxRightWidth: -1, cyTopHeight: -1, cyBottomHeight: -1 };
            // SAFETY: hwnd válido; margens -1 = janela inteira.
            unsafe { DwmExtendFrameIntoClientArea(hwnd, &m)? };
        }
        AcrylicKind::A2 => set_accent(hwnd)?,
        AcrylicKind::Host => set_attr(hwnd, DWMWA_USE_HOSTBACKDROPBRUSH, &BOOL(1))?,
    }
    Ok(())
}

fn set_accent(hwnd: HWND) -> Result<()> {
    // SAFETY: função não documentada carregada dinamicamente; assinatura conhecida e estável desde o Win10.
    unsafe {
        let user32 = GetModuleHandleW(w!("user32.dll"))?;
        let Some(proc) = GetProcAddress(user32, s!("SetWindowCompositionAttribute")) else {
            return Err(Error::new(E_NOTIMPL, "SetWindowCompositionAttribute ausente"));
        };
        let f: SetWindowCompositionAttribute = std::mem::transmute(proc);
        let mut policy = AccentPolicy { state: ACCENT_ENABLE_ACRYLICBLURBEHIND, flags: 2, gradient_color: TINT_AABBGGRR, animation_id: 0 };
        let mut data = WinCompAttrData { attrib: WCA_ACCENT_POLICY, data: &mut policy as *mut _ as *mut c_void, size: size_of::<AccentPolicy>() };
        f(hwnd, &mut data).ok()
    }
}
```

Adicionar `"Win32_UI_Controls"` à lista de features do `Cargo.toml` (por causa de `MARGINS`).

- [ ] **Step 3: Escrever `backend.rs`**

```rust
use windows::Win32::Foundation::{E_NOTIMPL, HWND};
use windows::Win32::Graphics::Direct2D::ID2D1DeviceContext;
use windows::core::{Error, Result};

use crate::args::PathKind;
use crate::gfx::Gfx;
use crate::spring::SpringParams;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LayerKind { Bar, Flyout }

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Pose { pub opacity: f32, pub scale: f32, pub offset_y: f32 }

impl Pose {
    pub const OPEN: Pose = Pose { opacity: 1.0, scale: 1.0, offset_y: 0.0 };
    pub const CLOSED: Pose = Pose { opacity: 0.0, scale: 0.96, offset_y: -8.0 };
}

pub type Paint<'a> = &'a dyn Fn(&ID2D1DeviceContext) -> Result<()>;

/// O que os dois compositores (A e B) precisam saber fazer.
pub trait Backend {
    fn add_layer(&mut self, hwnd: HWND, width: u32, height: u32, kind: LayerKind) -> Result<usize>;
    /// Desenha com Direct2D; o `dc` já vem transladado e limpo (transparente).
    fn draw(&mut self, layer: usize, gfx: &Gfx, paint: Paint) -> Result<()>;
    fn set_pose(&mut self, layer: usize, pose: Pose) -> Result<()>;
    /// Anima até `pose` no compositor, com a mola dada.
    fn animate_to(&mut self, layer: usize, pose: Pose, params: SpringParams) -> Result<()>;
    fn commit(&mut self) -> Result<()>;
}

pub fn create(path: PathKind, gfx: &Gfx) -> Result<Box<dyn Backend>> {
    match path {
        PathKind::A => Ok(Box::new(crate::path_a::DcompBackend::new(gfx)?)),
        PathKind::B => Err(Error::new(E_NOTIMPL, "o caminho B chega na Task 8")),
    }
}
```

- [ ] **Step 4: Escrever `path_a.rs`**

```rust
use windows::Foundation::Numerics::Matrix3x2;
use windows::Win32::Foundation::{HWND, POINT};
use windows::Win32::Graphics::Direct2D::Common::D2D1_COLOR_F;
use windows::Win32::Graphics::Direct2D::ID2D1DeviceContext;
use windows::Win32::Graphics::DirectComposition::*;
use windows::Win32::Graphics::Dxgi::Common::{DXGI_ALPHA_MODE_PREMULTIPLIED, DXGI_FORMAT_B8G8R8A8_UNORM};
use windows::core::{Interface, Result};

use crate::backend::{Backend, LayerKind, Paint, Pose};
use crate::gfx::Gfx;
use crate::spring::{self, SpringParams};

struct Layer {
    _target: IDCompositionTarget,
    _visual: IDCompositionVisual2,
    surface: IDCompositionSurface,
    effect: IDCompositionEffectGroup,
    scale: IDCompositionScaleTransform,
    translate: IDCompositionTranslateTransform,
    pose: Pose,
}

pub struct DcompBackend {
    device: IDCompositionDesktopDevice,
    layers: Vec<Layer>,
}

impl DcompBackend {
    pub fn new(gfx: &Gfx) -> Result<Self> {
        // SAFETY: dispositivo D2D válido.
        let device: IDCompositionDesktopDevice = unsafe { DCompositionCreateDevice3(&gfx.d2d_device)? };
        Ok(Self { device, layers: Vec::new() })
    }

    fn curve(&self, from: f32, to: f32, params: SpringParams) -> Result<IDCompositionAnimation> {
        let samples = spring::sample(from, to, params, 1.0 / 240.0, 2.0);
        // SAFETY: curva montada com segmentos lineares entre amostras da mola.
        unsafe {
            let anim = self.device.CreateAnimation()?;
            for w in samples.windows(2) {
                let ((t0, v0), (t1, v1)) = (w[0], w[1]);
                anim.AddCubic(t0 as f64, v0, (v1 - v0) / (t1 - t0), 0.0, 0.0)?;
            }
            let (tl, vl) = *samples.last().expect("amostras");
            anim.End(tl as f64, vl)?;
            Ok(anim)
        }
    }
}

impl Backend for DcompBackend {
    fn add_layer(&mut self, hwnd: HWND, width: u32, height: u32, kind: LayerKind) -> Result<usize> {
        // SAFETY: hwnd válido com WS_EX_NOREDIRECTIONBITMAP; objetos DComp do mesmo dispositivo.
        unsafe {
            let target = self.device.CreateTargetForHwnd(hwnd, true)?;
            let visual = self.device.CreateVisual()?;
            let surface = self.device.CreateSurface(width, height, DXGI_FORMAT_B8G8R8A8_UNORM, DXGI_ALPHA_MODE_PREMULTIPLIED)?;
            visual.SetContent(&surface)?;
            let effect = self.device.CreateEffectGroup()?;
            visual.SetEffect(&effect)?;
            let scale = self.device.CreateScaleTransform()?;
            scale.SetCenterX2(width as f32 / 2.0)?;
            scale.SetCenterY2(0.0)?;
            let translate = self.device.CreateTranslateTransform()?;
            let group = self.device.CreateTransformGroup(&[Some(scale.cast()?), Some(translate.cast()?)])?;
            visual.SetTransform(&group)?;
            if kind == LayerKind::Flyout {
                let clip = self.device.CreateRectangleClip()?;
                clip.SetRight2(width as f32)?;
                clip.SetBottom2(height as f32)?;
                for set in [IDCompositionRectangleClip::SetTopLeftRadiusX2, IDCompositionRectangleClip::SetTopLeftRadiusY2,
                            IDCompositionRectangleClip::SetTopRightRadiusX2, IDCompositionRectangleClip::SetTopRightRadiusY2,
                            IDCompositionRectangleClip::SetBottomLeftRadiusX2, IDCompositionRectangleClip::SetBottomLeftRadiusY2,
                            IDCompositionRectangleClip::SetBottomRightRadiusX2, IDCompositionRectangleClip::SetBottomRightRadiusY2] {
                    set(&clip, 12.0)?;
                }
                visual.SetClip(&clip)?;
            }
            target.SetRoot(&visual)?;
            self.layers.push(Layer { _target: target, _visual: visual, surface, effect, scale, translate, pose: Pose::OPEN });
        }
        Ok(self.layers.len() - 1)
    }

    fn draw(&mut self, layer: usize, _gfx: &Gfx, paint: Paint) -> Result<()> {
        let l = &self.layers[layer];
        // SAFETY: BeginDraw/EndDraw pareados; transform ajustada ao offset devolvido.
        unsafe {
            let mut offset = POINT::default();
            let dc: ID2D1DeviceContext = l.surface.BeginDraw(None, &mut offset)?;
            dc.SetTransform(&Matrix3x2::translation(offset.x as f32, offset.y as f32));
            dc.Clear(Some(&D2D1_COLOR_F { r: 0.0, g: 0.0, b: 0.0, a: 0.0 }));
            let result = paint(&dc);
            l.surface.EndDraw()?;
            result
        }
    }

    fn set_pose(&mut self, layer: usize, pose: Pose) -> Result<()> {
        let l = &mut self.layers[layer];
        // SAFETY: objetos DComp válidos.
        unsafe {
            l.effect.SetOpacity2(pose.opacity)?;
            l.scale.SetScaleX2(pose.scale)?;
            l.scale.SetScaleY2(pose.scale)?;
            l.translate.SetOffsetY2(pose.offset_y)?;
        }
        l.pose = pose;
        Ok(())
    }

    fn animate_to(&mut self, layer: usize, pose: Pose, params: SpringParams) -> Result<()> {
        let from = self.layers[layer].pose;
        let opacity = self.curve(from.opacity, pose.opacity, params)?;
        let scale = self.curve(from.scale, pose.scale, params)?;
        let offset = self.curve(from.offset_y, pose.offset_y, params)?;
        let l = &mut self.layers[layer];
        // SAFETY: animações do mesmo dispositivo.
        unsafe {
            l.effect.SetOpacity(&opacity)?;
            l.scale.SetScaleX(&scale)?;
            l.scale.SetScaleY(&scale)?;
            l.translate.SetOffsetY(&offset)?;
        }
        l.pose = pose;
        Ok(())
    }

    fn commit(&mut self) -> Result<()> {
        // SAFETY: dispositivo válido.
        unsafe { self.device.Commit() }
    }
}
```

- [ ] **Step 5: Escrever `app.rs`**

```rust
use std::time::Instant;
use windows::Win32::Foundation::{HWND, RECT};
use windows::Win32::Graphics::Dwm::DwmFlush;
use windows::Win32::UI::Shell::{ABN_FULLSCREENAPP, ABN_POSCHANGED};
use windows::Win32::UI::WindowsAndMessaging::*;
use windows::core::Result;

use crate::args::Args;
use crate::backend::{self, Backend, LayerKind, Pose};
use crate::events::Event;
use crate::gfx::{Gfx, draw_flyout_content, draw_test_content};
use crate::{appbar, backdrop, monitors, timing, window};

pub const BAR_HEIGHT_DIP: i32 = 32;
pub const FLYOUT_W_DIP: i32 = 320;
pub const FLYOUT_H_DIP: i32 = 200;

pub fn scale(v: i32, dpi: u32) -> i32 { v * dpi as i32 / 96 }

pub struct Bar { pub hwnd: HWND, pub layer: usize, pub monitor_rect: RECT, pub height: i32, pub dpi: u32, pub refresh_hz: u32 }
pub struct Flyout { pub hwnd: HWND, pub layer: usize, pub open: bool }

pub struct App {
    pub args: Args,
    pub gfx: Gfx,
    pub backend: Box<dyn Backend>,
    pub bars: Vec<Bar>,
    pub flyout: Flyout,
}

impl App {
    pub fn new(args: Args) -> Result<Self> {
        let gfx = Gfx::new()?;
        let mut backend = backend::create(args.path, &gfx)?;
        let bars = Self::create_bars(&args, &gfx, backend.as_mut())?;
        let flyout = Self::create_flyout(&args, &gfx, backend.as_mut(), &bars[0])?;
        backend.commit()?;
        Ok(Self { args, gfx, backend, bars, flyout })
    }

    fn create_bars(args: &Args, gfx: &Gfx, backend: &mut dyn Backend) -> Result<Vec<Bar>> {
        let mut bars = Vec::new();
        for m in monitors::list() {
            let height = scale(BAR_HEIGHT_DIP, m.dpi);
            let hwnd = window::create_popup(m.rect)?;
            let rect = appbar::register(hwnd, m.rect, height);
            let (w, h) = (rect.right - rect.left, rect.bottom - rect.top);
            // SAFETY: hwnd recém-criado.
            unsafe { SetWindowPos(hwnd, Some(HWND_TOPMOST), rect.left, rect.top, w, h, SWP_NOACTIVATE)? };
            backdrop::apply(hwnd, args.acrylic)?;
            let layer = backend.add_layer(hwnd, w as u32, h as u32, LayerKind::Bar)?;
            backend.draw(layer, gfx, &|dc| draw_test_content(dc, gfx, w as f32, h as f32, "#9184D9"))?;
            // SAFETY: hwnd válido.
            unsafe { let _ = ShowWindow(hwnd, SW_SHOWNOACTIVATE); }
            eprintln!("[bar] {} rect={rect:?} dpi={} hz={}", m.device, m.dpi, m.refresh_hz);
            bars.push(Bar { hwnd, layer, monitor_rect: m.rect, height, dpi: m.dpi, refresh_hz: m.refresh_hz });
        }
        Ok(bars)
    }

    fn create_flyout(args: &Args, gfx: &Gfx, backend: &mut dyn Backend, bar: &Bar) -> Result<Flyout> {
        let (w, h) = (scale(FLYOUT_W_DIP, bar.dpi), scale(FLYOUT_H_DIP, bar.dpi));
        let top = bar.monitor_rect.top + bar.height + scale(6, bar.dpi);
        let left = bar.monitor_rect.left + scale(8, bar.dpi);
        let hwnd = window::create_popup(RECT { left, top, right: left + w, bottom: top + h })?;
        backdrop::apply(hwnd, args.acrylic)?;
        let layer = backend.add_layer(hwnd, w as u32, h as u32, LayerKind::Flyout)?;
        backend.draw(layer, gfx, &|dc| draw_flyout_content(dc, gfx, w as f32, h as f32))?;
        backend.set_pose(layer, Pose::CLOSED)?;
        Ok(Flyout { hwnd, layer, open: false })
    }

    /// Versão instantânea (sem animação). A Task 9 troca pela animada.
    pub fn toggle_flyout(&mut self, clicked_at: Option<Instant>) -> Result<()> {
        self.flyout.open = !self.flyout.open;
        let (pose, show) = if self.flyout.open { (Pose::OPEN, SW_SHOWNOACTIVATE) } else { (Pose::CLOSED, SW_HIDE) };
        // SAFETY: hwnd do painel.
        unsafe { let _ = ShowWindow(self.flyout.hwnd, show); }
        self.backend.set_pose(self.flyout.layer, pose)?;
        self.backend.commit()?;
        if let Some(t0) = clicked_at {
            // SAFETY: espera a próxima composição do DWM.
            unsafe { let _ = DwmFlush(); }
            timing::log("clique->quadro", t0.elapsed());
        }
        Ok(())
    }

    pub fn shutdown(&mut self) {
        for b in &self.bars {
            appbar::unregister(b.hwnd);
            // SAFETY: janelas criadas por nós.
            unsafe { let _ = DestroyWindow(b.hwnd); }
        }
        // SAFETY: janela criada por nós.
        unsafe { let _ = DestroyWindow(self.flyout.hwnd); }
        self.bars.clear();
    }

    pub fn rebuild(&mut self) -> Result<()> {
        let args = self.args.clone();
        self.shutdown();
        *self = App::new(args)?;
        Ok(())
    }

    /// Devolve `false` quando o app deve encerrar.
    pub fn handle(&mut self, e: Event) -> Result<bool> {
        match e {
            Event::Click { hwnd, at, .. } if self.bars.iter().any(|b| b.hwnd == hwnd) => self.toggle_flyout(Some(at))?,
            Event::AppBar { hwnd, notification, flag } if notification == ABN_FULLSCREENAPP => {
                let show = if flag != 0 { SW_HIDE } else { SW_SHOWNOACTIVATE };
                // SAFETY: hwnd é uma das barras.
                unsafe { let _ = ShowWindow(hwnd, show); }
            }
            Event::AppBar { notification, .. } if notification == ABN_POSCHANGED => {
                for b in &self.bars { appbar::set_pos(b.hwnd, b.monitor_rect, b.height); }
            }
            Event::DpiChanged { .. } | Event::DisplayChanged => {
                eprintln!("[monitor] mudou: recriando");
                self.rebuild()?;
            }
            Event::TaskbarCreated => {
                eprintln!("[explorer] TaskbarCreated: registrando AppBars de novo");
                for b in &self.bars { appbar::register(b.hwnd, b.monitor_rect, b.height); }
            }
            Event::Close => {
                self.shutdown();
                return Ok(false);
            }
            _ => {}
        }
        Ok(true)
    }
}
```

- [ ] **Step 6: Substituir `main.rs`**

```rust
mod app;
mod appbar;
mod args;
mod backdrop;
mod backend;
mod events;
mod gfx;
mod monitors;
mod path_a;
mod spring;
mod timing;
mod window;

use windows::Win32::Graphics::Dwm::DwmFlush;
use windows::Win32::UI::HiDpi::{DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2, SetProcessDpiAwarenessContext};
use windows::Win32::UI::WindowsAndMessaging::PostQuitMessage;

fn main() -> windows::core::Result<()> {
    timing::mark_start();
    let raw: Vec<String> = std::env::args().skip(1).collect();
    let refs: Vec<&str> = raw.iter().map(String::as_str).collect();
    let args = args::parse(&refs).unwrap_or_else(|e| {
        eprintln!("{e}\nuso: s1-render [--path a|b] [--acrylic a1|a2|host] [--anim compositor|frame] [--cycles N]");
        std::process::exit(2)
    });
    eprintln!("[args] {args:?}");
    // SAFETY: chamada única no início do processo.
    unsafe { SetProcessDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2)? };
    window::register_class()?;
    let mut app = app::App::new(args)?;
    // SAFETY: espera a primeira composição com as barras visíveis.
    unsafe { let _ = DwmFlush(); }
    eprintln!("[timing] primeiro quadro: {:.2} ms", timing::since_start_ms());

    while window::pump_blocking() {
        for e in events::drain() {
            if !app.handle(e)? {
                // SAFETY: encerra o loop de mensagens.
                unsafe { PostQuitMessage(0) };
            }
        }
    }
    Ok(())
}
```

- [ ] **Step 7: Rodar o caminho A com as duas formas de acrílico**

Run: `cargo run --release --manifest-path spikes/s1-render/Cargo.toml -- --path a --acrylic a1`
Expected: barra no topo dos 2 monitores com pílula roxa, ícone de volume, texto "qua 14  22:53" e o ícone SVG roxo, sobre fundo acrílico. Clique esquerdo na barra mostra/esconde o painel; o log imprime `primeiro quadro` e `clique->quadro`. Botão direito fecha.

Run: `cargo run --release --manifest-path spikes/s1-render/Cargo.toml -- --path a --acrylic a2`
Expected: mesma coisa com o acrílico do caminho a2.

Anotar (para o ADR): se o acrílico aparece, se as bordas do painel ficam arredondadas e sem "caixa" de desfoque fora da forma. Tirar captura de tela de cada um (`Win+Shift+S`) e salvar em `docs/adr/media/0001-a1.png` e `0001-a2.png` no worktree principal.

- [ ] **Step 8: Commit**

```powershell
git add spikes/s1-render
git commit -m "spike(s1): path A with DirectComposition, Direct2D content and DWM acrylic" -m "Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

---

## Task 8: S1 — Caminho B (Windows.UI.Composition) com acrílico recortado

**Files:**
- Create: `spikes/s1-render/src/path_b.rs`
- Modify: `spikes/s1-render/src/backend.rs` (função `create`), `spikes/s1-render/src/main.rs` (declarar `mod path_b;`)

**Interfaces:**
- Consumes: Task 7 (`Backend`, `Pose`, `LayerKind`, `Gfx`), Task 5 (`SpringParams::to_period_and_ratio`).
- Produces: `path_b::WucBackend::new(&Gfx) -> Result<Self>` implementando `Backend`.

- [ ] **Step 1: Escrever `path_b.rs`**

```rust
use windows::Foundation::Numerics::{Matrix3x2, Vector2, Vector3};
use windows::Foundation::{IReference, PropertyValue, Size, TimeSpan};
use windows::Graphics::DirectX::{DirectXAlphaMode, DirectXPixelFormat};
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

use crate::backend::{Backend, LayerKind, Paint, Pose};
use crate::gfx::Gfx;
use crate::spring::SpringParams;

struct Layer {
    _target: DesktopWindowTarget,
    root: ContainerVisual,
    surface: CompositionDrawingSurface,
}

pub struct WucBackend {
    _queue: DispatcherQueueController,
    compositor: Compositor,
    gdev: CompositionGraphicsDevice,
    layers: Vec<Layer>,
}

impl WucBackend {
    pub fn new(gfx: &Gfx) -> Result<Self> {
        let options = DispatcherQueueOptions {
            dwSize: size_of::<DispatcherQueueOptions>() as u32,
            threadType: DQTYPE_THREAD_CURRENT,
            apartmentType: DQTAT_COM_STA,
        };
        // SAFETY: cria a fila de despacho desta thread (exigida pelo Compositor).
        let queue = unsafe { CreateDispatcherQueueController(options)? };
        let compositor = Compositor::new()?;
        let interop: ICompositorInterop = compositor.cast()?;
        // SAFETY: dispositivo D2D válido.
        let gdev = unsafe { interop.CreateGraphicsDevice(&gfx.d2d_device)? };
        Ok(Self { _queue: queue, compositor, gdev, layers: Vec::new() })
    }

    fn spring(&self, value: f32, params: SpringParams) -> Result<SpringScalarNaturalMotionAnimation> {
        let (period, ratio) = params.to_period_and_ratio();
        let a = self.compositor.CreateSpringScalarAnimation()?;
        a.SetDampingRatio(ratio)?;
        a.SetPeriod(TimeSpan { Duration: (period as f64 * 1e7) as i64 })?;
        a.SetFinalValue(&PropertyValue::CreateSingle(value)?.cast::<IReference<f32>>()?)?;
        Ok(a)
    }
}

impl Backend for WucBackend {
    fn add_layer(&mut self, hwnd: HWND, width: u32, height: u32, kind: LayerKind) -> Result<usize> {
        let desktop: ICompositorDesktopInterop = self.compositor.cast()?;
        // SAFETY: hwnd válido com WS_EX_NOREDIRECTIONBITMAP.
        let target = unsafe { desktop.CreateDesktopWindowTarget(hwnd, true)? };
        let size = Vector2 { X: width as f32, Y: height as f32 };
        let root = self.compositor.CreateContainerVisual()?;
        root.SetSize(size)?;

        // Fundo: acrílico do host (desfoque do que está atrás) + tom Nocturne.
        let backdrop = self.compositor.CreateSpriteVisual()?;
        backdrop.SetBrush(&self.compositor.CreateHostBackdropBrush()?)?;
        backdrop.SetSize(size)?;
        let tint = self.compositor.CreateSpriteVisual()?;
        tint.SetBrush(&self.compositor.CreateColorBrushWithColor(Color { A: 0x99, R: 0x16, G: 0x18, B: 0x26 })?)?;
        tint.SetSize(size)?;

        let surface = self.gdev.CreateDrawingSurface(
            Size { Width: width as f32, Height: height as f32 },
            DirectXPixelFormat::B8G8R8A8UIntNormalized,
            DirectXAlphaMode::Premultiplied,
        )?;
        let content = self.compositor.CreateSpriteVisual()?;
        content.SetBrush(&self.compositor.CreateSurfaceBrushWithSurface(&surface)?)?;
        content.SetSize(size)?;

        let children = root.Children()?;
        children.InsertAtTop(&backdrop)?;
        children.InsertAtTop(&tint)?;
        children.InsertAtTop(&content)?;

        // Recorte arredondado: o acrílico respeita a forma.
        let radius = if kind == LayerKind::Flyout { 12.0 } else { 0.0 };
        let geom = self.compositor.CreateRoundedRectangleGeometry()?;
        geom.SetSize(size)?;
        geom.SetCornerRadius(Vector2 { X: radius, Y: radius })?;
        root.SetClip(&self.compositor.CreateGeometricClipWithGeometry(&geom)?)?;
        root.SetCenterPoint(Vector3 { X: width as f32 / 2.0, Y: 0.0, Z: 0.0 })?;

        target.SetRoot(&root)?;
        self.layers.push(Layer { _target: target, root, surface });
        Ok(self.layers.len() - 1)
    }

    fn draw(&mut self, layer: usize, _gfx: &Gfx, paint: Paint) -> Result<()> {
        let interop: ICompositionDrawingSurfaceInterop = self.layers[layer].surface.cast()?;
        // SAFETY: BeginDraw/EndDraw pareados.
        unsafe {
            let mut offset = POINT::default();
            let dc: ID2D1DeviceContext = interop.BeginDraw(None, &mut offset)?;
            dc.SetTransform(&Matrix3x2::translation(offset.x as f32, offset.y as f32));
            dc.Clear(Some(&D2D1_COLOR_F { r: 0.0, g: 0.0, b: 0.0, a: 0.0 }));
            let result = paint(&dc);
            interop.EndDraw()?;
            result
        }
    }

    fn set_pose(&mut self, layer: usize, pose: Pose) -> Result<()> {
        let root = &self.layers[layer].root;
        root.SetOpacity(pose.opacity)?;
        root.SetScale(Vector3 { X: pose.scale, Y: pose.scale, Z: 1.0 })?;
        root.SetOffset(Vector3 { X: 0.0, Y: pose.offset_y, Z: 0.0 })?;
        Ok(())
    }

    fn animate_to(&mut self, layer: usize, pose: Pose, params: SpringParams) -> Result<()> {
        for (prop, value) in [("Opacity", pose.opacity), ("Scale.X", pose.scale), ("Scale.Y", pose.scale), ("Offset.Y", pose.offset_y)] {
            let anim = self.spring(value, params)?;
            self.layers[layer].root.StartAnimation(&HSTRING::from(prop), &anim)?;
        }
        Ok(())
    }

    fn commit(&mut self) -> Result<()> {
        // O Windows.UI.Composition confirma sozinho ao processar as mensagens da thread.
        Ok(())
    }
}
```

- [ ] **Step 2: Ligar o caminho B**

Em `backend.rs`, trocar a linha do `PathKind::B` por:

```rust
        PathKind::B => Ok(Box::new(crate::path_b::WucBackend::new(gfx)?)),
```

e remover `E_NOTIMPL` e `Error` dos `use` se o compilador avisar que não são usados. Em `main.rs`, adicionar `mod path_b;` em ordem alfabética.

- [ ] **Step 3: Rodar o caminho B**

Run: `cargo run --release --manifest-path spikes/s1-render/Cargo.toml -- --path b`
Expected: mesma barra e painel da Task 7, com o acrílico desenhado pelo compositor; o painel tem cantos de 12 px **com o desfoque cortado na curva** (sem retângulo de desfoque nos cantos).

Se a barra aparecer **sem desfoque** (só o tom), anotar para o ADR: testar com "Efeitos de transparência" ligado em Configurações → Personalização → Cores, e conferir que o atributo `DWMWA_USE_HOSTBACKDROPBRUSH` foi aplicado sem erro (o log não pode ter erro de `backdrop::apply`).

Salvar captura em `docs/adr/media/0001-b-host.png`.

- [ ] **Step 4: Commit**

```powershell
git add spikes/s1-render
git commit -m "spike(s1): path B with Windows.UI.Composition, host backdrop and rounded clip" -m "Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

---

## Task 9: S1 — Animação do painel, medições internas e dispositivo perdido

**Files:**
- Create: `spikes/s1-render/src/anim.rs`
- Modify: `spikes/s1-render/src/app.rs` (campos novos, `toggle_flyout`, `handle`, `frame_tick`, `start_timers`), `spikes/s1-render/src/main.rs` (loop e `mod anim;`)

**Interfaces:**
- Consumes: Tasks 5–8.
- Produces:
  - `anim::{lerp_pose(a: Pose, b: Pose, t: f32) -> Pose, wait_for_next_frame()}`.
  - `App::frame_animating() -> bool`, `App::frame_tick() -> Result<()>`, `App::start_timers()`; IDs de timer `CYCLE_TIMER = 1`, `HEALTH_TIMER = 2`, `HIDE_TIMER = 3`.

- [ ] **Step 1: Escrever o teste de interpolação (falhando)**

`spikes/s1-render/src/anim.rs`:

```rust
use crate::backend::Pose;

pub fn lerp_pose(_a: Pose, _b: Pose, _t: f32) -> Pose {
    unimplemented!()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn interpolates_each_field_and_allows_overshoot() {
        assert_eq!(lerp_pose(Pose::CLOSED, Pose::OPEN, 0.0), Pose::CLOSED);
        assert_eq!(lerp_pose(Pose::CLOSED, Pose::OPEN, 1.0), Pose::OPEN);
        let mid = lerp_pose(Pose::CLOSED, Pose::OPEN, 0.5);
        assert!((mid.opacity - 0.5).abs() < 1e-6);
        assert!((mid.scale - 0.98).abs() < 1e-6);
        assert!((mid.offset_y + 4.0).abs() < 1e-6);
        // Mola elástica passa de 1.0: escala passa de 1, opacidade fica em 1.
        let over = lerp_pose(Pose::CLOSED, Pose::OPEN, 1.1);
        assert!(over.scale > 1.0);
        assert_eq!(over.opacity, 1.0);
    }
}
```

Adicionar `mod anim;` ao `main.rs`.

- [ ] **Step 2: Rodar para ver falhar**

Run: `cargo test --manifest-path spikes/s1-render/Cargo.toml anim`
Expected: FAIL (`not implemented`).

- [ ] **Step 3: Implementar `anim.rs`**

Substituir o conteúdo acima do `#[cfg(test)]` por:

```rust
use std::sync::OnceLock;
use windows::Win32::Foundation::HANDLE;
use windows::Win32::Graphics::Dwm::DwmFlush;
use windows::Win32::System::LibraryLoader::{GetProcAddress, LoadLibraryW};
use windows::core::{s, w};

use crate::backend::Pose;

pub fn lerp_pose(a: Pose, b: Pose, t: f32) -> Pose {
    let l = |x: f32, y: f32| x + (y - x) * t;
    Pose { opacity: l(a.opacity, b.opacity).clamp(0.0, 1.0), scale: l(a.scale, b.scale), offset_y: l(a.offset_y, b.offset_y) }
}

type WaitForCompositorClock = unsafe extern "system" fn(u32, *const HANDLE, u32) -> u32;

/// `DCompositionWaitForCompositorClock` só existe no Win11: carregar dinamicamente.
fn compositor_clock() -> Option<WaitForCompositorClock> {
    static F: OnceLock<Option<WaitForCompositorClock>> = OnceLock::new();
    *F.get_or_init(|| {
        // SAFETY: carrega dcomp.dll e procura a função pelo nome; transmute para a assinatura documentada.
        unsafe {
            let lib = LoadLibraryW(w!("dcomp.dll")).ok()?;
            GetProcAddress(lib, s!("DCompositionWaitForCompositorClock")).map(|p| std::mem::transmute(p))
        }
    })
}

/// Bloqueia até o próximo quadro do compositor.
pub fn wait_for_next_frame() {
    match compositor_clock() {
        // SAFETY: sem handles extras; timeout de 100 ms.
        Some(f) => unsafe { f(0, std::ptr::null(), 100); },
        // SAFETY: chamada simples ao DWM.
        None => unsafe { let _ = DwmFlush(); },
    }
}
```

- [ ] **Step 4: Rodar o teste**

Run: `cargo test --manifest-path spikes/s1-render/Cargo.toml anim`
Expected: PASS.

- [ ] **Step 5: Animar o painel no `app.rs`**

5a. Adicionar os imports e constantes no topo de `app.rs`:

```rust
use std::time::Duration;
use crate::anim::{lerp_pose, wait_for_next_frame};
use crate::args::AnimMode;
use crate::spring::{Spring, SpringParams};
use windows::Win32::Graphics::Dxgi::{DXGI_ERROR_DEVICE_REMOVED, DXGI_ERROR_DEVICE_RESET};

pub const CYCLE_TIMER: usize = 1;
pub const HEALTH_TIMER: usize = 2;
pub const HIDE_TIMER: usize = 3;
```

5b. Adicionar campos à `struct App`:

```rust
    pub progress: Spring,
    pub frame_params: SpringParams,
    pub last_tick: Instant,
    pub cycles_left: u32,
```

e inicializá-los em `App::new` (no `Ok(Self { … })`):

```rust
        Ok(Self {
            cycles_left: args.cycles * 2,
            args, gfx, backend, bars, flyout,
            progress: Spring::new(0.0),
            frame_params: SpringParams::DEFAULT,
            last_tick: Instant::now(),
        })
```

5c. Substituir `toggle_flyout` por:

```rust
    pub fn toggle_flyout(&mut self, clicked_at: Option<Instant>) -> Result<()> {
        self.flyout.open = !self.flyout.open;
        let open = self.flyout.open;
        let params = if open { SpringParams::GROW } else { SpringParams::SHRINK };
        if open {
            // SAFETY: hwnd do painel.
            unsafe { let _ = ShowWindow(self.flyout.hwnd, SW_SHOWNOACTIVATE); }
        }
        match self.args.anim {
            AnimMode::Compositor => {
                let target = if open { Pose::OPEN } else { Pose::CLOSED };
                self.backend.animate_to(self.flyout.layer, target, params)?;
                self.backend.commit()?;
                if !open {
                    // Esconde depois que a mola de fechar terminar.
                    // SAFETY: timer na primeira barra.
                    unsafe { SetTimer(Some(self.bars[0].hwnd), HIDE_TIMER, 450, None) };
                }
            }
            AnimMode::Frame => {
                self.progress.target = if open { 1.0 } else { 0.0 };
                self.frame_params = params;
                self.last_tick = Instant::now();
            }
        }
        if let Some(t0) = clicked_at {
            // SAFETY: espera a próxima composição do DWM.
            unsafe { let _ = DwmFlush(); }
            timing::log("clique->quadro", t0.elapsed());
        }
        Ok(())
    }

    pub fn frame_animating(&self) -> bool {
        self.args.anim == AnimMode::Frame && !self.progress.is_settled()
    }

    /// Um quadro da animação na thread de UI (método do Dynamic Island).
    pub fn frame_tick(&mut self) -> Result<()> {
        let now = Instant::now();
        let dt = (now - self.last_tick).as_secs_f32();
        self.last_tick = now;
        self.progress.step(dt, self.frame_params);
        let pose = lerp_pose(Pose::CLOSED, Pose::OPEN, self.progress.value);
        self.backend.set_pose(self.flyout.layer, pose)?;
        self.backend.commit()?;
        wait_for_next_frame();
        if self.progress.is_settled() && self.progress.target == 0.0 {
            // SAFETY: hwnd do painel.
            unsafe { let _ = ShowWindow(self.flyout.hwnd, SW_HIDE); }
        }
        Ok(())
    }

    pub fn start_timers(&self) {
        // SAFETY: timers na primeira barra; IDs fixos.
        unsafe {
            SetTimer(Some(self.bars[0].hwnd), HEALTH_TIMER, 1000, None);
            if self.cycles_left > 0 {
                SetTimer(Some(self.bars[0].hwnd), CYCLE_TIMER, 600, None);
            }
        }
    }

    fn device_lost(&self) -> bool {
        // SAFETY: consulta simples ao dispositivo D3D.
        match unsafe { self.gfx.d3d.GetDeviceRemovedReason() } {
            Ok(()) => false,
            Err(e) => {
                eprintln!("[gpu] dispositivo perdido: {e} (DEVICE_REMOVED={}, DEVICE_RESET={})", e.code() == DXGI_ERROR_DEVICE_REMOVED, e.code() == DXGI_ERROR_DEVICE_RESET);
                true
            }
        }
    }
```

5d. Em `handle`, adicionar antes do `_ => {}`:

```rust
            Event::Timer { id: HIDE_TIMER } => {
                // SAFETY: timer e janela nossos.
                unsafe {
                    let _ = KillTimer(Some(self.bars[0].hwnd), HIDE_TIMER);
                    if !self.flyout.open { let _ = ShowWindow(self.flyout.hwnd, SW_HIDE); }
                }
            }
            Event::Timer { id: CYCLE_TIMER } => {
                if self.cycles_left == 0 {
                    // SAFETY: timer nosso.
                    unsafe { let _ = KillTimer(Some(self.bars[0].hwnd), CYCLE_TIMER); }
                    eprintln!("[cycles] terminado");
                } else {
                    self.cycles_left -= 1;
                    self.toggle_flyout(None)?;
                }
            }
            Event::Timer { id: HEALTH_TIMER } if self.device_lost() => {
                eprintln!("[gpu] recriando dispositivos e camadas");
                self.rebuild()?;
                self.start_timers();
            }
```

5e. Substituir o loop do `main.rs` por:

```rust
    app.start_timers();
    loop {
        let alive = if app.frame_animating() { window::pump_nonblocking() } else { window::pump_blocking() };
        if !alive {
            break;
        }
        for e in events::drain() {
            if !app.handle(e)? {
                // SAFETY: encerra o loop de mensagens.
                unsafe { PostQuitMessage(0) };
            }
        }
        if app.frame_animating() {
            app.frame_tick()?;
        }
    }
```

- [ ] **Step 6: Verificar a recuperação de dispositivo perdido (Review Focus 3)**

Run: `cargo run --release --manifest-path spikes/s1-render/Cargo.toml -- --path b`

Com a barra visível, apertar `Win+Ctrl+Shift+B` (reinicia o driver de vídeo; a tela pisca).

Expected: em até 1 s o log mostra `[gpu] dispositivo perdido` e `[gpu] recriando dispositivos e camadas`, e a barra volta a aparecer com conteúdo, sem o processo fechar. Repetir com `--path a`. Se o dispositivo **não** for dado como perdido mas a barra ficar vazia, anotar no ADR e acrescentar também o evento `RenderingDeviceReplaced` do `CompositionGraphicsDevice` (caminho B) como gatilho de `rebuild`.

- [ ] **Step 7: Verificar os dois métodos de animação a olho**

Run: `cargo run --release --manifest-path spikes/s1-render/Cargo.toml -- --path b --anim compositor`
Run: `cargo run --release --manifest-path spikes/s1-render/Cargo.toml -- --path b --anim frame`

Expected: nos dois, o painel abre crescendo levemente (0,96 → 1), descendo 8 px e aparecendo; fecha ao contrário. O log mostra `clique->quadro` a cada clique.

- [ ] **Step 8: Commit**

```powershell
git add spikes/s1-render
git commit -m "spike(s1): spring flyout animation (compositor and per-frame), cycles mode and device-lost recovery" -m "Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

---

## Task 10: S1 — Medições e ADR-0001

**Files:**
- Create (no worktree principal `~/Projects/zefiro`): `docs/adr/0001-pipeline-de-renderizacao.md`, `docs/adr/media/0001-*.png`
- Saídas temporárias: `scripts/out/s1-*.json`, `scripts/out/s1-*.csv`

**Interfaces:**
- Consumes: Tasks 2, 3, 5–9.
- Produces: decisão do compositor (A ou B), do acrílico e do método de animação — usada no S6 (Task 14) e no F3 (Task 27).

- [ ] **Step 1: Compilar em release**

Run: `cargo build --release --manifest-path ../zefiro-s1/spikes/s1-render/Cargo.toml`
Expected: `target/release/s1-render.exe` gerado.

- [ ] **Step 2: Medir RAM e CPU em repouso (5 combinações)**

Para cada combinação, abrir o spike num terminal e medir em outro (fechar o spike com botão direito entre uma e outra):

| Nome | Argumentos |
|---|---|
| `b-host-comp` | `--path b --acrylic host --anim compositor` |
| `b-host-frame` | `--path b --acrylic host --anim frame` |
| `a-a1-comp` | `--path a --acrylic a1 --anim compositor` |
| `a-a1-frame` | `--path a --acrylic a1 --anim frame` |
| `a-a2-comp` | `--path a --acrylic a2 --anim compositor` |

```powershell
pwsh -NoProfile -File scripts/perf.ps1 -Process s1-render -WarmupSeconds 60 -SampleSeconds 300 -Out scripts/out/s1-<nome>-idle.json
```

Abrir e fechar o painel uma vez antes de iniciar a medição (a meta inclui "2 barras + 1 painel aberto e fechado").

- [ ] **Step 3: Medir quadros e CPU durante 50 ciclos a 144 Hz**

Para cada combinação, em três terminais, na ordem:

```powershell
PresentMon --process_name dwm.exe --output_file scripts/out/s1-<nome>-cycles.csv --timed 70 --terminate_after_timed
```

```powershell
pwsh -NoProfile -File scripts/perf.ps1 -Process s1-render -WarmupSeconds 2 -SampleSeconds 60 -IntervalSeconds 2 -Out scripts/out/s1-<nome>-anim.json
```

```powershell
../zefiro-s1/spikes/s1-render/target/release/s1-render.exe <argumentos> --cycles 50
```

Depois:

```powershell
pwsh -NoProfile -File scripts/frames.ps1 -Csv scripts/out/s1-<nome>-cycles.csv -RefreshHz 144
```

Anotar `dropped` (meta ≤ 1) e o `cpu_percent_avg` do `-anim.json`.

- [ ] **Step 4: Anotar primeiro quadro e clique→quadro**

Rodar cada combinação 3 vezes e copiar do log `[timing] primeiro quadro` (meta ≤ 250 ms) e 10 linhas `clique->quadro` (meta ≤ 16 ms; anotar mediana e máximo).

- [ ] **Step 5: Testar o vencedor provisório no Windows 10**

Copiar `s1-render.exe` para a VM Windows 10 22H2 e rodar a melhor combinação. Anotar: abre? acrílico aparece? (Se a VM não existir, anotar "Win10: não testado" como risco aberto.)

- [ ] **Step 6: Escrever `docs/adr/0001-pipeline-de-renderizacao.md`**

Seguir o modelo `0000-modelo.md`, com a tabela abaixo preenchida com os números medidos:

```markdown
## Números medidos

| Métrica | b-host-comp | b-host-frame | a-a1-comp | a-a1-frame | a-a2-comp | Meta |
|---|---|---|---|---|---|---|
| RAM working set (MB) | | | | | | ≤ 25 |
| RAM private (MB) | | | | | | — |
| CPU repouso (%) | | | | | | ≤ 0,2 |
| CPU durante 50 ciclos (%) | | | | | | menor é melhor |
| Quadros perdidos / 50 ciclos | | | | | | ≤ 1 |
| Primeiro quadro (ms, mediana) | | | | | | ≤ 250 |
| Clique→quadro (ms, mediana / máx.) | | | | | | ≤ 16 |
| Acrílico recortado sem serrilhado | | | | | | sim |
| Win10 | | | | | | funciona |
```

Na **Decisão**, aplicar a regra da spec (S1, "Como decidir"): cumprir todas as metas; desempate por qualidade do acrílico recortado, depois CPU em animação, depois simplicidade. Registrar também: método de animação escolhido, forma de acrílico, e qualquer diferença de API em relação ao plano (ex.: nomes de métodos da crate `windows`).

- [ ] **Step 7: Commit no `main`**

```powershell
cd ~/Projects/zefiro
git add docs/adr/0001-pipeline-de-renderizacao.md docs/adr/media
git commit -m "docs(adr): 0001 rendering pipeline decision with S1 measurements" -m "Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

---

## Task 11: S6 — Projeto do spike e analisador de espectro (FFT)

**Files:**
- Create: `spikes/s6-island/` (cópia de `spikes/s1-render/`), `spikes/s6-island/src/analyzer.rs`
- Modify: `spikes/s6-island/Cargo.toml` (nome do pacote e features)

**Interfaces:**
- Consumes: Task 10 (ADR-0001: compositor escolhido), módulos do S1 (`gfx`, `backdrop`, `window`, `events`, `monitors`, `spring`, `timing`, `anim`).
- Produces: `analyzer::{Analyzer, WINDOW = 1024, HOP = 512, BANDS = 24, band_index_for(freq, sample_rate) -> usize}`; `Analyzer::new(sample_rate: f32)`, `push(&mut self, mono: &[f32]) -> bool` (true quando houve análise nova), `bands() -> [f32; BANDS]`, `peak() -> f32`.

- [ ] **Step 1: Criar a branch a partir do S1 e copiar o projeto**

```powershell
cd ~/Projects/zefiro
git worktree add ../zefiro-s6 -b spike/s6-island spike/s1-render
cd ../zefiro-s6
Copy-Item -Recurse spikes/s1-render spikes/s6-island
Remove-Item -Recurse -Force spikes/s6-island/target -ErrorAction SilentlyContinue
```

Em `spikes/s6-island/Cargo.toml`, trocar `name = "s1-render"` por `name = "s6-island"` e acrescentar às features da crate `windows`:

```toml
  "Win32_Media_Audio", "Win32_System_Threading", "Win32_System_Power",
  "Win32_System_RemoteDesktop", "Win32_System_Registry", "Win32_UI_Input_KeyboardAndMouse",
```

- [ ] **Step 2: Escrever os testes do analisador (falhando)**

`spikes/s6-island/src/analyzer.rs`:

```rust
//! Analisador de espectro portado do Dynamic Island for Windows (MIT):
//! janela Hann de 1024, FFT radix-2, 24 bandas logarítmicas, ganho automático,
//! subida 0,65 / queda 0,14, portão de silêncio.

pub const WINDOW: usize = 1024;
pub const HOP: usize = 512;
pub const BANDS: usize = 24;
pub const LOW_HZ: f32 = 40.0;
pub const HIGH_HZ: f32 = 16_000.0;

pub struct Analyzer {
    ring: Vec<f32>,
    write: usize,
    since_hop: usize,
    window: Vec<f32>,
    tw_cos: Vec<f32>,
    tw_sin: Vec<f32>,
    bit_rev: Vec<usize>,
    bin_lo: [usize; BANDS],
    bin_hi: [usize; BANDS],
    smooth: [f32; BANDS],
    agc: f32,
    peak: f32,
}

#[cfg(test)]
mod tests {
    use super::*;

    const SR: f32 = 48_000.0;

    fn sine(freq: f32, amp: f32, secs: f32) -> Vec<f32> {
        (0..(SR * secs) as usize).map(|i| amp * (2.0 * std::f32::consts::PI * freq * i as f32 / SR).sin()).collect()
    }

    fn feed(a: &mut Analyzer, samples: &[f32]) {
        for chunk in samples.chunks(480) {
            a.push(chunk);
        }
    }

    fn argmax(b: &[f32; BANDS]) -> usize {
        b.iter().enumerate().max_by(|x, y| x.1.total_cmp(y.1)).map(|(i, _)| i).unwrap()
    }

    #[test]
    fn silence_stays_silent() {
        let mut a = Analyzer::new(SR);
        feed(&mut a, &vec![0.0; WINDOW * 10]);
        assert!(a.bands().iter().all(|v| *v < 0.01), "{:?}", a.bands());
    }

    #[test]
    fn a_1khz_tone_lights_its_own_band() {
        let mut a = Analyzer::new(SR);
        feed(&mut a, &sine(1000.0, 0.5, 2.0));
        assert_eq!(argmax(&a.bands()), band_index_for(1000.0, SR));
    }

    #[test]
    fn a_low_tone_lights_a_lower_band_than_a_high_tone() {
        let mut low = Analyzer::new(SR);
        feed(&mut low, &sine(120.0, 0.5, 2.0));
        let mut high = Analyzer::new(SR);
        feed(&mut high, &sine(6000.0, 0.5, 2.0));
        assert!(argmax(&low.bands()) < argmax(&high.bands()));
    }

    #[test]
    fn loud_tone_is_normalized_near_full_scale() {
        let mut a = Analyzer::new(SR);
        feed(&mut a, &sine(1000.0, 0.9, 3.0));
        let max = a.bands().iter().copied().fold(0.0, f32::max);
        assert!(max > 0.7, "{max}");
        assert!(max <= 1.0);
    }

    #[test]
    fn push_reports_a_new_analysis_every_hop() {
        let mut a = Analyzer::new(SR);
        assert!(!a.push(&vec![0.0; HOP - 1]));
        assert!(a.push(&[0.0]));
    }
}
```

Adicionar `mod analyzer;` ao `main.rs` do S6.

- [ ] **Step 3: Rodar para ver falhar**

Run: `cargo test --manifest-path spikes/s6-island/Cargo.toml analyzer`
Expected: FAIL de compilação (`new`, `push`, `bands`, `band_index_for` não existem).

- [ ] **Step 4: Implementar o analisador**

Adicionar acima do `#[cfg(test)]`:

```rust
fn band_edges(sample_rate: f32) -> [f32; BANDS + 1] {
    let high = HIGH_HZ.min(sample_rate * 0.45);
    let mut e = [0.0; BANDS + 1];
    for (b, edge) in e.iter_mut().enumerate() {
        *edge = LOW_HZ * (high / LOW_HZ).powf(b as f32 / BANDS as f32);
    }
    e
}

/// Índice da banda que contém `freq`.
pub fn band_index_for(freq: f32, sample_rate: f32) -> usize {
    let e = band_edges(sample_rate);
    (0..BANDS).find(|&b| freq >= e[b] && freq < e[b + 1]).unwrap_or(BANDS - 1)
}

impl Analyzer {
    pub fn new(sample_rate: f32) -> Self {
        let n = WINDOW;
        let window = (0..n).map(|i| 0.5 - 0.5 * (2.0 * std::f32::consts::PI * i as f32 / (n - 1) as f32).cos()).collect();
        let tw_cos = (0..n / 2).map(|k| (2.0 * std::f32::consts::PI * k as f32 / n as f32).cos()).collect();
        let tw_sin = (0..n / 2).map(|k| -(2.0 * std::f32::consts::PI * k as f32 / n as f32).sin()).collect();
        let bits = n.trailing_zeros();
        let bit_rev = (0..n).map(|i| i.reverse_bits() >> (usize::BITS - bits)).collect();
        let bin_hz = sample_rate / n as f32;
        let nyquist_bin = n / 2;
        let e = band_edges(sample_rate);
        let mut bin_lo = [0; BANDS];
        let mut bin_hi = [0; BANDS];
        for b in 0..BANDS {
            let lo = ((e[b] / bin_hz).floor() as usize).clamp(1, nyquist_bin);
            let hi = ((e[b + 1] / bin_hz).ceil() as usize).max(lo + 1).min(nyquist_bin + 1);
            bin_lo[b] = lo;
            bin_hi[b] = hi;
        }
        Self {
            ring: vec![0.0; n], write: 0, since_hop: 0, window, tw_cos, tw_sin, bit_rev,
            bin_lo, bin_hi, smooth: [0.0; BANDS], agc: 0.04, peak: 0.0,
        }
    }

    pub fn bands(&self) -> [f32; BANDS] {
        self.smooth
    }

    pub fn peak(&self) -> f32 {
        self.peak
    }

    pub fn push(&mut self, mono: &[f32]) -> bool {
        let mut analyzed = false;
        for &s in mono {
            self.ring[self.write] = s;
            self.write = (self.write + 1) % WINDOW;
            self.since_hop += 1;
            if self.since_hop >= HOP {
                self.since_hop = 0;
                self.analyze();
                analyzed = true;
            }
        }
        analyzed
    }

    fn fft(&self, re: &mut [f32], im: &mut [f32]) {
        for i in 0..WINDOW {
            let j = self.bit_rev[i];
            if j > i {
                re.swap(i, j);
                im.swap(i, j);
            }
        }
        let mut len = 2;
        while len <= WINDOW {
            let half = len / 2;
            let step = WINDOW / len;
            for i in (0..WINDOW).step_by(len) {
                for k in 0..half {
                    let (wr, wi) = (self.tw_cos[k * step], self.tw_sin[k * step]);
                    let (a, b) = (i + k, i + k + half);
                    let tr = re[b] * wr - im[b] * wi;
                    let ti = re[b] * wi + im[b] * wr;
                    re[b] = re[a] - tr;
                    im[b] = im[a] - ti;
                    re[a] += tr;
                    im[a] += ti;
                }
            }
            len <<= 1;
        }
    }

    fn analyze(&mut self) {
        let mut re = vec![0.0f32; WINDOW];
        let mut im = vec![0.0f32; WINDOW];
        for i in 0..WINDOW {
            re[i] = self.ring[(self.write + i) % WINDOW] * self.window[i];
        }
        self.fft(&mut re, &mut im);
        let mut raw = [0.0f32; BANDS];
        let mut peak = 0.0f32;
        for b in 0..BANDS {
            let (mut sum, mut max) = (0.0f32, 0.0f32);
            for k in self.bin_lo[b]..self.bin_hi[b] {
                let p = re[k] * re[k] + im[k] * im[k];
                sum += p;
                max = max.max(p);
            }
            // Metade média, metade bin mais forte.
            let power = 0.5 * (sum / (self.bin_hi[b] - self.bin_lo[b]) as f32) + 0.5 * max;
            // 4/N normaliza uma senoide com janela Hann para ~sua amplitude.
            let mut amp = power.sqrt() * (4.0 / WINDOW as f32);
            // Música tem inclinação espectral forte: levanta os agudos.
            amp *= 1.0 + 2.2 * b as f32 / (BANDS - 1) as f32;
            raw[b] = amp;
            peak = peak.max(amp);
        }
        self.peak = peak;
        self.agc = peak.max(self.agc * 0.9985);
        let norm = 1.0 / self.agc.max(0.02);
        let gate = (peak * 40.0).clamp(0.0, 1.0);
        for b in 0..BANDS {
            let v = (raw[b] * norm).clamp(0.0, 1.0).sqrt() * gate;
            let rate = if v > self.smooth[b] { 0.65 } else { 0.14 };
            self.smooth[b] += rate * (v - self.smooth[b]);
        }
    }
}
```

- [ ] **Step 5: Rodar os testes**

Run: `cargo test --manifest-path spikes/s6-island/Cargo.toml analyzer`
Expected: PASS (5 testes).

- [ ] **Step 6: Commit**

```powershell
git add spikes/s6-island
git commit -m "spike(s6): copy S1 base and add spectrum analyzer ported from Dynamic Island" -m "Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

---

## Task 12: S6 — Captura do som do sistema (WASAPI loopback)

**Files:**
- Create: `spikes/s6-island/src/capture.rs`
- Modify: `spikes/s6-island/src/main.rs` (`mod capture;` e um modo de teste `--capture-only`)

**Interfaces:**
- Consumes: Task 11 (`Analyzer`).
- Produces: `capture::{Capture, Capture::start(sink) -> Capture, Capture::stop(self)}`, com `sink: Box<dyn FnMut(&[f32], f32) + Send>` recebendo amostras **mono** e a taxa de amostragem. A captura se reinicia sozinha quando o dispositivo de saída padrão muda ou é invalidado.

- [ ] **Step 1: Escrever `capture.rs`**

```rust
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};
use windows::Win32::Media::Audio::*;
use windows::Win32::System::Com::*;
use windows::core::Result;

pub type Sink = Box<dyn FnMut(&[f32], f32) + Send>;

pub struct Capture {
    stop: Arc<AtomicBool>,
    handle: Option<JoinHandle<()>>,
}

impl Capture {
    pub fn start(mut sink: Sink) -> Capture {
        let stop = Arc::new(AtomicBool::new(false));
        let flag = stop.clone();
        let handle = std::thread::Builder::new()
            .name("zefiro-audio".into())
            .spawn(move || {
                // SAFETY: COM inicializado nesta thread, liberado no fim.
                unsafe { let _ = CoInitializeEx(None, COINIT_MULTITHREADED); }
                while !flag.load(Ordering::Relaxed) {
                    match run_once(&flag, &mut sink) {
                        Ok(()) => {}
                        Err(e) => {
                            eprintln!("[audio] erro: {e}; tentando de novo em 1 s");
                            std::thread::sleep(Duration::from_secs(1));
                        }
                    }
                }
                // SAFETY: par do CoInitializeEx.
                unsafe { CoUninitialize() };
            })
            .expect("thread de áudio");
        Capture { stop, handle: Some(handle) }
    }

    pub fn stop(mut self) {
        self.stop.store(true, Ordering::Relaxed);
        if let Some(h) = self.handle.take() {
            let _ = h.join();
        }
    }
}

fn default_device_id(e: &IMMDeviceEnumerator) -> Result<String> {
    // SAFETY: COM válido; string liberada pelo CoTaskMemFree implícito em to_string.
    unsafe {
        let id = e.GetDefaultAudioEndpoint(eRender, eConsole)?.GetId()?;
        let s = id.to_string().unwrap_or_default();
        CoTaskMemFree(Some(id.0 as _));
        Ok(s)
    }
}

/// Captura até o dispositivo padrão mudar, ficar inválido ou pedirem para parar.
fn run_once(stop: &AtomicBool, sink: &mut Sink) -> Result<()> {
    // SAFETY: sequência padrão de captura em loopback; buffers liberados com ReleaseBuffer.
    unsafe {
        let enumerator: IMMDeviceEnumerator = CoCreateInstance(&MMDeviceEnumerator, None, CLSCTX_ALL)?;
        let device_id = default_device_id(&enumerator)?;
        let device = enumerator.GetDefaultAudioEndpoint(eRender, eConsole)?;
        let client: IAudioClient = device.Activate(CLSCTX_ALL, None)?;
        let fmt = client.GetMixFormat()?;
        let channels = (*fmt).nChannels as usize;
        let rate = (*fmt).nSamplesPerSec as f32;
        let bits = (*fmt).wBitsPerSample;
        eprintln!("[audio] dispositivo={device_id} canais={channels} taxa={rate} bits={bits}");
        // O mix format compartilhado é float 32 bits na prática; outros formatos são ignorados no spike.
        if bits != 32 {
            CoTaskMemFree(Some(fmt as _));
            return Err(windows::core::Error::new(windows::Win32::Foundation::E_NOTIMPL, "formato de mix diferente de float32"));
        }
        client.Initialize(AUDCLNT_SHAREMODE_SHARED, AUDCLNT_STREAMFLAGS_LOOPBACK, 200_000, 0, fmt, None)?;
        CoTaskMemFree(Some(fmt as _));
        let capture: IAudioCaptureClient = client.GetService()?;
        client.Start()?;

        let mut mono = Vec::with_capacity(4096);
        let mut last_check = Instant::now();
        let result = loop {
            if stop.load(Ordering::Relaxed) {
                break Ok(());
            }
            std::thread::sleep(Duration::from_millis(10));
            if last_check.elapsed() > Duration::from_secs(2) {
                last_check = Instant::now();
                if default_device_id(&enumerator).ok().as_deref() != Some(device_id.as_str()) {
                    eprintln!("[audio] dispositivo padrão mudou: reiniciando captura");
                    break Ok(());
                }
            }
            let mut packet = match capture.GetNextPacketSize() {
                Ok(p) => p,
                Err(e) => break Err(e), // inclui AUDCLNT_E_DEVICE_INVALIDATED
            };
            while packet > 0 {
                let mut data = std::ptr::null_mut();
                let mut frames = 0u32;
                let mut flags = 0u32;
                capture.GetBuffer(&mut data, &mut frames, &mut flags, None, None)?;
                mono.clear();
                if flags & AUDCLNT_BUFFERFLAGS_SILENT.0 as u32 != 0 || data.is_null() {
                    mono.resize(frames as usize, 0.0);
                } else {
                    let samples = std::slice::from_raw_parts(data as *const f32, frames as usize * channels);
                    mono.extend(samples.chunks(channels).map(|c| c.iter().sum::<f32>() / channels as f32));
                }
                capture.ReleaseBuffer(frames)?;
                sink(&mono, rate);
                packet = capture.GetNextPacketSize()?;
            }
        };
        let _ = client.Stop();
        result
    }
}
```

- [ ] **Step 2: Adicionar o modo `--capture-only` no `main.rs`**

No início do `main` do S6 (antes de qualquer janela), acrescentar:

```rust
    if std::env::args().any(|a| a == "--capture-only") {
        use std::sync::{Arc, Mutex};
        let analyzer: Arc<Mutex<Option<analyzer::Analyzer>>> = Arc::new(Mutex::new(None));
        let shared = analyzer.clone();
        let cap = capture::Capture::start(Box::new(move |mono, rate| {
            let mut a = shared.lock().unwrap();
            let a = a.get_or_insert_with(|| analyzer::Analyzer::new(rate));
            a.push(mono);
        }));
        for _ in 0..20 {
            std::thread::sleep(std::time::Duration::from_millis(500));
            if let Some(a) = analyzer.lock().unwrap().as_ref() {
                let bars: String = a.bands().iter().map(|v| [' ', '▁', '▂', '▃', '▄', '▅', '▆', '▇', '█'][(v * 8.0).round() as usize]).collect();
                eprintln!("[viz] {bars} pico={:.3}", a.peak());
            }
        }
        cap.stop();
        return Ok(());
    }
```

e declarar `mod capture;`.

- [ ] **Step 3: Rodar com música tocando**

Tocar uma música (Spotify ou YouTube) e rodar:

Run: `cargo run --release --manifest-path spikes/s6-island/Cargo.toml -- --capture-only`
Expected: a cada 0,5 s uma linha `[viz]` com barras que se mexem; `pico` > 0.

Pausar a música: as barras caem para vazio em ~1 s e `pico` fica perto de 0.

- [ ] **Step 4: Rodar sem nenhum som**

Run: `cargo run --release --manifest-path spikes/s6-island/Cargo.toml -- --capture-only`
Expected: barras vazias o tempo todo, sem erro (sem pacotes = sem chamadas ao `sink`; isso é esperado).

- [ ] **Step 5: Medir o custo da captura**

Com música tocando e `--capture-only` rodando (ampliar o laço para 120 iterações temporariamente, se precisar de mais tempo):

```powershell
pwsh -NoProfile -File scripts/perf.ps1 -Process s6-island -WarmupSeconds 5 -SampleSeconds 40 -IntervalSeconds 2 -Out scripts/out/s6-capture.json
```

Anotar o `cpu_percent_avg` (vai para o ADR-0006).

- [ ] **Step 6: Trocar o dispositivo de saída durante a captura (Review Focus 4)**

Com música tocando e `--capture-only` rodando, trocar a saída padrão (`Win+Ctrl+V` ou conectar um fone).

Expected: em até 2 s o log mostra `[audio] dispositivo padrão mudou: reiniciando captura`, depois uma linha `[audio] dispositivo=…` com o novo ID, e as barras voltam a se mexer. Nenhum panic.

- [ ] **Step 7: Commit**

```powershell
git add spikes/s6-island
git commit -m "spike(s6): WASAPI loopback capture with default-device change restart" -m "Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

---

## Task 13: S6 — Máquina de estados da ilha e cor da capa

**Files:**
- Create: `spikes/s6-island/src/island.rs`, `spikes/s6-island/src/color.rs`, `spikes/s6-island/src/media.rs`

**Interfaces:**
- Consumes: nada além da biblioteca padrão.
- Produces:
  - `island::{IslandState::{Compact, Peek, Expanded}, IslandInput::{TrackChanged, HoverEnter, HoverLeave, Click, ClickOutside, Tick}, Island, Island::new(), Island::handle(input, now_secs: f64) -> bool, Geometry { width, height, radius }, target_geometry(state, compact_width, bar_height) -> Geometry, PEEK_SECS = 3.0, PEEK_WIDTH = 380.0, EXPANDED = (400.0, 184.0, 22.0)}`.
  - `color::dominant_color(bgra: &[u8], width: usize, height: usize) -> [u8; 3]`.
  - `media::{FakeTrack { title, artist, source, hue }, TRACKS: [FakeTrack; 3], fake_cover(hue: [u8; 3]) -> Vec<u8>, COVER_SIZE = 32}`.

- [ ] **Step 1: Escrever os testes da máquina de estados (falhando)**

`spikes/s6-island/src/island.rs`:

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IslandState { Compact, Peek, Expanded }

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IslandInput { TrackChanged, HoverEnter, HoverLeave, Click, ClickOutside, Tick }

pub const PEEK_SECS: f64 = 3.0;
pub const PEEK_WIDTH: f32 = 380.0;
pub const EXPANDED: (f32, f32, f32) = (400.0, 184.0, 22.0);

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Geometry { pub width: f32, pub height: f32, pub radius: f32 }

#[derive(Debug, Clone)]
pub struct Island { pub state: IslandState, peek_until: Option<f64>, hovering: bool }

#[cfg(test)]
mod tests {
    use super::*;
    use IslandInput::*;
    use IslandState::*;

    #[test]
    fn track_change_peeks_for_three_seconds_then_returns() {
        let mut i = Island::new();
        assert!(i.handle(TrackChanged, 10.0));
        assert_eq!(i.state, Peek);
        assert!(!i.handle(Tick, 12.9));
        assert_eq!(i.state, Peek);
        assert!(i.handle(Tick, 13.0));
        assert_eq!(i.state, Compact);
    }

    #[test]
    fn hover_keeps_peek_open_until_leaving() {
        let mut i = Island::new();
        i.handle(HoverEnter, 0.0);
        assert_eq!(i.state, Peek);
        i.handle(Tick, 60.0);
        assert_eq!(i.state, Peek, "passando o mouse, não fecha sozinho");
        i.handle(HoverLeave, 61.0);
        assert_eq!(i.state, Compact);
    }

    #[test]
    fn leaving_hover_during_a_track_peek_waits_for_the_timeout() {
        let mut i = Island::new();
        i.handle(TrackChanged, 0.0);
        i.handle(HoverEnter, 1.0);
        i.handle(HoverLeave, 2.0);
        assert_eq!(i.state, Peek);
        i.handle(Tick, 3.0);
        assert_eq!(i.state, Compact);
    }

    #[test]
    fn click_toggles_expanded_and_click_outside_collapses() {
        let mut i = Island::new();
        i.handle(Click, 0.0);
        assert_eq!(i.state, Expanded);
        i.handle(TrackChanged, 1.0);
        assert_eq!(i.state, Expanded, "troca de faixa não fecha o expandido");
        i.handle(Click, 2.0);
        assert_eq!(i.state, Compact);
        i.handle(Click, 3.0);
        i.handle(ClickOutside, 4.0);
        assert_eq!(i.state, Compact);
    }

    #[test]
    fn geometry_per_state() {
        assert_eq!(target_geometry(Compact, 220.0, 32.0), Geometry { width: 220.0, height: 28.0, radius: 14.0 });
        assert_eq!(target_geometry(Peek, 220.0, 32.0), Geometry { width: 380.0, height: 28.0, radius: 14.0 });
        assert_eq!(target_geometry(Expanded, 220.0, 32.0), Geometry { width: 400.0, height: 184.0, radius: 22.0 });
    }
}
```

- [ ] **Step 2: Rodar para ver falhar**

Adicionar `mod island;` ao `main.rs`.

Run: `cargo test --manifest-path spikes/s6-island/Cargo.toml island`
Expected: FAIL de compilação (`new`, `handle`, `target_geometry`).

- [ ] **Step 3: Implementar a máquina de estados**

Adicionar acima do `#[cfg(test)]`:

```rust
impl Default for Island {
    fn default() -> Self {
        Self::new()
    }
}

impl Island {
    pub fn new() -> Self {
        Self { state: IslandState::Compact, peek_until: None, hovering: false }
    }

    /// Aplica um evento. Devolve `true` se o estado mudou.
    pub fn handle(&mut self, input: IslandInput, now: f64) -> bool {
        use IslandInput::*;
        use IslandState::*;
        let before = self.state;
        match (input, self.state) {
            (TrackChanged, Compact | Peek) => {
                self.state = Peek;
                self.peek_until = Some(now + PEEK_SECS);
            }
            (HoverEnter, _) => {
                self.hovering = true;
                if self.state == Compact {
                    self.state = Peek;
                }
            }
            (HoverLeave, _) => {
                self.hovering = false;
                if self.state == Peek && self.peek_until.is_none_or(|t| now >= t) {
                    self.state = Compact;
                    self.peek_until = None;
                }
            }
            (Tick, Peek) => {
                if !self.hovering && self.peek_until.is_some_and(|t| now >= t) {
                    self.state = Compact;
                    self.peek_until = None;
                }
            }
            (Click, Compact | Peek) => {
                self.state = Expanded;
                self.peek_until = None;
            }
            (Click | ClickOutside, Expanded) => self.state = Compact,
            _ => {}
        }
        self.state != before
    }
}

pub fn target_geometry(state: IslandState, compact_width: f32, bar_height: f32) -> Geometry {
    let h = bar_height - 4.0;
    match state {
        IslandState::Compact => Geometry { width: compact_width, height: h, radius: h / 2.0 },
        IslandState::Peek => Geometry { width: PEEK_WIDTH, height: h, radius: h / 2.0 },
        IslandState::Expanded => Geometry { width: EXPANDED.0, height: EXPANDED.1, radius: EXPANDED.2 },
    }
}
```

- [ ] **Step 4: Rodar os testes**

Run: `cargo test --manifest-path spikes/s6-island/Cargo.toml island`
Expected: PASS (5 testes).

- [ ] **Step 5: Escrever os testes da cor dominante (falhando)**

`spikes/s6-island/src/color.rs`:

```rust
/// Cor dominante de uma imagem BGRA: amostra uma grade 16×16 e pondera pela saturação.
pub fn dominant_color(_bgra: &[u8], _width: usize, _height: usize) -> [u8; 3] {
    unimplemented!()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn image(w: usize, h: usize, f: impl Fn(usize, usize) -> [u8; 3]) -> Vec<u8> {
        let mut v = Vec::with_capacity(w * h * 4);
        for y in 0..h {
            for x in 0..w {
                let [r, g, b] = f(x, y);
                v.extend_from_slice(&[b, g, r, 255]);
            }
        }
        v
    }

    #[test]
    fn solid_color_is_returned_as_is() {
        let img = image(32, 32, |_, _| [200, 30, 40]);
        assert_eq!(dominant_color(&img, 32, 32), [200, 30, 40]);
    }

    #[test]
    fn saturated_half_beats_gray_half() {
        let img = image(32, 32, |x, _| if x < 16 { [128, 128, 128] } else { [0, 0, 255] });
        let [r, _, b] = dominant_color(&img, 32, 32);
        assert!(b > 180 && r < 80, "{r} {b}");
    }

    #[test]
    fn empty_or_tiny_image_returns_neutral_gray() {
        assert_eq!(dominant_color(&[], 0, 0), [128, 128, 128]);
    }
}
```

Adicionar `mod color;` ao `main.rs`.

- [ ] **Step 6: Rodar para ver falhar**

Run: `cargo test --manifest-path spikes/s6-island/Cargo.toml color`
Expected: FAIL (`not implemented`).

- [ ] **Step 7: Implementar a cor dominante**

```rust
pub fn dominant_color(bgra: &[u8], width: usize, height: usize) -> [u8; 3] {
    if width == 0 || height == 0 || bgra.len() < width * height * 4 {
        return [128, 128, 128];
    }
    let (mut r, mut g, mut b, mut total) = (0.0f64, 0.0f64, 0.0f64, 0.0f64);
    for gy in 0..16 {
        for gx in 0..16 {
            let x = (gx * width + width / 2) / 16;
            let y = (gy * height + height / 2) / 16;
            let i = (y.min(height - 1) * width + x.min(width - 1)) * 4;
            let (pb, pg, pr) = (bgra[i] as f64, bgra[i + 1] as f64, bgra[i + 2] as f64);
            let sat = (pr.max(pg).max(pb) - pr.min(pg).min(pb)) / 255.0;
            let w = sat * sat * 4.0 + 0.05;
            r += pr * w;
            g += pg * w;
            b += pb * w;
            total += w;
        }
    }
    [(r / total).round() as u8, (g / total).round() as u8, (b / total).round() as u8]
}
```

- [ ] **Step 8: Rodar os testes**

Run: `cargo test --manifest-path spikes/s6-island/Cargo.toml color`
Expected: PASS (3 testes).

- [ ] **Step 9: Escrever `media.rs` (fonte falsa)**

```rust
//! Fonte de mídia falsa do S6: a integração real com o SMTC é o S4.

pub const COVER_SIZE: usize = 32;

#[derive(Debug, Clone, Copy)]
pub struct FakeTrack {
    pub title: &'static str,
    pub artist: &'static str,
    pub source: &'static str,
    pub hue: [u8; 3],
}

pub const TRACKS: [FakeTrack; 3] = [
    FakeTrack { title: "Nocturne", artist: "Cavalari", source: "Spotify", hue: [145, 132, 217] },
    FakeTrack { title: "Live coding", artist: "Canal Dev", source: "YouTube", hue: [232, 80, 70] },
    FakeTrack { title: "Speedrun", artist: "Streamer", source: "Twitch", hue: [120, 70, 230] },
];

/// Capa falsa BGRA: gradiente na cor dada sobre um fundo cinza.
pub fn fake_cover(hue: [u8; 3]) -> Vec<u8> {
    let mut v = Vec::with_capacity(COVER_SIZE * COVER_SIZE * 4);
    for y in 0..COVER_SIZE {
        for x in 0..COVER_SIZE {
            let t = (x + y) as f32 / (2 * COVER_SIZE) as f32;
            let mix = |c: u8| (60.0 * (1.0 - t) + c as f32 * t) as u8;
            v.extend_from_slice(&[mix(hue[2]), mix(hue[1]), mix(hue[0]), 255]);
        }
    }
    v
}
```

Adicionar `mod media;` ao `main.rs`.

- [ ] **Step 10: Commit**

```powershell
git add spikes/s6-island
git commit -m "spike(s6): island state machine, cover dominant color and fake media source" -m "Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

---

## Task 14: S6 — Janela da ilha, transformação animada, visualizador e cliques que atravessam

**Files:**
- Create: `spikes/s6-island/src/island_view.rs`, `spikes/s6-island/src/click.rs`, `spikes/s6-island/src/island_args.rs`
- Modify: `spikes/s6-island/src/events.rs` (eventos de mouse), `spikes/s6-island/src/window.rs` (`WM_MOUSEMOVE`, `WM_MOUSELEAVE`, `create_island`), `spikes/s6-island/src/main.rs` (substituir o `main` inteiro, mantendo o bloco `--capture-only` no início)

**Interfaces:**
- Consumes: Tasks 11–13; ADR-0001 (compositor). O código abaixo usa **Windows.UI.Composition** (caminho B). Se o ADR-0001 escolheu o caminho A, ver o Passo 4b.
- Produces:
  - `island_args::{IslandArgs { click: ClickMode, style: SpringStyle, speed: f32, visualizer: bool }, ClickMode::{Resize, Region, Transparent}, parse(&[&str]) -> Result<IslandArgs, String>}`.
  - `island_view::IslandView` com `new(gfx, hwnd, window_w, window_h, scale)`, `draw_strip(gfx, text, cover_rgb)`, `draw_expanded(gfx, track, playing)`, `go_to(state, geo, params)`, `set_bars(&[f32; BANDS])`, `set_tint([u8; 3])`, `set_backdrop_enabled(bool)`, `slide(out: bool)`, campos `springs_enabled`, `style`, `speed`, `window_w`.
  - `click::{apply_region(hwnd, rect, radius), set_transparent(hwnd, bool), cursor_inside(hwnd, rect) -> bool}`.

- [ ] **Step 1: Escrever `island_args.rs` com testes**

```rust
use crate::spring::SpringStyle;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClickMode { Resize, Region, Transparent }

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct IslandArgs { pub click: ClickMode, pub style: SpringStyle, pub speed: f32, pub visualizer: bool }

pub fn parse(args: &[&str]) -> Result<IslandArgs, String> {
    let mut a = IslandArgs { click: ClickMode::Region, style: SpringStyle::Default, speed: 1.0, visualizer: true };
    let mut it = args.iter();
    while let Some(flag) = it.next() {
        let mut value = || it.next().copied().ok_or_else(|| format!("faltou o valor de {flag}"));
        match *flag {
            "--click" => a.click = match value()? { "resize" => ClickMode::Resize, "region" => ClickMode::Region, "transparent" => ClickMode::Transparent, v => return Err(format!("--click inválido: {v}")) },
            "--style" => a.style = match value()? { "smooth" => SpringStyle::Smooth, "default" => SpringStyle::Default, "bouncy" => SpringStyle::Bouncy, "snappy" => SpringStyle::Snappy, v => return Err(format!("--style inválido: {v}")) },
            "--speed" => {
                a.speed = value()?.parse().map_err(|_| "--speed precisa ser número".to_string())?;
                if !(0.5..=2.0).contains(&a.speed) { return Err("--speed vai de 0.5 a 2.0".into()); }
            }
            "--visualizer" => a.visualizer = match value()? { "on" => true, "off" => false, v => return Err(format!("--visualizer inválido: {v}")) },
            other => return Err(format!("argumento desconhecido: {other}")),
        }
    }
    Ok(a)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults() {
        assert_eq!(parse(&[]).unwrap(), IslandArgs { click: ClickMode::Region, style: SpringStyle::Default, speed: 1.0, visualizer: true });
    }

    #[test]
    fn parses_flags_and_validates_speed() {
        let a = parse(&["--click", "transparent", "--style", "bouncy", "--speed", "1.5", "--visualizer", "off"]).unwrap();
        assert_eq!(a, IslandArgs { click: ClickMode::Transparent, style: SpringStyle::Bouncy, speed: 1.5, visualizer: false });
        assert!(parse(&["--speed", "3"]).is_err());
        assert!(parse(&["--click", "magic"]).is_err());
    }
}
```

Run: `cargo test --manifest-path spikes/s6-island/Cargo.toml island_args`
Expected: PASS (2 testes). (A função já vem implementada: é só parsing; os testes protegem a validação.)

- [ ] **Step 2: Eventos de mouse e janela da ilha**

Em `events.rs`, acrescentar ao `enum Event`:

```rust
    MouseMove { hwnd: HWND },
    MouseLeave { hwnd: HWND },
```

Em `window.rs`, acrescentar no `match msg` do `wndproc` (antes do `_ =>`):

```rust
        WM_MOUSEMOVE => {
            let mut tme = windows::Win32::UI::Input::KeyboardAndMouse::TRACKMOUSEEVENT {
                cbSize: size_of::<windows::Win32::UI::Input::KeyboardAndMouse::TRACKMOUSEEVENT>() as u32,
                dwFlags: windows::Win32::UI::Input::KeyboardAndMouse::TME_LEAVE,
                hwndTrack: hwnd,
                dwHoverTime: 0,
            };
            // SAFETY: estrutura válida; pede WM_MOUSELEAVE.
            unsafe { let _ = windows::Win32::UI::Input::KeyboardAndMouse::TrackMouseEvent(&mut tme); }
            push(Event::MouseMove { hwnd })
        }
        WM_MOUSELEAVE => push(Event::MouseLeave { hwnd }),
```

e a função:

```rust
/// Janela da ilha. Com `layered = true` (modo Transparent) usa WS_EX_LAYERED para
/// que WS_EX_TRANSPARENT faça os cliques atravessarem para outros processos.
pub fn create_island(rect: RECT, layered: bool) -> Result<HWND> {
    let mut ex = WS_EX_TOOLWINDOW | WS_EX_NOREDIRECTIONBITMAP | WS_EX_TOPMOST | WS_EX_NOACTIVATE;
    if layered {
        ex |= WS_EX_LAYERED;
    }
    // SAFETY: classe registrada; sem parâmetros de criação.
    unsafe {
        let instance = GetModuleHandleW(None)?;
        let hwnd = CreateWindowExW(ex, CLASS, w!("Zefiro S6"), WS_POPUP, rect.left, rect.top,
            rect.right - rect.left, rect.bottom - rect.top, None, None, Some(instance.into()), None)?;
        if layered {
            SetLayeredWindowAttributes(hwnd, windows::Win32::Foundation::COLORREF(0), 255, LWA_ALPHA)?;
        }
        Ok(hwnd)
    }
}
```

- [ ] **Step 3: Escrever `click.rs`**

```rust
use windows::Win32::Foundation::{HWND, POINT, RECT};
use windows::Win32::Graphics::Gdi::{CreateRoundRectRgn, SetWindowRgn};
use windows::Win32::UI::WindowsAndMessaging::*;

/// Método (ii): região de janela com a forma da pílula (coordenadas da janela).
pub fn apply_region(hwnd: HWND, rect: RECT, radius: i32) {
    // SAFETY: a região passa a pertencer ao sistema após SetWindowRgn.
    unsafe {
        let rgn = CreateRoundRectRgn(rect.left, rect.top, rect.right + 1, rect.bottom + 1, radius * 2, radius * 2);
        SetWindowRgn(hwnd, Some(rgn), true);
    }
}

/// Método (iii): liga/desliga WS_EX_TRANSPARENT (exige WS_EX_LAYERED na criação).
pub fn set_transparent(hwnd: HWND, transparent: bool) {
    // SAFETY: leitura/escrita do estilo estendido da própria janela.
    unsafe {
        let ex = GetWindowLongPtrW(hwnd, GWL_EXSTYLE);
        let flag = WS_EX_TRANSPARENT.0 as isize;
        let new = if transparent { ex | flag } else { ex & !flag };
        if new != ex {
            SetWindowLongPtrW(hwnd, GWL_EXSTYLE, new);
        }
    }
}

/// O cursor está dentro de `rect` (coordenadas da janela)?
pub fn cursor_inside(hwnd: HWND, rect: RECT) -> bool {
    let mut p = POINT::default();
    let mut w = RECT::default();
    // SAFETY: buffers válidos.
    unsafe {
        if GetCursorPos(&mut p).is_err() || GetWindowRect(hwnd, &mut w).is_err() {
            return false;
        }
    }
    let (x, y) = (p.x - w.left, p.y - w.top);
    x >= rect.left && x < rect.right && y >= rect.top && y < rect.bottom
}
```

- [ ] **Step 4: Escrever `island_view.rs` (Windows.UI.Composition)**

```rust
use windows::Foundation::Numerics::{Matrix3x2, Vector2, Vector3};
use windows::Foundation::{IReference, PropertyValue, Size, TimeSpan};
use windows::Graphics::DirectX::{DirectXAlphaMode, DirectXPixelFormat};
use windows::System::DispatcherQueueController;
use windows::UI::Color;
use windows::UI::Composition::Desktop::DesktopWindowTarget;
use windows::UI::Composition::*;
use windows::Win32::Foundation::{HWND, POINT};
use windows::Win32::Graphics::Direct2D::Common::*;
use windows::Win32::Graphics::Direct2D::*;
use windows::Win32::Graphics::DirectWrite::*;
use windows::Win32::System::WinRT::Composition::{ICompositionDrawingSurfaceInterop, ICompositorDesktopInterop, ICompositorInterop};
use windows::Win32::System::WinRT::{CreateDispatcherQueueController, DQTAT_COM_STA, DQTYPE_THREAD_CURRENT, DispatcherQueueOptions};
use windows::core::{HSTRING, Interface, Result};

use crate::analyzer::BANDS;
use crate::gfx::{Gfx, rgba};
use crate::island::{EXPANDED, Geometry, IslandState, PEEK_WIDTH};
use crate::media::FakeTrack;
use crate::spring::{SpringParams, SpringStyle};

pub const STRIP_H: f32 = 28.0;
const BAR_W: f32 = 2.0;
const BAR_GAP: f32 = 1.0;
const BAR_MAX_H: f32 = 14.0;
const FADE_SECS: f64 = 0.32;
const SLIDE_SHIFT: f32 = 14.0;

pub struct IslandView {
    _queue: DispatcherQueueController,
    compositor: Compositor,
    _gdev: CompositionGraphicsDevice,
    _target: DesktopWindowTarget,
    pub window_w: f32,
    scale: f32,
    geom: CompositionRoundedRectangleGeometry,
    backdrop: SpriteVisual,
    tint_brush: CompositionColorBrush,
    strip_group: ContainerVisual,
    strip_surface: CompositionDrawingSurface,
    expanded: SpriteVisual,
    expanded_surface: CompositionDrawingSurface,
    bars: Vec<SpriteVisual>,
    pub springs_enabled: bool,
    pub style: SpringStyle,
    pub speed: f32,
}

fn draw_on(surface: &CompositionDrawingSurface, paint: impl FnOnce(&ID2D1DeviceContext) -> Result<()>) -> Result<()> {
    let interop: ICompositionDrawingSurfaceInterop = surface.cast()?;
    // SAFETY: BeginDraw/EndDraw pareados.
    unsafe {
        let mut off = POINT::default();
        let dc: ID2D1DeviceContext = interop.BeginDraw(None, &mut off)?;
        dc.SetTransform(&Matrix3x2::translation(off.x as f32, off.y as f32));
        dc.Clear(Some(&D2D1_COLOR_F { r: 0.0, g: 0.0, b: 0.0, a: 0.0 }));
        let r = paint(&dc);
        interop.EndDraw()?;
        r
    }
}

fn text(dc: &ID2D1DeviceContext, s: &str, fmt: &IDWriteTextFormat, rect: D2D_RECT_F, brush: &ID2D1SolidColorBrush) {
    let utf16: Vec<u16> = s.encode_utf16().collect();
    // SAFETY: buffers válidos.
    unsafe { dc.DrawText(&utf16, fmt, &rect, brush, D2D1_DRAW_TEXT_OPTIONS_NONE, DWRITE_MEASURING_MODE_NATURAL) };
}

impl IslandView {
    pub fn new(gfx: &Gfx, hwnd: HWND, window_w: f32, window_h: f32, scale: f32) -> Result<Self> {
        let options = DispatcherQueueOptions { dwSize: size_of::<DispatcherQueueOptions>() as u32, threadType: DQTYPE_THREAD_CURRENT, apartmentType: DQTAT_COM_STA };
        // SAFETY: fila de despacho desta thread.
        let queue = unsafe { CreateDispatcherQueueController(options)? };
        let compositor = Compositor::new()?;
        let interop: ICompositorInterop = compositor.cast()?;
        // SAFETY: dispositivo D2D válido.
        let gdev = unsafe { interop.CreateGraphicsDevice(&gfx.d2d_device)? };
        let desktop: ICompositorDesktopInterop = compositor.cast()?;
        // SAFETY: hwnd com WS_EX_NOREDIRECTIONBITMAP.
        let target = unsafe { desktop.CreateDesktopWindowTarget(hwnd, true)? };

        let full = Vector2 { X: window_w, Y: window_h };
        let root = compositor.CreateContainerVisual()?;
        root.SetSize(full)?;
        let pill = compositor.CreateContainerVisual()?;
        pill.SetSize(full)?;
        let geom = compositor.CreateRoundedRectangleGeometry()?;
        pill.SetClip(&compositor.CreateGeometricClipWithGeometry(&geom)?)?;

        let backdrop = compositor.CreateSpriteVisual()?;
        backdrop.SetBrush(&compositor.CreateHostBackdropBrush()?)?;
        backdrop.SetSize(full)?;
        let tint_brush = compositor.CreateColorBrushWithColor(Color { A: 0xB0, R: 0x16, G: 0x18, B: 0x26 })?;
        let tint = compositor.CreateSpriteVisual()?;
        tint.SetBrush(&tint_brush)?;
        tint.SetSize(full)?;

        let strip_size = Size { Width: PEEK_WIDTH * scale, Height: STRIP_H * scale };
        let strip_surface = gdev.CreateDrawingSurface(strip_size, DirectXPixelFormat::B8G8R8A8UIntNormalized, DirectXAlphaMode::Premultiplied)?;
        let strip = compositor.CreateSpriteVisual()?;
        strip.SetBrush(&compositor.CreateSurfaceBrushWithSurface(&strip_surface)?)?;
        strip.SetSize(Vector2 { X: strip_size.Width, Y: strip_size.Height })?;
        let strip_group = compositor.CreateContainerVisual()?;
        strip_group.SetSize(Vector2 { X: strip_size.Width, Y: strip_size.Height })?;
        strip_group.Children()?.InsertAtTop(&strip)?;

        let bar_color = compositor.CreateColorBrushWithColor(Color { A: 0xFF, R: 0xE9, G: 0xE9, B: 0xED })?;
        let mut bars = Vec::with_capacity(BANDS);
        for i in 0..BANDS {
            let b = compositor.CreateSpriteVisual()?;
            b.SetBrush(&bar_color)?;
            b.SetSize(Vector2 { X: BAR_W * scale, Y: BAR_MAX_H * scale })?;
            b.SetOffset(Vector3 { X: (10.0 + i as f32 * (BAR_W + BAR_GAP)) * scale, Y: (STRIP_H - BAR_MAX_H) / 2.0 * scale, Z: 0.0 })?;
            b.SetCenterPoint(Vector3 { X: 0.0, Y: BAR_MAX_H * scale, Z: 0.0 })?;
            b.SetScale(Vector3 { X: 1.0, Y: 0.08, Z: 1.0 })?;
            strip_group.Children()?.InsertAtTop(&b)?;
            bars.push(b);
        }

        let exp_size = Size { Width: EXPANDED.0 * scale, Height: EXPANDED.1 * scale };
        let expanded_surface = gdev.CreateDrawingSurface(exp_size, DirectXPixelFormat::B8G8R8A8UIntNormalized, DirectXAlphaMode::Premultiplied)?;
        let expanded = compositor.CreateSpriteVisual()?;
        expanded.SetBrush(&compositor.CreateSurfaceBrushWithSurface(&expanded_surface)?)?;
        expanded.SetSize(Vector2 { X: exp_size.Width, Y: exp_size.Height })?;
        expanded.SetOpacity(0.0)?;

        let children = pill.Children()?;
        children.InsertAtTop(&backdrop)?;
        children.InsertAtTop(&tint)?;
        children.InsertAtTop(&strip_group)?;
        children.InsertAtTop(&expanded)?;
        root.Children()?.InsertAtTop(&pill)?;
        target.SetRoot(&root)?;

        Ok(Self {
            _queue: queue, compositor, _gdev: gdev, _target: target, window_w, scale, geom, backdrop, tint_brush,
            strip_group, strip_surface, expanded, expanded_surface, bars,
            springs_enabled: true, style: SpringStyle::Default, speed: 1.0,
        })
    }

    pub fn pill_left(&self, geo: &Geometry) -> f32 {
        (self.window_w - geo.width * self.scale) / 2.0
    }

    fn spring(&self, value: f32, params: SpringParams) -> Result<SpringScalarNaturalMotionAnimation> {
        let (period, ratio) = params.styled(self.style).to_period_and_ratio();
        let a = self.compositor.CreateSpringScalarAnimation()?;
        a.SetDampingRatio(ratio)?;
        a.SetPeriod(TimeSpan { Duration: (period as f64 / self.speed as f64 * 1e7) as i64 })?;
        a.SetFinalValue(&PropertyValue::CreateSingle(value)?.cast::<IReference<f32>>()?)?;
        Ok(a)
    }

    fn fade(&self, target: &impl Interface, to: f32, secs: f64) -> Result<()> {
        let obj: CompositionObject = target.cast()?;
        let a = self.compositor.CreateScalarKeyFrameAnimation()?;
        a.InsertKeyFrame(1.0, to)?;
        a.SetDuration(TimeSpan { Duration: (secs / self.speed as f64 * 1e7) as i64 })?;
        obj.StartAnimation(&HSTRING::from("Opacity"), &a)
    }

    /// Transforma a pílula para `geo`. Com molas desligadas, aplica na hora.
    pub fn go_to(&self, state: IslandState, geo: Geometry, params: SpringParams) -> Result<()> {
        let s = self.scale;
        let left = self.pill_left(&geo);
        let strip_visible = if state == IslandState::Expanded { 0.0 } else { 1.0 };
        let expanded_visible = 1.0 - strip_visible;
        if !self.springs_enabled {
            self.geom.SetSize(Vector2 { X: geo.width * s, Y: geo.height * s })?;
            self.geom.SetCornerRadius(Vector2 { X: geo.radius * s, Y: geo.radius * s })?;
            self.geom.SetOffset(Vector2 { X: left, Y: 2.0 * s })?;
            self.strip_group.SetOffset(Vector3 { X: left, Y: 2.0 * s, Z: 0.0 })?;
            self.expanded.SetOffset(Vector3 { X: left, Y: 2.0 * s, Z: 0.0 })?;
            self.strip_group.SetOpacity(strip_visible)?;
            self.expanded.SetOpacity(expanded_visible)?;
            return Ok(());
        }
        let geom: CompositionObject = self.geom.cast()?;
        for (prop, v) in [("Size.X", geo.width * s), ("Size.Y", geo.height * s), ("CornerRadius.X", geo.radius * s), ("CornerRadius.Y", geo.radius * s), ("Offset.X", left)] {
            geom.StartAnimation(&HSTRING::from(prop), &self.spring(v, params)?)?;
        }
        self.geom.SetOffset(Vector2 { X: self.geom.Offset()?.X, Y: 2.0 * s })?;
        self.strip_group.StartAnimation(&HSTRING::from("Offset.X"), &self.spring(left, params)?)?;
        self.expanded.StartAnimation(&HSTRING::from("Offset.X"), &self.spring(left, params)?)?;
        self.strip_group.SetOffset(Vector3 { X: self.strip_group.Offset()?.X, Y: 2.0 * s, Z: 0.0 })?;
        self.expanded.SetOffset(Vector3 { X: self.expanded.Offset()?.X, Y: 2.0 * s, Z: 0.0 })?;
        self.fade(&self.strip_group, strip_visible, FADE_SECS)?;
        self.fade(&self.expanded, expanded_visible, FADE_SECS)?;
        Ok(())
    }

    /// Troca de fonte: sai deslizando (0,17 s) ou entra (0,24 s), com 14 px.
    pub fn slide(&self, out: bool) -> Result<()> {
        let (from, to, secs, op) = if out { (0.0, -SLIDE_SHIFT, 0.17, 0.0) } else { (SLIDE_SHIFT, 0.0, 0.24, 1.0) };
        let content: CompositionObject = self.expanded.cast()?;
        let a = self.compositor.CreateScalarKeyFrameAnimation()?;
        a.InsertKeyFrame(0.0, from * self.scale)?;
        a.InsertKeyFrame(1.0, to * self.scale)?;
        a.SetDuration(TimeSpan { Duration: (secs / self.speed as f64 * 1e7) as i64 })?;
        content.StartAnimation(&HSTRING::from("TransformMatrix._41"), &a)?;
        self.fade(&self.expanded, op, secs)
    }

    pub fn set_bars(&self, bands: &[f32; BANDS]) -> Result<()> {
        for (b, v) in self.bars.iter().zip(bands) {
            b.SetScale(Vector3 { X: 1.0, Y: v.max(0.08), Z: 1.0 })?;
        }
        Ok(())
    }

    pub fn set_tint(&self, rgb: [u8; 3]) -> Result<()> {
        // Mistura 35 % da cor da capa no tom Nocturne.
        let mix = |a: u8, b: u8| (a as f32 * 0.65 + b as f32 * 0.35) as u8;
        self.tint_brush.SetColor(Color { A: 0xB0, R: mix(0x16, rgb[0]), G: mix(0x18, rgb[1]), B: mix(0x26, rgb[2]) })
    }

    pub fn set_backdrop_enabled(&self, on: bool) -> Result<()> {
        self.backdrop.SetIsVisible(on)?;
        let mut c = self.tint_brush.Color()?;
        c.A = if on { 0xB0 } else { 0xFF };
        self.tint_brush.SetColor(c)
    }

    pub fn draw_strip(&self, gfx: &Gfx, label: &str, cover_rgb: [u8; 3]) -> Result<()> {
        let s = self.scale;
        draw_on(&self.strip_surface, |dc| {
            // SAFETY: dentro de BeginDraw/EndDraw.
            unsafe {
                let fg = dc.CreateSolidColorBrush(&rgba(0xE9E9EDFF), None)?;
                let cover = dc.CreateSolidColorBrush(&D2D1_COLOR_F { r: cover_rgb[0] as f32 / 255.0, g: cover_rgb[1] as f32 / 255.0, b: cover_rgb[2] as f32 / 255.0, a: 1.0 }, None)?;
                let x0 = (10.0 + BANDS as f32 * (BAR_W + BAR_GAP) + 6.0) * s;
                dc.FillRoundedRectangle(&D2D1_ROUNDED_RECT { rect: D2D_RECT_F { left: x0, top: 4.0 * s, right: x0 + 20.0 * s, bottom: 24.0 * s }, radiusX: 4.0 * s, radiusY: 4.0 * s }, &cover);
                text(dc, label, &gfx.text, D2D_RECT_F { left: x0 + 28.0 * s, top: 0.0, right: PEEK_WIDTH * s - 10.0 * s, bottom: STRIP_H * s }, &fg);
            }
            Ok(())
        })
    }

    pub fn draw_expanded(&self, gfx: &Gfx, track: &FakeTrack, playing: bool) -> Result<()> {
        let s = self.scale;
        draw_on(&self.expanded_surface, |dc| {
            // SAFETY: dentro de BeginDraw/EndDraw.
            unsafe {
                let fg = dc.CreateSolidColorBrush(&rgba(0xE9E9EDFF), None)?;
                let dim = dc.CreateSolidColorBrush(&rgba(0xE9E9ED99), None)?;
                let track_bg = dc.CreateSolidColorBrush(&rgba(0xFFFFFF33), None)?;
                let accent = dc.CreateSolidColorBrush(&D2D1_COLOR_F { r: track.hue[0] as f32 / 255.0, g: track.hue[1] as f32 / 255.0, b: track.hue[2] as f32 / 255.0, a: 1.0 }, None)?;
                let r = |l: f32, t: f32, rr: f32, b: f32| D2D_RECT_F { left: l * s, top: t * s, right: rr * s, bottom: b * s };
                dc.FillRoundedRectangle(&D2D1_ROUNDED_RECT { rect: r(16.0, 16.0, 80.0, 80.0), radiusX: 10.0 * s, radiusY: 10.0 * s }, &accent);
                text(dc, track.title, &gfx.text, r(96.0, 18.0, 384.0, 40.0), &fg);
                text(dc, &format!("{} · {}", track.artist, track.source), &gfx.text, r(96.0, 42.0, 384.0, 64.0), &dim);
                text(dc, "1:12", &gfx.text, r(16.0, 92.0, 56.0, 108.0), &dim);
                text(dc, "3:40", &gfx.text, r(352.0, 92.0, 392.0, 108.0), &dim);
                dc.FillRoundedRectangle(&D2D1_ROUNDED_RECT { rect: r(60.0, 98.0, 344.0, 102.0), radiusX: 2.0 * s, radiusY: 2.0 * s }, &track_bg);
                dc.FillRoundedRectangle(&D2D1_ROUNDED_RECT { rect: r(60.0, 98.0, 150.0, 102.0), radiusX: 2.0 * s, radiusY: 2.0 * s }, &accent);
                let play = if playing { "\u{E769}" } else { "\u{E768}" };
                for (glyph, x) in [("\u{E892}", 150.0), (play, 190.0), ("\u{E893}", 230.0)] {
                    text(dc, glyph, &gfx.icons, r(x, 112.0, x + 24.0, 136.0), &fg);
                }
                for (i, name) in ["Spotify", "YouTube", "Twitch"].iter().enumerate() {
                    let x = 16.0 + i as f32 * 124.0;
                    let brush = if *name == track.source { &accent } else { &track_bg };
                    dc.FillRoundedRectangle(&D2D1_ROUNDED_RECT { rect: r(x, 148.0, x + 116.0, 172.0), radiusX: 12.0 * s, radiusY: 12.0 * s }, brush);
                    text(dc, name, &gfx.text, r(x + 12.0, 148.0, x + 116.0, 172.0), &fg);
                }
            }
            Ok(())
        })
    }
}
```

Se `StartAnimation("Size.X")` na geometria devolver erro (subcanal não suportado), trocar por um `ExpressionAnimation` ligado a uma `CompositionPropertySet` com um escalar animado por mola, e anotar no ADR-0006.

- [ ] **Step 4b: Se o ADR-0001 escolheu o caminho A (DirectComposition)**

Trocar a geometria e as molas por: `IDCompositionRectangleClip` no visual da pílula (`SetLeft/SetRight/SetBottom` e os 8 raios recebendo `IDCompositionAnimation` feitas com `DcompBackend::curve`, copiada para cá), opacidade com `IDCompositionEffectGroup::SetOpacity` e a translação com `IDCompositionTranslateTransform::SetOffsetX`. O acrílico vem do DWM na janela inteira: nesse caso, **anotar no ADR-0006** que o desfoque não acompanha a forma e usar o fundo sólido semitransparente (plano B da spec).

- [ ] **Step 5: Substituir o `main` do S6**

Manter o bloco `--capture-only` (Task 12) no início do `main`. O restante do arquivo fica:

```rust
mod analyzer;
mod anim;
mod appbar;
mod args;
mod backdrop;
mod capture;
mod click;
mod color;
mod events;
mod gfx;
mod island;
mod island_args;
mod island_view;
mod media;
mod monitors;
mod quality;
mod spring;
mod timing;
mod window;

use std::sync::{Arc, Mutex};
use std::time::Instant;
use events::Event;
use island::{Island, IslandInput, IslandState, target_geometry};
use island_args::ClickMode;
use spring::SpringParams;
use windows::Win32::Foundation::{HWND, RECT};
use windows::Win32::UI::HiDpi::{DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2, SetProcessDpiAwarenessContext};
use windows::Win32::UI::WindowsAndMessaging::*;

const TICK_TIMER: usize = 10;
const TRACK_TIMER: usize = 11;
const QUALITY_TIMER: usize = 12;
const CLICK_TIMER: usize = 13;
const SLIDE_TIMER: usize = 14;
const SHRINK_TIMER: usize = 15;
const COMPACT_W: f32 = 220.0;
const BAR_H: f32 = 32.0;

struct Ctx {
    hwnd: HWND,
    view: island_view::IslandView,
    gfx: gfx::Gfx,
    args: island_args::IslandArgs,
    island: Island,
    track: usize,
    playing: bool,
    window_rect: RECT,
    scale: f32,
    bands: Arc<Mutex<[f32; analyzer::BANDS]>>,
    capture: Option<capture::Capture>,
    last_sound: Arc<Mutex<Instant>>,
    started: Instant,
    frame_watch: quality::FrameWatch,
    tick_ms: u32,
}

impl Ctx {
    fn now(&self) -> f64 { self.started.elapsed().as_secs_f64() }

    /// Retângulo da pílula em coordenadas da janela, para um estado.
    fn pill_rect(&self, state: IslandState) -> RECT {
        let g = target_geometry(state, COMPACT_W, BAR_H);
        let left = self.view.pill_left(&g) as i32;
        RECT { left, top: (2.0 * self.scale) as i32, right: left + (g.width * self.scale) as i32, bottom: ((2.0 + g.height) * self.scale) as i32 }
    }

    fn apply_click_shape(&self, state: IslandState) {
        let r = self.pill_rect(state);
        match self.args.click {
            ClickMode::Region => click::apply_region(self.hwnd, r, (target_geometry(state, COMPACT_W, BAR_H).radius * self.scale) as i32),
            ClickMode::Resize => {
                // Encolhe a janela até o retângulo da pílula (em coordenadas de tela).
                let w = &self.window_rect;
                // SAFETY: janela nossa.
                unsafe { let _ = SetWindowPos(self.hwnd, Some(HWND_TOPMOST), w.left + r.left, w.top + r.top, r.right - r.left, r.bottom - r.top, SWP_NOACTIVATE); }
            }
            ClickMode::Transparent => {}
        }
    }

    fn restore_full_window(&self) {
        if self.args.click == ClickMode::Resize {
            let w = &self.window_rect;
            // SAFETY: janela nossa.
            unsafe { let _ = SetWindowPos(self.hwnd, Some(HWND_TOPMOST), w.left, w.top, w.right - w.left, w.bottom - w.top, SWP_NOACTIVATE); }
        }
    }

    fn label(&self) -> String {
        let t = &media::TRACKS[self.track];
        match self.island.state {
            IslandState::Peek => format!("{} — {}", t.title, t.artist),
            _ => "qua 14  22:53".to_string(),
        }
    }

    fn redraw(&self) -> windows::core::Result<()> {
        let t = &media::TRACKS[self.track];
        let cover = color::dominant_color(&media::fake_cover(t.hue), media::COVER_SIZE, media::COVER_SIZE);
        self.view.set_tint(cover)?;
        self.view.draw_strip(&self.gfx, &self.label(), cover)?;
        self.view.draw_expanded(&self.gfx, t, self.playing)
    }

    fn transition(&mut self, input: IslandInput) -> windows::core::Result<()> {
        let before = self.island.state;
        if !self.island.handle(input, self.now()) {
            return Ok(());
        }
        let after = self.island.state;
        let growing = matches!((before, after), (IslandState::Compact, _) | (IslandState::Peek, IslandState::Expanded));
        let params = if growing { SpringParams::GROW } else { SpringParams::SHRINK };
        if growing {
            self.restore_full_window();
            self.apply_click_shape(after);
        } else {
            // SAFETY: timer nosso; aplica a forma menor depois da mola de encolher.
            unsafe { SetTimer(Some(self.hwnd), SHRINK_TIMER, 450, None) };
        }
        self.frame_watch.set_active(true);
        self.redraw()?;
        self.view.go_to(after, target_geometry(after, COMPACT_W, BAR_H), params)?;
        eprintln!("[island] {before:?} -> {after:?}");
        Ok(())
    }

    fn start_capture(&mut self) {
        if self.capture.is_some() || !self.args.visualizer {
            return;
        }
        let bands = self.bands.clone();
        let last_sound = self.last_sound.clone();
        let mut analyzer: Option<analyzer::Analyzer> = None;
        self.capture = Some(capture::Capture::start(Box::new(move |mono, rate| {
            let a = analyzer.get_or_insert_with(|| analyzer::Analyzer::new(rate));
            if a.push(mono) {
                *bands.lock().unwrap() = a.bands();
                if a.peak() > 0.001 {
                    *last_sound.lock().unwrap() = Instant::now();
                }
            }
        })));
        eprintln!("[audio] captura ligada");
    }

    fn stop_capture(&mut self) {
        if let Some(c) = self.capture.take() {
            c.stop();
            *self.bands.lock().unwrap() = [0.0; analyzer::BANDS];
            eprintln!("[audio] captura desligada");
        }
    }

    fn handle(&mut self, e: Event) -> windows::core::Result<bool> {
        match e {
            Event::Click { y, x, .. } => {
                let y_dip = y as f32 / self.scale;
                let x_dip = x as f32 - self.pill_rect(IslandState::Expanded).left as f32;
                if self.island.state == IslandState::Expanded && y_dip > 148.0 {
                    // Linha de fontes: troca com animação de deslizar.
                    let idx = ((x_dip / self.scale - 16.0) / 124.0).clamp(0.0, 2.0) as usize;
                    if idx != self.track {
                        self.track = idx;
                        self.view.slide(true)?;
                        // SAFETY: timer nosso.
                        unsafe { SetTimer(Some(self.hwnd), SLIDE_TIMER, 170, None) };
                    }
                } else if self.island.state == IslandState::Expanded && (112.0..136.0).contains(&y_dip) {
                    self.playing = !self.playing;
                    self.redraw()?;
                } else {
                    self.transition(IslandInput::Click)?;
                }
            }
            Event::MouseMove { .. } => self.transition(IslandInput::HoverEnter)?,
            Event::MouseLeave { .. } => self.transition(IslandInput::HoverLeave)?,
            Event::Timer { id: TICK_TIMER } => {
                self.transition(IslandInput::Tick)?;
                let b = *self.bands.lock().unwrap();
                self.view.set_bars(&b)?;
            }
            Event::Timer { id: TRACK_TIMER } => {
                self.track = (self.track + 1) % media::TRACKS.len();
                self.redraw()?;
                self.transition(IslandInput::TrackChanged)?;
            }
            Event::Timer { id: SLIDE_TIMER } => {
                // SAFETY: timer nosso.
                unsafe { let _ = KillTimer(Some(self.hwnd), SLIDE_TIMER); }
                self.redraw()?;
                self.view.slide(false)?;
            }
            Event::Timer { id: SHRINK_TIMER } => {
                // SAFETY: timer nosso.
                unsafe { let _ = KillTimer(Some(self.hwnd), SHRINK_TIMER); }
                self.apply_click_shape(self.island.state);
                self.frame_watch.set_active(self.args.visualizer && self.capture.is_some());
            }
            Event::Timer { id: CLICK_TIMER } => {
                let inside = click::cursor_inside(self.hwnd, self.pill_rect(self.island.state));
                click::set_transparent(self.hwnd, !inside);
            }
            Event::Timer { id: QUALITY_TIMER } => self.apply_quality()?,
            Event::Close => return Ok(false),
            _ => {}
        }
        Ok(true)
    }
}

fn main() -> windows::core::Result<()> {
    // (bloco --capture-only da Task 12 fica aqui)
    timing::mark_start();
    let raw: Vec<String> = std::env::args().skip(1).collect();
    let refs: Vec<&str> = raw.iter().map(String::as_str).collect();
    let args = island_args::parse(&refs).unwrap_or_else(|e| {
        eprintln!("{e}\nuso: s6-island [--click resize|region|transparent] [--style smooth|default|bouncy|snappy] [--speed 0.5-2] [--visualizer on|off]");
        std::process::exit(2)
    });
    eprintln!("[args] {args:?}");
    // SAFETY: chamada única no início do processo.
    unsafe { SetProcessDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2)? };
    window::register_class()?;

    let m = monitors::list().into_iter().next().expect("monitor primário");
    let scale = m.dpi as f32 / 96.0;
    let (w, h) = ((420.0 * scale) as i32, (200.0 * scale) as i32);
    let cx = (m.rect.left + m.rect.right) / 2;
    let window_rect = RECT { left: cx - w / 2, top: m.rect.top, right: cx + w / 2, bottom: m.rect.top + h };
    let hwnd = window::create_island(window_rect, args.click == ClickMode::Transparent)?;
    backdrop::apply(hwnd, args::AcrylicKind::Host)?;
    let gfx = gfx::Gfx::new()?;
    let mut view = island_view::IslandView::new(&gfx, hwnd, w as f32, h as f32, scale)?;
    view.style = args.style;
    view.speed = args.speed;

    let mut ctx = Ctx {
        hwnd, view, gfx, args, island: Island::new(), track: 0, playing: true, window_rect, scale,
        bands: Arc::new(Mutex::new([0.0; analyzer::BANDS])), capture: None,
        last_sound: Arc::new(Mutex::new(Instant::now())), started: Instant::now(),
        frame_watch: quality::FrameWatch::start(m.refresh_hz), tick_ms: 16,
    };
    ctx.redraw()?;
    ctx.view.go_to(IslandState::Compact, target_geometry(IslandState::Compact, COMPACT_W, BAR_H), SpringParams::DEFAULT)?;
    ctx.apply_click_shape(IslandState::Compact);
    ctx.start_capture();
    // SAFETY: janela e timers nossos.
    unsafe {
        let _ = ShowWindow(hwnd, SW_SHOWNOACTIVATE);
        SetTimer(Some(hwnd), TICK_TIMER, ctx.tick_ms, None);
        SetTimer(Some(hwnd), TRACK_TIMER, 10_000, None);
        SetTimer(Some(hwnd), QUALITY_TIMER, 1_000, None);
        if ctx.args.click == ClickMode::Transparent {
            SetTimer(Some(hwnd), CLICK_TIMER, 16, None);
        }
    }
    eprintln!("[timing] primeiro quadro: {:.2} ms", timing::since_start_ms());

    while window::pump_blocking() {
        for e in events::drain() {
            if !ctx.handle(e)? {
                ctx.stop_capture();
                // SAFETY: encerra o loop de mensagens.
                unsafe { PostQuitMessage(0) };
            }
        }
    }
    Ok(())
}
```

`Ctx::apply_quality` e `quality::FrameWatch` chegam na Task 15. Para compilar agora, criar `quality.rs` provisório:

```rust
pub struct FrameWatch;
impl FrameWatch {
    pub fn start(_refresh_hz: u32) -> Self { FrameWatch }
    pub fn set_active(&self, _on: bool) {}
}
```

e em `impl Ctx` um `fn apply_quality(&mut self) -> windows::core::Result<()> { Ok(()) }` provisório.

- [ ] **Step 6: Rodar e conferir os 3 estados**

Run: `cargo run --release --manifest-path spikes/s6-island/Cargo.toml`
Expected:
1. Pílula compacta no topo central do monitor principal: barras do visualizador (mexendo com música), quadradinho da capa e "qua 14  22:53".
2. A cada 10 s a faixa troca: a pílula **cresce na largura** mostrando "Título — Artista" e volta em ~3 s.
3. Passar o mouse: espia e fica aberta enquanto o mouse estiver em cima.
4. Clicar: a pílula "derrete" para baixo até 400×184, com capa, linha do tempo, controles e as 3 fontes; o conteúdo troca com *crossfade*.
5. No expandido: clicar numa fonte faz o conteúdo sair deslizando e entrar com a nova; clicar em ⏯ alterna o ícone; clicar em outro lugar volta ao compacto.
6. Testar `--style bouncy` (passa do alvo e volta) e `--style smooth` (sem passar), e `--speed 0.5`/`2`.

Gravar um vídeo curto dos 3 estados (`Win+Alt+R` na Game Bar) para `docs/adr/media/0006-ilha.mp4` no worktree principal.

- [ ] **Step 7: Testar os 3 métodos de clique (20 de 20)**

Posicionar uma janela do Explorador com ícones logo abaixo da ilha (em cima e dos lados da pílula compacta, dentro da área da janela da ilha). Para cada método:

Run: `cargo run --release --manifest-path spikes/s6-island/Cargo.toml -- --click region`
Run: `cargo run --release --manifest-path spikes/s6-island/Cargo.toml -- --click resize`
Run: `cargo run --release --manifest-path spikes/s6-island/Cargo.toml -- --click transparent`

Em cada estado (compacto, espiando, expandido), clicar 20 vezes em ícones fora da pílula. Anotar quantos cliques chegaram ao Explorador (meta 20 de 20), se a pílula continua clicável e se aparece algum quadro piscando ao redimensionar (método `resize`).

- [ ] **Step 8: Commit**

```powershell
git add spikes/s6-island
git commit -m "spike(s6): island window with compositor springs, visualizer bars, crossfade and click-through modes" -m "Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

---

## Task 15: S6 — Regras de degradação automática

**Files:**
- Modify: `spikes/s6-island/src/quality.rs` (substituir o provisório), `spikes/s6-island/src/main.rs` (`Ctx::apply_quality`)

**Interfaces:**
- Consumes: Tasks 12–14; `anim::wait_for_next_frame` (Task 9).
- Produces: `quality::{Conditions, Quality, decide(&Conditions) -> Quality, detect(...) -> Conditions, FrameWatch { start(refresh_hz), set_active(bool), slow_streak_secs() -> f32 }}`.

- [ ] **Step 1: Escrever os testes de `decide` (falhando)**

Substituir `quality.rs` por:

```rust
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::time::{Duration, Instant};

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Conditions {
    pub animations_enabled: bool,
    pub transparency_enabled: bool,
    pub fullscreen_app: bool,
    pub session_locked: bool,
    pub media_playing: bool,
    pub silence_secs: f32,
    pub energy_saver: bool,
    pub slow_frames_streak_secs: f32,
}

impl Conditions {
    pub const NORMAL: Conditions = Conditions {
        animations_enabled: true, transparency_enabled: true, fullscreen_app: false, session_locked: false,
        media_playing: true, silence_secs: 0.0, energy_saver: false, slow_frames_streak_secs: 0.0,
    };
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Quality {
    pub springs: bool,
    pub backdrop_blur: bool,
    pub island_visible: bool,
    pub capture_audio: bool,
    pub visualizer_fps: u32,
}

pub fn decide(_c: &Conditions) -> Quality {
    unimplemented!()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn with(f: impl FnOnce(&mut Conditions)) -> Quality {
        let mut c = Conditions::NORMAL;
        f(&mut c);
        decide(&c)
    }

    #[test]
    fn normal_conditions_enable_everything_at_60fps() {
        assert_eq!(decide(&Conditions::NORMAL), Quality { springs: true, backdrop_blur: true, island_visible: true, capture_audio: true, visualizer_fps: 60 });
    }

    #[test]
    fn animations_off_disables_springs_only() {
        let q = with(|c| c.animations_enabled = false);
        assert!(!q.springs && q.backdrop_blur && q.capture_audio);
    }

    #[test]
    fn transparency_off_disables_blur() {
        assert!(!with(|c| c.transparency_enabled = false).backdrop_blur);
    }

    #[test]
    fn fullscreen_or_lock_hides_and_stops_capture() {
        for q in [with(|c| c.fullscreen_app = true), with(|c| c.session_locked = true)] {
            assert!(!q.capture_audio);
            assert_eq!(q.visualizer_fps, 0);
        }
        assert!(!with(|c| c.fullscreen_app = true).island_visible);
        assert!(with(|c| c.session_locked = true).island_visible);
    }

    #[test]
    fn paused_media_or_two_seconds_of_silence_stops_capture() {
        assert!(!with(|c| c.media_playing = false).capture_audio);
        assert!(with(|c| c.silence_secs = 1.9).capture_audio);
        assert!(!with(|c| c.silence_secs = 2.0).capture_audio);
    }

    #[test]
    fn energy_saver_halves_visualizer() {
        assert_eq!(with(|c| c.energy_saver = true).visualizer_fps, 30);
    }

    #[test]
    fn three_seconds_of_slow_frames_drop_the_blur() {
        assert!(with(|c| c.slow_frames_streak_secs = 2.9).backdrop_blur);
        assert!(!with(|c| c.slow_frames_streak_secs = 3.0).backdrop_blur);
    }
}
```

- [ ] **Step 2: Rodar para ver falhar**

Run: `cargo test --manifest-path spikes/s6-island/Cargo.toml quality`
Expected: FAIL (`not implemented`; o `main.rs` pode não compilar por causa do `FrameWatch` removido — completar o Passo 3 antes de rodar de novo).

- [ ] **Step 3: Implementar `decide`, `FrameWatch` e `detect`**

Substituir o `decide` e acrescentar, acima do `#[cfg(test)]`:

```rust
pub fn decide(c: &Conditions) -> Quality {
    let hidden_context = c.fullscreen_app || c.session_locked;
    let capture = !hidden_context && c.media_playing && c.silence_secs < 2.0;
    Quality {
        springs: c.animations_enabled,
        backdrop_blur: c.transparency_enabled && c.slow_frames_streak_secs < 3.0,
        island_visible: !c.fullscreen_app,
        capture_audio: capture,
        visualizer_fps: if !capture { 0 } else if c.energy_saver { 30 } else { 60 },
    }
}

/// Mede atrasos de quadro numa thread própria usando o relógio do compositor.
/// Um quadro é "atrasado" quando o intervalo passa de 1,5× o período do monitor.
pub struct FrameWatch {
    active: Arc<AtomicBool>,
    slow_ms: Arc<AtomicU32>,
}

impl FrameWatch {
    pub fn start(refresh_hz: u32) -> Self {
        let active = Arc::new(AtomicBool::new(false));
        let slow_ms = Arc::new(AtomicU32::new(0));
        let (a, s) = (active.clone(), slow_ms.clone());
        let period = Duration::from_secs_f64(1.0 / refresh_hz.max(30) as f64);
        std::thread::Builder::new().name("zefiro-framewatch".into()).spawn(move || {
            let mut last = Instant::now();
            let mut window_start = Instant::now();
            let mut late_in_window = 0u32;
            loop {
                if !a.load(Ordering::Relaxed) {
                    std::thread::sleep(Duration::from_millis(100));
                    last = Instant::now();
                    s.store(0, Ordering::Relaxed);
                    continue;
                }
                crate::anim::wait_for_next_frame();
                let now = Instant::now();
                if now - last > period.mul_f32(1.5) {
                    late_in_window += 1;
                }
                last = now;
                if now - window_start >= Duration::from_secs(1) {
                    // Segundo "lento" = mais de 2 quadros atrasados.
                    if late_in_window > 2 { s.fetch_add(1000, Ordering::Relaxed); } else { s.store(0, Ordering::Relaxed); }
                    late_in_window = 0;
                    window_start = now;
                }
            }
        }).expect("thread do FrameWatch");
        Self { active, slow_ms }
    }

    pub fn set_active(&self, on: bool) {
        self.active.store(on, Ordering::Relaxed);
    }

    pub fn slow_streak_secs(&self) -> f32 {
        self.slow_ms.load(Ordering::Relaxed) as f32 / 1000.0
    }
}

pub mod detect {
    use windows::Win32::Foundation::BOOL;
    use windows::Win32::System::Power::{GetSystemPowerStatus, SYSTEM_POWER_STATUS};
    use windows::Win32::System::Registry::{HKEY_CURRENT_USER, RRF_RT_REG_DWORD, RegGetValueW};
    use windows::Win32::UI::Shell::{QUNS_BUSY, QUNS_PRESENTATION_MODE, QUNS_RUNNING_D3D_FULL_SCREEN, SHQueryUserNotificationState};
    use windows::Win32::UI::WindowsAndMessaging::{SPI_GETCLIENTAREAANIMATION, SYSTEM_PARAMETERS_INFO_UPDATE_FLAGS, SystemParametersInfoW};
    use windows::core::w;

    pub fn animations_enabled() -> bool {
        let mut on = BOOL(1);
        // SAFETY: ponteiro para BOOL.
        unsafe { let _ = SystemParametersInfoW(SPI_GETCLIENTAREAANIMATION, 0, Some(&mut on as *mut _ as _), SYSTEM_PARAMETERS_INFO_UPDATE_FLAGS(0)); }
        on.as_bool()
    }

    pub fn transparency_enabled() -> bool {
        let mut v = 1u32;
        let mut size = 4u32;
        // SAFETY: buffer de 4 bytes para um DWORD.
        let r = unsafe {
            RegGetValueW(HKEY_CURRENT_USER, w!("Software\\Microsoft\\Windows\\CurrentVersion\\Themes\\Personalize"),
                w!("EnableTransparency"), RRF_RT_REG_DWORD, None, Some(&mut v as *mut _ as _), Some(&mut size))
        };
        r.is_err() || v != 0
    }

    pub fn fullscreen_app() -> bool {
        // SAFETY: chamada sem parâmetros de saída além do retorno.
        match unsafe { SHQueryUserNotificationState() } {
            Ok(s) => s == QUNS_RUNNING_D3D_FULL_SCREEN || s == QUNS_PRESENTATION_MODE || s == QUNS_BUSY,
            Err(_) => false,
        }
    }

    pub fn energy_saver() -> bool {
        let mut s = SYSTEM_POWER_STATUS::default();
        // SAFETY: estrutura válida.
        unsafe { GetSystemPowerStatus(&mut s).is_ok() && s.SystemStatusFlag == 1 }
    }
}
```

Para a tela bloqueada, registrar na janela (no `main`, depois de criar o `hwnd`):

```rust
    // SAFETY: registra a janela para receber WM_WTSSESSION_CHANGE.
    unsafe { let _ = windows::Win32::System::RemoteDesktop::WTSRegisterSessionNotification(hwnd, windows::Win32::System::RemoteDesktop::NOTIFY_FOR_THIS_SESSION); }
```

Em `events.rs`, acrescentar `SessionLock(bool)` ao `enum Event`; em `window.rs`, no `wndproc`:

```rust
        WM_WTSSESSION_CHANGE => match wparam.0 as u32 {
            WTS_SESSION_LOCK => push(Event::SessionLock(true)),
            WTS_SESSION_UNLOCK => push(Event::SessionLock(false)),
            _ => {}
        },
```

(com `use windows::Win32::System::RemoteDesktop::{WTS_SESSION_LOCK, WTS_SESSION_UNLOCK};` e `WM_WTSSESSION_CHANGE` vindo de `WindowsAndMessaging`).

- [ ] **Step 4: Rodar os testes**

Run: `cargo test --manifest-path spikes/s6-island/Cargo.toml quality`
Expected: PASS (7 testes).

- [ ] **Step 5: Aplicar a qualidade no `main.rs`**

Adicionar o campo `locked: bool` ao `Ctx` (iniciar com `false`), tratar o evento em `handle`:

```rust
            Event::SessionLock(on) => { self.locked = on; self.apply_quality()?; }
```

e substituir o `apply_quality` provisório por:

```rust
    fn apply_quality(&mut self) -> windows::core::Result<()> {
        let c = quality::Conditions {
            animations_enabled: quality::detect::animations_enabled(),
            transparency_enabled: quality::detect::transparency_enabled(),
            fullscreen_app: quality::detect::fullscreen_app(),
            session_locked: self.locked,
            media_playing: self.playing,
            silence_secs: self.last_sound.lock().unwrap().elapsed().as_secs_f32(),
            energy_saver: quality::detect::energy_saver(),
            slow_frames_streak_secs: self.frame_watch.slow_streak_secs(),
        };
        let q = quality::decide(&c);
        self.view.springs_enabled = q.springs;
        self.view.set_backdrop_enabled(q.backdrop_blur)?;
        // SAFETY: janela e timer nossos.
        unsafe { let _ = ShowWindow(self.hwnd, if q.island_visible { SW_SHOWNOACTIVATE } else { SW_HIDE }); }
        // Silêncio só desliga a captura; ela volta quando a mídia muda (faixa ou play).
        if q.capture_audio { self.start_capture(); } else if self.capture.is_some() { self.stop_capture(); }
        let tick = if q.visualizer_fps >= 60 { 16 } else { 33 };
        if tick != self.tick_ms {
            self.tick_ms = tick;
            // SAFETY: timer nosso.
            unsafe { SetTimer(Some(self.hwnd), TICK_TIMER, tick, None) };
        }
        self.frame_watch.set_active(q.capture_audio);
        eprintln!("[quality] {c:?} => {q:?}");
        Ok(())
    }
```

Como a captura para no silêncio, reiniciar a contagem quando a mídia muda: no `TRACK_TIMER` e no clique em ⏯ (quando `self.playing` vira `true`), fazer `*self.last_sound.lock().unwrap() = Instant::now();` antes de `self.apply_quality()?`.

- [ ] **Step 6: Verificar cada regra uma vez**

Com o spike rodando e música tocando, conferir no log `[quality]` e na tela:

| Ação | Esperado |
|---|---|
| Configurações → Acessibilidade → Efeitos visuais → **Efeitos de animação** desligado | `springs: false`; transições instantâneas |
| Configurações → Personalização → Cores → **Efeitos de transparência** desligado | `backdrop_blur: false`; fundo sólido |
| Abrir um jogo ou vídeo em tela cheia (F11 no YouTube) | ilha some; `[audio] captura desligada` |
| `Win+L` e voltar | `session_locked: true` durante o bloqueio; captura desligada |
| Pausar (⏯ no expandido) | `[audio] captura desligada` |
| Ficar 2 s sem som com a música "tocando" (volume do player em 0) | captura desligada |
| Economia de energia ligada (Configurações → Sistema → Energia) | `visualizer_fps: 30` |

Religar tudo o que foi desligado.

- [ ] **Step 7: Commit**

```powershell
git add spikes/s6-island
git commit -m "spike(s6): automatic quality degradation rules and frame watch" -m "Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

---

## Task 16: S6 — Matriz de hardware, ADR-0006 e créditos

**Files:**
- Create (no `main`): `scripts/gpu.ps1`, `scripts/tests/gpu.Tests.ps1`, `docs/adr/0006-ilha-de-midia-e-limites-de-hardware.md`
- Modify (no `main`): `THIRD_PARTY_NOTICES.md`

**Interfaces:**
- Consumes: Tasks 2, 3, 11–15.
- Produces: `scripts/gpu.ps1 -Process <nome> -Seconds <n>` → JSON `{ process, gpu_percent_avg }`; ADR-0006 com o método de clique, conversão das molas e regras de degradação validadas.

- [ ] **Step 1: Escrever o teste do `gpu.ps1` (falhando)**

`scripts/tests/gpu.Tests.ps1` (no worktree principal):

```powershell
$ErrorActionPreference = 'Stop'
$root = Split-Path -Parent (Split-Path -Parent $PSCommandPath)
$r = & (Join-Path $root 'gpu.ps1') -Process 'dwm' -Seconds 3 | ConvertFrom-Json
if ($r.process -ne 'dwm') { throw 'processo errado' }
if ($r.gpu_percent_avg -lt 0) { throw 'GPU não pode ser negativa' }
Write-Host 'gpu.Tests: OK'
```

Run: `pwsh -NoProfile -File scripts/tests/gpu.Tests.ps1`
Expected: FAIL (`gpu.ps1` não existe).

- [ ] **Step 2: Implementar `scripts/gpu.ps1`**

```powershell
<#
.SYNOPSIS
  Média de uso da GPU (motor 3D) de um processo, via contadores "GPU Engine".
#>
param(
    [Parameter(Mandatory)] [string] $Process,
    [int] $Seconds = 60
)
$ErrorActionPreference = 'Stop'
$pids = (Get-Process -Name $Process -ErrorAction Stop).Id
$samples = @()
for ($i = 0; $i -lt $Seconds; $i++) {
    $c = Get-Counter '\GPU Engine(*engtype_3D)\Utilization Percentage' -ErrorAction SilentlyContinue
    $sum = 0.0
    foreach ($s in $c.CounterSamples) {
        foreach ($p in $pids) { if ($s.InstanceName -like "pid_${p}_*") { $sum += $s.CookedValue } }
    }
    $samples += $sum
}
[ordered]@{ process = $Process; gpu_percent_avg = [math]::Round((($samples | Measure-Object -Average).Average), 3) } | ConvertTo-Json
```

Run: `pwsh -NoProfile -File scripts/tests/gpu.Tests.ps1`
Expected: `gpu.Tests: OK`

- [ ] **Step 3: Rodar a matriz de hardware**

Primeiro, acrescentar ao S6 (no worktree `../zefiro-s6`) um modo de ciclos automáticos controlado pela variável de ambiente `ZEFIRO_CYCLES` (`expand` ou `peek`). Em `main.rs`:

```rust
const CYCLE_TIMER: usize = 16;
```

Campos novos no `Ctx` (iniciar com `cycles_mode: std::env::var("ZEFIRO_CYCLES").ok()` e `cycles_left: 100`):

```rust
    cycles_mode: Option<String>,
    cycles_left: u32,
```

No `main`, junto dos outros `SetTimer`:

```rust
        if ctx.cycles_mode.is_some() {
            SetTimer(Some(hwnd), CYCLE_TIMER, 700, None);
        }
```

No `Ctx::handle`, antes do `_ => {}`:

```rust
            Event::Timer { id: CYCLE_TIMER } => {
                if self.cycles_left == 0 {
                    // SAFETY: timer nosso.
                    unsafe { let _ = KillTimer(Some(self.hwnd), CYCLE_TIMER); }
                    eprintln!("[cycles] terminado");
                } else {
                    self.cycles_left -= 1;
                    let input = match self.cycles_mode.as_deref() {
                        Some("peek") if self.cycles_left % 2 == 1 => IslandInput::HoverEnter,
                        Some("peek") => IslandInput::HoverLeave,
                        _ => IslandInput::Click,
                    };
                    self.transition(input)?;
                }
            }
```

100 disparos = 50 ciclos. Compilar em release (`cargo build --release --manifest-path ../zefiro-s6/spikes/s6-island/Cargo.toml`), usando o melhor método de clique da Task 14, e fazer commit na branch do spike (`spike(s6): automatic cycles mode for measurements`).

Para cada linha (taxa do monitor alterada em Configurações → Tela → Avançado; **pedir ao usuário para mudar a taxa**):

| Cenário | Env/args | 60 Hz | 144 Hz |
|---|---|---|---|
| compacto↔expandido, sem visualizador | `ZEFIRO_CYCLES=expand` + `--visualizer off` | ✔ | ✔ |
| compacto↔espiando, sem visualizador | `ZEFIRO_CYCLES=peek` + `--visualizer off` | ✔ | ✔ |
| compacto↔expandido, com visualizador (música tocando) | `ZEFIRO_CYCLES=expand` | ✔ | ✔ |
| 30 min de música, ilha compacta | sem `ZEFIRO_CYCLES` | — | ✔ |
| GPU sob carga (jogo ou benchmark em janela) | `ZEFIRO_CYCLES=expand` | — | ✔ |

Medir em paralelo, em cada cenário:

```powershell
PresentMon --process_name dwm.exe --output_file scripts/out/s6-<cenario>-<hz>.csv --timed 80 --terminate_after_timed
pwsh -NoProfile -File scripts/perf.ps1 -Process s6-island -WarmupSeconds 2 -SampleSeconds 70 -IntervalSeconds 2 -Out scripts/out/s6-<cenario>-<hz>.json
pwsh -NoProfile -File scripts/gpu.ps1 -Process s6-island -Seconds 70 > scripts/out/s6-<cenario>-<hz>-gpu.json
pwsh -NoProfile -File scripts/frames.ps1 -Csv scripts/out/s6-<cenario>-<hz>.csv -RefreshHz <hz>
```

Para os 30 min: `perf.ps1 -WarmupSeconds 60 -SampleSeconds 1800` e comparar o working set do início e do fim (meta: crescimento ≤ 1 MB).

- [ ] **Step 4: Escrever `docs/adr/0006-ilha-de-midia-e-limites-de-hardware.md`**

Seguir o modelo, com:
- a matriz da Task 16 preenchida (quadros perdidos, CPU, GPU, RAM) contra as metas da spec (≤ 1 quadro perdido em 50 ciclos sem carga; CPU ≤ 1 % em transição e ≤ 1,5 % com visualizador; GPU ≤ 2 %; RAM +≤ 1 MB em 30 min);
- a tabela de cliques da Task 14 (20/20 por método e estado) e o **método escolhido**;
- a conversão das molas (rigidez/amortecimento → período/razão) e se ficou visualmente igual ao mod;
- as regras de degradação da Task 15, cada uma marcada como validada ou não;
- custo da captura de áudio (Task 12, Passo 5);
- limitações encontradas (ex.: subcanal `Size.X` da geometria, `WS_EX_LAYERED` com compositor).

- [ ] **Step 5: Registrar o Dynamic Island em `THIRD_PARTY_NOTICES.md`**

Pegar a linha de copyright exata da licença do projeto:

```powershell
gh api repos/devcode90/Dynamic-Island-for-Windows/contents/LICENSE --jq .content | % { [Text.Encoding]::UTF8.GetString([Convert]::FromBase64String($_)) }
```

Substituir a linha `_(Nenhum ainda. …)_` da seção "Código portado" por:

```markdown
### Dynamic Island for Windows

- Repositório: https://github.com/devcode90/Dynamic-Island-for-Windows
- Licença: MIT
- O que foi portado (reimplementado em Rust): modelo de mola (passos fixos de 0,5 ms, constantes de crescer/encolher e multiplicadores de estilo), analisador de espectro (Hann 1024, FFT radix-2, 24 bandas, ganho automático, subida/queda), tempos de transição (crossfade 0,32 s, troca de fonte 0,17/0,24 s, 14 px).

<cole aqui o texto integral da licença MIT do projeto, com a linha de copyright exata obtida acima>
```

- [ ] **Step 6: Commit no `main`**

```powershell
git add scripts/gpu.ps1 scripts/tests/gpu.Tests.ps1 docs/adr/0006-ilha-de-midia-e-limites-de-hardware.md docs/adr/media THIRD_PARTY_NOTICES.md
git commit -m "docs(adr): 0006 media island and hardware limits; credit Dynamic Island (MIT)" -m "Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

---

## Task 17: S2 — Notificações (contador do sino) e ADR-0002

**Files:**
- Create (branch `spike/s2-notifications`): `spikes/s2-notifications/Cargo.toml`, `src/main.rs`, `src/wnf.rs`, `send-toast.ps1`
- Create (no `main`): `docs/adr/0002-notificacoes-e-empacotamento.md`

**Interfaces:**
- Consumes: Task 1.
- Produces: resposta "o `UserNotificationListener` funciona sem pacote?" + decisão de empacotamento para a Fase 2; `wnf::subscribe(state_name: u64, on_change: fn())` reaproveitável no S5.

- [ ] **Step 1: Criar branch, worktree e projeto**

```powershell
cd ~/Projects/zefiro
git worktree add ../zefiro-s2 -b spike/s2-notifications main
cd ../zefiro-s2
cargo new spikes/s2-notifications --edition 2024 --vcs none
```

`spikes/s2-notifications/Cargo.toml`:

```toml
[package]
name = "s2-notifications"
version = "0.0.0"
edition = "2024"
publish = false

[workspace]

[dependencies]
windows-core = "=0.61.2"
windows = { version = "=0.61.3", features = [
  "Foundation", "Foundation_Collections", "UI_Notifications", "UI_Notifications_Management",
  "Win32_Foundation", "Win32_System_LibraryLoader",
] }
```

- [ ] **Step 2: Escrever `src/wnf.rs`**

```rust
//! Assinatura de estados WNF (API não documentada da ntdll), carregada dinamicamente.
use std::ffi::c_void;
use std::sync::OnceLock;
use windows::Win32::System::LibraryLoader::{GetModuleHandleW, GetProcAddress};
use windows::core::{s, w};

/// Valor público conhecido de WNF_SHEL_NOTIFICATIONS. Conferir na lista de nomes
/// WNF do repositório `ionescu007/wnfun` antes de confiar; anotar no ADR.
pub const WNF_SHEL_NOTIFICATIONS: u64 = 0x0D83_063E_A3BC_1035;

type Callback = unsafe extern "system" fn(u64, u32, *const c_void, *const c_void, *const c_void, u32) -> i32;
type RtlSubscribe = unsafe extern "system" fn(*mut *mut c_void, u64, u32, Callback, *const c_void, *const c_void, u32, u32) -> i32;

static ON_CHANGE: OnceLock<fn()> = OnceLock::new();

unsafe extern "system" fn trampoline(_: u64, _: u32, _: *const c_void, _: *const c_void, _: *const c_void, _: u32) -> i32 {
    if let Some(f) = ON_CHANGE.get() {
        f();
    }
    0
}

/// Assina `state_name`; `on_change` roda numa thread do sistema a cada mudança.
pub fn subscribe(state_name: u64, on_change: fn()) -> Result<(), String> {
    let _ = ON_CHANGE.set(on_change);
    // SAFETY: função exportada pela ntdll; assinatura conhecida e estável desde o Win8.
    unsafe {
        let ntdll = GetModuleHandleW(w!("ntdll.dll")).map_err(|e| e.to_string())?;
        let proc = GetProcAddress(ntdll, s!("RtlSubscribeWnfStateChangeNotification")).ok_or("RtlSubscribeWnfStateChangeNotification ausente")?;
        let f: RtlSubscribe = std::mem::transmute(proc);
        let mut sub = std::ptr::null_mut();
        let status = f(&mut sub, state_name, 0, trampoline, std::ptr::null(), std::ptr::null(), 0, 0);
        if status < 0 { Err(format!("NTSTATUS 0x{status:08X}")) } else { Ok(()) }
    }
}
```

- [ ] **Step 3: Escrever `src/main.rs`**

```rust
mod wnf;

use std::sync::OnceLock;
use std::time::Instant;
use windows::UI::Notifications::Management::{UserNotificationListener, UserNotificationListenerAccessStatus};
use windows::UI::Notifications::NotificationKinds;

static START: OnceLock<Instant> = OnceLock::new();

fn ts() -> f64 {
    START.get().map(|s| s.elapsed().as_secs_f64()).unwrap_or(0.0)
}

fn count() -> windows::core::Result<u32> {
    let l = UserNotificationListener::Current()?;
    l.GetNotificationsAsync(NotificationKinds::Toast)?.get()?.Size()
}

fn on_wnf() {
    eprintln!("[{:7.3}] WNF: mudou -> count={:?}", ts(), count());
}

fn main() -> windows::core::Result<()> {
    let _ = START.set(Instant::now());
    let listener = match UserNotificationListener::Current() {
        Ok(l) => l,
        Err(e) => {
            eprintln!("[s2] UserNotificationListener::Current falhou: {e} (provável exigência de identidade de pacote)");
            return Ok(());
        }
    };
    eprintln!("[s2] status antes: {:?}", listener.GetAccessStatus()?);
    let status = listener.RequestAccessAsync()?.get()?;
    eprintln!("[s2] RequestAccessAsync -> {status:?}");
    if status != UserNotificationListenerAccessStatus::Allowed {
        eprintln!("[s2] sem acesso: liberar em Configurações > Privacidade > Notificações, ou o app precisa de identidade de pacote");
    }
    eprintln!("[s2] notificações agora: {:?}", count());
    match wnf::subscribe(wnf::WNF_SHEL_NOTIFICATIONS, on_wnf) {
        Ok(()) => eprintln!("[s2] WNF assinado"),
        Err(e) => eprintln!("[s2] WNF falhou: {e}"),
    }
    // Polling de referência, para comparar a latência com o WNF.
    let mut last = count().ok();
    loop {
        std::thread::sleep(std::time::Duration::from_millis(250));
        let now = count().ok();
        if now != last {
            eprintln!("[{:7.3}] polling: count {last:?} -> {now:?}", ts());
            last = now;
        }
    }
}
```

- [ ] **Step 4: Escrever `send-toast.ps1` (roda no Windows PowerShell 5.1)**

```powershell
# Uso: powershell.exe -NoProfile -File spikes/s2-notifications/send-toast.ps1 "texto"
param([string] $Text = 'Teste Zefiro')
[Windows.UI.Notifications.ToastNotificationManager, Windows.UI.Notifications, ContentType = WindowsRuntime] > $null
$xml = [Windows.UI.Notifications.ToastNotificationManager]::GetTemplateContent([Windows.UI.Notifications.ToastTemplateType]::ToastText01)
$xml.GetElementsByTagName('text')[0].AppendChild($xml.CreateTextNode($Text)) > $null
$aumid = '{1AC14E77-02E7-4E5D-B744-2EB1AE5198B7}\WindowsPowerShell\v1.0\powershell.exe'
[Windows.UI.Notifications.ToastNotificationManager]::CreateToastNotifier($aumid).Show([Windows.UI.Notifications.ToastNotification]::new($xml))
```

- [ ] **Step 5: Rodar sem pacote**

Run: `cargo run --manifest-path spikes/s2-notifications/Cargo.toml`
Depois, em outro terminal, 3 vezes com 2 s de intervalo: `powershell.exe -NoProfile -File spikes/s2-notifications/send-toast.ps1 "toast N"`.
Depois limpar a central (`Win+N` → Limpar tudo).

Anotar:
1. A linha `RequestAccessAsync -> …` (`Allowed`, `Denied` ou erro).
2. Se o `count` sobe 1 a cada toast e volta a 0 ao limpar.
3. A diferença de tempo entre o envio e as linhas `WNF:` e `polling:` (meta ≤ 1 s).

- [ ] **Step 6: Se exigir identidade, testar com pacote sparse**

Só se o Passo 5 mostrar que o acesso não funciona sem pacote. Os passos de certificado **são feitos pelo usuário** (mudança de segurança do sistema):

6a. Criar `spikes/s2-notifications/sparse/AppxManifest.xml`:

```xml
<?xml version="1.0" encoding="utf-8"?>
<Package xmlns="http://schemas.microsoft.com/appx/manifest/foundation/windows10"
         xmlns:uap="http://schemas.microsoft.com/appx/manifest/uap/windows10"
         xmlns:uap3="http://schemas.microsoft.com/appx/manifest/uap/windows10/3"
         xmlns:uap10="http://schemas.microsoft.com/appx/manifest/uap/windows10/10"
         xmlns:rescap="http://schemas.microsoft.com/appx/manifest/foundation/windows10/restrictedcapabilities"
         IgnorableNamespaces="uap uap3 uap10 rescap">
  <Identity Name="Zefiro.Dev" Publisher="CN=Zefiro Dev" Version="0.0.1.0" ProcessorArchitecture="x64" />
  <Properties>
    <DisplayName>Zefiro (dev)</DisplayName>
    <PublisherDisplayName>Felipe Cavalari</PublisherDisplayName>
    <Logo>logo.png</Logo>
    <uap10:AllowExternalContent>true</uap10:AllowExternalContent>
  </Properties>
  <Dependencies>
    <TargetDeviceFamily Name="Windows.Desktop" MinVersion="10.0.19041.0" MaxVersionTested="10.0.26200.0" />
  </Dependencies>
  <Resources><Resource Language="pt-BR" /></Resources>
  <Capabilities>
    <rescap:Capability Name="runFullTrust" />
    <rescap:Capability Name="unvirtualizedResources" />
    <uap3:Capability Name="userNotificationListener" />
  </Capabilities>
  <Applications>
    <Application Id="Zefiro" Executable="s2-notifications.exe" uap10:TrustLevel="mediumIL" uap10:RuntimeBehavior="win32App">
      <uap:VisualElements DisplayName="Zefiro (dev)" Description="Zefiro" BackgroundColor="transparent"
                          Square150x150Logo="logo.png" Square44x44Logo="logo.png" AppListEntry="none" />
    </Application>
  </Applications>
</Package>
```

Copiar qualquer PNG 150×150 como `sparse/logo.png`.

6b. Criar `target/debug/s2-notifications.exe.manifest` (manifesto externo do exe):

```xml
<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<assembly manifestVersion="1.0" xmlns="urn:schemas-microsoft-com:asm.v1">
  <assemblyIdentity version="0.0.1.0" name="Zefiro.Dev" />
  <msix xmlns="urn:schemas-microsoft-com:msix.v1" publisher="CN=Zefiro Dev" packageName="Zefiro.Dev" applicationId="Zefiro" />
</assembly>
```

6c. **Usuário:** criar e confiar no certificado de teste, empacotar e registrar:

```powershell
$cert = New-SelfSignedCertificate -Type CodeSigningCert -Subject "CN=Zefiro Dev" -CertStoreLocation Cert:\CurrentUser\My
$pwd = ConvertTo-SecureString -String "zefiro-dev" -Force -AsPlainText
Export-PfxCertificate -Cert $cert -FilePath $env:TEMP\zefiro-dev.pfx -Password $pwd
Import-PfxCertificate -FilePath $env:TEMP\zefiro-dev.pfx -CertStoreLocation Cert:\LocalMachine\TrustedPeople -Password $pwd
MakeAppx.exe pack /d spikes\s2-notifications\sparse /p $env:TEMP\zefiro-sparse.msix /nv
SignTool.exe sign /fd SHA256 /f $env:TEMP\zefiro-dev.pfx /p zefiro-dev $env:TEMP\zefiro-sparse.msix
Add-AppxPackage -Path $env:TEMP\zefiro-sparse.msix -ExternalLocation (Resolve-Path spikes\s2-notifications\target\debug)
```

(`MakeAppx.exe` e `SignTool.exe` vêm do Windows SDK instalado na Task 0, em `C:\Program Files (x86)\Windows Kits\10\bin\<versão>\x64\`.)

6d. Repetir o Passo 5 rodando `target\debug\s2-notifications.exe` direto. Anotar o resultado.

6e. Limpeza (usuário): `Get-AppxPackage Zefiro.Dev | Remove-AppxPackage` e remover o certificado de `TrustedPeople` se não for mais usar.

- [ ] **Step 7: Commit do spike**

```powershell
git add spikes/s2-notifications
git commit -m "spike(s2): UserNotificationListener access, WNF subscription and sparse package test" -m "Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

- [ ] **Step 8: Escrever `docs/adr/0002-notificacoes-e-empacotamento.md` no `main`**

Conteúdo mínimo: resultado sem pacote (status, contador, latência WNF × polling), resultado com sparse (se testado), o valor do nome WNF conferido, e a **decisão de empacotamento da Fase 2**: zip portátil (se funcionou sem pacote) **ou** sparse (se exigiu identidade) **ou** MSIX completo. Se nada funcionou: o sino vira só botão (`Win+N`) na Fase 1 e o contador vai para a Fase 2.

```powershell
cd ~/Projects/zefiro
git add docs/adr/0002-notificacoes-e-empacotamento.md
git commit -m "docs(adr): 0002 notifications access and packaging decision" -m "Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

---

## Task 18: S3 — Brilho via DDC/CI e ADR-0003

**Files:**
- Create (branch `spike/s3-ddc`): `spikes/s3-ddc/Cargo.toml`, `src/main.rs`, `src/coalesce.rs`, `src/ddc.rs`
- Create (no `main`): `docs/adr/0003-brilho-ddc-ci.md`

**Interfaces:**
- Consumes: Task 1.
- Produces: `coalesce::{Coalescer::new(min_interval), request(u32), next_write(now) -> Option<u32>, complete(now), percentile(&[f64], p) -> f64}` (portado para a Fase 1); latências medidas e parâmetros de agrupamento.

- [ ] **Step 1: Criar branch, worktree e projeto**

```powershell
cd ~/Projects/zefiro
git worktree add ../zefiro-s3 -b spike/s3-ddc main
cd ../zefiro-s3
cargo new spikes/s3-ddc --edition 2024 --vcs none
```

`spikes/s3-ddc/Cargo.toml`:

```toml
[package]
name = "s3-ddc"
version = "0.0.0"
edition = "2024"
publish = false

[workspace]

[dependencies]
windows-core = "=0.61.2"
windows = { version = "=0.61.3", features = ["Win32_Foundation", "Win32_Devices_Display", "Win32_Graphics_Gdi"] }
```

- [ ] **Step 2: Escrever os testes do agrupador (falhando)**

`spikes/s3-ddc/src/coalesce.rs`:

```rust
use std::time::{Duration, Instant};

/// Guarda só o último valor pedido e libera uma escrita por vez,
/// respeitando um intervalo mínimo entre o fim de uma e o início da próxima.
#[derive(Debug)]
pub struct Coalescer {
    pending: Option<u32>,
    in_flight: bool,
    last_done: Option<Instant>,
    min_interval: Duration,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_the_last_requested_value_is_written() {
        let t0 = Instant::now();
        let mut c = Coalescer::new(Duration::ZERO);
        c.request(10);
        assert_eq!(c.next_write(t0), Some(10));
        for v in 11..=60 {
            c.request(v);
        }
        assert_eq!(c.next_write(t0), None, "escrita em andamento bloqueia a próxima");
        c.complete(t0);
        assert_eq!(c.next_write(t0), Some(60));
        c.complete(t0);
        assert_eq!(c.next_write(t0), None, "nada pendente");
    }

    #[test]
    fn respects_the_minimum_interval() {
        let t0 = Instant::now();
        let mut c = Coalescer::new(Duration::from_millis(50));
        c.request(1);
        assert_eq!(c.next_write(t0), Some(1));
        c.complete(t0);
        c.request(2);
        assert_eq!(c.next_write(t0 + Duration::from_millis(49)), None);
        assert_eq!(c.next_write(t0 + Duration::from_millis(50)), Some(2));
    }

    #[test]
    fn percentile_of_sorted_samples() {
        let v: Vec<f64> = (1..=100).map(f64::from).collect();
        assert_eq!(percentile(&v, 0.5), 50.0);
        assert_eq!(percentile(&v, 0.95), 95.0);
        assert_eq!(percentile(&[], 0.5), 0.0);
    }
}
```

`src/main.rs` provisório: `mod coalesce; fn main() {}`.

- [ ] **Step 3: Rodar para ver falhar**

Run: `cargo test --manifest-path spikes/s3-ddc/Cargo.toml`
Expected: FAIL de compilação (`new`, `request`, `next_write`, `complete`, `percentile`).

- [ ] **Step 4: Implementar**

Acrescentar acima do `#[cfg(test)]`:

```rust
impl Coalescer {
    pub fn new(min_interval: Duration) -> Self {
        Self { pending: None, in_flight: false, last_done: None, min_interval }
    }

    pub fn request(&mut self, value: u32) {
        self.pending = Some(value);
    }

    pub fn next_write(&mut self, now: Instant) -> Option<u32> {
        if self.in_flight {
            return None;
        }
        if let Some(done) = self.last_done {
            if now.duration_since(done) < self.min_interval {
                return None;
            }
        }
        let v = self.pending.take()?;
        self.in_flight = true;
        Some(v)
    }

    pub fn complete(&mut self, now: Instant) {
        self.in_flight = false;
        self.last_done = Some(now);
    }
}

/// Percentil (método "nearest rank") de amostras já ordenadas.
pub fn percentile(sorted: &[f64], p: f64) -> f64 {
    if sorted.is_empty() {
        return 0.0;
    }
    let rank = (p * sorted.len() as f64).ceil() as usize;
    sorted[rank.clamp(1, sorted.len()) - 1]
}
```

Run: `cargo test --manifest-path spikes/s3-ddc/Cargo.toml`
Expected: PASS (3 testes).

- [ ] **Step 5: Escrever `src/ddc.rs`**

```rust
use std::sync::mpsc;
use std::time::Duration;
use windows::Win32::Devices::Display::*;
use windows::Win32::Foundation::{BOOL, HANDLE, LPARAM, RECT, TRUE};
use windows::Win32::Graphics::Gdi::{EnumDisplayMonitors, HDC, HMONITOR};

pub const VCP_BRIGHTNESS: u8 = 0x10;

pub struct Monitor {
    pub handle: HANDLE,
    pub name: String,
}

// SAFETY: o handle físico é usado por uma thread de cada vez no spike.
unsafe impl Send for Monitor {}

pub fn list() -> Vec<Monitor> {
    let mut hmons: Vec<HMONITOR> = Vec::new();
    unsafe extern "system" fn cb(h: HMONITOR, _: HDC, _: *mut RECT, data: LPARAM) -> BOOL {
        // SAFETY: `data` aponta para o Vec abaixo.
        unsafe { (*(data.0 as *mut Vec<HMONITOR>)).push(h) };
        TRUE
    }
    // SAFETY: callback respeita o contrato.
    unsafe { let _ = EnumDisplayMonitors(None, None, Some(cb), LPARAM(&mut hmons as *mut _ as isize)); }

    let mut out = Vec::new();
    for h in hmons {
        let mut n = 0u32;
        // SAFETY: ponteiros válidos; vetor do tamanho informado.
        unsafe {
            if GetNumberOfPhysicalMonitorsFromHMONITOR(h, &mut n).is_err() || n == 0 {
                continue;
            }
            let mut phys = vec![PHYSICAL_MONITOR::default(); n as usize];
            if GetPhysicalMonitorsFromHMONITOR(h, &mut phys).is_err() {
                continue;
            }
            for p in phys {
                let name = String::from_utf16_lossy(&p.szPhysicalMonitorDescription).trim_end_matches('\0').to_string();
                out.push(Monitor { handle: p.hPhysicalMonitor, name });
            }
        }
    }
    out
}

pub fn read(m: &Monitor) -> Option<(u32, u32)> {
    let (mut cur, mut max) = (0u32, 0u32);
    // SAFETY: handle físico válido; ponteiros de saída válidos.
    let ok = unsafe { GetVCPFeatureAndVCPFeatureReply(m.handle, VCP_BRIGHTNESS, None, &mut cur, Some(&mut max)) };
    (ok != 0).then_some((cur, max))
}

pub fn write(m: &Monitor, value: u32) -> bool {
    // SAFETY: handle físico válido.
    unsafe { SetVCPFeature(m.handle, VCP_BRIGHTNESS, value) != 0 }
}

/// Executa `f` numa thread com tempo limite (DDC/CI pode travar em monitor dormindo).
pub fn with_timeout<T: Send + 'static>(timeout: Duration, f: impl FnOnce() -> T + Send + 'static) -> Option<T> {
    let (tx, rx) = mpsc::channel();
    std::thread::spawn(move || { let _ = tx.send(f()); });
    rx.recv_timeout(timeout).ok()
}
```

Se `GetNumberOfPhysicalMonitorsFromHMONITOR`/`GetPhysicalMonitorsFromHMONITOR` devolverem `BOOL`/`i32` em vez de `Result` na versão fixada, trocar `.is_err()` por `== 0`/`!as_bool()` e anotar no ADR.

- [ ] **Step 6: Escrever `src/main.rs` com os modos de teste**

```rust
mod coalesce;
mod ddc;

use coalesce::{Coalescer, percentile};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

fn bench(m: &ddc::Monitor) {
    let Some((start, max)) = ddc::read(m) else { eprintln!("[{}] sem DDC/CI", m.name); return; };
    let mut reads = Vec::new();
    let mut writes = Vec::new();
    for i in 0..100 {
        let t = Instant::now();
        let _ = ddc::read(m);
        reads.push(t.elapsed().as_secs_f64() * 1000.0);
        let t = Instant::now();
        ddc::write(m, if i % 2 == 0 { start.saturating_sub(5) } else { start });
        writes.push(t.elapsed().as_secs_f64() * 1000.0);
    }
    ddc::write(m, start);
    reads.sort_by(f64::total_cmp);
    writes.sort_by(f64::total_cmp);
    println!("[{}] max={max} leitura p50={:.1} p95={:.1} ms | escrita p50={:.1} p95={:.1} ms",
        m.name, percentile(&reads, 0.5), percentile(&reads, 0.95), percentile(&writes, 0.5), percentile(&writes, 0.95));
}

/// Simula arrastar o slider: 30 valores por segundo durante 2 s, terminando num alvo diferente a cada tentativa.
fn drag(m: ddc::Monitor, trial: u32, min_interval_ms: u64, coalesced: bool) -> bool {
    let target = 20 + (trial * 7) % 60;
    let values: Vec<u32> = (0..60u32).map(|i| 20 + i * 60 / 59).chain(std::iter::once(target)).collect();
    let m = Arc::new(m);
    if !coalesced {
        for v in &values {
            ddc::write(&m, *v);
            std::thread::sleep(Duration::from_millis(33));
        }
    } else {
        let c = Arc::new(Mutex::new(Coalescer::new(Duration::from_millis(min_interval_ms))));
        let (wc, wm) = (c.clone(), m.clone());
        let done = Arc::new(std::sync::atomic::AtomicBool::new(false));
        let wd = done.clone();
        let writer = std::thread::spawn(move || loop {
            let next = wc.lock().unwrap().next_write(Instant::now());
            match next {
                Some(v) => { ddc::write(&wm, v); wc.lock().unwrap().complete(Instant::now()); }
                None if wd.load(std::sync::atomic::Ordering::Relaxed) => {
                    if wc.lock().unwrap().next_write(Instant::now() + Duration::from_secs(1)).is_none() { break; }
                }
                None => std::thread::sleep(Duration::from_millis(2)),
            }
        });
        for v in &values {
            c.lock().unwrap().request(*v);
            std::thread::sleep(Duration::from_millis(33));
        }
        done.store(true, std::sync::atomic::Ordering::Relaxed);
        let _ = writer.join();
    }
    std::thread::sleep(Duration::from_millis(300));
    let read = ddc::read(&m).map(|(c, _)| c);
    let ok = read == Some(target);
    println!("[{}] arrasto agrupado={coalesced} intervalo={min_interval_ms}ms alvo={target} lido={read:?} ok={ok}", m.name);
    ok
}

fn main() {
    let mode = std::env::args().nth(1).unwrap_or_else(|| "list".into());
    match mode.as_str() {
        "list" => for m in ddc::list() { println!("{} -> {:?}", m.name, ddc::read(&m)); },
        "bench" => for m in ddc::list() { bench(&m); },
        "drag" | "drag-coalesced" => {
            let interval: u64 = std::env::args().nth(2).and_then(|s| s.parse().ok()).unwrap_or(0);
            for idx in 0..ddc::list().len() {
                let ok = (0..20u32).filter(|&t| drag(ddc::list().remove(idx), t, interval, mode == "drag-coalesced")).count();
                println!("=> monitor {idx}: {ok}/20 corretos");
            }
        }
        "timeout" => for m in ddc::list() {
            let name = m.name.clone();
            match ddc::with_timeout(Duration::from_secs(2), move || ddc::read(&m)) {
                Some(r) => println!("[{name}] respondeu: {r:?}"),
                None => println!("[{name}] sem resposta em 2 s (timeout)"),
            }
        },
        other => eprintln!("modo desconhecido: {other} (use list|bench|drag|drag-coalesced <ms>|timeout)"),
    }
}
```

- [ ] **Step 7: Medir**

Run: `cargo run --release --manifest-path spikes/s3-ddc/Cargo.toml -- list`
Expected: os 2 monitores com `Some((atual, máximo))`. Monitor sem DDC/CI aparece com `None` (anotar).

Run: `cargo run --release --manifest-path spikes/s3-ddc/Cargo.toml -- bench`
Anotar p50/p95 de leitura e escrita por monitor (meta: p95 da escrita ≤ 100 ms).

Run: `cargo run --release --manifest-path spikes/s3-ddc/Cargo.toml -- drag`
Run: `cargo run --release --manifest-path spikes/s3-ddc/Cargo.toml -- drag-coalesced 0`
Run: `cargo run --release --manifest-path spikes/s3-ddc/Cargo.toml -- drag-coalesced 50`

Anotar quantos dos 20 arrastos terminaram no valor certo (meta 20/20 com agrupamento) e se o monitor travou ou piscou. Se `0 ms` não chegar a 20/20, subir o intervalo (50, 100, 150) até chegar, e anotar o menor intervalo que funciona.

- [ ] **Step 8: Testar monitor dormindo e reconexão**

Desligar um monitor pelo botão físico (sem desconectar o cabo) e rodar `-- timeout`. Expected: o monitor desligado aparece como `sem resposta em 2 s` ou com erro, **sem travar o programa**. Religar, desconectar e reconectar o cabo, rodar `-- list` de novo. Expected: os dois aparecem de novo (handles recriados a cada `list`).

- [ ] **Step 9: Commit do spike e ADR-0003 no `main`**

```powershell
git add spikes/s3-ddc
git commit -m "spike(s3): DDC/CI brightness latency, coalesced drag and timeout handling" -m "Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

`docs/adr/0003-brilho-ddc-ci.md` (no `main`): tabela de latência por monitor (p50/p95 leitura e escrita), resultado dos arrastos (sem agrupamento, agrupado 0 ms, agrupado com o intervalo mínimo que deu 20/20), comportamento com monitor dormindo, e a **decisão**: intervalo mínimo do agrupador, timeout de 2 s por chamada, UI otimista.

```powershell
cd ~/Projects/zefiro
git add docs/adr/0003-brilho-ddc-ci.md
git commit -m "docs(adr): 0003 DDC/CI brightness latency and coalescing parameters" -m "Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

---

## Task 19: S4 — Mídia via SMTC e ADR-0004

**Files:**
- Create (branch `spike/s4-smtc`): `spikes/s4-smtc/Cargo.toml`, `src/main.rs`
- Create (no `main`): `docs/adr/0004-midia-smtc.md`

**Interfaces:**
- Consumes: Task 1.
- Produces: tabela de cenários (sessões, AUMIDs, capa, frequência da linha do tempo, controles) usada na Fase 1 (ilha de mídia real).

- [ ] **Step 1: Criar branch, worktree e projeto**

```powershell
cd ~/Projects/zefiro
git worktree add ../zefiro-s4 -b spike/s4-smtc main
cd ../zefiro-s4
cargo new spikes/s4-smtc --edition 2024 --vcs none
```

`spikes/s4-smtc/Cargo.toml`:

```toml
[package]
name = "s4-smtc"
version = "0.0.0"
edition = "2024"
publish = false

[workspace]

[dependencies]
windows-core = "=0.61.2"
windows = { version = "=0.61.3", features = ["Foundation", "Foundation_Collections", "Media_Control", "Storage_Streams"] }
```

- [ ] **Step 2: Escrever `src/main.rs`**

```rust
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::Instant;
use windows::Foundation::TypedEventHandler;
use windows::Media::Control::{GlobalSystemMediaTransportControlsSession as Session, GlobalSystemMediaTransportControlsSessionManager as Manager};
use windows::core::Result;

fn describe(i: usize, s: &Session) -> Result<()> {
    let aumid = s.SourceAppUserModelId()?;
    let props = s.TryGetMediaPropertiesAsync()?.get()?;
    let cover_bytes = props.Thumbnail().ok().and_then(|t| t.OpenReadAsync().ok()?.get().ok()?.Size().ok());
    let pb = s.GetPlaybackInfo()?;
    let tl = s.GetTimelineProperties()?;
    println!(
        "#{i} aumid={aumid} | {} — {} | status={:?} | capa={cover_bytes:?} bytes | pos={:.1}s fim={:.1}s",
        props.Title()?, props.Artist()?, pb.PlaybackStatus()?,
        tl.Position()?.Duration as f64 / 1e7, tl.EndTime()?.Duration as f64 / 1e7,
    );
    Ok(())
}

fn main() -> Result<()> {
    let mgr = Manager::RequestAsync()?.get()?;
    let args: Vec<String> = std::env::args().skip(1).collect();
    let sessions: Vec<Session> = mgr.GetSessions()?.into_iter().collect();
    match args.first().map(String::as_str).unwrap_or("list") {
        "list" => {
            println!("{} sessões", sessions.len());
            for (i, s) in sessions.iter().enumerate() {
                describe(i, s)?;
            }
        }
        cmd @ ("toggle" | "next" | "prev") => {
            let i: usize = args.get(1).and_then(|s| s.parse().ok()).unwrap_or(0);
            let s = &sessions[i];
            let ok = match cmd {
                "toggle" => s.TryTogglePlayPauseAsync()?.get()?,
                "next" => s.TrySkipNextAsync()?.get()?,
                _ => s.TrySkipPreviousAsync()?.get()?,
            };
            println!("{cmd} #{i} -> {ok}");
        }
        "watch" => {
            let start = Instant::now();
            let timeline: Arc<Mutex<HashMap<String, Vec<f64>>>> = Arc::default();
            let _sc = mgr.SessionsChanged(&TypedEventHandler::new(move |_, _| {
                println!("[{:7.2}] SessionsChanged", start.elapsed().as_secs_f64());
                Ok(())
            }))?;
            let mut tokens = Vec::new();
            for s in &sessions {
                let id = s.SourceAppUserModelId()?.to_string();
                let (a, b, c) = (id.clone(), id.clone(), id.clone());
                let tl = timeline.clone();
                tokens.push(s.MediaPropertiesChanged(&TypedEventHandler::new(move |_, _| { println!("[{:7.2}] {a}: MediaPropertiesChanged", start.elapsed().as_secs_f64()); Ok(()) }))?);
                tokens.push(s.PlaybackInfoChanged(&TypedEventHandler::new(move |_, _| { println!("[{:7.2}] {b}: PlaybackInfoChanged", start.elapsed().as_secs_f64()); Ok(()) }))?);
                tokens.push(s.TimelinePropertiesChanged(&TypedEventHandler::new(move |_, _| {
                    tl.lock().unwrap().entry(c.clone()).or_default().push(start.elapsed().as_secs_f64());
                    Ok(())
                }))?);
            }
            let secs: u64 = args.get(1).and_then(|s| s.parse().ok()).unwrap_or(60);
            std::thread::sleep(std::time::Duration::from_secs(secs));
            for (id, times) in timeline.lock().unwrap().iter() {
                let gaps: Vec<f64> = times.windows(2).map(|w| w[1] - w[0]).collect();
                let avg = if gaps.is_empty() { 0.0 } else { gaps.iter().sum::<f64>() / gaps.len() as f64 };
                println!("{id}: {} eventos de linha do tempo, intervalo médio {avg:.2}s", times.len());
            }
            drop(tokens);
        }
        other => eprintln!("comando desconhecido: {other} (use list | toggle N | next N | prev N | watch [segundos])"),
    }
    Ok(())
}
```

- [ ] **Step 3: Rodar os 6 cenários da spec**

Para cada cenário (spec S4): preparar os players, rodar `cargo run --release --manifest-path spikes/s4-smtc/Cargo.toml -- list`, testar `toggle 0`, `next 0`, `prev 0` em cada sessão listada, e `-- watch 60` com a música tocando.

| # | Cenário |
|---|---|
| 1 | Só Spotify desktop |
| 2 | Só YouTube no Brave |
| 3 | Só Twitch no Brave |
| 4 | YouTube + Twitch em abas diferentes do mesmo Brave |
| 5 | Spotify + YouTube tocando ao mesmo tempo |
| 6 | YouTube no Edge (ou Chrome) para comparar |

Anotar por cenário: número de sessões, AUMID de cada uma, se tem capa (bytes > 0), se os 3 controles funcionam e o intervalo médio da linha do tempo (se for > 1 s, a ilha precisa interpolar a posição).

- [ ] **Step 4: Commit do spike e ADR-0004 no `main`**

```powershell
git add spikes/s4-smtc
git commit -m "spike(s4): SMTC sessions, metadata, controls and timeline frequency" -m "Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

`docs/adr/0004-midia-smtc.md`: tabela dos 6 cenários, mapa AUMID → nome/ícone (ex.: `Spotify.exe`, `Brave`), limitação de várias abas (se o Brave expõe uma sessão por navegador), necessidade de interpolar a linha do tempo, e a decisão para a ilha (como escolher a sessão "atual" e como mostrar várias fontes).

```powershell
cd ~/Projects/zefiro
git add docs/adr/0004-midia-smtc.md
git commit -m "docs(adr): 0004 SMTC media sessions behavior" -m "Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

---

## Task 20: S5 — Modo foco (Não Perturbe), modo noturno e ADR-0005

**Files:**
- Create (branch `spike/s5-focus-night`): `spikes/s5-focus-night/Cargo.toml`, `src/main.rs`, `src/dnd.rs`, `src/night.rs`, `src/blob.rs`, `diff-blobs.ps1`
- Create (no `main`): `docs/adr/0005-foco-e-modo-noturno.md`

**Interfaces:**
- Consumes: Task 1; strings do YASB (módulo `dnd_api`).
- Produces: `blob::{is_on(&[u8]) -> bool, set_on(&[u8], bool) -> Vec<u8>}`; decisão de como ler/alternar Não Perturbe e luz noturna (ou plano B).

- [ ] **Step 1: Criar branch, worktree e projeto**

```powershell
cd ~/Projects/zefiro
git worktree add ../zefiro-s5 -b spike/s5-focus-night main
cd ../zefiro-s5
cargo new spikes/s5-focus-night --edition 2024 --vcs none
```

`spikes/s5-focus-night/Cargo.toml`:

```toml
[package]
name = "s5-focus-night"
version = "0.0.0"
edition = "2024"
publish = false

[workspace]

[dependencies]
windows-core = "=0.61.2"
windows = { version = "=0.61.3", features = [
  "Foundation", "UI_Shell", "Win32_Foundation", "Win32_System_Com", "Win32_System_Registry", "Win32_System_Threading",
] }
```

- [ ] **Step 2: Conferir os identificadores do Quiet Hours usados pelo YASB**

```powershell
$tmp = Join-Path $env:TEMP 'yasb-dnd'
New-Item -ItemType Directory -Force $tmp | Out-Null
tar -xf "C:\Program Files\YASB\lib\library.zip" -C $tmp core/widgets/services/dnd/dnd_api.pyc
[Text.Encoding]::ASCII.GetString([IO.File]::ReadAllBytes("$tmp\core\widgets\services\dnd\dnd_api.pyc")) |
  Select-String -Pattern '[0-9A-Fa-f]{8}-[0-9A-Fa-f]{4}-[0-9A-Fa-f]{4}-[0-9A-Fa-f]{4}-[0-9A-Fa-f]{12}' -AllMatches |
  ForEach-Object { $_.Matches.Value } | Sort-Object -Unique
```

Expected: GUIDs impressos. Comparar com os valores usados abaixo — CLSID `f53321fa-34f8-4b7f-b9a3-361877cb94cf` (QuietHoursSettings) e IID `6bff4732-81ec-4ffb-ae67-b6c1bc29631f` (IQuietHoursSettings). Se forem diferentes, usar os do YASB e anotar no ADR.

- [ ] **Step 3: Escrever `src/dnd.rs`**

```rust
use windows::Win32::System::Com::{CLSCTX_LOCAL_SERVER, CoCreateInstance, CoTaskMemFree};
use windows::core::{GUID, HRESULT, IUnknown, IUnknown_Vtbl, PCWSTR, PWSTR, Result, interface};

pub const UNRESTRICTED: &str = "Microsoft.QuietHoursProfile.Unrestricted";
pub const PRIORITY_ONLY: &str = "Microsoft.QuietHoursProfile.PriorityOnly";
pub const ALARMS_ONLY: &str = "Microsoft.QuietHoursProfile.AlarmsOnly";

const CLSID_QUIET_HOURS_SETTINGS: GUID = GUID::from_u128(0xf53321fa_34f8_4b7f_b9a3_361877cb94cf);

#[interface("6bff4732-81ec-4ffb-ae67-b6c1bc29631f")]
unsafe trait IQuietHoursSettings: IUnknown {
    fn get_UserSelectedProfile(&self, profile: *mut PWSTR) -> HRESULT;
    fn put_UserSelectedProfile(&self, profile: PCWSTR) -> HRESULT;
}

fn settings() -> Result<IQuietHoursSettings> {
    // SAFETY: servidor COM fora do processo; COM já inicializado pelo chamador.
    unsafe { CoCreateInstance(&CLSID_QUIET_HOURS_SETTINGS, None, CLSCTX_LOCAL_SERVER) }
}

pub fn get() -> Result<String> {
    let s = settings()?;
    let mut p = PWSTR::null();
    // SAFETY: string alocada pelo servidor; liberada com CoTaskMemFree.
    unsafe {
        s.get_UserSelectedProfile(&mut p).ok()?;
        let out = p.to_string().unwrap_or_default();
        CoTaskMemFree(Some(p.0 as _));
        Ok(out)
    }
}

pub fn set(profile: &str) -> Result<()> {
    let s = settings()?;
    let wide: Vec<u16> = profile.encode_utf16().chain(std::iter::once(0)).collect();
    // SAFETY: string terminada em zero viva durante a chamada.
    unsafe { s.put_UserSelectedProfile(PCWSTR(wide.as_ptr())).ok() }
}

/// Alternativa documentada no Win11: sessão de foco do Relógio (liga o Não Perturbe).
pub fn focus_session(on: bool) -> Result<String> {
    use windows::UI::Shell::FocusSessionManager;
    if !FocusSessionManager::IsSupported()? {
        return Ok("FocusSessionManager não suportado".into());
    }
    let m = FocusSessionManager::GetDefault()?;
    if on { m.TryStartFocusSession()?; } else { m.DeactivateFocus()?; }
    Ok(format!("IsFocusActive={}", m.IsFocusActive()?))
}
```

- [ ] **Step 4: Escrever os testes do blob da luz noturna (falhando)**

`src/blob.rs`:

```rust
//! Blob `Data` de bluelightreductionstate. Heurística conhecida em scripts da comunidade:
//! byte 18 = 0x15 (ligado) / 0x13 (desligado); ligado tem 2 bytes extras (0x10, 0x00) na posição 23.
//! O Passo 6 valida contra o diff real e ajusta se for diferente.

pub const STATE_BYTE: usize = 18;
pub const ON: u8 = 0x15;
pub const OFF: u8 = 0x13;
pub const INSERT_AT: usize = 23;

#[cfg(test)]
mod tests {
    use super::*;

    fn off_blob() -> Vec<u8> {
        let mut v: Vec<u8> = (0..43u8).collect();
        v[STATE_BYTE] = OFF;
        v
    }

    #[test]
    fn detects_state() {
        assert!(!is_on(&off_blob()));
        let on = set_on(&off_blob(), true);
        assert!(is_on(&on));
    }

    #[test]
    fn turning_on_inserts_two_bytes_and_off_removes_them() {
        let off = off_blob();
        let on = set_on(&off, true);
        assert_eq!(on.len(), off.len() + 2);
        assert_eq!(&on[INSERT_AT..INSERT_AT + 2], &[0x10, 0x00]);
        let back = set_on(&on, false);
        assert_eq!(back, off);
    }

    #[test]
    fn setting_the_current_state_is_a_no_op() {
        let off = off_blob();
        assert_eq!(set_on(&off, false), off);
        let on = set_on(&off, true);
        assert_eq!(set_on(&on, true), on);
    }

    #[test]
    fn short_or_empty_blob_is_left_untouched() {
        assert_eq!(set_on(&[1, 2, 3], true), vec![1, 2, 3]);
        assert!(!is_on(&[]));
    }
}
```

`src/main.rs` provisório: `mod blob; fn main() {}`.

Run: `cargo test --manifest-path spikes/s5-focus-night/Cargo.toml`
Expected: FAIL de compilação (`is_on`, `set_on`).

- [ ] **Step 5: Implementar o blob**

Acrescentar acima do `#[cfg(test)]`:

```rust
pub fn is_on(blob: &[u8]) -> bool {
    blob.get(STATE_BYTE) == Some(&ON)
}

pub fn set_on(blob: &[u8], on: bool) -> Vec<u8> {
    if blob.len() <= INSERT_AT + 2 || is_on(blob) == on {
        return blob.to_vec();
    }
    let mut v = blob.to_vec();
    if on {
        v.splice(INSERT_AT..INSERT_AT, [0x10, 0x00]);
        v[STATE_BYTE] = ON;
    } else {
        v.drain(INSERT_AT..INSERT_AT + 2);
        v[STATE_BYTE] = OFF;
    }
    v
}
```

Run: `cargo test --manifest-path spikes/s5-focus-night/Cargo.toml`
Expected: PASS (4 testes).

- [ ] **Step 6: Capturar o blob real e comparar com a heurística**

Escrever `src/night.rs`:

```rust
use std::path::PathBuf;
use windows::Win32::System::Registry::*;
use windows::core::{PCWSTR, Result, w};

const KEY: PCWSTR = w!("Software\\Microsoft\\Windows\\CurrentVersion\\CloudStore\\Store\\DefaultAccount\\Current\\default$windows.data.bluelightreduction.bluelightreductionstate\\windows.data.bluelightreduction.bluelightreductionstate");

pub fn read() -> Result<Vec<u8>> {
    let mut size = 0u32;
    // SAFETY: primeira chamada só pergunta o tamanho; segunda preenche o buffer.
    unsafe {
        RegGetValueW(HKEY_CURRENT_USER, KEY, w!("Data"), RRF_RT_REG_BINARY, None, None, Some(&mut size)).ok()?;
        let mut buf = vec![0u8; size as usize];
        RegGetValueW(HKEY_CURRENT_USER, KEY, w!("Data"), RRF_RT_REG_BINARY, None, Some(buf.as_mut_ptr() as _), Some(&mut size)).ok()?;
        buf.truncate(size as usize);
        Ok(buf)
    }
}

pub fn write(data: &[u8]) -> Result<()> {
    // SAFETY: buffer válido do tamanho informado.
    unsafe { RegSetKeyValueW(HKEY_CURRENT_USER, KEY, w!("Data"), REG_BINARY.0, Some(data.as_ptr() as _), data.len() as u32).ok() }
}

pub fn backup_path() -> PathBuf {
    PathBuf::from(std::env::var("LOCALAPPDATA").unwrap_or_default()).join("zefiro").join("spike-s5").join("nightlight-backup.bin")
}

/// Grava o backup só na primeira vez (para nunca sobrescrever o estado original).
pub fn ensure_backup() -> std::io::Result<()> {
    let p = backup_path();
    if !p.exists() {
        std::fs::create_dir_all(p.parent().unwrap())?;
        std::fs::write(&p, read().map_err(std::io::Error::other)?)?;
    }
    Ok(())
}

pub fn restore() -> std::io::Result<()> {
    let data = std::fs::read(backup_path())?;
    write(&data).map_err(std::io::Error::other)
}

/// Bloqueia até a chave mudar (luz noturna alterada por qualquer app).
pub fn wait_change() -> Result<()> {
    let mut hkey = HKEY::default();
    // SAFETY: chave aberta só para notificação e fechada no fim.
    unsafe {
        RegOpenKeyExW(HKEY_CURRENT_USER, KEY, None, KEY_NOTIFY, &mut hkey).ok()?;
        let r = RegNotifyChangeKeyValue(hkey, false, REG_NOTIFY_CHANGE_LAST_SET, None, false).ok();
        let _ = RegCloseKey(hkey);
        r
    }
}
```

`diff-blobs.ps1`:

```powershell
param([Parameter(Mandatory)] [string] $Off, [Parameter(Mandatory)] [string] $On)
$a = [IO.File]::ReadAllBytes($Off); $b = [IO.File]::ReadAllBytes($On)
"tamanho: desligado=$($a.Length) ligado=$($b.Length)"
for ($i = 0; $i -lt [math]::Max($a.Length, $b.Length); $i++) {
    $x = if ($i -lt $a.Length) { '{0:X2}' -f $a[$i] } else { '--' }
    $y = if ($i -lt $b.Length) { '{0:X2}' -f $b[$i] } else { '--' }
    if ($x -ne $y) { "{0,3}: {1} -> {2}" -f $i, $x, $y }
}
```

Substituir `src/main.rs` por:

```rust
mod blob;
mod dnd;
mod night;

use std::time::Instant;
use windows::Win32::System::Com::{COINIT_APARTMENTTHREADED, CoInitializeEx};

fn main() {
    // SAFETY: COM para o servidor Quiet Hours.
    unsafe { let _ = CoInitializeEx(None, COINIT_APARTMENTTHREADED); }
    let args: Vec<String> = std::env::args().skip(1).collect();
    let a: Vec<&str> = args.iter().map(String::as_str).collect();
    match a.as_slice() {
        ["dnd", "get"] => println!("{:?}", dnd::get()),
        ["dnd", "set", p] => {
            let profile = match *p { "priority" => dnd::PRIORITY_ONLY, "alarms" => dnd::ALARMS_ONLY, _ => dnd::UNRESTRICTED };
            println!("{:?} -> agora {:?}", dnd::set(profile), dnd::get());
        }
        ["dnd", "watch"] => {
            let start = Instant::now();
            let mut last = dnd::get().ok();
            loop {
                std::thread::sleep(std::time::Duration::from_millis(250));
                let now = dnd::get().ok();
                if now != last { println!("[{:6.2}] {last:?} -> {now:?}", start.elapsed().as_secs_f64()); last = now; }
            }
        }
        ["focus", on] => println!("{:?}", dnd::focus_session(*on == "on")),
        ["night", "dump", file] => { std::fs::write(file, night::read().unwrap()).unwrap(); println!("salvo em {file}"); }
        ["night", "get"] => println!("ligado={}", blob::is_on(&night::read().unwrap())),
        ["night", state @ ("on" | "off")] => {
            night::ensure_backup().unwrap();
            let cur = night::read().unwrap();
            night::write(&blob::set_on(&cur, *state == "on")).unwrap();
            println!("escrito; leitura agora: ligado={}", blob::is_on(&night::read().unwrap()));
        }
        ["night", "restore"] => println!("{:?}", night::restore()),
        ["night", "watch"] => {
            let start = Instant::now();
            loop {
                night::wait_change().unwrap();
                println!("[{:6.2}] mudou: ligado={}", start.elapsed().as_secs_f64(), blob::is_on(&night::read().unwrap()));
            }
        }
        _ => eprintln!("uso: dnd get|set priority|alarms|unrestricted|watch · focus on|off · night dump <arq>|get|on|off|restore|watch"),
    }
}
```

Capturar e comparar:

1. Configurações → Sistema → Tela → **Luz noturna desligada**. Run: `cargo run --manifest-path spikes/s5-focus-night/Cargo.toml -- night dump $env:TEMP\night-off.bin`
2. **Ligar** a luz noturna em Configurações. Run: `cargo run --manifest-path spikes/s5-focus-night/Cargo.toml -- night dump $env:TEMP\night-on.bin`
3. Run: `pwsh -NoProfile -File spikes/s5-focus-night/diff-blobs.ps1 -Off $env:TEMP\night-off.bin -On $env:TEMP\night-on.bin`

Expected: o diff mostra o byte 18 (`13 -> 15`), os 2 bytes inseridos perto da posição 23 e, possivelmente, alguns bytes de carimbo de tempo (posições ~10–15). Se o padrão real for diferente da heurística, **ajustar `STATE_BYTE`, `INSERT_AT` e os testes** para refletir o diff real, rodar `cargo test` de novo e anotar no ADR.

- [ ] **Step 7: Testar alternar, restaurar e escutar mudanças**

1. Run: `cargo run --manifest-path spikes/s5-focus-night/Cargo.toml -- night watch` (deixar rodando). Alternar a luz noturna pela Central de Ações. Expected: linha `mudou` em ≤ 1 s.
2. Num segundo terminal: `-- night on` e depois `-- night off`. Expected: a tela muda de cor **na hora**, sem logoff; o `watch` registra as mudanças. Se a tela não mudar, anotar se é preciso alterar os bytes de carimbo de tempo (segundo o diff do Passo 6).
3. `-- night restore`. Expected: volta ao estado de antes do primeiro `on/off`.

- [ ] **Step 8: Testar o Não Perturbe**

1. `-- dnd get` → anotar o perfil atual.
2. `-- dnd watch` (deixar rodando) e alternar o Não Perturbe pela Central de Notificações (`Win+N`). Expected: mudanças aparecem em ≤ 1 s.
3. `-- dnd set priority` e `-- dnd set unrestricted`. Expected: o ícone de Não Perturbe aparece e some na barra de tarefas.
4. `-- focus on` e `-- focus off` (alternativa documentada do Win11). Anotar se funciona sem pacote.
5. Repetir 1–3 na VM Windows 10 (se existir) e anotar.

- [ ] **Step 9: Commit do spike e ADR-0005 no `main`**

```powershell
git add spikes/s5-focus-night
git commit -m "spike(s5): Quiet Hours profile, focus session and night light blob toggle" -m "Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

`docs/adr/0005-foco-e-modo-noturno.md`: GUIDs conferidos, resultado de `dnd get/set/watch` e `focus` (Win11 e Win10), diff real do blob e se a heurística bateu, se aplica sem logoff, latência do `watch`, e a **decisão** para os widgets (API escolhida para cada um, ou plano B: abrir `ms-settings:nightlight` / configurações de notificação).

```powershell
cd ~/Projects/zefiro
git add docs/adr/0005-foco-e-modo-noturno.md
git commit -m "docs(adr): 0005 focus (do not disturb) and night light control" -m "Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

---

## Task 21: F2 — Workspace Rust

**Files:**
- Create: `Cargo.toml`, `rust-toolchain.toml`, `crates/core/{Cargo.toml,src/lib.rs}`, `crates/ui/{Cargo.toml,src/lib.rs}`, `crates/resources/{Cargo.toml,src/lib.rs}`, `crates/platform/{Cargo.toml,src/lib.rs}`, `crates/platform-windows/{Cargo.toml,src/lib.rs}`, `crates/app/{Cargo.toml,src/main.rs}`

**Interfaces:**
- Consumes: ADRs 0001–0006 (os spikes terminaram).
- Produces: workspace com 6 crates compilando, lints do workspace ativos e as crates portáveis checadas para Linux.

- [ ] **Step 1: Criar a raiz do workspace**

No worktree principal (`~/Projects/zefiro`, branch `main`).

`Cargo.toml`:

```toml
[workspace]
resolver = "3"
members = [
  "crates/core",
  "crates/ui",
  "crates/resources",
  "crates/platform",
  "crates/platform-windows",
  "crates/app",
]

[workspace.package]
version = "0.0.1"
edition = "2024"
license = "MIT OR Apache-2.0"
repository = "https://github.com/Felipe-Cavalari/zefiro"
publish = false

[workspace.lints.rust]
unsafe_op_in_unsafe_fn = "deny"

[workspace.lints.clippy]
all = { level = "deny", priority = -1 }

[workspace.dependencies]
zefiro-core = { path = "crates/core" }
zefiro-ui = { path = "crates/ui" }
zefiro-resources = { path = "crates/resources" }
zefiro-platform = { path = "crates/platform" }
zefiro-platform-windows = { path = "crates/platform-windows" }
windows = { version = "=0.61.3", features = ["Win32_Foundation"] }
windows-core = "=0.61.2"
tracing = "0.1"
tracing-appender = "0.2"
tracing-subscriber = { version = "0.3", features = ["fmt", "env-filter"] }
tempfile = "3"
```

`rust-toolchain.toml`:

```toml
[toolchain]
channel = "stable"
components = ["clippy", "rustfmt"]
targets = ["x86_64-unknown-linux-gnu"]
```

- [ ] **Step 2: Criar as crates**

Para cada crate de biblioteca (`core`, `ui`, `resources`, `platform`), `crates/<nome>/Cargo.toml`:

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

e `crates/<nome>/src/lib.rs` com uma linha de documentação:

| Crate | `src/lib.rs` |
|---|---|
| `core` | `//! Núcleo portável do Zefiro: integrações, estado, agendador, supervisor e observabilidade.` |
| `ui` | `//! Modelo de interface portável: árvore de nós, layout e molas.` |
| `resources` | `//! Recursos declarativos (temas, itens, expressões). Implementado na Fase 1.` |
| `platform` | `//! Contratos (traits) que cada sistema operacional implementa.` |

`crates/platform-windows/Cargo.toml`:

```toml
[package]
name = "zefiro-platform-windows"
version.workspace = true
edition.workspace = true
license.workspace = true
publish.workspace = true

[lints]
workspace = true

[target.'cfg(windows)'.dependencies]
windows.workspace = true
zefiro-platform.workspace = true
```

`crates/platform-windows/src/lib.rs`:

```rust
//! Implementação Windows dos contratos de `zefiro-platform`. Preenchida a partir da Fase 1,
//! com base nos ADRs 0001 (compositor) e 0006 (ilha de mídia).
#![cfg(windows)]
```

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

[dependencies]
zefiro-core.workspace = true
```

`crates/app/src/main.rs` (provisório; a Task 23 completa):

```rust
fn main() {
    println!("zefiro {}", env!("CARGO_PKG_VERSION"));
}
```

- [ ] **Step 3: Verificar**

Run: `cargo build --workspace`
Expected: compila sem avisos.

Run: `cargo clippy --workspace --all-targets -- -D warnings`
Expected: sem erros.

Run: `cargo check -p zefiro-core -p zefiro-ui -p zefiro-resources --target x86_64-unknown-linux-gnu`
Expected: compila (prova de que as crates portáveis não dependem do Windows).

Run: `cargo run -p zefiro-app`
Expected: imprime `zefiro 0.0.1`.

- [ ] **Step 4: Commit**

```powershell
git add Cargo.toml Cargo.lock rust-toolchain.toml crates
git commit -m "build: create Rust workspace with core, ui, resources, platform, platform-windows and app crates" -m "Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

---

## Task 22: F2 — `Secret<T>` (tokens nunca formatados)

**Files:**
- Create: `crates/core/src/secret.rs`
- Modify: `crates/core/src/lib.rs` (`pub mod secret;`)

**Interfaces:**
- Consumes: Task 21.
- Produces: `zefiro_core::secret::Secret<T>` com `new(T)`, `expose(&self) -> &T`, `into_inner(self) -> T`; `Debug` e `Display` sempre imprimem `***`; `Clone` quando `T: Clone`.

- [ ] **Step 1: Escrever os testes (falhando)**

`crates/core/src/secret.rs`:

```rust
//! Valor sensível (token, senha) que nunca aparece quando formatado.

pub struct Secret<T>(T);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn debug_and_display_hide_the_value() {
        let s = Secret::new("tok-123".to_string());
        assert_eq!(format!("{s:?}"), "***");
        assert_eq!(format!("{s}"), "***");
        assert_eq!(format!("{s:#?}"), "***");
    }

    #[test]
    fn expose_and_into_inner_return_the_value() {
        let s = Secret::new(String::from("tok-123"));
        assert_eq!(s.expose(), "tok-123");
        assert_eq!(s.clone().into_inner(), "tok-123");
    }

    #[test]
    fn hidden_inside_derived_debug_of_a_struct() {
        #[derive(Debug)]
        #[allow(dead_code)]
        struct Credentials { user: &'static str, token: Secret<&'static str> }
        let c = Credentials { user: "felipe", token: Secret::new("tok-123") };
        let out = format!("{c:?}");
        assert!(out.contains("felipe"));
        assert!(out.contains("***"));
        assert!(!out.contains("tok-123"));
    }
}
```

Em `crates/core/src/lib.rs`, acrescentar `pub mod secret;`.

- [ ] **Step 2: Rodar para ver falhar**

Run: `cargo test -p zefiro-core secret`
Expected: FAIL de compilação (`new`, `expose`, `Debug`…).

- [ ] **Step 3: Implementar**

Acrescentar acima do `#[cfg(test)]`:

```rust
use std::fmt;

impl<T> Secret<T> {
    pub fn new(value: T) -> Self {
        Self(value)
    }

    /// Acesso explícito ao valor. Use só no ponto exato em que ele é necessário (ex.: header HTTP).
    pub fn expose(&self) -> &T {
        &self.0
    }

    pub fn into_inner(self) -> T {
        self.0
    }
}

impl<T: Clone> Clone for Secret<T> {
    fn clone(&self) -> Self {
        Self(self.0.clone())
    }
}

impl<T> fmt::Debug for Secret<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("***")
    }
}

impl<T> fmt::Display for Secret<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("***")
    }
}
```

- [ ] **Step 4: Rodar os testes**

Run: `cargo test -p zefiro-core secret`
Expected: PASS (3 testes).

- [ ] **Step 5: Commit**

```powershell
git add crates/core
git commit -m "feat(core): add Secret<T> that never formats its value" -m "Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

---

## Task 23: F2 — Logs, relatório de crash e binário `zefiro`

**Files:**
- Create: `crates/core/src/observability.rs`, `crates/core/tests/logging.rs`, `crates/core/tests/panic_hook.rs`
- Modify: `crates/core/Cargo.toml` (dependências), `crates/core/src/lib.rs` (`pub mod observability;`), `crates/app/src/main.rs`

**Interfaces:**
- Consumes: Task 22 (`Secret`).
- Produces: `zefiro_core::observability::{log_dir_from(local_app_data, xdg_state_home, home) -> PathBuf, default_log_dir() -> PathBuf, init_logging(dir: &Path, filter: &str) -> io::Result<LogGuard>, install_panic_hook(dir: PathBuf, version: &'static str), LogGuard}`.

- [ ] **Step 1: Escrever os testes (falhando)**

Em `crates/core/Cargo.toml`, acrescentar:

```toml
[dependencies]
tracing.workspace = true
tracing-appender.workspace = true
tracing-subscriber.workspace = true

[dev-dependencies]
tempfile.workspace = true
```

`crates/core/src/observability.rs` (só os testes de unidade por enquanto):

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

`crates/core/tests/logging.rs` (binário de teste próprio, porque o subscriber é global):

```rust
use zefiro_core::observability::init_logging;
use zefiro_core::secret::Secret;

#[test]
fn writes_events_to_a_daily_file_without_leaking_secrets() {
    let dir = tempfile::tempdir().unwrap();
    let guard = init_logging(dir.path(), "info").unwrap();
    let token = Secret::new("tok-123");
    tracing::info!(token = ?token, "evento de teste");
    drop(guard); // força o flush do escritor não bloqueante

    let mut content = String::new();
    for entry in std::fs::read_dir(dir.path()).unwrap() {
        let path = entry.unwrap().path();
        assert!(path.file_name().unwrap().to_string_lossy().starts_with("zefiro"), "{path:?}");
        content.push_str(&std::fs::read_to_string(path).unwrap());
    }
    assert!(content.contains("evento de teste"));
    assert!(content.contains("***"));
    assert!(!content.contains("tok-123"));
}
```

`crates/core/tests/panic_hook.rs` (**Review Focus 5**):

```rust
use zefiro_core::observability::install_panic_hook;
use zefiro_core::secret::Secret;

#[test]
fn panic_writes_crash_report_without_secret_values() {
    let dir = tempfile::tempdir().unwrap();
    install_panic_hook(dir.path().to_path_buf(), "0.0.1-test");
    let token = Secret::new("tok-456");
    let r = std::panic::catch_unwind(|| panic!("falhou com {token:?}"));
    assert!(r.is_err());

    let reports: Vec<_> = std::fs::read_dir(dir.path()).unwrap().map(|e| e.unwrap().path()).collect();
    assert_eq!(reports.len(), 1, "{reports:?}");
    let name = reports[0].file_name().unwrap().to_string_lossy().to_string();
    assert!(name.starts_with("crash-") && name.ends_with(".txt"), "{name}");
    let body = std::fs::read_to_string(&reports[0]).unwrap();
    assert!(body.contains("0.0.1-test"));
    assert!(body.contains("falhou com ***"));
    assert!(body.contains("backtrace"));
    assert!(!body.contains("tok-456"));
}
```

Em `crates/core/src/lib.rs`, acrescentar `pub mod observability;`.

- [ ] **Step 2: Rodar para ver falhar**

Run: `cargo test -p zefiro-core`
Expected: FAIL de compilação (`log_dir_from`, `init_logging`, `install_panic_hook`).

- [ ] **Step 3: Implementar**

Acrescentar em `observability.rs`, acima do `#[cfg(test)]`:

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
Expected: PASS (testes de unidade + `logging` + `panic_hook`; o teste de panic imprime a mensagem padrão no stderr, isso é esperado).

- [ ] **Step 5: Ligar no binário**

`crates/app/src/main.rs`:

```rust
use zefiro_core::observability::{default_log_dir, init_logging, install_panic_hook};

const VERSION: &str = env!("CARGO_PKG_VERSION");

fn main() {
    let dir = default_log_dir();
    install_panic_hook(dir.clone(), VERSION);
    let _guard = match init_logging(&dir, "info") {
        Ok(g) => Some(g),
        Err(e) => {
            eprintln!("logs indisponíveis em {}: {e}", dir.display());
            None
        }
    };
    tracing::info!(version = VERSION, "zefiro iniciado (Fase 0: ainda sem interface)");
}
```

Acrescentar `tracing.workspace = true` às dependências de `crates/app/Cargo.toml`.

Run: `cargo run -p zefiro-app`
Expected: cria `%LOCALAPPDATA%\zefiro\logs\zefiro.<data>.log` com a linha "zefiro iniciado".

- [ ] **Step 6: Commit**

```powershell
git add crates/core crates/app Cargo.lock
git commit -m "feat(core): daily rolling logs and crash reports; wire them into the zefiro binary" -m "Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

---

## Task 24: F3 — Estado, contrato de integração e agendador

**Files:**
- Create: `crates/core/src/state.rs`, `crates/core/src/integration.rs`, `crates/core/src/scheduler.rs`
- Modify: `crates/core/src/lib.rs`

**Interfaces:**
- Consumes: Task 21.
- Produces:
  - `state::{Value::{Null, Bool, Int, Float, Text}, State}`; `State::new()`, `set(key, impl Into<Value>) -> &mut Self`, `get(&str) -> Option<&Value>`, `keys()`.
  - `integration::{Trigger::{Poll(Duration), Event, Both(Duration)}, Trigger::poll_interval() -> Option<Duration>, ActionId(String), Action { id, title }, Args = BTreeMap<String, Value>, Ctx { now: Instant }, IntegrationError { message }, Result<T>, Integration}` (trait conforme `PLANNING.md` §3.3).
  - `scheduler::Scheduler` com `new()`, `schedule(id: usize, at: Instant)`, `next_deadline() -> Option<Instant>`, `pop_due(now) -> Vec<usize>`.

- [ ] **Step 1: Escrever os testes (falhando)**

`crates/core/src/state.rs`:

```rust
//! Estado tipado de uma integração: o que as expressões dos itens leem (ex.: `cpu.usage`).

use std::collections::BTreeMap;

#[derive(Debug, Clone, PartialEq)]
pub enum Value { Null, Bool(bool), Int(i64), Float(f64), Text(String) }

#[derive(Debug, Clone, Default, PartialEq)]
pub struct State { values: BTreeMap<String, Value> }

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn set_get_and_keys_in_order() {
        let mut s = State::new();
        s.set("media.title", "Nocturne").set("cpu.usage", 42.5).set("media.playing", true).set("count", 3i64);
        assert_eq!(s.get("media.title"), Some(&Value::Text("Nocturne".into())));
        assert_eq!(s.get("cpu.usage"), Some(&Value::Float(42.5)));
        assert_eq!(s.get("media.playing"), Some(&Value::Bool(true)));
        assert_eq!(s.get("count"), Some(&Value::Int(3)));
        assert_eq!(s.get("nada"), None);
        assert_eq!(s.keys().collect::<Vec<_>>(), vec!["count", "cpu.usage", "media.playing", "media.title"]);
    }
}
```

`crates/core/src/integration.rs`:

```rust
//! Contrato que toda integração (provider) implementa.

use std::collections::BTreeMap;
use std::fmt;
use std::time::{Duration, Instant};

use crate::state::{State, Value};

#[cfg(test)]
mod tests {
    use super::*;

    struct Counter { n: i64 }

    impl Integration for Counter {
        fn id(&self) -> &'static str { "counter" }
        fn trigger(&self) -> Trigger { Trigger::Poll(Duration::from_secs(1)) }
        fn refresh(&mut self, _ctx: &Ctx) -> Result<State> {
            self.n += 1;
            let mut s = State::new();
            s.set("counter.value", self.n);
            Ok(s)
        }
        fn actions(&self, _state: &State) -> Vec<Action> {
            vec![Action { id: ActionId("counter.reset".into()), title: "Zerar".into() }]
        }
        fn invoke(&mut self, action: &ActionId, _args: &Args, _ctx: &Ctx) -> Result<()> {
            match action.0.as_str() {
                "counter.reset" => { self.n = 0; Ok(()) }
                other => Err(IntegrationError::new(format!("ação desconhecida: {other}"))),
            }
        }
    }

    #[test]
    fn a_fake_integration_refreshes_lists_and_invokes_actions() {
        let mut c: Box<dyn Integration> = Box::new(Counter { n: 0 });
        let ctx = Ctx { now: Instant::now() };
        let s = c.refresh(&ctx).unwrap();
        assert_eq!(s.get("counter.value"), Some(&Value::Int(1)));
        assert_eq!(c.actions(&s)[0].id, ActionId("counter.reset".into()));
        c.invoke(&ActionId("counter.reset".into()), &Args::new(), &ctx).unwrap();
        assert_eq!(c.refresh(&ctx).unwrap().get("counter.value"), Some(&Value::Int(1)));
        let err = c.invoke(&ActionId("x".into()), &Args::new(), &ctx).unwrap_err();
        assert_eq!(err.to_string(), "ação desconhecida: x");
    }

    #[test]
    fn poll_interval_per_trigger() {
        assert_eq!(Trigger::Poll(Duration::from_secs(2)).poll_interval(), Some(Duration::from_secs(2)));
        assert_eq!(Trigger::Both(Duration::from_secs(5)).poll_interval(), Some(Duration::from_secs(5)));
        assert_eq!(Trigger::Event.poll_interval(), None);
    }
}
```

`crates/core/src/scheduler.rs`:

```rust
//! Agendador por prazo: diz quais integrações devem atualizar agora.

use std::cmp::Reverse;
use std::collections::BinaryHeap;
use std::time::Instant;

#[derive(Debug, Default)]
pub struct Scheduler { heap: BinaryHeap<Reverse<(Instant, usize)>> }

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[test]
    fn pops_only_due_ids_in_deadline_order() {
        let t0 = Instant::now();
        let mut s = Scheduler::new();
        s.schedule(7, t0 + Duration::from_secs(3));
        s.schedule(1, t0 + Duration::from_secs(1));
        s.schedule(4, t0 + Duration::from_secs(2));
        assert_eq!(s.next_deadline(), Some(t0 + Duration::from_secs(1)));
        assert_eq!(s.pop_due(t0), Vec::<usize>::new());
        assert_eq!(s.pop_due(t0 + Duration::from_secs(2)), vec![1, 4]);
        assert_eq!(s.next_deadline(), Some(t0 + Duration::from_secs(3)));
        assert_eq!(s.pop_due(t0 + Duration::from_secs(10)), vec![7]);
        assert_eq!(s.next_deadline(), None);
    }

    #[test]
    fn same_deadline_pops_lower_id_first() {
        let t0 = Instant::now();
        let mut s = Scheduler::new();
        s.schedule(9, t0);
        s.schedule(2, t0);
        assert_eq!(s.pop_due(t0), vec![2, 9]);
    }
}
```

Em `crates/core/src/lib.rs`, acrescentar:

```rust
pub mod integration;
pub mod scheduler;
pub mod state;
```

- [ ] **Step 2: Rodar para ver falhar**

Run: `cargo test -p zefiro-core --lib`
Expected: FAIL de compilação (métodos e tipos ainda não existem).

- [ ] **Step 3: Implementar `state.rs`**

Acrescentar acima do `#[cfg(test)]`:

```rust
impl From<bool> for Value { fn from(v: bool) -> Self { Value::Bool(v) } }
impl From<i64> for Value { fn from(v: i64) -> Self { Value::Int(v) } }
impl From<f64> for Value { fn from(v: f64) -> Self { Value::Float(v) } }
impl From<&str> for Value { fn from(v: &str) -> Self { Value::Text(v.to_string()) } }
impl From<String> for Value { fn from(v: String) -> Self { Value::Text(v) } }

impl State {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn set(&mut self, key: impl Into<String>, value: impl Into<Value>) -> &mut Self {
        self.values.insert(key.into(), value.into());
        self
    }

    pub fn get(&self, key: &str) -> Option<&Value> {
        self.values.get(key)
    }

    pub fn keys(&self) -> impl Iterator<Item = &str> {
        self.values.keys().map(String::as_str)
    }
}
```

- [ ] **Step 4: Implementar `integration.rs`**

Acrescentar acima do `#[cfg(test)]`:

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Trigger {
    /// Atualiza a cada intervalo.
    Poll(Duration),
    /// Atualiza só quando o sistema avisa (callback).
    Event,
    /// Eventos, com um polling de segurança.
    Both(Duration),
}

impl Trigger {
    pub fn poll_interval(&self) -> Option<Duration> {
        match self {
            Trigger::Poll(d) | Trigger::Both(d) => Some(*d),
            Trigger::Event => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ActionId(pub String);

#[derive(Debug, Clone, PartialEq)]
pub struct Action {
    pub id: ActionId,
    pub title: String,
}

pub type Args = BTreeMap<String, Value>;

#[derive(Debug, Clone, Copy)]
pub struct Ctx {
    pub now: Instant,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IntegrationError {
    pub message: String,
}

impl IntegrationError {
    pub fn new(message: impl Into<String>) -> Self {
        Self { message: message.into() }
    }
}

impl fmt::Display for IntegrationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.message)
    }
}

impl std::error::Error for IntegrationError {}

pub type Result<T> = std::result::Result<T, IntegrationError>;

pub trait Integration: Send + 'static {
    fn id(&self) -> &'static str;
    fn trigger(&self) -> Trigger;
    fn refresh(&mut self, ctx: &Ctx) -> Result<State>;
    fn actions(&self, state: &State) -> Vec<Action>;
    fn invoke(&mut self, action: &ActionId, args: &Args, ctx: &Ctx) -> Result<()>;
}
```

- [ ] **Step 5: Implementar `scheduler.rs`**

Acrescentar acima do `#[cfg(test)]`:

```rust
impl Scheduler {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn schedule(&mut self, id: usize, at: Instant) {
        self.heap.push(Reverse((at, id)));
    }

    pub fn next_deadline(&self) -> Option<Instant> {
        self.heap.peek().map(|Reverse((at, _))| *at)
    }

    pub fn pop_due(&mut self, now: Instant) -> Vec<usize> {
        let mut due = Vec::new();
        while let Some(Reverse((at, id))) = self.heap.peek().copied() {
            if at > now {
                break;
            }
            self.heap.pop();
            due.push(id);
        }
        due
    }
}
```

- [ ] **Step 6: Rodar os testes**

Run: `cargo test -p zefiro-core --lib`
Expected: PASS (todos os testes de unidade, incluindo os 5 novos).

- [ ] **Step 7: Commit**

```powershell
git add crates/core
git commit -m "feat(core): typed State, Integration contract and deadline scheduler" -m "Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

---

## Task 25: F3 — Supervisor de integração (isolamento de falhas)

**Files:**
- Create: `crates/core/src/supervisor.rs`
- Modify: `crates/core/src/lib.rs` (`pub mod supervisor;`)

**Interfaces:**
- Consumes: Task 24 (`Integration`, `Ctx`, `State`).
- Produces: `supervisor::{Supervisor, Outcome::{Updated(State), Failed { error: String, retry_in: Duration }}, backoff(failures: u32) -> Duration, BACKOFF_START = 1 s, BACKOFF_MAX = 300 s}`; `Supervisor::new()`, `run(&mut self, &mut dyn Integration, &Ctx) -> Outcome`, `failures() -> u32`.

- [ ] **Step 1: Escrever os testes (falhando)**

`crates/core/src/supervisor.rs`:

```rust
//! Roda `refresh` isolado: erro ou panic viram `Outcome::Failed` com espera crescente (1 s → 5 min).

use std::panic::{AssertUnwindSafe, catch_unwind};
use std::time::Duration;

use crate::integration::{Ctx, Integration};
use crate::state::State;

pub const BACKOFF_START: Duration = Duration::from_secs(1);
pub const BACKOFF_MAX: Duration = Duration::from_secs(300);

#[derive(Debug, PartialEq)]
pub enum Outcome {
    Updated(State),
    Failed { error: String, retry_in: Duration },
}

#[derive(Debug, Default)]
pub struct Supervisor { failures: u32 }

#[cfg(test)]
mod tests {
    use super::*;
    use crate::integration::{Action, ActionId, Args, IntegrationError, Result, Trigger};
    use std::time::Instant;

    enum Mode { Ok, Err, Panic }
    struct Flaky { mode: Mode }

    impl Integration for Flaky {
        fn id(&self) -> &'static str { "flaky" }
        fn trigger(&self) -> Trigger { Trigger::Event }
        fn refresh(&mut self, _ctx: &Ctx) -> Result<State> {
            match self.mode {
                Mode::Ok => { let mut s = State::new(); s.set("ok", true); Ok(s) }
                Mode::Err => Err(IntegrationError::new("token expirado")),
                Mode::Panic => panic!("índice fora do limite"),
            }
        }
        fn actions(&self, _state: &State) -> Vec<Action> { Vec::new() }
        fn invoke(&mut self, _a: &ActionId, _args: &Args, _ctx: &Ctx) -> Result<()> { Ok(()) }
    }

    fn ctx() -> Ctx { Ctx { now: Instant::now() } }

    #[test]
    fn backoff_doubles_from_one_second_up_to_five_minutes() {
        let secs: Vec<u64> = (1..=11).map(|f| backoff(f).as_secs()).collect();
        assert_eq!(secs, vec![1, 2, 4, 8, 16, 32, 64, 128, 256, 300, 300]);
        assert_eq!(backoff(1000), BACKOFF_MAX);
    }

    #[test]
    fn errors_back_off_and_success_resets() {
        let mut sup = Supervisor::new();
        let mut f = Flaky { mode: Mode::Err };
        assert_eq!(sup.run(&mut f, &ctx()), Outcome::Failed { error: "token expirado".into(), retry_in: Duration::from_secs(1) });
        assert_eq!(sup.run(&mut f, &ctx()), Outcome::Failed { error: "token expirado".into(), retry_in: Duration::from_secs(2) });
        f.mode = Mode::Ok;
        assert!(matches!(sup.run(&mut f, &ctx()), Outcome::Updated(_)));
        assert_eq!(sup.failures(), 0);
    }

    #[test]
    fn panics_are_contained_and_reported() {
        let mut sup = Supervisor::new();
        let mut f = Flaky { mode: Mode::Panic };
        match sup.run(&mut f, &ctx()) {
            Outcome::Failed { error, retry_in } => {
                assert_eq!(error, "panic: índice fora do limite");
                assert_eq!(retry_in, Duration::from_secs(1));
            }
            other => panic!("esperava Failed, veio {other:?}"),
        }
        assert_eq!(sup.failures(), 1);
    }
}
```

Em `crates/core/src/lib.rs`, acrescentar `pub mod supervisor;`.

- [ ] **Step 2: Rodar para ver falhar**

Run: `cargo test -p zefiro-core supervisor`
Expected: FAIL de compilação (`backoff`, `new`, `run`, `failures`).

- [ ] **Step 3: Implementar**

Acrescentar acima do `#[cfg(test)]`:

```rust
/// Espera depois de `failures` falhas seguidas: 1 s, 2 s, 4 s… até 5 min.
pub fn backoff(failures: u32) -> Duration {
    let exp = failures.saturating_sub(1).min(16);
    BACKOFF_START.saturating_mul(1u32 << exp).min(BACKOFF_MAX)
}

impl Supervisor {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn failures(&self) -> u32 {
        self.failures
    }

    pub fn run(&mut self, integration: &mut dyn Integration, ctx: &Ctx) -> Outcome {
        let result = catch_unwind(AssertUnwindSafe(|| integration.refresh(ctx)));
        let error = match result {
            Ok(Ok(state)) => {
                self.failures = 0;
                return Outcome::Updated(state);
            }
            Ok(Err(e)) => e.message,
            Err(payload) => {
                let msg = payload
                    .downcast_ref::<&str>()
                    .map(|s| s.to_string())
                    .or_else(|| payload.downcast_ref::<String>().cloned())
                    .unwrap_or_else(|| "sem mensagem".into());
                format!("panic: {msg}")
            }
        };
        self.failures += 1;
        tracing::warn!(integration = integration.id(), failures = self.failures, %error, "integração falhou");
        Outcome::Failed { error, retry_in: backoff(self.failures) }
    }
}
```

- [ ] **Step 4: Rodar os testes**

Run: `cargo test -p zefiro-core supervisor`
Expected: PASS (3 testes; o teste de panic imprime a mensagem padrão no stderr, isso é esperado).

- [ ] **Step 5: Commit**

```powershell
git add crates/core
git commit -m "feat(core): integration supervisor with panic isolation and exponential backoff" -m "Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

---

## Task 26: F3 — Modelo de mola portável (`zefiro-ui`)

**Files:**
- Create: `crates/ui/src/spring.rs`
- Modify: `crates/ui/src/lib.rs` (`pub mod spring;`)

**Interfaces:**
- Consumes: ADR-0006 (constantes validadas; se o ADR mudou algum valor, usar o do ADR).
- Produces: `zefiro_ui::spring::{Spring, SpringParams, SpringStyle, FIXED_DT, SETTLE_EPS, MAX_DT, sample}` — mesma API do spike (Task 5).

- [ ] **Step 1: Conferir o ADR-0006**

Abrir `docs/adr/0006-ilha-de-midia-e-limites-de-hardware.md` e anotar se as constantes `GROW` (380/26), `SHRINK` (200/28), `DEFAULT` (280/24) ou os multiplicadores de estilo mudaram. Se mudaram, usar os valores do ADR no código e nos testes abaixo.

- [ ] **Step 2: Escrever os testes (falhando)**

`crates/ui/src/spring.rs`:

```rust
//! Modelo de mola portado do Dynamic Island for Windows (MIT; ver THIRD_PARTY_NOTICES.md):
//! Euler semi-implícito com passos fixos de 0,5 ms, massa 1.

pub const FIXED_DT: f32 = 0.0005;
pub const SETTLE_EPS: f32 = 0.01;
pub const MAX_DT: f32 = 0.050;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SpringStyle { Smooth, Default, Bouncy, Snappy }

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SpringParams { pub stiffness: f32, pub damping: f32 }

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Spring { pub value: f32, pub velocity: f32, pub target: f32 }

#[cfg(test)]
mod tests {
    use super::*;

    fn run(mut s: Spring, p: SpringParams, secs: f32) -> (Spring, f32) {
        let mut max = s.value;
        let mut t = 0.0;
        while t < secs {
            s.step(1.0 / 144.0, p);
            max = max.max(s.value);
            t += 1.0 / 144.0;
        }
        (s, max)
    }

    #[test]
    fn every_style_settles_on_target_within_two_seconds() {
        for style in [SpringStyle::Smooth, SpringStyle::Default, SpringStyle::Bouncy, SpringStyle::Snappy] {
            for base in [SpringParams::GROW, SpringParams::SHRINK, SpringParams::DEFAULT] {
                let mut s = Spring::new(0.0);
                s.target = 100.0;
                let (s, _) = run(s, base.styled(style), 2.0);
                assert_eq!(s.value, 100.0, "{style:?} {base:?}");
                assert!(s.is_settled());
            }
        }
    }

    #[test]
    fn bouncy_overshoots_and_smooth_does_not() {
        let mut s = Spring::new(0.0);
        s.target = 100.0;
        let (_, bouncy_max) = run(s, SpringParams::DEFAULT.styled(SpringStyle::Bouncy), 2.0);
        let (_, smooth_max) = run(s, SpringParams::DEFAULT.styled(SpringStyle::Smooth), 2.0);
        assert!(bouncy_max > 101.0, "elástico deveria passar do alvo: {bouncy_max}");
        assert!(smooth_max < 100.1, "suave não deveria passar do alvo: {smooth_max}");
    }

    #[test]
    fn style_multipliers_match_dynamic_island() {
        let p = SpringParams::DEFAULT.styled(SpringStyle::Snappy);
        assert!((p.stiffness - 420.0).abs() < 1e-3);
        assert!((p.damping - 30.0).abs() < 1e-3);
    }

    #[test]
    fn huge_dt_is_clamped_to_50ms() {
        let mut a = Spring::new(0.0);
        a.target = 100.0;
        let mut b = a;
        a.step(10.0, SpringParams::DEFAULT);
        b.step(MAX_DT, SpringParams::DEFAULT);
        assert_eq!(a, b);
    }

    #[test]
    fn converts_to_compositor_period_and_ratio() {
        let (period, ratio) = SpringParams::DEFAULT.to_period_and_ratio();
        assert!((period - 0.37549).abs() < 1e-3, "{period}");
        assert!((ratio - 0.71714).abs() < 1e-3, "{ratio}");
    }

    #[test]
    fn sample_starts_at_from_and_ends_at_target() {
        let s = sample(0.0, 1.0, SpringParams::GROW, 1.0 / 240.0, 2.0);
        assert_eq!(s.first().copied(), Some((0.0, 0.0)));
        assert_eq!(s.last().unwrap().1, 1.0);
        assert!(s.windows(2).all(|w| w[1].0 > w[0].0));
    }
}
```

Em `crates/ui/src/lib.rs`, acrescentar `pub mod spring;`.

- [ ] **Step 3: Rodar para ver falhar**

Run: `cargo test -p zefiro-ui`
Expected: FAIL de compilação.

- [ ] **Step 4: Implementar**

Acrescentar acima do `#[cfg(test)]`:

```rust
impl SpringStyle {
    /// (multiplicador de rigidez, multiplicador de amortecimento)
    pub fn multipliers(self) -> (f32, f32) {
        match self {
            SpringStyle::Smooth => (1.0, 1.35),
            SpringStyle::Default => (1.0, 1.0),
            SpringStyle::Bouncy => (1.1, 0.70),
            SpringStyle::Snappy => (1.5, 1.25),
        }
    }
}

impl SpringParams {
    pub const GROW: Self = Self { stiffness: 380.0, damping: 26.0 };
    pub const SHRINK: Self = Self { stiffness: 200.0, damping: 28.0 };
    pub const DEFAULT: Self = Self { stiffness: 280.0, damping: 24.0 };

    pub fn styled(self, style: SpringStyle) -> Self {
        let (s, d) = style.multipliers();
        Self { stiffness: self.stiffness * s, damping: self.damping * d }
    }

    /// Converte para o modelo do compositor: (período em segundos, razão de amortecimento).
    pub fn to_period_and_ratio(self) -> (f32, f32) {
        let omega = self.stiffness.sqrt();
        (2.0 * std::f32::consts::PI / omega, self.damping / (2.0 * omega))
    }
}

impl Spring {
    pub fn new(value: f32) -> Self {
        Self { value, velocity: 0.0, target: value }
    }

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

/// Amostra a mola de `from` até `to` em passos de `step` segundos.
pub fn sample(from: f32, to: f32, p: SpringParams, step: f32, max_secs: f32) -> Vec<(f32, f32)> {
    let mut s = Spring::new(from);
    s.target = to;
    let mut out = vec![(0.0, from)];
    let mut t = 0.0;
    while t < max_secs {
        s.step(step, p);
        t += step;
        out.push((t, s.value));
        if s.is_settled() {
            break;
        }
    }
    if let Some(last) = out.last_mut() {
        last.1 = to;
    }
    out
}
```

- [ ] **Step 5: Rodar os testes**

Run: `cargo test -p zefiro-ui`
Expected: PASS (6 testes).

- [ ] **Step 6: Commit**

```powershell
git add crates/ui
git commit -m "feat(ui): portable spring model ported from Dynamic Island (MIT)" -m "Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

---

## Task 27: F3 — Contratos de plataforma (`zefiro-platform`)

**Files:**
- Create: `crates/platform/src/geometry.rs`
- Modify: `crates/platform/src/lib.rs`

**Interfaces:**
- Consumes: ADR-0001 e ADR-0006 (o que o compositor e a ilha precisaram).
- Produces:
  - `geometry::{Point { x, y }, Rect { x, y, width, height }, Rect::contains(Point) -> bool, Color { r, g, b, a }, Color::from_hex(&str) -> Option<Color>}`.
  - `PlatformError { message }`, `PlatformResult<T>`, `MonitorId(u64)`, `MonitorInfo { id, bounds, scale, primary, refresh_hz }`, `WindowId(u64)`, `WindowRole::{Bar, Island, Flyout}`, `TextStyle { size, weight, mono }`, `BackdropKind::{Acrylic, Mica, Solid}`, `AudioSink`.
  - Traits: `Monitors::list`, `WindowHost::{create, set_bounds, show, destroy}`, `Canvas::{clear, fill_rounded_rect, draw_text, draw_icon}`, `Backdrop::set`, `AudioCapture::{start, stop, is_running}`.

- [ ] **Step 1: Escrever os testes de geometria (falhando)**

`crates/platform/src/geometry.rs`:

```rust
//! Tipos geométricos e de cor, em pixels lógicos (DIP).

#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Point { pub x: f32, pub y: f32 }

#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Rect { pub x: f32, pub y: f32, pub width: f32, pub height: f32 }

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Color { pub r: u8, pub g: u8, pub b: u8, pub a: u8 }

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_rgb_and_rgba_hex() {
        assert_eq!(Color::from_hex("#9184d9"), Some(Color { r: 0x91, g: 0x84, b: 0xd9, a: 0xff }));
        assert_eq!(Color::from_hex("#161826B0"), Some(Color { r: 0x16, g: 0x18, b: 0x26, a: 0xb0 }));
    }

    #[test]
    fn rejects_invalid_hex() {
        for bad in ["", "#", "9184d9", "#12345", "#gggggg", "#1234567", "#123456789"] {
            assert_eq!(Color::from_hex(bad), None, "{bad}");
        }
    }

    #[test]
    fn rect_contains_is_half_open() {
        let r = Rect { x: 10.0, y: 10.0, width: 20.0, height: 10.0 };
        assert!(r.contains(Point { x: 10.0, y: 10.0 }));
        assert!(r.contains(Point { x: 29.9, y: 19.9 }));
        assert!(!r.contains(Point { x: 30.0, y: 15.0 }));
        assert!(!r.contains(Point { x: 15.0, y: 20.0 }));
    }
}
```

`crates/platform/src/lib.rs`:

```rust
//! Contratos (traits) que cada sistema operacional implementa.

pub mod geometry;
```

- [ ] **Step 2: Rodar para ver falhar**

Run: `cargo test -p zefiro-platform`
Expected: FAIL de compilação (`from_hex`, `contains`).

- [ ] **Step 3: Implementar a geometria**

Acrescentar acima do `#[cfg(test)]` em `geometry.rs`:

```rust
impl Rect {
    pub fn contains(&self, p: Point) -> bool {
        p.x >= self.x && p.x < self.x + self.width && p.y >= self.y && p.y < self.y + self.height
    }
}

impl Color {
    /// Aceita `#RRGGBB` ou `#RRGGBBAA`.
    pub fn from_hex(s: &str) -> Option<Color> {
        let hex = s.strip_prefix('#')?;
        if !(hex.len() == 6 || hex.len() == 8) || !hex.bytes().all(|b| b.is_ascii_hexdigit()) {
            return None;
        }
        let byte = |i: usize| u8::from_str_radix(&hex[i..i + 2], 16).ok();
        Some(Color { r: byte(0)?, g: byte(2)?, b: byte(4)?, a: if hex.len() == 8 { byte(6)? } else { 0xff } })
    }
}
```

Run: `cargo test -p zefiro-platform`
Expected: PASS (3 testes).

- [ ] **Step 4: Escrever o teste dos contratos com implementações falsas (falhando)**

Substituir `crates/platform/src/lib.rs` por:

```rust
//! Contratos (traits) que cada sistema operacional implementa.
//! As assinaturas vêm do que os spikes S1 (compositor) e S6 (ilha) precisaram.

pub mod geometry;

pub use geometry::{Color, Point, Rect};

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Default)]
    struct RecordingCanvas { calls: Vec<String> }

    impl Canvas for RecordingCanvas {
        fn clear(&mut self) { self.calls.push("clear".into()); }
        fn fill_rounded_rect(&mut self, rect: Rect, radius: f32, _color: Color) { self.calls.push(format!("pill {}x{} r{radius}", rect.width, rect.height)); }
        fn draw_text(&mut self, text: &str, _rect: Rect, style: TextStyle, _color: Color) { self.calls.push(format!("text {text} {}", style.size)); }
        fn draw_icon(&mut self, icon: &str, _rect: Rect, _color: Color) { self.calls.push(format!("icon {icon}")); }
    }

    struct OneMonitor;
    impl Monitors for OneMonitor {
        fn list(&self) -> Vec<MonitorInfo> {
            vec![MonitorInfo { id: MonitorId(1), bounds: Rect { x: 0.0, y: 0.0, width: 1920.0, height: 1080.0 }, scale: 1.0, primary: true, refresh_hz: 144 }]
        }
    }

    struct FakeCapture { running: bool }
    impl AudioCapture for FakeCapture {
        fn start(&mut self, mut sink: AudioSink) -> PlatformResult<()> { sink(&[0.0; 4], 48_000.0); self.running = true; Ok(()) }
        fn stop(&mut self) { self.running = false; }
        fn is_running(&self) -> bool { self.running }
    }

    fn paint(c: &mut dyn Canvas) {
        c.clear();
        c.fill_rounded_rect(Rect { x: 0.0, y: 0.0, width: 220.0, height: 28.0 }, 14.0, Color::from_hex("#9184d9").unwrap());
        c.draw_text("22:53", Rect::default(), TextStyle { size: 13.0, weight: 600, mono: false }, Color::from_hex("#e9e9ed").unwrap());
        c.draw_icon("fluent:volume", Rect::default(), Color::from_hex("#e9e9ed").unwrap());
    }

    #[test]
    fn traits_are_usable_as_trait_objects() {
        let mut canvas = RecordingCanvas::default();
        paint(&mut canvas);
        assert_eq!(canvas.calls, vec!["clear", "pill 220x28 r14", "text 22:53 13", "icon fluent:volume"]);

        let monitors: &dyn Monitors = &OneMonitor;
        assert_eq!(monitors.list()[0].refresh_hz, 144);

        let mut cap: Box<dyn AudioCapture> = Box::new(FakeCapture { running: false });
        cap.start(Box::new(|samples, rate| assert_eq!((samples.len(), rate), (4, 48_000.0)))).unwrap();
        assert!(cap.is_running());
        cap.stop();
        assert!(!cap.is_running());
    }
}
```

Run: `cargo test -p zefiro-platform`
Expected: FAIL de compilação (`Canvas`, `Monitors`, `AudioCapture`… não existem).

- [ ] **Step 5: Implementar os contratos**

Acrescentar em `lib.rs`, acima do `#[cfg(test)]`:

```rust
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlatformError {
    pub message: String,
}

impl std::fmt::Display for PlatformError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.message)
    }
}

impl std::error::Error for PlatformError {}

pub type PlatformResult<T> = Result<T, PlatformError>;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct MonitorId(pub u64);

#[derive(Debug, Clone, PartialEq)]
pub struct MonitorInfo {
    pub id: MonitorId,
    /// Em pixels lógicos.
    pub bounds: Rect,
    /// DPI / 96.
    pub scale: f32,
    pub primary: bool,
    pub refresh_hz: u32,
}

pub trait Monitors {
    fn list(&self) -> Vec<MonitorInfo>;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct WindowId(pub u64);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WindowRole {
    /// Reserva espaço na tela (AppBar no Windows, layer-shell no Linux).
    Bar,
    /// Sobreposta, sem foco, com área transparente que deixa o clique passar.
    Island,
    /// Painel temporário ancorado a um item.
    Flyout,
}

pub trait WindowHost {
    fn create(&mut self, role: WindowRole, monitor: MonitorId, bounds: Rect) -> PlatformResult<WindowId>;
    fn set_bounds(&mut self, id: WindowId, bounds: Rect) -> PlatformResult<()>;
    fn show(&mut self, id: WindowId, visible: bool) -> PlatformResult<()>;
    fn destroy(&mut self, id: WindowId);
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TextStyle {
    pub size: f32,
    pub weight: u16,
    pub mono: bool,
}

/// Desenho 2D. No Windows: Direct2D/DirectWrite sobre a superfície do compositor (ADR-0001).
pub trait Canvas {
    fn clear(&mut self);
    fn fill_rounded_rect(&mut self, rect: Rect, radius: f32, color: Color);
    fn draw_text(&mut self, text: &str, rect: Rect, style: TextStyle, color: Color);
    /// `icon` no formato `fonte:nome` (ex.: `fluent:volume`) ou caminho de SVG.
    fn draw_icon(&mut self, icon: &str, rect: Rect, color: Color);
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BackdropKind {
    Acrylic,
    Mica,
    Solid,
}

pub trait Backdrop {
    fn set(&mut self, window: WindowId, kind: BackdropKind, tint: Color) -> PlatformResult<()>;
}

/// Recebe amostras mono e a taxa de amostragem.
pub type AudioSink = Box<dyn FnMut(&[f32], f32) + Send>;

/// Captura do som do sistema (WASAPI loopback no Windows; ADR-0006).
pub trait AudioCapture {
    fn start(&mut self, sink: AudioSink) -> PlatformResult<()>;
    fn stop(&mut self);
    fn is_running(&self) -> bool;
}
```

Se o ADR-0001 ou o ADR-0006 registraram uma necessidade que estas assinaturas não cobrem (ex.: animação de forma no compositor, região de clique da ilha), acrescentar o método correspondente ao trait certo **com um teste na mesma linha do teste acima** e registrar o motivo no comentário do método.

- [ ] **Step 6: Rodar os testes e a checagem Linux**

Run: `cargo test -p zefiro-platform`
Expected: PASS (4 testes).

Run: `cargo check -p zefiro-platform --target x86_64-unknown-linux-gnu`
Expected: compila.

- [ ] **Step 7: Commit**

```powershell
git add crates/platform
git commit -m "feat(platform): portable platform contracts (monitors, windows, canvas, backdrop, audio capture)" -m "Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

---

## Task 28: F4 — CI e repositório no GitHub

**Files:**
- Create: `.github/workflows/ci.yml`

**Interfaces:**
- Consumes: Tasks 2, 3, 21–27.
- Produces: CI verde no Windows (fmt, clippy, testes, testes dos scripts) e no Linux (portabilidade de `core`, `ui`, `resources`, `platform`).

- [ ] **Step 1: Escrever `.github/workflows/ci.yml`**

```yaml
name: CI

on:
  push:
    branches: [main]
  pull_request:

jobs:
  windows:
    runs-on: windows-latest
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable
        with:
          components: clippy, rustfmt
      - run: cargo fmt --all -- --check
      - run: cargo clippy --workspace --all-targets -- -D warnings
      - run: cargo test --workspace
      - name: Testes dos scripts
        shell: pwsh
        run: |
          ./scripts/tests/frames.Tests.ps1
          ./scripts/tests/perf.Tests.ps1

  linux-portability:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable
      - run: cargo check -p zefiro-core -p zefiro-ui -p zefiro-resources -p zefiro-platform
      - run: cargo test -p zefiro-core -p zefiro-ui -p zefiro-resources -p zefiro-platform
```

- [ ] **Step 2: Rodar localmente o que o CI roda**

```powershell
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
pwsh -NoProfile -File scripts/tests/frames.Tests.ps1
pwsh -NoProfile -File scripts/tests/perf.Tests.ps1
cargo check -p zefiro-core -p zefiro-ui -p zefiro-resources -p zefiro-platform --target x86_64-unknown-linux-gnu
```

Expected: tudo passa. Se `cargo fmt --check` falhar, rodar `cargo fmt --all` e revisar o diff.

- [ ] **Step 3: Commit**

```powershell
git add .github/workflows/ci.yml
git commit -m "ci: add Windows build/test and Linux portability jobs" -m "Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

- [ ] **Step 4: Criar o repositório privado (só com confirmação do usuário)**

Perguntar ao usuário no chat: "Posso criar o repositório **privado** `Felipe-Cavalari/zefiro` no GitHub e enviar o `main` e as branches dos spikes?" Só com um "sim" explícito:

```powershell
gh repo create Felipe-Cavalari/zefiro --private --source . --remote origin
git push -u origin main
git push origin spike/s1-render spike/s6-island spike/s2-notifications spike/s3-ddc spike/s4-smtc spike/s5-focus-night
```

- [ ] **Step 5: Conferir o CI uma vez**

Run: `gh run list --limit 1`
Expected: a execução do push no `main` aparece. Abrir com `gh run view --web` se quiser acompanhar. Se algum job falhar, corrigir localmente, fazer commit e push (o push de correção também precisa de confirmação se for o primeiro da sessão).

---

## Task 29: Fechamento da Fase 0

**Files:**
- Modify: `docs/PLANNING.md` (§12 Fase 0 e §13 Riscos)

**Interfaces:**
- Consumes: ADRs 0001–0006, `docs/baseline.md`, Tasks 21–28.
- Produces: Fase 0 marcada como concluída e riscos atualizados; pronto para o brainstorming da Fase 1.

- [ ] **Step 1: Atualizar a tabela de riscos (§13)**

Acrescentar a coluna **"Resultado da Fase 0"** à tabela de riscos do `PLANNING.md` e preencher cada linha com um destes valores, citando o ADR: **resolvido** (funciona como planejado), **plano B** (qual), **corta** (sai do escopo e para onde vai), ou **aberto** (ex.: Win10 não testado). Linhas sem spike correspondente (licenças, escopo, linguagem de expressões) ficam **aberto — Fase 1**.

- [ ] **Step 2: Atualizar o roteiro (§12)**

Na seção "Fase 0", acrescentar uma linha **"Status: concluída em AAAA-MM-DD"** e, para cada spike, o resumo da decisão em uma linha (ex.: "S1: Windows.UI.Composition, acrílico via HostBackdropBrush, molas no compositor — ADR-0001").

- [ ] **Step 3: Conferir o critério de pronto da spec (§10)**

Marcar cada item contra a realidade:

- [ ] Pré-requisitos instalados e "hello world" com a crate `windows` rodando (Task 0).
- [ ] `docs/baseline.md` com os números do YASB (Task 4).
- [ ] ADRs 0001–0006 escritos, cada um com números medidos e uma decisão clara (Tasks 10, 16–20).
- [ ] Metas do S1 e do S6 atingidas **ou** decisão consciente registrada nos ADRs 0001 e 0006.
- [ ] `THIRD_PARTY_NOTICES.md` com a entrada do Dynamic Island (Task 16).
- [ ] F1–F4 no `main`, com CI verde nos dois sistemas (Tasks 1–3, 21–28).
- [ ] `PLANNING.md` §13 atualizado com o resultado de cada risco (Passo 1).

Se algum item não estiver cumprido, **não** marcar a fase como concluída: registrar o que falta no fim do `PLANNING.md` (§14) e avisar o usuário.

- [ ] **Step 4: Commit**

```powershell
git add docs/PLANNING.md
git commit -m "docs: close Phase 0 with spike outcomes and updated risk table" -m "Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

- [ ] **Step 5: Avisar o usuário**

Resumo curto no chat: decisões de cada ADR, números principais contra as metas, o que ficou em plano B ou aberto, e o próximo passo: brainstorming e spec da **Fase 1**. Perguntar antes de fazer push do commit de fechamento.
