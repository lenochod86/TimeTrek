; Instalátor TimeTrek (NSIS). Sestavení: pwsh installer/Build-Installer.ps1
; Parametry příkazové řádky makensis:
;   /DVERSION=0.1.5            (povinné, musí odpovídat Cargo.toml)
;   /DEXE_PATH=cesta\timetrek.exe  (výchozí target\release\timetrek.exe)
;   /DOUT_DIR=cesta            (výchozí target\release\bundle\nsis)

Unicode true
SetCompressor /SOLID lzma

!ifndef VERSION
  !error "Zadej verzi: makensis /DVERSION=x.y.z installer\timetrek.nsi"
!endif
!ifndef EXE_PATH
  !define EXE_PATH "..\target\release\timetrek.exe"
!endif
!ifndef OUT_DIR
  !define OUT_DIR "..\target\release\bundle\nsis"
!endif

!define PRODUCT "TimeTrek"
!define EXE "timetrek.exe"
!define UNINST_KEY "Software\Microsoft\Windows\CurrentVersion\Uninstall\${PRODUCT}"
!define RUN_KEY "Software\Microsoft\Windows\CurrentVersion\Run"

!include "MUI2.nsh"
!include "hooks.nsh"

Name "${PRODUCT}"
OutFile "${OUT_DIR}\TimeTrek_${VERSION}_x64-setup.exe"
; Stejná složka jako dřívější instalátory, aby se aktualizace přepsala na místě
; (a sedělo spuštění po přihlášení uložené v registru).
InstallDir "$LOCALAPPDATA\${PRODUCT}"
InstallDirRegKey HKCU "Software\${PRODUCT}" ""
RequestExecutionLevel user
BrandingText "${PRODUCT} ${VERSION}"

!define MUI_ICON "..\assets\icon\timetrek.ico"
!define MUI_UNICON "..\assets\icon\timetrek.ico"
!define MUI_ABORTWARNING

!insertmacro MUI_PAGE_DIRECTORY
!insertmacro MUI_PAGE_INSTFILES
!define MUI_FINISHPAGE_RUN "$INSTDIR\${EXE}"
!define MUI_FINISHPAGE_RUN_TEXT "Spustit ${PRODUCT}"
!insertmacro MUI_PAGE_FINISH

!insertmacro MUI_UNPAGE_CONFIRM
!insertmacro MUI_UNPAGE_INSTFILES

!insertmacro MUI_LANGUAGE "Czech"

VIProductVersion "${VERSION}.0"
VIAddVersionKey /LANG=${LANG_CZECH} "ProductName" "${PRODUCT}"
VIAddVersionKey /LANG=${LANG_CZECH} "FileDescription" "Instalátor ${PRODUCT}"
VIAddVersionKey /LANG=${LANG_CZECH} "LegalCopyright" "MIT"
VIAddVersionKey /LANG=${LANG_CZECH} "FileVersion" "${VERSION}"
VIAddVersionKey /LANG=${LANG_CZECH} "ProductVersion" "${VERSION}"

Section "Install"
  ; Běžící instance (v systray) by držela soubor; ukončí se.
  nsExec::Exec 'taskkill /F /IM ${EXE}'
  Sleep 500

  SetOutPath "$INSTDIR"
  File "${EXE_PATH}"
  WriteUninstaller "$INSTDIR\uninstall.exe"

  CreateShortcut "$SMPROGRAMS\${PRODUCT}.lnk" "$INSTDIR\${EXE}"
  ; Zástupce na ploše, jen pokud ho už uživatel má (starší verze ho vytvářely).
  IfFileExists "$DESKTOP\${PRODUCT}.lnk" 0 +2
    CreateShortcut "$DESKTOP\${PRODUCT}.lnk" "$INSTDIR\${EXE}"

  WriteRegStr HKCU "Software\${PRODUCT}" "" "$INSTDIR"
  WriteRegStr HKCU "${UNINST_KEY}" "DisplayName" "${PRODUCT}"
  WriteRegStr HKCU "${UNINST_KEY}" "DisplayVersion" "${VERSION}"
  WriteRegStr HKCU "${UNINST_KEY}" "DisplayIcon" "$INSTDIR\${EXE}"
  WriteRegStr HKCU "${UNINST_KEY}" "InstallLocation" "$INSTDIR"
  WriteRegStr HKCU "${UNINST_KEY}" "UninstallString" '"$INSTDIR\uninstall.exe"'
  WriteRegStr HKCU "${UNINST_KEY}" "Publisher" "${PRODUCT}"
  WriteRegDWORD HKCU "${UNINST_KEY}" "NoModify" 1
  WriteRegDWORD HKCU "${UNINST_KEY}" "NoRepair" 1

  ; Verze do 0.1.5 byly postavené na Tauri a WebView2; jeho mezipaměť už nikdo nepoužívá.
  ; Uživatelská data (%AppData%\TimeTrek) se tu nikdy nemažou.
  RMDir /r "$LOCALAPPDATA\com.timetrek.app"
SectionEnd

Section "Uninstall"
  nsExec::Exec 'taskkill /F /IM ${EXE}'
  Sleep 500

  Delete "$INSTDIR\${EXE}"
  Delete "$INSTDIR\uninstall.exe"
  RMDir "$INSTDIR"
  Delete "$SMPROGRAMS\${PRODUCT}.lnk"
  Delete "$DESKTOP\${PRODUCT}.lnk"

  ; Spuštění po přihlášení (název shodný s src/autostart.rs).
  DeleteRegValue HKCU "${RUN_KEY}" "${PRODUCT}"
  DeleteRegKey HKCU "${UNINST_KEY}"
  DeleteRegKey HKCU "Software\${PRODUCT}"

  !insertmacro TT_ASK_DELETE_DATA
SectionEnd
