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
