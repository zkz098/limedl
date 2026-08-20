@echo off
REM Tauri dev wrapper for Windows — ensures MSVC env before cargo
REM Usage: scripts\tauri-dev.cmd  or  pnpm run tauri:dev:win

set "VCVARS=C:\Program Files\Microsoft Visual Studio\18\Community\VC\Auxiliary\Build\vcvarsall.bat"
if not exist "%VCVARS%" set "VCVARS=C:\Program Files\Microsoft Visual Studio\2022\Community\VC\Auxiliary\Build\vcvarsall.bat"
if not exist "%VCVARS%" (
  echo 找不到 vcvarsall.bat，请确认已安装 Visual Studio + Windows SDK
  exit /b 1
)

echo → 初始化 MSVC 环境: %VCVARS% x64
call "%VCVARS%" x64
if errorlevel 1 exit /b %errorlevel%

pnpm run tauri dev %*
