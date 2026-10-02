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
#   pwsh scripts/fetch-misans.ps1 [-Force] [-Verify] [-FromPath <path|url>] [-ZipUrl <url>]
#
#   -Force        re-download even when the local copy already matches the
#                 pinned size/hash.
#   -Verify       check the local copy only; never touch the network. The CI
#                 jobs that receive the font as an artifact run this.
#   -FromPath     take the extracted MiSansVF.ttf from a local file or an
#                 alternate URL instead of Xiaomi's CDN (the pinned hash is
#                 still enforced). `LIMEDL_MISANS_TTF` does the same without
#                 the flag, which is the escape hatch when the CDN is blocked
#                 or a copy is already sitting on disk.
#   -ZipUrl       point at a mirror of the zip archive instead.
#
# Transport: `curl` when available (GitHub runners and modern Windows ship it),
# with a .NET `HttpWebRequest` fallback. The range-request path only needs
# ~16 MB of the 217 MB archive; the fallback downloads the whole zip with
# resume + retries, because the CDN occasionally drops long connections (that
# is what broke the Linux CI job when this used `Invoke-WebRequest`).
#
# Hardening (each item traces back to the 2026-10-01 CDN outage, which failed
# CI and the release-cache warm-up on two platforms — no test was involved):
#   * Every request walks a transport ladder: HTTP/2 (when the local curl has
#     it), then HTTP/1.1, then HTTP/1.1 over IPv4. `curl (92) HTTP/2 stream was
#     not closed cleanly` and truncated 227 MB bodies are edge problems, not
#     request problems, and curl's own `--retry` cannot fix them — a different
#     protocol/family can.
#   * The entry is pulled in 2 MiB chunks, so a connection killed mid-body
#     costs one chunk instead of the whole download.
#   * The archive size comes from a 1-byte ranged GET (`Content-Range`) rather
#     than HEAD: the CDN answered HEAD without a Content-Length during the
#     outage, which silently downgraded the 16 MB fast path to 217 MB.
#   * Nothing lands in the repo until the extracted font matches the pinned
#     size + sha256; the verified file is moved into place afterwards, so a
#     failed fetch can never destroy a good local copy.
#   * Two passes 30 s apart cover an edge that stays broken for about a minute.
#     .github/actions/fetch-misans adds the job-level retry and the annotation.
#
# The script is ASCII-only and works on Windows PowerShell 5.1 and pwsh 7+
# (Linux/macOS CI runners included).
[CmdletBinding()]
param(
    # Re-download even if the font already exists and matches the pinned hash.
    [switch]$Force,
    # Only check the existing file. Never downloads.
    [switch]$Verify,
    # Use this .ttf (local path or URL) instead of Xiaomi's CDN.
    [string]$FromPath = $env:LIMEDL_MISANS_TTF,
    # Alternate URL for the zip archive (a mirror of MiSans.zip).
    [string]$ZipUrl = "https://hyperos.mi.com/font-download/MiSans.zip"
)

$ErrorActionPreference = "Stop"
$ProgressPreference = "SilentlyContinue"

# ── Constants (update together when Xiaomi ships a new font build) ────────
$EntrySuffix    = "MiSansVF.ttf"            # unique entry name inside the zip
$ExpectedSize   = 20093424
$ExpectedSha256 = "0ddef90648998900175cfdca9a6f087a2544c182f130b0ad4f7e94a03a115e79"
$ZipTotalBytes  = 227880072

# ── Tuning ────────────────────────────────────────────────────────────────
$ChunkSize             = 2MB   # range-fetch granularity for the zip entry
$ProbeTimeoutSec       = 60    # 1-byte size probe / HEAD
$ChunkTimeoutSec       = 120   # one 2 MiB entry chunk
$FullDownloadTimeoutSec = 240  # last resort: the whole 217 MB archive
$RetryCount            = 2     # curl retries per transport (so 3 attempts each)
$RetryDelaySec         = 3
$FullRetryCount        = 3
$FullRetryDelaySec     = 10
$Passes                = 2
$PassPauseSec          = 30

# Canonicalised once. PowerShell's FileSystem provider and the .NET file APIs do
# not agree on a path that still contains "..": on the Linux runners `Get-Item`
# reported "Could not find item" for a file that `Test-Path` had just confirmed,
# which turned a perfectly good download into a red job. Everything below works
# on a resolved path, and the file checks avoid the provider altogether.
$OutDir  = [System.IO.Path]::GetFullPath((Join-Path $PSScriptRoot "..\crates\limedl-native\assets\fonts"))
$OutFile = Join-Path $OutDir "MiSansVF.ttf"

