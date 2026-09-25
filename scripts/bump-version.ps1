param(
    [Parameter(Position = 0, Mandatory = $true)]
    [ValidateSet("patch", "minor", "major")]
    [string]$Level,

    [switch]$NoPush,
    [switch]$DryRun
)

$ErrorActionPreference = "Stop"

$scriptDir = Split-Path -Parent $MyInvocation.MyCommand.Path
$root = Resolve-Path (Join-Path $scriptDir "..")

# Read current version from root Cargo.toml
$cargoPath = Join-Path $root "Cargo.toml"
$cargoContent = Get-Content -Raw -Path $cargoPath -Encoding utf8

if ($cargoContent -match '(?m)^version\s*=\s*"(\d+\.\d+\.\d+)"') {
    $currentVersion = $Matches[1]
} else {
    throw "Could not parse version from Cargo.toml"
}

# Calculate new version
$parts = [int[]]($currentVersion -split '\.')
switch ($Level) {
    "major" {
        $parts[0]++
        $parts[1] = 0
        $parts[2] = 0
    }
    "minor" {
        $parts[1]++
        $parts[2] = 0
    }
    "patch" {
        $parts[2]++
    }
}
$newVersion = $parts -join "."

Write-Host -ForegroundColor Cyan "$currentVersion -> $newVersion ($Level)"

if ($DryRun) {
    Write-Host -ForegroundColor Yellow "[dry-run] Would update:"
    Write-Host "  Cargo.toml : $currentVersion -> $newVersion"
    Write-Host "  Cargo.lock : $currentVersion -> $newVersion"
    Write-Host "  website    : $currentVersion -> $newVersion"
    exit 0
}

# Update Cargo.toml
$newCargoContent = [regex]::Replace($cargoContent, '(?m)^version\s*=\s*"[^"]+"', "version = `"$newVersion`"", 1)
[System.IO.File]::WriteAllText($cargoPath, $newCargoContent, [System.Text.UTF8Encoding]::new($false))
Write-Host -ForegroundColor Green "  Updated: Cargo.toml"

# Update Cargo.lock
$lockPath = Join-Path $root "Cargo.lock"
if (Test-Path $lockPath) {
    $lockContent = Get-Content -Raw -Path $lockPath -Encoding utf8
    $newLockContent = [regex]::Replace(
        $lockContent,
        '(?m)(name = "limedl(?:-core|-native)?"\r?\nversion = ")[^"]+(")',
        "`${1}$newVersion`${2}"
    )
    [System.IO.File]::WriteAllText($lockPath, $newLockContent, [System.Text.UTF8Encoding]::new($false))
    Write-Host -ForegroundColor Green "  Updated: Cargo.lock"
}

# Update website/package.json
$webPkgPath = Join-Path $root "website/package.json"
if (Test-Path $webPkgPath) {
    $webPkg = Get-Content -Raw -Path $webPkgPath -Encoding utf8
    $newWebPkg = [regex]::Replace($webPkg, '"version":\s*"[^"]+"', "`"version`": `"$newVersion`"", 1)
    [System.IO.File]::WriteAllText($webPkgPath, $newWebPkg, [System.Text.UTF8Encoding]::new($false))
    Write-Host -ForegroundColor Green "  Updated: website/package.json"
}

# Update website files referencing version
$webFiles = @(
    "website/src/components/Header.astro",
    "website/src/components/OsDownloadButton.astro",
    "website/src/pages/download.astro",
    "website/src/pages/en/download.astro",
    "website/src/content/docs/getting-started/installation.md",
    "website/src/content/docs/en/getting-started/installation.md"
)
foreach ($rel in $webFiles) {
    $filePath = Join-Path $root $rel
    if (Test-Path $filePath) {
        $content = Get-Content -Raw -Path $filePath -Encoding utf8
        $newContent = $content -replace [regex]::Escape("v$currentVersion"), "v$newVersion"
        $newContent = $newContent -replace [regex]::Escape($currentVersion), $newVersion
        [System.IO.File]::WriteAllText($filePath, $newContent, [System.Text.UTF8Encoding]::new($false))
        Write-Host -ForegroundColor Green "  Updated: $rel"
    }
}

if ($NoPush) {
    exit 0
}

# Git commit, tag, push
$files = @("Cargo.toml", "Cargo.lock", "website")
git add $files
git commit -m "chore: bump version to $newVersion"
git push origin main
git tag "v$newVersion" -m "v$newVersion"
git push origin "v$newVersion"

Write-Host -ForegroundColor Green "Pushed commit + tag v$newVersion"
