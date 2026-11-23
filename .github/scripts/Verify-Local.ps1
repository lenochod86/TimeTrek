<#
.SYNOPSIS
Spustí na počítači to, co by dělalo CI, aby se necommitoval červený stav.

.DESCRIPTION
1. Shoda verzí (Check-Versions.ps1).
2. Překlad všeho včetně UI ve Slintu a příkladů (cargo check) bez varování.
3. Testy v Rustu (cargo test) bez varování.

.PARAMETER SkipBuild
Přeskočí překlad (cargo check); testy se stejně přeloží.
#>
param(
    [switch] $SkipBuild
)

$ErrorActionPreference = 'Stop'
Set-Location (Resolve-Path (Join-Path $PSScriptRoot '..\..'))

# Nový shell nemusí mít cargo na PATH (instalováno přes winget); PATH se jen doplní, nepřepisuje.
$env:Path = "$env:Path;" + [Environment]::GetEnvironmentVariable('Path', 'Machine') + ';' + [Environment]::GetEnvironmentVariable('Path', 'User')

function Step([string] $name, [scriptblock] $action) {
    Write-Host "== $name" -ForegroundColor Cyan
    & $action
    if ($LASTEXITCODE -ne 0) {
        Write-Host "SELHALO: $name" -ForegroundColor Red
        exit 1
    }
}

Step 'Verze' { & ./.github/scripts/Check-Versions.ps1 }
# Varování berou jako chybu, ať se nehromadí.
$env:RUSTFLAGS = '-D warnings'
if (-not $SkipBuild) {
    Step 'Překlad (cargo check)' { cargo check --all-targets --message-format short }
}
Step 'Testy v Rustu (cargo test)' { cargo test --message-format short }

Write-Host 'Vše je zelené.' -ForegroundColor Green
exit 0