function Get-FileSha256([string]$Path) {
    # .NET instead of Get-FileHash, for the same reason as the checks below: no
    # provider round-trip on a path that [System.IO.File] wrote a moment ago.
    $sha = [System.Security.Cryptography.SHA256]::Create()
    try {
        $stream = [System.IO.File]::OpenRead($Path)
        try {
            return (($sha.ComputeHash($stream) | ForEach-Object { $_.ToString("x2") }) -join "")
        } finally { $stream.Dispose() }
    } finally { $sha.Dispose() }
}

function Test-FontHash([string]$Path) {
    if (-not [System.IO.File]::Exists($Path)) { return $false }
    if ((New-Object System.IO.FileInfo($Path)).Length -ne $ExpectedSize) { return $false }
    return ((Get-FileSha256 $Path) -eq $ExpectedSha256)
}

function Format-FontState([string]$Path) {
    if (-not [System.IO.File]::Exists($Path)) { return "missing" }
    return "{0} bytes, sha256={1}" -f (New-Object System.IO.FileInfo($Path)).Length, (Get-FileSha256 $Path)
}

function Show-Status([string]$Message) {
    # The script's only Write-Host. Progress has to reach the console GitHub
    # parses (annotations included), which rules out Write-Output: that lands on
    # the success stream and would vanish the moment a caller captures it. S8677
    # wants host writers named Show-*, hence the wrapper instead of a bare
    # Write-Host at every call site.
    Write-Host $Message
}

function Show-FailureAnnotation([string]$Message) {
    # GitHub only parses the rest of the line as the annotation text, so this
    # stays short and single-line; the caller's exception carries the detail.
    if ($env:GITHUB_ACTIONS -eq "true") { Show-Status "::error::$Message" }
}

# ── Early exits: nothing to do, or nothing to do but check ────────────────

$fontOk = Test-FontHash $OutFile

if ($Verify) {
    if (-not $fontOk) {
        Show-FailureAnnotation "MiSans VF is missing or is not the pinned build (see the log)"
        throw "MiSans VF missing or not the pinned build at $OutFile`n  expected: $ExpectedSize bytes, sha256=$ExpectedSha256`n  actual:   $(Format-FontState $OutFile)`nRun: pwsh scripts/fetch-misans.ps1"
    }
    Show-Status "MiSans VF present and verified: $OutFile"
    return
}

if (-not $Force -and $fontOk) {
    Show-Status "MiSans VF already present and up to date: $OutFile"
    return
}

# ── Transport helpers ─────────────────────────────────────────────────────

function Get-CurlPath {
    # NOTE: in Windows PowerShell `curl` is an alias for Invoke-WebRequest, so
    # only an actual application (curl.exe / /usr/bin/curl) counts.
    foreach ($name in @("curl.exe", "curl")) {
        $cmd = Get-Command $name -CommandType Application -ErrorAction SilentlyContinue |
            Select-Object -First 1
        if ($cmd -and $cmd.Source) { return $cmd.Source }
    }
    return $null
}

$Curl = Get-CurlPath

function Get-CurlFeatures {
    # The curl bundled with Windows is built without HTTP/2 (`--http2` exits 2
    # with "the installed libcurl version does not support this"), and the
    # runners' copies differ, so the ladder is built from what `curl --version`
    # reports instead of assuming it.
    $line = & $Curl --version | Where-Object { $_ -match "^\s*Features:" } | Select-Object -First 1
    if (-not $line) { return @() }
    return @(($line -replace "^\s*Features:\s*", "") -split "\s+" | Where-Object { $_ })
}

# The ladder every request walks. During the 2026-10-01 outage the file stayed
# unreachable for ~35 s no matter how often it was retried on HTTP/2 (that is
# what `--retry-all-errors` produced: six identical `curl (92)` failures in a
# row), so the retries are spread across the protocols/families below instead
# of being repeated on one of them. A curl without HTTP/2 already speaks
# HTTP/1.1 by default, so dropping that rung loses nothing. (Only the curl
# paths use the ladder; the .NET fallback has one transport by construction.)
$script:Transports = New-Object System.Collections.ArrayList
if ($Curl -and ((Get-CurlFeatures) -contains "HTTP2")) {
    [void]$script:Transports.Add(@{ Label = "http2"; Args = @("--http2") })
}
[void]$script:Transports.Add(@{ Label = "http1.1"; Args = @("--http1.1") })
[void]$script:Transports.Add(@{ Label = "ipv4"; Args = @("--http1.1", "--ipv4") })

