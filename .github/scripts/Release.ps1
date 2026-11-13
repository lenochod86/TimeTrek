<#
.SYNOPSIS
Vydá verzi: ověří stav, vytvoří a pushne tag vX.Y.Z. Instalátor pak sestaví a GitHub Release
vytvoří GitHub Actions na serveru (.github/workflows/release.yml), ne tento počítač.

.DESCRIPTION
Verze se bere z tauri.conf.json (musí být zvýšená, zacommitovaná a pushnutá).
Tag a release jsou venku viditelné, proto skript pouštěj jen na výslovné přání.

.PARAMETER NoWatch
Nečeká na dokončení buildu na serveru.
#>
param(
    [switch] $NoWatch
)

$ErrorActionPreference = 'Stop'
Set-Location (Resolve-Path (Join-Path $PSScriptRoot '..\..'))

if (git status --porcelain) { throw 'Pracovní strom má necommitnuté změny.' }
git fetch --quiet origin
if ((git rev-parse HEAD) -ne (git rev-parse origin/main)) { throw 'HEAD se neshoduje s origin/main (pushni, nebo si stáhni změny).' }

& ./.github/scripts/Check-Versions.ps1
if ($LASTEXITCODE -ne 0) { exit 1 }

$version = (Get-Content src-tauri/tauri.conf.json -Raw | ConvertFrom-Json).version
$tag = "v$version"
if (git tag --list $tag) { throw "Tag $tag už existuje; zvyš verzi." }
if (git ls-remote --tags origin $tag) { throw "Tag $tag už existuje na GitHubu; zvyš verzi." }

git tag $tag
git push origin $tag
Write-Host "Tag $tag pushnut, instalátor se sestavuje na serveru." -ForegroundColor Green

if (-not $NoWatch) {
    Start-Sleep -Seconds 8
    $id = gh run list --workflow release.yml --branch $tag --limit 1 --json databaseId --jq '.[0].databaseId'
    if ($id) {
        gh run watch $id --exit-status
        if ($LASTEXITCODE -ne 0) { throw "Build na serveru selhal (gh run view $id --log-failed)." }
        Write-Host "Vydáno: https://github.com/lenochod86/TimeTrek/releases/tag/$tag" -ForegroundColor Green
    } else {
        Write-Host 'Běh workflow jsem zatím nenašel; sleduj záložku Actions na GitHubu.' -ForegroundColor Yellow
    }
}
