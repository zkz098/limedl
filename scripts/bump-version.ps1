<#
.SYNOPSIS
  Bump the project version (patch / minor / major) and update all version files.
.DESCRIPTION
  Reads the current version from Cargo.toml, bumps the specified semver component,
  writes the new version to Cargo.toml, package.json, and src-tauri/tauri.conf.json.
  Optionally commits, tags, and pushes.
.PARAMETER Level
  One of: patch, minor, major
.PARAMETER NoPush
  Skip git commit, tag, and push — only update files.
.PARAMETER DryRun
  Show what would be changed without writing anything.
.EXAMPLE
  .\scripts\bump-version.ps1 patch
  .\scripts\bump-version.ps1 minor -NoPush
#>

param(
  [Parameter(Mandatory = $true, Position = 0)]
  [ValidateSet('patch', 'minor', 'major')]
  [string]$Level,

  [switch]$NoPush,
  [switch]$DryRun
)

$ErrorActionPreference = 'Stop'
$root = Split-Path -Parent $PSScriptRoot

# Files to update (relative to repo root)
$files = @(
  'Cargo.toml',
  'package.json',
  'src-tauri\tauri.conf.json'
)

# --- Read current version from Cargo.toml ---
$cargoPath = Join-Path $root 'Cargo.toml'
$currentVersion = (Select-String -Path $cargoPath -Pattern '^version\s*=\s*"(?<ver>\d+\.\d+\.\d+)"').Matches[0].Groups['ver'].Value
if (-not $currentVersion) { throw "Could not parse version from Cargo.toml" }

# --- Parse and bump ---
$parts = [int[]]($currentVersion -split '\.')
switch ($Level) {
  'major'  { $parts[0]++; $parts[1] = 0; $parts[2] = 0 }
  'minor'  { $parts[1]++; $parts[2] = 0 }
  'patch'  { $parts[2]++ }
}
$newVersion = $parts -join '.'

Write-Host "$currentVersion → $newVersion ($Level)" -ForegroundColor Cyan

if ($DryRun) {
  Write-Host '[dry-run] Would update:' -ForegroundColor Yellow
  foreach ($f in $files) {
    $path = Join-Path $root $f
    Write-Host "  $path : $currentVersion → $newVersion"
  }
  exit 0
}

# --- Update files ---
foreach ($f in $files) {
  $path = Join-Path $root $f
  $content = Get-Content -Path $path -Raw
  # Cargo.toml uses `version = "x.y.z"`, others use `"version": "x.y.z"`
  $updated = $content -replace $currentVersion, $newVersion
  Set-Content -Path $path -Value $updated -NoNewline
  Write-Host "  Updated: $f" -ForegroundColor Green
}

if ($NoPush) { exit 0 }

# --- Git commit, tag, push ---
Push-Location $root
try {
  git add $files
  git commit -m "chore: bump version to $newVersion"
  git tag "v$newVersion"
  git push origin main
  git push origin "v$newVersion"
  Write-Host "Pushed commit + tag v$newVersion" -ForegroundColor Green
} finally {
  Pop-Location
}
