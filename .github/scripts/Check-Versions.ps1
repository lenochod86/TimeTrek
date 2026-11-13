<#
.SYNOPSIS
Zkontroluje, že verze v package.json, src-tauri/Cargo.toml a src-tauri/tauri.conf.json je stejná.

.PARAMETER Tag
Volitelný git tag (např. v0.2.0); pak se musí shodovat i s verzí v souborech.
#>
param(
    [string] $Tag = ''
)

$ErrorActionPreference = 'Stop'
Set-Location (Resolve-Path (Join-Path $PSScriptRoot '..\..'))

$pkg = (Get-Content package.json -Raw | ConvertFrom-Json).version
$tauri = (Get-Content src-tauri/tauri.conf.json -Raw | ConvertFrom-Json).version

# První "version = ..." v Cargo.toml patří do [package].
$cargo = $null
foreach ($line in Get-Content src-tauri/Cargo.toml) {
    if ($line -match '^\s*version\s*=\s*"([^"]+)"') { $cargo = $Matches[1]; break }
}

Write-Host "package.json: $pkg, Cargo.toml: $cargo, tauri.conf.json: $tauri"
$ok = ($pkg -eq $cargo) -and ($pkg -eq $tauri)
if (-not $ok) {
    Write-Host 'CHYBA: verze v package.json, Cargo.toml a tauri.conf.json se neshodují.' -ForegroundColor Red
    exit 1
}
if ($Tag -ne '' -and $Tag.TrimStart('v') -ne $pkg) {
    Write-Host "CHYBA: tag $Tag neodpovídá verzi $pkg v souborech." -ForegroundColor Red
    exit 1
}
Write-Host "Verze jsou v pořádku ($pkg)." -ForegroundColor Green
exit 0
