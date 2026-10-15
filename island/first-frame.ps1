<#
.SYNOPSIS
  Abre e fecha o zefiro-island.exe N vezes e lê, do log, o tempo até o
  primeiro quadro apresentado (medido pela própria ilha desde a criação do
  processo; ver zefiro::NoteFramePresented).
.EXAMPLE
  ./island/first-frame.ps1 -Runs 5
#>
param(
    [int] $Runs = 5,
    [string] $Exe = (Join-Path $PSScriptRoot 'build\zefiro-island.exe')
)
$ErrorActionPreference = 'Stop'

$log = Join-Path $env:LOCALAPPDATA ("zefiro\logs\island-{0}.log" -f (Get-Date -Format yyyyMMdd))
if (Get-Process zefiro-island -ErrorAction SilentlyContinue) {
    & $Exe --stop
    Start-Sleep -Seconds 2
}

$results = @()
for ($i = 1; $i -le $Runs; $i++) {
    $before = if (Test-Path $log) { (Get-Content $log).Count } else { 0 }
    $p = Start-Process $Exe -PassThru
    $ms = $null
    $deadline = (Get-Date).AddSeconds(10)
    while (-not $ms -and (Get-Date) -lt $deadline) {
        Start-Sleep -Milliseconds 200
        if (Test-Path $log) {
            $new = Get-Content $log | Select-Object -Skip $before
            $hit = $new | Select-String 'Primeiro quadro apresentado: (\d+) ms' | Select-Object -First 1
            if ($hit) { $ms = [int]$hit.Matches[0].Groups[1].Value }
        }
    }
    $results += $ms
    Write-Host "Execução ${i}: $ms ms"
    & $Exe --stop
    $p.WaitForExit(10000) | Out-Null
    Start-Sleep -Seconds 2
}
$sorted = $results | Where-Object { $_ } | Sort-Object
$median = $sorted[[int][math]::Floor(($sorted.Count - 1) / 2)]
Write-Host ("Primeiro quadro: mínimo {0} ms, mediana {1} ms, máximo {2} ms ({3} execuções)" -f $sorted[0], $median, $sorted[-1], $sorted.Count)
