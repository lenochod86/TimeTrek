# Architektura TimeTrek

```
UI (React + TypeScript, Vite)          src/
  pages/      DayPage, StatsPage, ReportsPage
  components/ Timeline, SegmentList, AppRanking, Shots, RangeBar, Dialogs, TitleBar
  i18n/       slovníky textů (zatím cs)
        │  Tauri commands (invoke)
Core (Rust)                            src-tauri/src/
  tracker.rs   smyčka 1× / s: aktivní okno + nečinnost → segmenty v SQLite
  capture.rs   vlákno pro screenshoty (nezdržuje tracker)
  favicon.rs   vlákno pro stahování faviconů (přímo z webu, cache v DB)
  db.rs        SQLite (WAL), schéma, migrace
  commands.rs  příkazy volané z UI
  tray.rs      systray: Otevřít / Zavřít
  config.rs    settings.json
  platform/    JEDINÉ místo závislé na OS
```

## Měření

Záznam (*segment*) = souvislý úsek ve stejném okně (aplikace + titulek + doména).
Změní-li se titulek, začne nový segment – stejně jako v seznamu ve vzoru.
Nečinnost (≥ 5 min bez vstupu, zamčená obrazovka) je segment typu `idle`; začátek
se zpětně posune na okamžik posledního vstupu, takže se nečinnost nezapočítá.
Spánek počítače (>15 s mezera) uzavře otevřený segment.

## Přenositelnost (macOS)

Vše OS-specifické je za traitem `platform::PlatformApi`:
`foreground_window`, `idle_seconds`, `app_name`, `app_icon_png`, `browser_address`, `capture_window`.
Windows implementace (`platform/windows.rs`) používá Win32 + UI Automation.
Pro macOS stačí doplnit `platform/macos.rs` (NSWorkspace, CGWindowList, Accessibility API);
tracker, databáze ani UI se nemění.

## Weby a favicony

Adresa aktivní záložky se čte z adresního řádku přes UI Automation
(Chromium omnibox má třídu `OmniboxViewViews` nezávisle na jazyce). Ukládá se jen **doména**.
Favicon se stáhne přímo z navštíveného webu (`<link rel=icon>` → `/favicon.ico`),
zmenší na 32×32 PNG a uloží do tabulky `favicons`. Žádná externí služba.
Anonymní okna (InPrivate / Incognito / „Anonymní“) se neukládají ani nefotí.

## Databáze

`apps`, `segments`, `screenshots`, `favicons`, `tags`, `tag_ranges` – viz `db.rs`.
Čas je v milisekundách od epochy; hranice dne se počítají v UI (lokální půlnoc).

## Známá omezení

- UWP aplikace běží pod `ApplicationFrameHost.exe` (rozlišení konkrétní aplikace zatím chybí)
- Firefox: doména se nezjišťuje (jen Chromium prohlížeče)
- screenshot minimalizovaného okna se nepořizuje
