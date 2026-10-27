<#
.SYNOPSIS
    Bump Palimpsest's version everywhere it is declared, then tag and push.

.DESCRIPTION
    `release.yml` refuses to build unless the pushed tag matches BOTH
    `Cargo.toml` and `crates/palimpsest-app/tauri.conf.json`, because a tag that
    disagrees would ship an update that either never offers itself or offers
    itself forever. Keeping three numbers in step by hand is how that check ends
    up failing after the tag is already public, so this does all three together.

    The CI guard stays exactly as it is — this script makes the common path
    correct, it does not replace the safety net.

.PARAMETER Part
    Which component to increment: patch (default), minor, or major.

.PARAMETER Version
    An explicit x.y.z to set instead of incrementing. Overrides -Part.

.PARAMETER NoPush
    Commit and tag locally but do not push. Use when you want to inspect the
    commit first; `git push origin main v<version>` finishes the job.

.PARAMETER DryRun
    Print what would change and exit without touching anything.

.EXAMPLE
    ./scripts/bump.ps1                    # 0.3.0 -> 0.3.1, tag, push
    ./scripts/bump.ps1 -Part minor        # 0.3.0 -> 0.4.0
    ./scripts/bump.ps1 -Version 1.0.0
    ./scripts/bump.ps1 -Part minor -DryRun
#>
[CmdletBinding()]
param(
    [ValidateSet('major', 'minor', 'patch')]
    [string]$Part = 'patch',
    [ValidatePattern('^\d+\.\d+\.\d+$')]
    [string]$Version,
    [switch]$NoPush,
    [switch]$DryRun
)

$ErrorActionPreference = 'Stop'

$root = Resolve-Path (Join-Path $PSScriptRoot '..')
$cargoToml = Join-Path $root 'Cargo.toml'
$tauriConf = Join-Path $root 'crates/palimpsest-app/tauri.conf.json'

# The workspace version is the one line in Cargo.toml that starts with `version`
# — dependency versions are all `name = "..."`, so this anchor is unambiguous.
# It is the same pattern release.yml checks with.
$cargoText = Get-Content $cargoToml -Raw
$match = [regex]::Match($cargoText, '(?m)^version = "(\d+\.\d+\.\d+)"')
if (-not $match.Success) { throw "Couldn't find the workspace version in $cargoToml" }
$current = $match.Groups[1].Value

if ($Version) {
    $next = $Version
} else {
    $n = $current.Split('.') | ForEach-Object { [int]$_ }
    switch ($Part) {
        'major' { $next = "$($n[0] + 1).0.0" }
        'minor' { $next = "$($n[0]).$($n[1] + 1).0" }
        'patch' { $next = "$($n[0]).$($n[1]).$($n[2] + 1)" }
    }
}

if ($next -eq $current) { throw "Version is already $current — nothing to bump." }
$tag = "v$next"

Write-Host "Palimpsest $current -> $next  (tag $tag)"

if ($DryRun) {
    Write-Host "Dry run: would rewrite Cargo.toml and tauri.conf.json, commit, tag $tag" -ForegroundColor Yellow
    if (-not $NoPush) { Write-Host "Dry run: would push main and $tag" -ForegroundColor Yellow }
    exit 0
}

# A dirty tree would sweep unrelated work into the release commit, and the tag
# would then name a state nobody reviewed.
$dirty = git -C $root status --porcelain
if ($dirty) { throw "Working tree is not clean. Commit or stash first:`n$dirty" }

if (git -C $root tag --list $tag) { throw "Tag $tag already exists." }

# Cargo.toml: the anchored workspace version only.
$cargoText = [regex]::Replace($cargoText, '(?m)^version = "\d+\.\d+\.\d+"', "version = `"$next`"")
Set-Content -Path $cargoToml -Value $cargoText -NoNewline

# tauri.conf.json: the top-level "version" key. Rewritten textually rather than
# through ConvertTo-Json so the file's formatting and key order survive.
$tauriText = Get-Content $tauriConf -Raw
$tauriNew = [regex]::Replace($tauriText, '"version": "\d+\.\d+\.\d+"', "`"version`": `"$next`"", 1)
if ($tauriNew -eq $tauriText) { throw "Couldn't find the version in $tauriConf" }
Set-Content -Path $tauriConf -Value $tauriNew -NoNewline

# Cargo.lock is tracked and records the workspace members' versions, so it has to
# move too or the release commit leaves the tree dirty.
Push-Location $root
try {
    cargo update --workspace --offline --quiet
    if ($LASTEXITCODE -ne 0) { throw "cargo update failed; Cargo.lock was not refreshed." }
} finally {
    Pop-Location
}

git -C $root add Cargo.toml Cargo.lock crates/palimpsest-app/tauri.conf.json
git -C $root commit -m "Release $tag"
git -C $root tag $tag

if ($NoPush) {
    Write-Host "Committed and tagged $tag. Push with: git push origin main $tag" -ForegroundColor Green
    exit 0
}

$branch = (git -C $root rev-parse --abbrev-ref HEAD).Trim()
git -C $root push origin $branch $tag
Write-Host "Pushed $branch and $tag. The Release workflow takes it from here." -ForegroundColor Green
