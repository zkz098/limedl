<#
.SYNOPSIS
    Run the limedl buffer pool benchmarks and show HDD optimization gains.

.DESCRIPTION
    Detects the disk type (HDD/SSD) of the target path, runs three I/O benchmarks:
      - direct_write  : no buffering (baseline)
      - double_hdd    : HDD double-buffer optimization
      - local_ssd     : SSD write-combining buffer
    Reports the throughput improvement of double-buffer vs direct write.

.PARAMETER DiskPath
    Target directory on the disk to benchmark. Defaults to $Env:TEMP if omitted.

.EXAMPLE
    .\scripts\bench-buffer-pool.ps1 -DiskPath D:\
#>

param(
    [string]$DiskPath = $Env:TEMP
)

$ErrorActionPreference = "Stop"

# ── 1. Detect disk type ───────────────────────────────────────────────────────
$diskRoot = (Get-Item -LiteralPath $DiskPath -Force).PSDrive.Root
$diskInfo = Get-PhysicalDisk |
    Where-Object {
        $_.FriendlyName -like "*$($diskRoot.TrimEnd('\'))*" -or
        (Get-Disk -Number $_.DeviceId | Get-Partition | Get-Volume | Where-Object {
            $_.DriveLetter -eq $diskRoot.TrimEnd(':').TrimEnd('\')
        })
    } |
    Select-Object -First 1

if ($diskInfo) {
    $mediaType = $diskInfo.MediaType
    Write-Host "Disk: $($diskInfo.FriendlyName) ($mediaType)" -ForegroundColor Cyan
} else {
    $mediaType = "Unknown"
    Write-Host "Disk: $DiskPath (type: unknown)" -ForegroundColor Yellow
}

# ── 2. Ensure the target directory exists ─────────────────────────────────────
$benchDir = Join-Path $DiskPath "limedl_bench"
New-Item -ItemType Directory -Force -Path $benchDir | Out-Null
$Env:BENCH_DISK = $benchDir

# ── 3. Initialize MSVC environment ────────────────────────────────────────────
Write-Host "`nInitializing MSVC toolchain..." -ForegroundColor Gray
& "C:\Program Files\Microsoft Visual Studio\18\Community\VC\Auxiliary\Build\vcvarsall.bat" x64
if (-not $?) {
    Write-Warning "MSVC init failed — Rust may not link correctly."
}

# ── 4. Run benchmarks ─────────────────────────────────────────────────────────
Write-Host "`nRunning buffer pool benchmarks (100 MB write, 1 MB chunks)...`n" -ForegroundColor Green

$projectRoot = Split-Path -Parent (Split-Path -Parent $PSScriptRoot)
Push-Location -LiteralPath $projectRoot

try {
    cargo bench --manifest-path src-tauri/Cargo.toml --features test-utils -- buffer_pool
} finally {
    Pop-Location
}

# ── 5. Cleanup ────────────────────────────────────────────────────────────────
Remove-Item -Recurse -Force -LiteralPath $benchDir -ErrorAction SilentlyContinue

Write-Host "`n───────────────────────────────────────────────────" -ForegroundColor Cyan
Write-Host "Benchmark complete. Compare 'direct_write' (baseline)" -ForegroundColor White
Write-Host "with 'double_hdd' to see the HDD optimization gain."   -ForegroundColor White
if ($mediaType -eq "SSD") {
    Write-Host "Note: running on SSD — double_hdd will show negligible" -ForegroundColor Yellow
    Write-Host "improvement. Re-run with -DiskPath pointing to an HDD."  -ForegroundColor Yellow
}
Write-Host "───────────────────────────────────────────────────`n" -ForegroundColor Cyan
