; Vlastní kroky instalátoru (NSIS). Zapojeno v tauri.conf.json -> bundle.windows.nsis.installerHooks.
;
; Generovaný odinstalátor Tauri při volbě "Delete the application data" maže jen složky podle
; identifikátoru aplikace (%AppData%\com.timetrek.app). Data TimeTrek jsou ale v hezky
; pojmenované složce %AppData%\TimeTrek, proto ji tu mažeme se stejnou podmínkou jako Tauri
; (zaškrtnutá volba a nejde o aktualizaci).

!ifndef TT_DATA_DIRNAME
  !define TT_DATA_DIRNAME "TimeTrek"
!endif

!macro NSIS_HOOK_POSTUNINSTALL
  ${If} $DeleteAppDataCheckboxState = 1
  ${AndIf} $UpdateMode <> 1
    SetShellVarContext current
    RmDir /r "$APPDATA\${TT_DATA_DIRNAME}"
  ${EndIf}
!macroend
