$ErrorActionPreference = 'Stop'
$root = Split-Path -Parent (Split-Path -Parent $PSCommandPath)
$out = Join-Path $env:TEMP "perf-cpu-$PID.json"
# Processo que gira um núcleo inteiro por ~15 s: a CPU medida precisa ser claramente > 0.
$busy = Start-Process pwsh -ArgumentList '-NoProfile', '-Command', '$e=(Get-Date).AddSeconds(15); while((Get-Date) -lt $e){}' -WindowStyle Hidden -PassThru
try {
    & (Join-Path $root 'perf.ps1') -Process 'pwsh' -WarmupSeconds 1 -SampleSeconds 6 -IntervalSeconds 2 -Out $out | Out-Null
    $r = Get-Content $out -Raw | ConvertFrom-Json
    $min = 100.0 / [Environment]::ProcessorCount * 0.5
    if ($r.cpu_percent_avg -lt $min) { throw "CPU de um núcleo ocupado deveria passar de $min %, veio $($r.cpu_percent_avg)" }
    Write-Host "perf-cpu.Tests: OK ($($r.cpu_percent_avg) %)"
} finally {
    Stop-Process -Id $busy.Id -ErrorAction SilentlyContinue
    Remove-Item $out -ErrorAction SilentlyContinue
}
