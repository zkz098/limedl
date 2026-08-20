# Tauri dev wrapper for Windows — ensures MSVC + Windows SDK env is loaded before cargo
# Usage: powershell -ExecutionPolicy Bypass -File scripts/tauri-dev.ps1
# Or:    pnpm run tauri:dev:win

$ErrorActionPreference = "Stop"

$vcvars = "C:\Program Files\Microsoft Visual Studio\18\Community\VC\Auxiliary\Build\vcvarsall.bat"
if (-not (Test-Path $vcvars)) {
  # Fallback: try VS 2022 path
  $vcvars = "C:\Program Files\Microsoft Visual Studio\2022\Community\VC\Auxiliary\Build\vcvarsall.bat"
}
if (-not (Test-Path $vcvars)) {
  Write-Error "找不到 vcvarsall.bat，请确认已安装 Visual Studio 2022/2026 + Windows SDK"
  exit 1
}

Write-Host "→ 初始化 MSVC 环境: $vcvars x64" -ForegroundColor Cyan
# Use cmd to call vcvarsall and then run pnpm in the same cmd session so env propagates
$cmd = "call `"$vcvars`" x64 && pnpm run tauri dev -- %*"
# %* passes through args if any
if ($args.Count -gt 0) {
  $extra = $args -join " "
  $cmd = "call `"$vcvars`" x64 && pnpm run tauri dev $extra"
}
cmd /c $cmd
exit $LASTEXITCODE
