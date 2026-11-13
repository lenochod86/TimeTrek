<#
.SYNOPSIS
Vydá verzi bez placených minut Actions: ověří, sestaví instalátor na tomto počítači,
vytvoří tag vX.Y.Z a GitHub Release s instalátorem.

.DESCRIPTION
Verze se bere z tauri.conf.json (musí být zvýšená a zacommitovaná i pushnutá).
Skript vytváří veřejně viditelnou věc (tag + release), proto ho pouštěj jen na výslovné přání.

.PARAMETER Draft
Vytvoří release jako koncept (nepublikovaný).
#>
param(
    [switch] $Draft
)

$ErrorActionPreference = 'Stop'
Set-Location (Resolve-Path (Join-Path $PSScriptRoot '..\..'))
$env:Path = [Environment]::GetEnvironmentVariable('Path', 'Machine') + ';' + [Environment]::GetEnvironmentVariable('Path', 'User')

if (git status --porcelain) { throw 'Pracovní strom má necommitnuté změny.' }
git fetch --quiet origin
if ((git rev-parse HEAD) -ne (git rev-parse origin/main)) { throw 'HEAD se neshoduje s origin/main (pushni, nebo si stáhni změny).' }

& ./.github/scripts/Verify-Local.ps1 -SkipBuild
if ($LASTEXITCODE -ne 0) { exit 1 }

$version = (Get-Content src-tauri/tauri.conf.json -Raw | ConvertFrom-Json).version
$tag = "v$version"
if (git tag --list $tag) { throw "Tag $tag už existuje; zvyš verzi." }

npx tauri build
if ($LASTEXITCODE -ne 0) { throw 'Build instalátoru selhal.' }

$installer = "src-tauri/target/release/bundle/nsis/TimeTrek_${version}_x64-setup.exe"
if (-not (Test-Path $installer)) { throw "Instalátor $installer nenalezen." }

git tag $tag
git push origin $tag
$args = @('release', 'create', $tag, $installer, '--title', "TimeTrek $version", '--generate-notes')
if ($Draft) { $args += '--draft' }
gh @args
Write-Host "Vydáno: $tag" -ForegroundColor Green
