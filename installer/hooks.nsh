; Vlastní kroky instalátoru (NSIS), vkládá je installer/timetrek.nsi.
;
; Data TimeTrek jsou ve složce %AppData%\TimeTrek (databáze, snímky, nastavení). Při odinstalaci
; se uživatele zeptáme, jestli je smazat; v tichém režimu (/S) a při aktualizaci se nemažou nikdy.
; Název složky musí odpovídat kódu v src/app.rs (`data_dir().join("TimeTrek")`).

!ifndef TT_DATA_DIRNAME
  !define TT_DATA_DIRNAME "TimeTrek"
!endif

!macro TT_ASK_DELETE_DATA
  MessageBox MB_YESNO|MB_ICONQUESTION|MB_DEFBUTTON2 \
    "Smazat také data aplikace (databázi, snímky a nastavení ve složce $APPDATA\${TT_DATA_DIRNAME})?" \
    /SD IDNO IDNO tt_keep_data
  SetShellVarContext current
  RMDir /r "$APPDATA\${TT_DATA_DIRNAME}"
  tt_keep_data:
!macroend
