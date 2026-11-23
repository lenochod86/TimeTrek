<#
.SYNOPSIS
Sestaví instalátor: release build a balíček NSIS -> target\release\bundle\nsis\TimeTrek_<verze>_x64-setup.exe

.DESCRIPTION
Potřebuje Rust (MSVC) a NSIS (winget install NSIS.NSIS). Verze se bere z Cargo.toml.
#>
$ErrorActionPreference = 'Stop'
Set-Location (Resolve-Path (Join-Path $PSScriptRoot '..'))
$env:Path = "$env:Path;" + [Environment]::GetEnvironmentVariable('Path', 'Machine') + ';' + [Environment]::GetEnvironmentVariable('Path', 'User')

$version = $null
foreach ($line in Get-Content Cargo.toml) {
    if ($line -match '^\s*version\s*=\s*"([^"]+)"') { $version = $Matches[1]; break }
}
if (-not $version) { throw 'Verzi se nepodařilo přečíst z Cargo.toml.' }

$makensis = (Get-Command makensis -ErrorAction SilentlyContinue).Source
if (-not $makensis) {
    foreach ($dir in @("${env:ProgramFiles(x86)}\NSIS", "$env:ProgramFiles\NSIS")) {
        if (Test-Path "$dir\makensis.exe") { $makensis = "$dir\makensis.exe"; break }
    }
}
if (-not $makensis) { throw 'Chybí NSIS (makensis). Nainstaluj: winget install NSIS.NSIS' }

cargo build --release
if ($LASTEXITCODE -ne 0) { throw 'cargo build --release selhal.' }

$out = Join-Path (Get-Location) 'target\release\bundle\nsis'
New-Item -ItemType Directory -Force $out | Out-Null
& $makensis "/DVERSION=$version" "/DEXE_PATH=$(Resolve-Path target\release\timetrek.exe)" "/DOUT_DIR=$out" installer\timetrek.nsi
if ($LASTEXITCODE -ne 0) { throw 'makensis selhal.' }
Write-Host "Hotovo: $out\TimeTrek_${version}_x64-setup.exe" -ForegroundColor Green
