<#
.SYNOPSIS
  Compila o Zefiro em release e gera o instalador dist\Zefiro-<versão>-setup.exe (Inno Setup 6).

.DESCRIPTION
  A versão vem de [workspace.package] no Cargo.toml. O zefiro-island.exe (ilha C++, só para
  engine = "externa") entra se existir em target\release; -BuildIsland recompila antes (MSVC).
  Com o Zefiro aberto a partir de target\release, o exe em uso é renomeado para a compilação passar.

.EXAMPLE
  pwsh -File scripts/package.ps1
#>
param([switch]$BuildIsland)
$ErrorActionPreference = 'Stop'
$root = Split-Path $PSScriptRoot -Parent
Set-Location $root

$version = (Select-String -Path Cargo.toml -Pattern '^version\s*=\s*"([^"]+)"' | Select-Object -First 1).Matches[0].Groups[1].Value
Write-Host "Zefiro $version"

$iscc = @("$env:LOCALAPPDATA\Programs\Inno Setup 6\ISCC.exe", "${env:ProgramFiles(x86)}\Inno Setup 6\ISCC.exe") | Where-Object { Test-Path $_ } | Select-Object -First 1
if (-not $iscc) { throw 'Inno Setup 6 não encontrado. Instale com: winget install JRSoftware.InnoSetup' }

# O exe em uso não pode ser sobrescrito, mas pode ser renomeado.
$exe = 'target\release\zefiro.exe'
if (Test-Path $exe) {
    Remove-Item "$exe.old" -ErrorAction SilentlyContinue
    try { Rename-Item $exe 'zefiro.exe.old' } catch { Write-Warning "não consegui renomear $exe; a compilação pode falhar" }
}
cargo build --release -p zefiro-app
if ($LASTEXITCODE -ne 0) { throw 'cargo build falhou' }

if ($BuildIsland) {
    & ./island/build.ps1
    if ($LASTEXITCODE -ne 0) { throw 'build da ilha falhou' }
    Copy-Item island/build/zefiro-island.exe target/release/ -Force
}

& $iscc "/DAppVersion=$version" "/DBuildDir=$root\target\release" installer\zefiro.iss
if ($LASTEXITCODE -ne 0) { throw 'ISCC falhou' }
$setup = Join-Path $root "dist\Zefiro-$version-setup.exe"
Write-Host "Instalador: $setup ($([math]::Round((Get-Item $setup).Length / 1MB, 1)) MB)"