function Invoke-WithTransports {
    param([Parameter(Mandatory = $true)][scriptblock]$Action)
    $failures = @()
    foreach ($transport in $script:Transports) {
        try {
            return & $Action $transport.Args
        } catch {
            $failures += "$($transport.Label): $($_.Exception.Message)"
            Write-Verbose "transport $($transport.Label): $($_.Exception.Message)"
        }
    }
    throw "all transports failed -- $($failures -join ' | ')"
}

function Invoke-CurlToFile {
    param(
        [string[]]$TransportArgs,
        [string[]]$Arguments,
        [string]$OutFile,
        [int]$TimeoutSec = 300,
        [int]$Retries = 0,
        [int]$RetryDelay = 3
    )
    $curlArgs = @("--silent", "--show-error", "--fail", "--location") + $TransportArgs + @(
        # --speed-limit/--speed-time abort a connection that has stalled instead
        # of holding the job until --max-time expires.
        "--retry", "$Retries", "--retry-delay", "$RetryDelay", "--retry-all-errors",
        "--speed-limit", "1024", "--speed-time", "30",
        "--connect-timeout", "20", "--max-time", "$TimeoutSec"
    ) + $Arguments + @("-o", $OutFile)
    & $Curl @curlArgs
    if ($LASTEXITCODE -ne 0) {
        throw "curl exited with $LASTEXITCODE for $($Arguments[-1])"
    }
}

function Invoke-CurlText {
    param(
        [string[]]$TransportArgs = @(),
        [string[]]$Arguments
    )
    # Build the full argument list first: `& $curl @(...) + $args` would splice
    # only the literal and append the rest to the *result*, dropping the URL.
    $curlArgs = @("--silent", "--show-error", "--fail", "--location") + $TransportArgs + @(
        "--connect-timeout", "20", "--max-time", "$ProbeTimeoutSec"
    ) + $Arguments
    $raw = & $Curl @curlArgs
    if ($LASTEXITCODE -ne 0) {
        throw "curl exited with $LASTEXITCODE"
    }
    return ($raw -join "`n")
}

function Invoke-CurlRequest {
    # One request into $OutFile, retried across the ladder until one attempt
    # produces a complete transfer.
    param(
        [string[]]$Arguments,
        [string]$OutFile,
        [int]$TimeoutSec = $ChunkTimeoutSec,
        [int]$Retries = $RetryCount,
        [int]$RetryDelay = $RetryDelaySec
    )
    Invoke-WithTransports {
        param($transportArgs)
        Invoke-CurlToFile -TransportArgs $transportArgs -Arguments $Arguments -OutFile $OutFile `
            -TimeoutSec $TimeoutSec -Retries $Retries -RetryDelay $RetryDelay
    } | Out-Null
}

function Invoke-DotnetToFile {
    # Fallback transport when curl is unavailable (no protocol/family choice,
    # so the ladder degenerates to this single attempt).
    param(
        [string]$Url,
        [string]$OutFile,
        [long]$RangeStart = -1,
        [long]$RangeEnd = -1
    )
    $req = [System.Net.HttpWebRequest]::Create($Url)
    $req.Method = "GET"
    $req.UserAgent = "limedl-build"
    $req.Timeout = 120000
    if ($RangeStart -ge 0) { $req.AddRange($RangeStart, $RangeEnd) }
    $resp = $req.GetResponse()
    try {
        $src = $resp.GetResponseStream()
        $dst = [System.IO.File]::Create($OutFile)
        try { $src.CopyTo($dst) } finally { $dst.Dispose(); $src.Dispose() }
    } finally { $resp.Close() }
}

function Get-RemoteLength([string]$Url) {
    # The fast path needs the exact archive size up front. A 1-byte ranged GET
    # reports it as `Content-Range: bytes 0-0/<total>` and doubles as a probe
    # that the endpoint serves ranges at all.
    if ($Curl) {
        $probe = Join-Path $tmp "size-probe.bin"
        try {
            $headers = Invoke-WithTransports {
                param($transportArgs)
                Invoke-CurlText -TransportArgs $transportArgs `
                    -Arguments @("-r", "0-0", "--dump-header", "-", "--output", $probe, $Url)
            }
            $range = [regex]::Match($headers, "(?im)^content-range:\s*bytes\s+\d+-\d+/(\d+)\s*$")
            if ($range.Success) {
                $total = [long]$range.Groups[1].Value
                if ($total -gt 0) {
                    Write-Verbose "archive size $total (from Content-Range)"
                    return $total
                }
            }
        } catch {
            Write-Verbose "range probe failed: $($_.Exception.Message)"
        }

        try {
            $headers = Invoke-WithTransports {
                param($transportArgs)
                Invoke-CurlText -TransportArgs $transportArgs -Arguments @("--head", $Url)
            }
            $match = [regex]::Matches($headers, "(?im)^content-length:\s*(\d+)\s*$")
            if ($match.Count -gt 0) {
                $length = [long]$match[$match.Count - 1].Groups[1].Value
                if ($length -gt 0) {
                    Write-Verbose "archive size $length (from HEAD Content-Length)"
                    return $length
                }
            }
        } catch {
            Write-Verbose "HEAD probe failed: $($_.Exception.Message)"
        }
    }

    $req = [System.Net.HttpWebRequest]::Create($Url)
    $req.Method = "HEAD"
    $req.UserAgent = "limedl-build"
    $resp = $req.GetResponse()
    try { return $resp.ContentLength } finally { $resp.Close() }
}

