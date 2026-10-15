$ErrorActionPreference = 'Stop'
$root = Split-Path -Parent (Split-Path -Parent $PSCommandPath)
$script = Join-Path $root 'check-windows-imports.ps1'

# Aceita o workspace atual.
& pwsh -NoProfile -File $script -Root (Split-Path -Parent $root)
if ($LASTEXITCODE -ne 0) { throw "o workspace atual deveria passar (saiu $LASTEXITCODE)" }

# Rejeita uma crate portável que depende de windows.
$tmp = Join-Path ([IO.Path]::GetTempPath()) ("zefiro-check-" + [guid]::NewGuid())
New-Item -ItemType Directory -Force (Join-Path $tmp 'crates\ui') | Out-Null
Set-Content (Join-Path $tmp 'crates\ui\Cargo.toml') "[package]`nname = `"zefiro-ui`"`n[dependencies]`nwindows.workspace = true`n"
try {
    & pwsh -NoProfile -File $script -Root $tmp
    if ($LASTEXITCODE -ne 1) { throw "uma crate portável com windows deveria falhar (saiu $LASTEXITCODE)" }
} finally {
    Remove-Item -Recurse -Force $tmp
}
Write-Host 'check-windows-imports: 2/2 ok'
