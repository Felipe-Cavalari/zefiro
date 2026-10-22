<#
.SYNOPSIS
  Confere as 2 semanas de uso diário sem crash do fim da Fase 1 e grava JSON na saída.
.DESCRIPTION
  Conta como incidente, desde -Since:
    - panic: um crash-<segundos>-<ms>-<pid>.txt na pasta de logs (gancho de panic do Rust);
    - crash: evento 1000 (Application Error) do zefiro.exe ou do zefiro-island.exe;
    - travamento: evento 1002 (Application Hang), o "parou de responder" que o Windows fecha.
  Os inícios da barra vêm da linha "primeiro quadro da barra" dos logs. Com um incidente, a contagem
  recomeça: rode de novo com -Since no dia seguinte e atualize o PLANNING.
#>
param(
    [datetime] $Since = [datetime]'2026-10-22',
    [datetime] $Now = (Get-Date),
    [string] $LogDir = (Join-Path $env:LOCALAPPDATA 'zefiro\logs'),
    # Para os testes: eventos [{time, id, app, version}] no lugar do log de eventos do Windows.
    [string] $EventsJson
)
$ErrorActionPreference = 'Stop'
$goalDays = 14
$apps = @('zefiro.exe', 'zefiro-island.exe')
$sinceUtc = $Since.ToUniversalTime()
$nowUtc = $Now.ToUniversalTime()
$inv = [Globalization.CultureInfo]::InvariantCulture

function ConvertTo-Utc($value) {
    if ($value -is [datetime]) { return $value.ToUniversalTime() }
    return [DateTimeOffset]::Parse([string]$value, $inv).UtcDateTime
}

function Get-AppEvents {
    if ($EventsJson) {
        return @(Get-Content -Raw $EventsJson | ConvertFrom-Json) | ForEach-Object {
            [pscustomobject]@{ At = ConvertTo-Utc $_.time; Id = [int]$_.id; App = [string]$_.app; Version = $_.version }
        }
    }
    try {
        $raw = Get-WinEvent -FilterHashtable @{ LogName = 'Application'; Id = 1000, 1002; StartTime = $Since } -ErrorAction Stop
    } catch {
        # Get-WinEvent falha quando não há nenhum evento no filtro.
        if ($_.FullyQualifiedErrorId -like 'NoMatchingEventsFound*') { return @() }
        throw
    }
    $raw | Where-Object { $_.ProviderName -in 'Application Error', 'Application Hang' } | ForEach-Object {
        [pscustomobject]@{ At = $_.TimeCreated.ToUniversalTime(); Id = $_.Id; App = [string]$_.Properties[0].Value; Version = [string]$_.Properties[1].Value }
    }
}

$incidents = [System.Collections.Generic.List[object]]::new()

foreach ($e in @(Get-AppEvents)) {
    if ($e.At -lt $sinceUtc -or $e.At -gt $nowUtc -or $e.App -notin $apps) { continue }
    $kind = if ($e.Id -eq 1002) { 'travamento' } else { 'crash' }
    $incidents.Add([pscustomobject]@{ at = $e.At; kind = $kind; app = $e.App; version = $e.Version; source = "evento $($e.Id)" })
}

$logs = @(Get-ChildItem -Path $LogDir -Filter 'zefiro.*.log' -ErrorAction SilentlyContinue)
foreach ($f in @(Get-ChildItem -Path $LogDir -Filter 'crash-*.txt' -ErrorAction SilentlyContinue)) {
    if ($f.Name -notmatch '^crash-(\d+)-\d+-\d+\.txt$') { continue }
    $at = [DateTimeOffset]::FromUnixTimeSeconds([long]$Matches[1]).UtcDateTime
    if ($at -lt $sinceUtc -or $at -gt $nowUtc) { continue }
    $version = (Get-Content -TotalCount 1 $f.FullName) -replace '^zefiro\s+', ''
    $incidents.Add([pscustomobject]@{ at = $at; kind = 'panic'; app = 'zefiro.exe'; version = $version; source = $f.Name })
}

$starts = 0
foreach ($m in @($logs | Select-String -SimpleMatch 'primeiro quadro da barra')) {
    $at = ConvertTo-Utc ($m.Line -split '\s+', 2)[0]
    if ($at -ge $sinceUtc -and $at -le $nowUtc) { $starts++ }
}

$sorted = @($incidents | Sort-Object at)
$elapsed = $nowUtc - $sinceUtc
$day = [int][math]::Floor($elapsed.TotalDays) + 1
$ok = $sorted.Count -eq 0
$oldest = $logs | Sort-Object Name | Select-Object -First 1

$result = [ordered]@{
    since     = $Since.ToString('yyyy-MM-dd')
    day       = $day
    goal_days = $goalDays
    starts    = $starts
    logs_from = if ($oldest) { $oldest.Name -replace '^zefiro\.|\.log$', '' } else { $null }
    incidents = @($sorted | ForEach-Object {
        [ordered]@{ at = $_.at.ToString('o'); kind = $_.kind; app = $_.app; version = $_.version; source = $_.source }
    })
    ok        = $ok
    done      = $ok -and $elapsed.TotalDays -ge $goalDays
}

if ($result.done) {
    Write-Host "Fase 1: $goalDays dias sem crash desde $($result.since) ($starts inícios da barra)."
} elseif ($ok) {
    Write-Host "Dia $day de $goalDays desde $($result.since): 0 crashes, $starts inícios da barra."
} else {
    Write-Host "Dia $day de ${goalDays}: $($sorted.Count) incidente(s) desde $($result.since); a contagem recomeça."
    foreach ($i in $sorted) { Write-Host ("  {0:dd/MM HH:mm}  {1,-10} {2} {3} ({4})" -f $i.at.ToLocalTime(), $i.kind, $i.app, $i.version, $i.source) }
}
$result | ConvertTo-Json -Depth 4
