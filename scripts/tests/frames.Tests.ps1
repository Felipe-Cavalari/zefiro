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