function Get-RangeBytes([string]$Url, [long]$Start, [long]$End) {
    # Small ranged read kept in memory (zip probes only: the 1 MiB tail and the
    # 512-byte local file header). The entry itself goes through Get-RangeToFile.
    if ($Curl) {
        $part = Join-Path $tmp ("range-" + [System.IO.Path]::GetRandomFileName())
        try {
            Invoke-CurlRequest -Arguments @("-r", "$Start-$End", $Url) -OutFile $part -TimeoutSec $ProbeTimeoutSec
            return ,[System.IO.File]::ReadAllBytes($part)
        } finally {
            Remove-Item $part -Force -ErrorAction SilentlyContinue
        }
    }

    $part = Join-Path $tmp ("dotnet-range-" + [System.IO.Path]::GetRandomFileName())
    try {
        Invoke-DotnetToFile -Url $Url -OutFile $part -RangeStart $Start -RangeEnd $End
        return ,[System.IO.File]::ReadAllBytes($part)
    } finally {
        Remove-Item $part -Force -ErrorAction SilentlyContinue
    }
}

function Get-RangeToFile([string]$Url, [long]$Start, [long]$End, [string]$OutPath) {
    # One chunk of the entry, verified by length. A body killed mid-transfer
    # fails here and is retried by the next ladder attempt instead of costing
    # the whole 16 MB entry.
    if ($Curl) {
        Invoke-CurlRequest -Arguments @("-r", "$Start-$End", $Url) -OutFile $OutPath
    } else {
        Invoke-DotnetToFile -Url $Url -OutFile $OutPath -RangeStart $Start -RangeEnd $End
    }
    $expected = $End - $Start + 1
    $actual = (Get-Item $OutPath).Length
    if ($actual -ne $expected) {
        throw "short range read ${Start}-${End}: got $actual of $expected bytes"
    }
}

# ── Extraction paths ──────────────────────────────────────────────────────

function Get-ZipEntryFromTail([byte[]]$Tail, [long]$Total) {
    # Parses the end-of-central-directory record out of the fetched 1 MiB tail
    # and walks the central directory for our entry. Returns the entry (method,
    # compressed size, local header offset) or $null when it is not there.
    $eocd = -1
    for ($i = $Tail.Length - 22; $i -ge 0; $i--) {
        if ($Tail[$i] -eq 0x50 -and $Tail[$i+1] -eq 0x4b -and $Tail[$i+2] -eq 0x05 -and $Tail[$i+3] -eq 0x06) { $eocd = $i; break }
    }
    if ($eocd -lt 0) { throw "zip end-of-central-directory not found" }
    $count  = [BitConverter]::ToUInt16($Tail, $eocd + 10)
    $cdSize = [BitConverter]::ToUInt32($Tail, $eocd + 12)
    $cdOff  = [BitConverter]::ToUInt32($Tail, $eocd + 16)
    if ($cdSize -gt $Tail.Length) { throw "central directory larger than fetched tail" }

    # locate the entry inside the central directory buffer
    $pos = $Tail.Length - [int]($Total - $cdOff)
    for ($n = 0; $n -lt $count -and $pos -lt $Tail.Length - 46; $n++) {
        $nameLen    = [BitConverter]::ToUInt16($Tail, $pos + 28)
        $extraLen   = [BitConverter]::ToUInt16($Tail, $pos + 30)
        $commentLen = [BitConverter]::ToUInt16($Tail, $pos + 32)
        $name = [System.Text.Encoding]::UTF8.GetString($Tail, $pos + 46, $nameLen)
        if ($name.EndsWith($EntrySuffix)) {
            return @{
                Method = [BitConverter]::ToUInt16($Tail, $pos + 10)
                CSize  = [BitConverter]::ToUInt32($Tail, $pos + 20)
                USize  = [BitConverter]::ToUInt32($Tail, $pos + 24)
                Lho    = [BitConverter]::ToUInt32($Tail, $pos + 42)
            }
        }
        $pos += 46 + $nameLen + $extraLen + $commentLen
    }
    return $null
}

