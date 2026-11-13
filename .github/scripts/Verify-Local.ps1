<#
.SYNOPSIS
Spustí na počítači to, co by dělalo CI, aby se necommitoval červený stav a nepálily se
placené minuty GitHub Actions (repozitář je privátní).

.DESCRIPTION
1. Shoda verzí (Check-Versions.ps1).
2. Typy frontendu (tsc).
3. Sestavení frontendu (vite build).
4. Testy v Rustu (cargo test).

.PARAMETER SkipBuild
Přeskočí sestavení frontendu (vite build).
#>
param(
    [switch] $SkipBuild
)

$ErrorActionPreference = 'Stop'
Set-Location (Resolve-Path (Join-Path $PSScriptRoot '..\..'))

# Nový shell nemusí mít node/cargo na PATH (instalováno přes winget).
$env:Path = [Environment]::GetEnvironmentVariable('Path', 'Machine') + ';' + [Environment]::GetEnvironmentVariable('Path', 'User')

function Step([string] $name, [scriptblock] $action) {
    Write-Host "== $name" -ForegroundColor Cyan
    & $action
    if ($LASTEXITCODE -ne 0) {
        Write-Host "SELHALO: $name" -ForegroundColor Red
        exit 1
    }
}

Step 'Verze' { & ./.github/scripts/Check-Versions.ps1 }
Step 'Typy frontendu (tsc)' { npx tsc --noEmit }
if (-not $SkipBuild) {
    Step 'Sestavení frontendu (vite)' { npx vite build }
}
Step 'Testy v Rustu (cargo test)' {
    Push-Location src-tauri
    try { cargo test --message-format short } finally { Pop-Location }
}

Write-Host 'Vše je zelené.' -ForegroundColor Green
exit 0
