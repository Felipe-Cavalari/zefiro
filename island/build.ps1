<#
.SYNOPSIS
  Compila island/build/zefiro-island.exe com o MSVC (VS 2022 Build Tools).
.EXAMPLE
  ./island/build.ps1            # Release
  ./island/build.ps1 -Debug     # sem otimização, para depurar
#>
param([switch] $Debug)
$ErrorActionPreference = 'Stop'

$here = $PSScriptRoot
$build = Join-Path $here 'build'
$gen = Join-Path $build 'gen'
$obj = Join-Path $build 'obj'
New-Item -ItemType Directory -Force $gen, $obj | Out-Null

# Defaults do island.ini, tirados do próprio mod.
& (Join-Path $here 'gen-settings.ps1') -Source (Join-Path $here 'dynamic-island-for-windows.wh.cpp') `
    -Out (Join-Path $gen 'settings_defaults.inc')

$vswhere = Join-Path ${env:ProgramFiles(x86)} 'Microsoft Visual Studio\Installer\vswhere.exe'
$vs = & $vswhere -products * -latest -requires Microsoft.VisualStudio.Component.VC.Tools.x86.x64 -property installationPath
if (-not $vs) { throw 'MSVC não encontrado (instale o VS 2022 Build Tools com "Desenvolvimento para desktop com C++").' }
$vcvars = Join-Path $vs 'VC\Auxiliary\Build\vcvars64.bat'

# Mesmas bibliotecas do cabeçalho @compilerOptions do mod.
$libs = 'ole32 oleaut32 shcore d2d1 dwrite dwmapi gdi32 user32 shell32 runtimeobject windowscodecs avrt setupapi winhttp pdh winmm advapi32' -split ' ' | ForEach-Object { "$_.lib" }

$opt = if ($Debug) { '/Od', '/RTC1', '/MTd' } else { '/O2', '/MT', '/GL' }
$ltcg = if ($Debug) { @() } else { '/LTCG' }
$cl = @(
    '/nologo', '/std:c++20', '/permissive-', '/EHsc', '/utf-8', '/bigobj',
    '/Zc:__cplusplus', '/Zc:preprocessor', '/W3', '/wd5105', '/Zi', '/MP',
    '/DZEFIRO_ISLAND', '/DUNICODE', '/D_UNICODE', '/D_CRT_SECURE_NO_WARNINGS',
    "/I`"$gen`"", "/Fo`"$obj\\`"", "/Fd`"$obj\\vc.pdb`"", "/Fe`"$build\zefiro-island.exe`""
) + $opt + @(
    "`"$here\zefiro_island_main.cpp`"", "`"$here\wh_shim.cpp`"",
    '/link', '/SUBSYSTEM:WINDOWS', '/DEBUG', '/OPT:REF', '/OPT:ICF', '/MANIFEST:EMBED',
    "/MANIFESTINPUT:`"$here\zefiro-island.manifest`"", "/PDB:`"$build\zefiro-island.pdb`""
) + $ltcg + $libs

$cmd = "call `"$vcvars`" >nul && cl $($cl -join ' ')"
$sw = [Diagnostics.Stopwatch]::StartNew()
cmd /c $cmd
if ($LASTEXITCODE -ne 0) { throw "Falha na compilação (código $LASTEXITCODE)." }
Write-Host ("zefiro-island.exe compilado em {0:N0} s -> {1}" -f $sw.Elapsed.TotalSeconds, (Join-Path $build 'zefiro-island.exe'))
