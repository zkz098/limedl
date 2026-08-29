; ═══════════════════════════════════════════════════════════════════════════
; limedl-native — per-user NSIS installer (Windows)
;
; Modeled on the Tauri NSIS template (MIT/Apache-2.0) with the same
; updater-facing command-line contract:
;   /S            fully silent (NSIS built-in), no UI at all
;   /P            passive: progress UI only, no questions, auto-closes
;   /R            launch the app after the install completes (updater path)
;   /D=<dir>      override install directory (NSIS built-in)
;
; Per-user by design: installs to %LOCALAPPDATA%\Programs\limedl, writes only
; to HKCU, and never triggers UAC — so the self-updater can run silently.
; The Uninstall registry key below (HKCU\...\Uninstall\limedl-native) is the
; marker crates/limedl-native/src/update.rs uses to detect the "installer"
; distribution channel.
; ═══════════════════════════════════════════════════════════════════════════

!define APP_NAME "limedl"
!define APP_EXE "limedl-native.exe"
!define UNINST_KEY "Software\Microsoft\Windows\CurrentVersion\Uninstall\limedl-native"
!define AUTORUN_KEY "Software\Microsoft\Windows\CurrentVersion\Run"
!define AUTORUN_VALUE "limedl-native"

; ── CI overrides ────────────────────────────────────────────────────────────
; makensis /DVERSION=0.2.1 /DOUTFILE=... installer\limedl-native.nsi
!ifndef VERSION
  !define VERSION "0.0.0"
!endif
!ifndef OUTFILE
  !define OUTFILE "limedl-native-setup.exe"
!endif

Unicode true
ManifestDPIAware true

Name "${APP_NAME} ${VERSION}"
OutFile "${OUTFILE}"
InstallDir "$LOCALAPPDATA\Programs\limedl"
; Reuse the previously chosen install dir on silent upgrades.
InstallDirRegKey HKCU "${UNINST_KEY}" "InstallLocation"
RequestExecutionLevel user
; /S gives full silence; passive is implemented below by skipping pages.
SilentInstall silent

Var InstallMode        ; "normal" | "passive" | "silent"
Var RestartApp         ; "1" when /R was passed

!include "MUI2.nsh"
!include "FileFunc.nsh"

!define MUI_ABORTWARNING
!define MUI_ICON "${NSISDIR}\Contrib\Graphics\Icons\modern-install.ico"
!define MUI_UNICON "${NSISDIR}\Contrib\Graphics\Icons\modern-uninstall.ico"

; Finish page with a run-after-install checkbox (interactive installs only).
!define MUI_FINISHPAGE_RUN "$INSTDIR\${APP_EXE}"
!define MUI_FINISHPAGE_RUN_CHECKED