function Get-ZipEntryRaw([string]$Url, [long]$DataStart, [long]$EntrySize) {
    # Chunked so that a dropped connection only costs one chunk. The parts are
    # concatenated into a temp file: the compressed entry is only the raw
    # material for the deflate step below.
    $parts = New-Object System.Collections.Generic.List[string]
    $offset = 0
    while ($offset -lt $EntrySize) {
        $end = [Math]::Min($offset + $ChunkSize - 1, $EntrySize - 1)
        $part = Join-Path $tmp ("entry-{0}.bin" -f $offset)
        Get-RangeToFile -Url $Url -Start ($DataStart + $offset) -End ($DataStart + $end) -OutPath $part
        [void]$parts.Add($part)
        $offset = $end + 1
    }

    $raw = Join-Path $tmp "entry.bin"
    $dst = [System.IO.File]::Create($raw)
    try {
        foreach ($part in $parts) {
            $src = [System.IO.File]::OpenRead($part)
            try { $src.CopyTo($dst) } finally { $src.Dispose() }
        }
    } finally { $dst.Dispose() }
    return $raw
}

function Expand-ZipEntry([string]$RawPath, [string]$OutPath, [int]$Method) {
    if ($Method -eq 8) {
        $in = [System.IO.File]::OpenRead($RawPath)
        $ds = New-Object System.IO.Compression.DeflateStream($in, [System.IO.Compression.CompressionMode]::Decompress)
        $fo = [System.IO.File]::Create($OutPath)
        try { $ds.CopyTo($fo) } finally { $fo.Dispose(); $ds.Dispose(); $in.Dispose() }
    } elseif ($Method -eq 0) {
        [System.IO.File]::Copy($RawPath, $OutPath, $true)
    } else {
        throw "unsupported zip compression method $Method"
    }
}

function Read-ZipEntryViaRange([string]$Url, [string]$OutPath) {
    # Fast path: HTTP Range requests pull only the needed entry (~16 MB) out of
    # the 217 MB zip. Reads the central directory, then the entry's chunks.
    $total = Get-RemoteLength $Url
    if ($total -le 0) { throw "server did not report content length" }

    $tail = Get-RangeBytes $Url ($total - 1048576) ($total - 1)
    $entry = Get-ZipEntryFromTail $tail $total
    if (-not $entry) { throw "entry $EntrySuffix not found in zip" }

    $lh = Get-RangeBytes $Url $entry.Lho ($entry.Lho + 511)
    if ([BitConverter]::ToUInt32($lh, 0) -ne 0x04034b50) { throw "bad local file header" }
    $dataStart = $entry.Lho + 30 + [BitConverter]::ToUInt16($lh, 26) + [BitConverter]::ToUInt16($lh, 28)

    $raw = Get-ZipEntryRaw -Url $Url -DataStart $dataStart -EntrySize ([long]$entry.CSize)
    Expand-ZipEntry -RawPath $raw -OutPath $OutPath -Method $entry.Method
}

