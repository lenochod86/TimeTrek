# Architektura TimeTrek

```
UI (React + TypeScript, Vite)          src/
  pages/      DayPage, StatsPage, ReportsPage
  components/ Timeline, SegmentList, AppRanking, Shots, RangeBar, Dialogs, TitleBar
  i18n/       slovníky textů (zatím cs)
        |  Tauri commands (invoke)
Jádro (Rust)                           src-tauri/src/
  tracker.rs   smyčka 1× / s -> Engine::step: aktivní okno + nečinnost + zámek -> segmenty v SQLite
  capture.rs   vlákno pro screenshoty (nezdržuje tracker)
  favicon.rs   vlákno pro stahování faviconů (přímo z webu, cache v DB)
  db.rs        SQLite (WAL), schéma, migrace
  commands.rs  příkazy volané z UI
  tray.rs      systray: Otevřít / Zavřít
  config.rs    settings.json, sdílené živě s trackerem
  platform/    JEDINÉ místo závislé na OS
```

## Měření

Záznam (*segment*) = souvislý úsek ve stejném okně (aplikace + titulek + doména).
Změní-li se titulek, začne nový segment, stejně jako v seznamu ve vzoru.
Úvodní symbolové znaky titulku se ignorují, protože terminály tam animují spinner.

`Engine::step` je čistá funkce (čas, sekundy nečinnosti, stav zámku, aktivní okno), proto je
chování pokryté testy se simulovaným časem:

- **Nečinnost** (žádný vstup po nastavený interval, výchozí 5 min) je segment typu `idle` (červeně).
  Začíná v okamžiku posledního vstupu, takže se doba nečinnosti odečte z odpracovaného času.
- **Zamčený počítač** se zjišťuje přímo (`OpenInputDesktop` selže na zabezpečené ploše; okno
  `LockApp.exe` se také bere jako zámek) a je nečinností od okamžiku zamčení.
- **Spánek / hibernace / vypnutí**: mezera mezi dvěma měřeními delší než 15 s uzavře otevřený segment
  tam, kde byl, a zvedne „podlahu“: nic před probuzením (nebo startem aplikace) se nikdy nedoplňuje,
  i když Windows po probuzení hlásí celý spánek jako nečinnost.
- Ukončení z tray otevřený segment uzavře.

## Přenositelnost (macOS)

Vše OS-specifické je za traitem `platform::PlatformApi`:
`foreground_window`, `idle_seconds`, `is_session_locked`, `app_name`, `app_icon_png`,
`browser_address`, `capture_window`. Windows implementace (`platform/windows.rs`) používá Win32
a UI Automation. Pro macOS stačí doplnit `platform/macos.rs` (NSWorkspace, CGWindowList,
Accessibility API); tracker, databáze ani UI se nemění.

## Weby a favicony

Adresa aktivní záložky se čte z adresního řádku přes UI Automation (Chromium omnibox má třídu
`OmniboxViewViews` nezávisle na jazyce). Ukládá se jen **doména**. Favicon se stáhne přímo
z navštíveného webu (`<link rel=icon>` → `/favicon.ico`), zmenší na 32×32 PNG a uloží do tabulky
`favicons`. Žádná externí služba. Anonymní okna (InPrivate / Incognito / „Anonymní“) se neukládají
ani nefotí.

## Databáze

`apps`, `segments`, `screenshots`, `favicons`, `tags`, `tag_ranges`; viz `db.rs`.
Čas je v milisekundách od epochy; hranice dne se počítají v UI (lokální půlnoc).

## Známá omezení

- UWP aplikace běží pod `ApplicationFrameHost.exe` (rozlišení konkrétní aplikace zatím chybí)
- Firefox: doména se nezjišťuje (jen Chromium prohlížeče)
- snímek minimalizovaného okna se nepořizuje
- okno UAC běží na zabezpečené ploše a po dobu jeho zobrazení se zapíše jako nečinnost
- ověřeno ve skutečné aplikaci: měření, favicony, schování do tray při zavření, snímky.
  Ověřeno jen testy (ne ve skutečné aplikaci): zámek, spánek, menu v tray, autostart.
