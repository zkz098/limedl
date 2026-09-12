# Generates latest-native.json — the self-update manifest for limedl-native.
#
# Reads the minisign .sig files written by `cargo xtask sign` next to each
# artifact, base64-encodes them into the manifest, computes sha256 digests, and
# emits asset URLs pointing at the release download endpoints (github.com domain
# — no api.github.com quota consumed). The manifest itself is signed afterwards
# (`cargo xtask sign dist/latest-native.json`), because the client verifies
# latest-native.json.sig before it parses anything.
#
# Usage:
#   pwsh scripts/gen-native-manifest.ps1 -Version 0.2.1 `
#       -SetupExe dist/limedl-native-v0.2.1-windows-x86_64-setup.exe `
#       -PortableZip dist/limedl-native-v0.2.1-windows-x86_64-portable.zip `
#       -Notes "<changelog>" -OutFile dist/latest-native.json

param(
    [Parameter(Mandatory = $true)][string]$Version,
    [Parameter(Mandatory = $true)][string]$SetupExe,
    [Parameter(Mandatory = $true)][string]$PortableZip,
    [Parameter(Mandatory = $true)][string]$Notes,
    [Parameter(Mandatory = $true)][string]$OutFile,
    [string]$Repo = "zkz098/limedl"
)

$ErrorActionPreference = "Stop"

foreach ($f in @($SetupExe, $PortableZip)) {
    if (-not (Test-Path $f)) { throw "artifact not found: $f" }
}

function Get-ArtifactEntry {
    param([string]$Kind, [string]$Path)
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

$manifest = [ordered]@{
    version  = $Version
    notes    = $Notes
    pub_date = (Get-Date).ToUniversalTime().ToString("yyyy-MM-ddTHH:mm:ss.fffZ")
    platforms = [ordered]@{
        "windows-x86_64"         = Get-ArtifactEntry -Kind "installer" -Path $SetupExe
        "windows-x86_64-portable" = Get-ArtifactEntry -Kind "portable" -Path $PortableZip
    }
}

$json = $manifest | ConvertTo-Json -Depth 5
[System.IO.File]::WriteAllText((Join-Path (Get-Location) $OutFile), $json, (New-Object System.Text.UTF8Encoding($false)))
Write-Host "wrote $OutFile"
Write-Host $json