function Read-ZipEntryViaFullDownload([string]$Url, [string]$OutPath) {
    # Fallback: download the whole zip (resumable + retried) and extract via ZipFile.
    $zipPath = Join-Path $tmp "MiSans.zip"
    Show-Status "Downloading full archive ($([Math]::Round($ZipTotalBytes/1MB)) MB)..."
    if ($Curl) {
        Invoke-CurlRequest -Arguments @("--continue-at", "-", $Url) -OutFile $zipPath `
            -TimeoutSec $FullDownloadTimeoutSec -Retries $FullRetryCount -RetryDelay $FullRetryDelaySec
    } else {
        Invoke-DotnetToFile -Url $Url -OutFile $zipPath
    }
    Add-Type -AssemblyName System.IO.Compression.FileSystem
    $zip = [System.IO.Compression.ZipFile]::OpenRead($zipPath)
    try {
        $entry = $zip.Entries | Where-Object { $_.FullName.EndsWith($EntrySuffix) } | Select-Object -First 1
        if (-not $entry) { throw "entry $EntrySuffix not found in zip" }
        [System.IO.Compression.ZipFileExtensions]::ExtractToFile($entry, $OutPath, $true)
    } finally { $zip.Dispose() }
    Remove-Item $zipPath -Force -ErrorAction SilentlyContinue
}

function Import-FontFromSource([string]$Source, [string]$OutPath) {
    # -FromPath / LIMEDL_MISANS_TTF: take an already-extracted MiSansVF.ttf from
    # a local file or an alternate URL. The pinned hash check below still runs.
    if ($Source -match "^(?i)https?://") {
        Show-Status "Fetching MiSans VF from $Source ..."
        if ($Curl) {
            Invoke-CurlRequest -Arguments @($Source) -OutFile $OutPath -TimeoutSec 300
        } else {
            Invoke-DotnetToFile -Url $Source -OutFile $OutPath
        }
        return
    }
    $resolved = (Resolve-Path -LiteralPath $Source -ErrorAction Stop).Path
    Show-Status "Using MiSans VF from $resolved ..."
    Copy-Item -LiteralPath $resolved -Destination $OutPath -Force
}

# ── Fetch ─────────────────────────────────────────────────────────────────

New-Item -ItemType Directory -Force -Path $OutDir | Out-Null
$tmp = Join-Path ([System.IO.Path]::GetTempPath()) ("misans-fetch-" + [System.IO.Path]::GetRandomFileName())
New-Item -ItemType Directory -Force -Path $tmp | Out-Null
# Staged next to the target so the final move is a same-volume rename: the
# pinned font either stays exactly as it was, or is replaced by a verified one.
# Deliberately not dot-prefixed: a hidden name is the other half of the path
# shape that the provider above could not resolve.
$staging = Join-Path $OutDir ("MiSansVF.ttf.staging-" + [System.IO.Path]::GetRandomFileName())

try {
    if ($FromPath) {
        Import-FontFromSource $FromPath $staging
    } else {
        $fetched = $false
        for ($pass = 1; $pass -le $Passes -and -not $fetched; $pass++) {
            if ($pass -gt 1) { Show-Status "Pass $pass of $Passes" }
            try {
                Show-Status "Fetching MiSans VF via HTTP range requests (about 16 MB)..."
                Read-ZipEntryViaRange $ZipUrl $staging
                $fetched = $true
            } catch {
                Write-Warning "Range extraction failed ($($_.Exception.Message))"
                # The 217 MB archive is the last resort and only worth trying on
                # the first pass: a CDN that cannot serve a 2 MiB chunk for a
                # minute is not going to serve 217 MB either.
                if ($pass -lt $Passes) {
                    try {
                        Read-ZipEntryViaFullDownload $ZipUrl $staging
                        $fetched = $true
                    } catch {
                        Write-Warning "Full download failed ($($_.Exception.Message))"
                    }
                }
            }
            if (-not $fetched) {
                Remove-Item $staging -Force -ErrorAction SilentlyContinue
                if ($pass -lt $Passes) {
                    Write-Warning "Retrying in $PassPauseSec s..."
                    Start-Sleep -Seconds $PassPauseSec
                }
            }
        }
        if (-not $fetched) {
            $details = "MiSans VF could not be fetched: $ZipUrl is unreachable or serving broken responses (see the warnings above)."
            Show-FailureAnnotation "MiSans VF could not be fetched from the CDN ($ZipUrl) - rerun this job once it recovers"
            throw "$details`nRerun the job once the CDN recovers, or point -FromPath / LIMEDL_MISANS_TTF at a copy of MiSansVF.ttf you already have."
        }
    }

    if (-not (Test-FontHash $staging)) {
        Show-FailureAnnotation "MiSans VF failed the pinned size/sha256 check (did Xiaomi change the font build?)"
        throw "Downloaded font failed verification.`n  expected: $ExpectedSize bytes, sha256=$ExpectedSha256`n  actual:   $(Format-FontState $staging)`nIf Xiaomi published a new font build, update the constants at the top of this script."
    }

    Move-Item -Force $staging $OutFile
    Show-Status "MiSans VF fetched OK: $OutFile"
} finally {
    Remove-Item $staging -Force -ErrorAction SilentlyContinue
    Remove-Item $tmp -Recurse -Force -ErrorAction SilentlyContinue
}
