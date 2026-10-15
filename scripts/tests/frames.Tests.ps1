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

# Pausas longas (DWM ocioso) não contam como quadro perdido quando -IdleGapMs é usado.
$idleCsv = Join-Path $env:TEMP "frames-idle-$PID.csv"
@('Application,ProcessID,MsBetweenDisplayChange','dwm.exe,1,6.06','dwm.exe,1,12.12','dwm.exe,1,250.0','dwm.exe,1,6.06') | Set-Content $idleCsv
$i = & $frames -Csv $idleCsv -RefreshHz 165 -IdleGapMs 50 | ConvertFrom-Json
Remove-Item $idleCsv
if ($i.dropped -ne 1) { throw "com IdleGapMs=50 esperava 1 perdido (12.12), veio $($i.dropped)" }
if ($i.idle_gaps -ne 1) { throw "esperava 1 pausa ociosa, veio $($i.idle_gaps)" }
Write-Host 'frames.Tests (idle): OK'

# O PresentMon 2.x escreve "NA" quando não há valor: essas linhas são ignoradas.
$naCsv = Join-Path $env:TEMP "frames-na-$PID.csv"
@('Application,ProcessID,MsBetweenDisplayChange','dwm.exe,1,NA','dwm.exe,1,6.06','dwm.exe,1,6.06') | Set-Content $naCsv
$n = & $frames -Csv $naCsv -RefreshHz 165 | ConvertFrom-Json
Remove-Item $naCsv
if ($n.frames -ne 2) { throw "esperava 2 quadros válidos (NA ignorado), veio $($n.frames)" }
Write-Host 'frames.Tests (NA): OK'
