param([switch]$Verify)
$ErrorActionPreference = 'Stop'
Push-Location (Join-Path $PSScriptRoot '../..')
try {
    cargo run -p zefiro-platform-windows --example display_contract
    if ($LASTEXITCODE -ne 0) { throw 'Falha na sonda de identidade de monitores.' }
    if ($Verify) {
        cargo test -p zefiro-platform-windows --test display_identity
        if ($LASTEXITCODE -ne 0) { throw 'Identidades inválidas ou ambíguas.' }
    }
} finally { Pop-Location }