; Pages: Welcome/Directory/Finish are skipped in passive mode (the page-skip
; hook Abort's before they show, leaving only the InstFiles progress bar).
; Each hook define must be !undef'd before the next use.
!define MUI_PAGE_CUSTOMFUNCTION_PRE SkipInPassive
!insertmacro MUI_PAGE_WELCOME
!undef MUI_PAGE_CUSTOMFUNCTION_PRE
!define MUI_PAGE_CUSTOMFUNCTION_PRE SkipInPassive
!insertmacro MUI_PAGE_DIRECTORY
!undef MUI_PAGE_CUSTOMFUNCTION_PRE
!insertmacro MUI_PAGE_INSTFILES
!define MUI_PAGE_CUSTOMFUNCTION_PRE SkipInPassive
!insertmacro MUI_PAGE_FINISH

!insertmacro MUI_UNPAGE_CONFIRM
!insertmacro MUI_UNPAGE_INSTFILES

!insertmacro MUI_LANGUAGE "SimpChinese"
!insertmacro MUI_LANGUAGE "English"

; ── .onInit: parse updater flags ────────────────────────────────────────────
Function .onInit
  StrCpy $InstallMode "normal"
  StrCpy $RestartApp ""

  ${GetParameters} $R0
  ; NSIS already switched to silent mode for /S; record it for page logic.
  ${If} ${Silent}
    StrCpy $InstallMode "silent"
  ${EndIf}

  ClearErrors
  ${GetOptions} $R0 "/P" $R1
  ${IfNot} ${Errors}
    StrCpy $InstallMode "passive"
  ${EndIf}

  ClearErrors
  ${GetOptions} $R0 "/R" $R1
  ${IfNot} ${Errors}
    StrCpy $RestartApp "1"
  ${EndIf}
FunctionEnd

; ── Page skipping: passive installs show progress only ──────────────────────
Function SkipInPassive
  ${If} $InstallMode == "passive"
    Abort
  ${EndIf}
FunctionEnd

; ── Helpers ─────────────────────────────────────────────────────────────────
; Stop a running instance: graceful WM_CLOSE first, then force. The updater
; exits before spawning this installer, so in the silent path this is a no-op
; safety net for manually launched installs.
Function StopRunningApp
  DetailPrint "Stopping running limedl instance..."
  nsExec::Exec 'taskkill /IM "${APP_EXE}"'
  Sleep 800
  nsExec::Exec 'taskkill /F /IM "${APP_EXE}"'
FunctionEnd

; ── Install section ─────────────────────────────────────────────────────────
Section "Install"
  Call StopRunningApp

  SetOutPath "$INSTDIR"
  File "${APP_EXE}"

  ; Start Menu shortcut (per-user).
  CreateDirectory "$SMPROGRAMS\${APP_NAME}"
  CreateShortCut "$SMPROGRAMS\${APP_NAME}\${APP_NAME}.lnk" "$INSTDIR\${APP_EXE}"

  ; Uninstall registry entries (HKCU — per-user marker for the updater).
  WriteRegStr HKCU "${UNINST_KEY}" "DisplayName" "${APP_NAME}"
  WriteRegStr HKCU "${UNINST_KEY}" "DisplayVersion" "${VERSION}"
  WriteRegStr HKCU "${UNINST_KEY}" "Publisher" "zkz098"
  WriteRegStr HKCU "${UNINST_KEY}" "InstallLocation" "$INSTDIR"
  WriteRegStr HKCU "${UNINST_KEY}" "DisplayIcon" "$INSTDIR\${APP_EXE}"
  WriteRegStr HKCU "${UNINST_KEY}" "UninstallString" '"$INSTDIR\uninstall.exe"'
  WriteRegStr HKCU "${UNINST_KEY}" "QuietUninstallString" '"$INSTDIR\uninstall.exe" /S'
  WriteRegDWORD HKCU "${UNINST_KEY}" "NoModify" 1
  WriteRegDWORD HKCU "${UNINST_KEY}" "NoRepair" 1

  ; Uninstaller.
  WriteUninstaller "$INSTDIR\uninstall.exe"
SectionEnd

; ── Post-install ────────────────────────────────────────────────────────────
Function .onInstSuccess
  ; Interactive installs launch via the MUI finish page checkbox; the /R
  ; updater path relaunches from passive/silent installs.
  ${If} $RestartApp == "1"
    ${If} $InstallMode == "passive"
    ${OrIf} $InstallMode == "silent"
      Exec '"$INSTDIR\${APP_EXE}"'
    ${EndIf}
  ${EndIf}

  ; Passive: close as soon as the install finishes.
  ${If} $InstallMode == "passive"
    SetAutoClose true
  ${EndIf}
FunctionEnd

; ── Uninstaller ─────────────────────────────────────────────────────────────
Section "Uninstall"
  nsExec::Exec 'taskkill /F /IM "${APP_EXE}"'
  Sleep 400

  Delete "$INSTDIR\uninstall.exe"
  Delete "$INSTDIR\${APP_EXE}"
  RMDir "$INSTDIR"

  Delete "$SMPROGRAMS\${APP_NAME}\${APP_NAME}.lnk"
  RMDir "$SMPROGRAMS\${APP_NAME}"

  DeleteRegKey HKCU "${UNINST_KEY}"
  ; Remove the registry autostart written by the app itself.
  DeleteRegValue HKCU "${AUTORUN_KEY}" "${AUTORUN_VALUE}"
SectionEnd
