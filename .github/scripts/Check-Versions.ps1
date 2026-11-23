<#
.SYNOPSIS
Zkontroluje, že verze v Cargo.toml a Cargo.lock je stejná.

.PARAMETER Tag
Volitelný git tag (např. v0.2.0); pak se musí shodovat i s verzí v souborech.
#>
param(
    [string] $Tag = ''
)

$ErrorActionPreference = 'Stop'
Set-Location (Resolve-Path (Join-Path $PSScriptRoot '..\..'))

# První "version = ..." v Cargo.toml patří do [package].
$cargo = $null
foreach ($line in Get-Content Cargo.toml) {
    if ($line -match '^\s*version\s*=\s*"([^"]+)"') { $cargo = $Matches[1]; break }
}

# V Cargo.lock je za řádkem name = "timetrek" verze téhož balíčku.
$lock = $null
$lines = Get-Content Cargo.lock
for ($i = 0; $i -lt $lines.Count - 1; $i++) {
    if ($lines[$i] -eq 'name = "timetrek"' -and $lines[$i + 1] -match '^version\s*=\s*"([^"]+)"') { $lock = $Matches[1]; break }
}

Write-Host "Cargo.toml: $cargo, Cargo.lock: $lock"
if (-not $cargo -or $cargo -ne $lock) {
    Write-Host 'CHYBA: verze v Cargo.toml a Cargo.lock se neshodují (spusť cargo build, aby se lock aktualizoval).' -ForegroundColor Red
    exit 1
}
if ($Tag -ne '' -and $Tag.TrimStart('v') -ne $cargo) {
    Write-Host "CHYBA: tag $Tag neodpovídá verzi $cargo v souborech." -ForegroundColor Red
    exit 1
}
Write-Host "Verze jsou v pořádku ($cargo)." -ForegroundColor Green
exit 0
