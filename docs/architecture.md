# Architektura TimeTrek

Jeden proces v Rustu, bez prohlížeče: jádro (měření, databáze) a rozhraní ve [Slintu](https://slint.dev)
se software rendererem (kreslí procesorem, bez GPU a bez WebView2).

```
ui/*.slint     rozhraní (jen vzhled): main, day, stats, reports, dialogs, widgets, theme, texts, state
src/
  app.rs       životní cyklus: data, tracker, tray a okno, které existuje jen když je otevřené
  ui/          Ctl (řadič okna): převádí data na modely UI a obsluhuje zpětná volání
    day.rs       záložka Den (osa, seznam, žebříček), timeline.rs (výpočty a kreslení pruhů do bitmapy)
    range.rs     Statistika a Přehledy, preview.rs (náhled snímku, prohlížeč), dialogs.rs, fmt.rs, colors.rs
  data.rs      dotazy do databáze pro UI (testovatelné bez okna)
  tracker.rs   smyčka 1× / s -> Engine::step: aktivní okno + nečinnost + zámek -> segmenty v SQLite
  capture.rs   vlákno pro screenshoty (nezdržuje tracker)
  favicon.rs   vlákno pro stahování faviconů (přímo z webu, cache v DB)
  db.rs        SQLite (WAL), schéma, migrace
  tray.rs      systray (tray-icon): Otevřít / Zavřít
  config.rs    settings.json, sdílené živě s trackerem
  single_instance.rs   jediná instance (lokální soket), autostart.rs   spuštění po přihlášení
  platform/    JEDINÉ místo závislé na OS (windows, macos, stub pro Linux)
installer/     instalátor NSIS (timetrek.nsi, hooks.nsh, Build-Installer.ps1)
examples/preview.rs   vykreslení UI do PNG bez okna (kontrola vzhledu)
```

## Okno a paměť

Hlavní důvod přechodu z Tauri/React: WebView2 držel několik procesů (stovky MB) i při skrytém okně.
Teď okno **existuje jen když je otevřené**. Zavření křížkem zahodí okno, `Ctl`, obrázky i spojení s databází
a v systray zůstane jen jádro (tracker a vlákna pro snímky a favicony). Otevření z tray (nebo druhé
spuštění aplikace) okno vytvoří znovu. Při startu s `--hidden` (autostart) se okno nevytváří vůbec.

- Smyčku událostí obsluhuje Slint (`run_event_loop_until_quit`); ikonu v systray a její menu obsluhuje
  `tray-icon` ve stejné smyčce a události přeposílá přes `slint::invoke_from_event_loop`.
- Rozhraní se kreslí software rendererem (`renderer-software`, systémová písma). Pruhy časové osy kreslí Rust
  přímo do bitmapy (`ui/timeline.rs`); tisíce obdélníků jako prvků UI by byly pomalé a paměťově drahé.
- Vlastní záhlaví okna z Tauri verze (tažení, minimalizace, maximalizace) se nepřenášelo; okno má standardní
  rám systému (méně kódu a rizika). Záložky, přepínač motivu, nastavení a o aplikaci jsou v liště pod rámem.
- Obrázky snímků se čtou z disku na požádání a drží se jen aktuální (žádná mezipaměť dekódovaných snímků).
- Motiv (tmavý / světlý) je v `ui/theme.slint` (globál `Theme`) a ukládá se do `settings.json` (`light_theme`).
- Texty UI jsou v `ui/texts.slint`, texty skládané v Rustu (tray, dny v týdnu, období) v `src/i18n.rs`.

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
Accessibility API); tracker, databáze ani UI se nemění. Na ostatních systémech (Linux) je `platform/stub.rs`,
který nic nezjišťuje; slouží jen k vývoji UI a ke spuštění testů.

## Weby a favicony

Adresa aktivní záložky se čte z adresního řádku přes UI Automation (Chromium omnibox má třídu
`OmniboxViewViews` nezávisle na jazyce). Ukládá se jen **doména**. Favicon se stáhne přímo
z navštíveného webu (`<link rel=icon>` → `/favicon.ico`), zmenší na 32×32 PNG a uloží do tabulky
`favicons`. Žádná externí služba. Anonymní okna (InPrivate / Incognito / „Anonymní“) se neukládají
ani nefotí.

## Databáze

`apps`, `segments`, `screenshots`, `favicons`, `tags`, `tag_ranges`; viz `db.rs`.
Čas je v milisekundách od epochy; hranice dne se počítají v UI (lokální půlnoc, `ui/fmt.rs`).

## Známá omezení

- UWP aplikace běží pod `ApplicationFrameHost.exe` (rozlišení konkrétní aplikace zatím chybí)
- Firefox: doména se nezjišťuje (jen Chromium prohlížeče)
- snímek minimalizovaného okna se nepořizuje
- okno UAC běží na zabezpečené ploše a po dobu jeho zobrazení se zapíše jako nečinnost
- Po přechodu na Slint (verze po 0.1.5) bylo UI ověřeno jen vykreslením bez okna (`examples/preview.rs`, obě
  témata) a testy; ve skutečné Windows aplikaci neotestováno: tray a okno, které se zahazuje a znovu vytváří,
  kolečko myši na časové ose (směr zoomu), tažení a výběr značek, instalátor (`installer/timetrek.nsi`), autostart
  a zachování volby autostartu z Tauri verzí (klíč `TimeTrek` v registru zůstal stejný).
- Dříve ověřeno ve skutečné aplikaci (Tauri verze): měření, favicony, snímky. Ověřeno jen testy: zámek, spánek.
- Slint nemá výběr data; datum se píše jako `D.M.RRRR` (nebo `RRRR-MM-DD`) a potvrzuje Enterem.
  Tooltipy (časová osa, heatmapa) řeší globál `Tip` v `ui/state.slint`.
