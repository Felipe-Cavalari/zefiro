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
