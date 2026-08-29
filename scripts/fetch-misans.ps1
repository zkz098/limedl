# Fetches the MiSans VF variable font from Xiaomi's official distribution and
# extracts only the VF TTF into crates/limedl-native/assets/fonts/.
#
# Why isn't the font committed? The MiSans Font IP License Agreement forbids
# re-distributing the font file itself; only works that EMBED it (apps etc.)
# may be distributed. So every developer/CI fetches it once from the official
# source before building limedl-native. The built app embedding the font is
# an explicitly licensed scenario. Attribution lives in the About page.
#
# Usage:
#   pwsh scripts/fetch-misans.ps1 [-Force]
#
# The script is ASCII-only and works on Windows PowerShell 5.1 and pwsh 7+
# (Linux/macOS CI runners included).
[CmdletBinding()]
param(
    # Re-download even if the font already exists and matches the pinned hash.
    [switch]$Force
)

$ErrorActionPreference = "Stop"
$ProgressPreference = "SilentlyContinue"

# ── Constants (update together when Xiaomi ships a new font build) ────────
$ZipUrl         = "https://hyperos.mi.com/font-download/MiSans.zip"
$EntrySuffix    = "MiSansVF.ttf"            # unique entry name inside the zip
$ExpectedSize   = 20093424
$ExpectedSha256 = "0ddef90648998900175cfdca9a6f087a2544c182f130b0ad4f7e94a03a115e79"

$OutDir  = Join-Path $PSScriptRoot "..\crates\limedl-native\assets\fonts"
$OutFile = Join-Path $OutDir "MiSansVF.ttf"

function Test-FontHash([string]$Path) {
    if (-not (Test-Path $Path)) { return $false }
    $item = Get-Item $Path
    if ($item.Length -ne $ExpectedSize) { return $false }
    return ((Get-FileHash $Path -Algorithm SHA256).Hash.ToLower() -eq $ExpectedSha256)
}

if (-not $Force -and (Test-FontHash $OutFile)) {
    Write-Host "MiSans VF already present and up to date: $OutFile"
    exit 0
}

New-Item -ItemType Directory -Force -Path $OutDir | Out-Null
$tmp = Join-Path ([System.IO.Path]::GetTempPath()) ("misans-fetch-" + [System.IO.Path]::GetRandomFileName())
New-Item -ItemType Directory -Force -Path $tmp | Out-Null

function Get-RangeBytes([string]$Url, [long]$Start, [long]$End) {
    $req = [System.Net.HttpWebRequest]::Create($Url)
    $req.Method = "GET"
    $req.UserAgent = "limedl-build"
    $req.AddRange($Start, $End)
    $resp = $req.GetResponse()
    try {
        $ms = New-Object System.IO.MemoryStream
        $resp.GetResponseStream().CopyTo($ms)
        return ,$ms.ToArray()
    } finally { $resp.Close() }
}

