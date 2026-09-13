# Generates latest-native.json — the self-update manifest for limedl-native.
#
# Input is the set of artifacts to advertise, described as JSON so a single
# script serves every platform: the per-platform release jobs produce their own
# entries and the manifest job merges them (see the `manifest` job in
# .github/workflows/release.yml). Keeping the generator platform-agnostic
# matters because the client derives its lookup key from the running OS/arch
# (update::manifest_key): a key nobody generates is an update that silently
# never appears.
#
# Reads the minisign .sig files written by `cargo xtask sign` next to each
# artifact, base64-encodes them into the manifest, computes sha256 digests, and
# emits asset URLs pointing at the release download endpoints (github.com domain
# — no api.github.com quota consumed). The manifest itself is signed afterwards
# (`cargo xtask sign dist/latest-native.json`), because the client verifies
# latest-native.json.sig before it parses anything.
#
# `assets` shape — a map of manifest key -> artifact path (the order of the JSON
# object is preserved, so the output groups Windows then macOS):
#
#   {
#     "windows-x86_64":          { "kind": "installer", "path": "dist/setup.exe" },
#     "windows-x86_64-portable": { "kind": "portable",  "path": "dist/app.zip" },
#     "darwin-aarch64-portable": { "kind": "portable",  "path": "dist/app.tar.gz" }
#   }
#
# Valid `kind` values are exactly the two the client validates against
# (update::check_for_update): "installer" (Windows NSIS) and "portable"
# (zip/tar.gz replaced in place).
#
# Usage:
#   pwsh scripts/gen-native-manifest.ps1 -Version 0.3.6 -AssetsJson dist/native-assets.json `
#       -Notes "<changelog>" -OutFile dist/latest-native.json

param(
    [Parameter(Mandatory = $true)][string]$Version,
    [Parameter(Mandatory = $true)][string]$AssetsJson,
    [Parameter(Mandatory = $true)][string]$Notes,
    [Parameter(Mandatory = $true)][string]$OutFile,
    [string]$Repo = "zkz098/limedl"
)

$ErrorActionPreference = "Stop"

if (-not (Test-Path $AssetsJson)) { throw "asset map not found: $AssetsJson" }
$assets = Get-Content $AssetsJson -Raw | ConvertFrom-Json
if (-not $assets) { throw "asset map is empty: $AssetsJson" }

$VALID_KINDS = @("installer", "portable")

function Get-ArtifactEntry {
    param([string]$Key, [string]$Kind, [string]$Path)

    if ($VALID_KINDS -notcontains $Kind) {
        throw "asset '$Key' has kind '$Kind' (expected one of: $($VALID_KINDS -join ', '))"
    }
    if (-not (Test-Path $Path)) { throw "artifact for '$Key' not found: $Path" }

    $sigPath = "$Path.sig"
    if (-not (Test-Path $sigPath)) {
        throw "signature file missing for $Path — run 'cargo xtask sign' first"
    }
    # base64(minisign signature file text) — the form minisign_verify's
    # Signature::decode expects after the client base64-decodes it.
    $sigText = [System.IO.File]::ReadAllText($sigPath).Trim()
    $sigB64 = [Convert]::ToBase64String([System.Text.Encoding]::UTF8.GetBytes($sigText))
    $sha256 = (Get-FileHash -Path $Path -Algorithm SHA256).Hash.ToLowerInvariant()
    $fileName = Split-Path $Path -Leaf
    return [ordered]@{
        kind      = $Kind
        url       = "https://github.com/$Repo/releases/download/v$Version/$fileName"
        signature = $sigB64
        sha256    = $sha256
    }
}

$platforms = [ordered]@{}
foreach ($key in $assets.PSObject.Properties.Name) {
    $entry = $assets.$key
    if (-not $entry.kind -or -not $entry.path) {
        throw "asset '$key' needs both 'kind' and 'path'"
    }
    $platforms[$key] = Get-ArtifactEntry -Key $key -Kind $entry.kind -Path $entry.path
}

# `version`, `notes` and `platforms` are the only fields the client parses
# (update::UpdateManifest); anything else is ignored, so none is emitted.
$manifest = [ordered]@{
    version   = $Version
    notes     = $Notes
    platforms = $platforms
}

$json = $manifest | ConvertTo-Json -Depth 6
[System.IO.File]::WriteAllText((Join-Path (Get-Location) $OutFile), $json, (New-Object System.Text.UTF8Encoding($false)))
Write-Host "wrote $OutFile ($($platforms.Keys -join ', '))"
Write-Host $json
