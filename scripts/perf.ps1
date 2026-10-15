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