function Read-ZipEntryViaRange([string]$Url, [string]$OutPath) {
    # Fast path: HTTP Range requests pull only the needed entry (~15 MB) out of
    # the 227 MB zip. Reads the central directory, then deflates the entry.
    $head = [System.Net.HttpWebRequest]::Create($Url)
    $head.Method = "HEAD"; $head.UserAgent = "limedl-build"
    $hresp = $head.GetResponse()
    try { $total = $hresp.ContentLength } finally { $hresp.Close() }
    if ($total -le 0) { throw "server did not report content length" }

    $tail = Get-RangeBytes $Url ($total - 1048576) ($total - 1)
    $eocd = -1
    for ($i = $tail.Length - 22; $i -ge 0; $i--) {
        if ($tail[$i] -eq 0x50 -and $tail[$i+1] -eq 0x4b -and $tail[$i+2] -eq 0x05 -and $tail[$i+3] -eq 0x06) { $eocd = $i; break }
    }
    if ($eocd -lt 0) { throw "zip end-of-central-directory not found" }
    $count  = [BitConverter]::ToUInt16($tail, $eocd + 10)
    $cdSize = [BitConverter]::ToUInt32($tail, $eocd + 12)
    $cdOff  = [BitConverter]::ToUInt32($tail, $eocd + 16)
    if ($cdSize -gt $tail.Length) { throw "central directory larger than fetched tail" }

    # locate the entry inside the central directory buffer
    $pos = $tail.Length - [int]($total - $cdOff)
    $entry = $null
    for ($n = 0; $n -lt $count -and $pos -lt $tail.Length - 46; $n++) {
        $nameLen    = [BitConverter]::ToUInt16($tail, $pos + 28)
        $extraLen   = [BitConverter]::ToUInt16($tail, $pos + 30)
        $commentLen = [BitConverter]::ToUInt16($tail, $pos + 32)
        $name = [System.Text.Encoding]::UTF8.GetString($tail, $pos + 46, $nameLen)
        if ($name.EndsWith($EntrySuffix)) {
            $entry = @{
                Method = [BitConverter]::ToUInt16($tail, $pos + 10)
                CSize  = [BitConverter]::ToUInt32($tail, $pos + 20)
                USize  = [BitConverter]::ToUInt32($tail, $pos + 24)
                Lho    = [BitConverter]::ToUInt32($tail, $pos + 42)
            }
            break
        }
        $pos += 46 + $nameLen + $extraLen + $commentLen
    }
    if (-not $entry) { throw "entry $EntrySuffix not found in zip" }

    $lh = Get-RangeBytes $Url $entry.Lho ($entry.Lho + 511)
    if ([BitConverter]::ToUInt32($lh, 0) -ne 0x04034b50) { throw "bad local file header" }
    $dataStart = $entry.Lho + 30 + [BitConverter]::ToUInt16($lh, 26) + [BitConverter]::ToUInt16($lh, 28)

    $comp = Get-RangeBytes $Url $dataStart ($dataStart + $entry.CSize - 1)
    $ms = New-Object System.IO.MemoryStream(,$comp)
    if ($entry.Method -eq 8) {
        $ds = New-Object System.IO.Compression.DeflateStream($ms, [System.IO.Compression.CompressionMode]::Decompress)
        $fo = [System.IO.File]::Create($OutPath)
        try { $ds.CopyTo($fo) } finally { $fo.Close(); $ds.Close() }
    } elseif ($entry.Method -eq 0) {
        [System.IO.File]::WriteAllBytes($OutPath, $comp)
    } else {
        throw "unsupported zip compression method $($entry.Method)"
    }
}

function Read-ZipEntryViaFullDownload([string]$Url, [string]$OutPath) {
    # Fallback: download the whole zip and extract via ZipFile.
    $zipPath = Join-Path $tmp "MiSans.zip"
    Write-Host "Downloading full archive ($([Math]::Round(227880072/1MB)) MB)..."
    Invoke-WebRequest -Uri $Url -OutFile $zipPath -UseBasicParsing -UserAgent "limedl-build"
    Add-Type -AssemblyName System.IO.Compression.FileSystem
    $zip = [System.IO.Compression.ZipFile]::OpenRead($zipPath)
    try {
        $entry = $zip.Entries | Where-Object { $_.FullName.EndsWith($EntrySuffix) } | Select-Object -First 1
        if (-not $entry) { throw "entry $EntrySuffix not found in zip" }
        [System.IO.Compression.ZipFileExtensions]::ExtractToFile($entry, $OutPath, $true)
    } finally { $zip.Dispose() }
    Remove-Item $zipPath -Force -ErrorAction SilentlyContinue
}

try {
    try {
        Write-Host "Fetching MiSans VF via HTTP range requests (about 15 MB)..."
        Read-ZipEntryViaRange $ZipUrl $OutFile
    } catch {
        Write-Warning "Range extraction failed ($($_.Exception.Message)); falling back to full download."
        Read-ZipEntryViaFullDownload $ZipUrl $OutFile
    }
} finally {
    Remove-Item $tmp -Recurse -Force -ErrorAction SilentlyContinue
}

if (-not (Test-FontHash $OutFile)) {
    $actual = if (Test-Path $OutFile) { "{0} bytes, sha256={1}" -f (Get-Item $OutFile).Length, (Get-FileHash $OutFile -Algorithm SHA256).Hash.ToLower() } else { "missing" }
    throw "Downloaded font failed verification.`n  expected: $ExpectedSize bytes, sha256=$ExpectedSha256`n  actual:   $actual`nIf Xiaomi published a new font build, update the constants at the top of this script."
}

Write-Host "MiSans VF fetched OK: $OutFile"
