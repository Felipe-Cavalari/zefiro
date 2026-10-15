<#
.SYNOPSIS
  Gera a tabela de configurações padrão a partir do bloco
  "// ==WindhawkModSettings==" do mod. Assim os defaults do island.ini
  acompanham o upstream sem cópia manual.
#>
param(
    [Parameter(Mandatory)] [string] $Source,
    [Parameter(Mandatory)] [string] $Out
)
$ErrorActionPreference = 'Stop'

$lines = [System.IO.File]::ReadAllLines($Source, [System.Text.Encoding]::UTF8)
$start = [Array]::IndexOf($lines, '// ==WindhawkModSettings==')
$end = [Array]::IndexOf($lines, '// ==/WindhawkModSettings==')
if ($start -lt 0 -or $end -lt $start) { throw "Bloco de configurações não encontrado em $Source" }

function Unquote([string] $v) {
    $v = $v.Trim()
    if ($v.Length -ge 2 -and (($v[0] -eq "'" -and $v[-1] -eq "'") -or ($v[0] -eq '"' -and $v[-1] -eq '"'))) {
        return $v.Substring(1, $v.Length - 2)
    }
    return $v
}

function CppString([string] $v) {
    if ($null -eq $v) { $v = '' }
    return 'L"' + ($v -replace '\\', '\\' -replace '"', '\"') + '"'
}

$items = [System.Collections.Generic.List[object]]::new()
$group = ''
$current = $null
foreach ($line in $lines[($start + 1)..($end - 1)]) {
    if ($line -match '^- (\w+):\s*(.*)$') {
        if ($Matches[2] -eq '') { $group = $Matches[1]; $current = $null; continue }
        $group = ''
        $current = [ordered]@{ Group = ''; Key = $Matches[1]; Value = (Unquote $Matches[2]); Label = ''; Options = @() }
        $items.Add($current); continue
    }
    if ($line -match '^  - (\w+):\s*(.*)$' -and $group) {
        $current = [ordered]@{ Group = $group; Key = $Matches[1]; Value = (Unquote $Matches[2]); Label = ''; Options = @() }
        $items.Add($current); continue
    }
    if ($line -match '^    \$name:\s*(.*)$' -and $current) { $current.Label = Unquote $Matches[1]; continue }
    if ($line -match '^      - (.+?):\s*(.*)$' -and $current) { $current.Options += (Unquote $Matches[1]); continue }
}
if ($items.Count -eq 0) { throw 'Nenhuma configuração encontrada.' }

$sb = [System.Text.StringBuilder]::new()
[void]$sb.AppendLine('// Gerado por island/gen-settings.ps1 a partir de dynamic-island-for-windows.wh.cpp. Não editar.')
foreach ($i in $items) {
    $opts = ($i.Options -join ' | ')
    [void]$sb.AppendLine("{$(CppString $i.Group), $(CppString $i.Key), $(CppString $i.Value), $(CppString $i.Label), $(CppString $opts)},")
}
New-Item -ItemType Directory -Force (Split-Path $Out) | Out-Null
[System.IO.File]::WriteAllText($Out, $sb.ToString(), [System.Text.UTF8Encoding]::new($false))
Write-Host "gen-settings: $($items.Count) configurações -> $Out"
