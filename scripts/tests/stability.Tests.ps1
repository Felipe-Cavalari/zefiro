$ErrorActionPreference = 'Stop'
$root = Split-Path -Parent (Split-Path -Parent $PSCommandPath)
$stability = Join-Path $root 'stability.ps1'

$dir = Join-Path $env:TEMP "stability-$PID"
New-Item -ItemType Directory -Force -Path $dir | Out-Null
$since = [datetime]'2026-10-22'
# Logs em UTC, como o tracing grava. A barra que abriu antes do início não conta.
@(
    '2026-10-21T23:00:00.000000Z  INFO zefiro: primeiro quadro da barra ms=50 monitors=2',
    '2026-10-22T12:02:52.144009Z  INFO zefiro: primeiro quadro da barra ms=56 monitors=2',
    '2026-10-22T13:00:00.000000Z  INFO zefiro::event_loop: painel abrindo monitor=0 chip=claude'
) | Set-Content (Join-Path $dir 'zefiro.2026-10-22.log')
'2026-10-23T09:00:00.000000Z  INFO zefiro: primeiro quadro da barra ms=41 monitors=2' | Set-Content (Join-Path $dir 'zefiro.2026-10-23.log')
# Eventos do Windows: antes do início, de outro programa e da ilha.
$events = Join-Path $dir 'events.json'
@(
    @{ time = '2026-10-21T23:59:51-03:00'; id = 1002; app = 'zefiro.exe' },
    @{ time = '2026-10-23T10:00:00-03:00'; id = 1000; app = 'brave.exe' }
) | ConvertTo-Json | Set-Content $events

try {
    $r = & $stability -Since $since -Now ([datetime]'2026-10-25 12:00') -LogDir $dir -EventsJson $events 6>$null | ConvertFrom-Json
    if ($r.day -ne 4) { throw "esperava o dia 4, veio $($r.day)" }
    if ($r.starts -ne 2) { throw "esperava 2 inícios depois de 22/10, veio $($r.starts)" }
    if ($r.incidents.Count -ne 0) { throw "esperava 0 incidentes, veio $($r.incidents.Count)" }
    if (-not $r.ok -or $r.done) { throw "esperava ok sem terminar: ok=$($r.ok) done=$($r.done)" }
    Write-Host 'stability.Tests (limpo): OK'

    # Um panic (crash-<segundos>-<ms>-<pid>.txt), um travamento da barra e um crash da ilha antiga.
    $panicAt = [DateTimeOffset]::new([datetime]'2026-10-24 08:00', [timespan]::Zero).ToUnixTimeSeconds()
    'zefiro 1.2.0' | Set-Content (Join-Path $dir "crash-$panicAt-123-4242.txt")
    @(
        @{ time = '2026-10-21T23:59:51-03:00'; id = 1002; app = 'zefiro.exe' },
        @{ time = '2026-10-23T10:00:00-03:00'; id = 1002; app = 'zefiro.exe' },
        @{ time = '2026-10-24T18:30:00-03:00'; id = 1000; app = 'zefiro-island.exe' }
    ) | ConvertTo-Json | Set-Content $events
    $c = & $stability -Since $since -Now ([datetime]'2026-10-25 12:00') -LogDir $dir -EventsJson $events 6>$null | ConvertFrom-Json
    $kinds = ($c.incidents | ForEach-Object { $_.kind }) -join ','
    # A ilha C++ saiu (ADR-0026): um crash de um zefiro-island.exe antigo não conta.
    if ($kinds -ne 'travamento,panic') { throw "esperava travamento,panic em ordem, veio '$kinds'" }
    if ($c.incidents | Where-Object { $_.app -eq 'zefiro-island.exe' }) { throw 'o crash da ilha antiga não conta mais' }
    if ($c.ok -or $c.done) { throw "com incidentes, ok e done precisam ser false: ok=$($c.ok) done=$($c.done)" }
    Write-Host 'stability.Tests (incidentes): OK'

    # 14 dias sem incidentes fecham a contagem.
    Remove-Item (Join-Path $dir 'crash-*.txt')
    '[]' | Set-Content $events
    $d = & $stability -Since $since -Now ([datetime]'2026-11-05 00:00') -LogDir $dir -EventsJson $events 6>$null | ConvertFrom-Json
    if (-not $d.done) { throw "com 14 dias limpos a contagem termina, veio done=$($d.done) dia=$($d.day)" }
    Write-Host 'stability.Tests (fim): OK'
} finally {
    Remove-Item -Recurse -Force $dir
}
