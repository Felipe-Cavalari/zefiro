<#
.SYNOPSIS
  Conta quadros perdidos num CSV do PresentMon.
.EXAMPLE
  PresentMon --process_name dwm.exe --output_file out.csv --timed 30 --terminate_after_timed
  ./frames.ps1 -Csv out.csv -RefreshHz 144
#>
param(
    [Parameter(Mandatory)] [string] $Csv,
    [Parameter(Mandatory)] [double] $RefreshHz,
    [string] $Column = 'MsBetweenDisplayChange',
    [string] $Process = 'dwm.exe'
)
$ErrorActionPreference = 'Stop'
$rows = Import-Csv $Csv
if (-not $rows -or -not ($rows[0].PSObject.Properties.Name -contains $Column)) {
    throw "Coluna '$Column' não encontrada em $Csv."
}
$values = $rows | Where-Object { $_.Application -ieq $Process } |
    ForEach-Object { [double]::Parse($_.$Column, [Globalization.CultureInfo]::InvariantCulture) } |
    Where-Object { $_ -gt 0 }
$period = 1000.0 / $RefreshHz
$sorted = $values | Sort-Object
function Pct([double[]] $s, [double] $p) {
    if ($s.Count -eq 0) { return 0 }
    $i = [math]::Min($s.Count - 1, [math]::Max(0, [int][math]::Ceiling($p * $s.Count) - 1))
    return $s[$i]
}
$p50 = if ($sorted.Count % 2) { $sorted[[int][math]::Floor($sorted.Count / 2)] } else { ($sorted[$sorted.Count / 2 - 1] + $sorted[$sorted.Count / 2]) / 2 }
[ordered]@{
    frames  = $values.Count
    dropped = @($values | Where-Object { $_ -gt 1.5 * $period }).Count
    p50_ms  = [math]::Round($p50, 3)
    p99_ms  = [math]::Round((Pct $sorted 0.99), 3)
} | ConvertTo-Json
