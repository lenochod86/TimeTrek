# TimeTrek

Malý hobby time tracker pro Windows (případně později macOS): běží v systray, měří, jak dlouho uživatel pracuje v jaké aplikaci a na jakém webu, pořizuje snímky aktivního okna a zobrazuje přehledy dne, statistiky a reporty. Stack: Tauri 2 (jádro v Rustu) + React/TypeScript UI.

Tyto instrukce platí v každém prostředí. Pracuj všude stejně.

## Jazyk

- **S uživatelem mluv česky.**
- **Vše v repozitáři je česky:** dokumentace, commit zprávy i texty v UI. Žádné pravidlo o angličtině tu neplatí.
- Identifikátory v kódu zůstávají anglicky (běžná praxe) a stávající anglické komentáře nepřepisuj. Nové komentáře piš česky.
- Texty UI jsou ve slovníku `src/i18n/cs.ts` (dále menu v tray v `src-tauri/src/tray.rs` a pár uložených zástupných titulků v trackeru), ne přímo v komponentách.

## Dokumenty

- `docs/architecture.md`: architektura, rozhodnutí, známá omezení. Před změnou architektury si ho přečti a průběžně ho aktualizuj.
- `README.md`: funkce, vývoj a build.

## Git (malý projekt: žádné issues ani pull requesty)

Repozitář: `lenochod86/TimeTrek`, větev `main`.

- Commituj rovnou do `main`. Žádné issues, větve, pull requesty ani labely.
- **Po každé hotové práci commitni a pushni do `origin/main` bez ptaní.**
- Malé, samostatné commity. Věta v rozkazovacím způsobu, nejvýše 72 znaků v předmětu; tělo říká proč a co.
- Před každým commitem spusť kontroly níže. Nikdy necommituj červený stav.
- Nikdy nedělej force push a nepřepisuj historii, která už je pushnutá.
- Na začátku session spusť `git status` a `git fetch`. Nikdy nepřepisuj ani nemaž necommitnuté změny; napřed se zeptej.
- Do repozitáře nikdy nedávej: hesla, tokeny, uživatelova data (`%AppData%\TimeTrek`: databáze, snímky, nastavení) ani snímky uživatelovy obrazovky. Generované soubory (`node_modules/`, `dist/`, `src-tauri/target/`, `src-tauri/gen/`) zůstávají v `.gitignore`.
- Když GitHub není dostupný, řekni to uživateli. Push nikdy potichu nevynechej.

## Kontroly před commitem

- `pwsh .github/scripts/Verify-Local.ps1`: shoda verzí, `tsc`, `vite build` a `cargo test` (žádná varování). To je to, co by dělalo CI, jen zdarma na tomto počítači.
- Cokoli, co mění chování, se navíc vyzkouší ve skutečné aplikaci (viz „Ověření ve skutečné aplikaci“).

## CI/CD a minuty GitHub Actions

Repozitář je privátní a minuty Actions jsou omezené (uživatel je pro tento měsíc vyčerpal; Windows runner se počítá 2×). Proto:

- Workflow v `.github/workflows/` se **nikdy nespouští automaticky při pushi ani PR**. `ci.yml` je jen ruční (`workflow_dispatch`), `release.yml` jen na tag `v*` nebo ručně. Nepřidávej triggery `push` / `pull_request` bez souhlasu uživatele.
- Běžnou kontrolu dělá lokálně `Verify-Local.ps1`; nepouštěj workflow jen kvůli ověření.
- Vydání verze: `pwsh .github/scripts/Release-Local.ps1` (ověří, sestaví instalátor na tomto počítači, vytvoří tag a GitHub Release; nepálí minuty). Je to venku viditelná akce, proto jen na výslovné přání uživatele. Verzi před tím zvyš a zacommituj (viz Build a verze).
- `Check-Versions.ps1` hlídá, že `package.json`, `Cargo.toml` a `tauri.conf.json` mají stejnou verzi (u tagu i shodu s tagem).

## Architektura a kód

- Jádro v Rustu (`src-tauri/src`): měření, ukládání, snímky, favicony, tray, příkazy. React UI (`src`) jen zobrazuje data a volá příkazy Tauri.
- **Veškerý kód závislý na OS je za `platform::PlatformApi`** (`platform/windows.rs`, `platform/macos.rs` jako zástupce). Tracker, databáze ani UI nevolají API operačního systému přímo. Nová schopnost OS znamená novou metodu traitu a implementaci pro každou platformu.
- Rozhodování měření je v `Engine::step` (`tracker.rs`), čisté funkci času, sekund nečinnosti, stavu zámku a aktivního okna. Tak to nech, aby šla testovat se simulovaným časem.
- Nikdy neblokuj smyčku trackeru: pomalá práce (snímky, stahování faviconů) běží na vlastním vlákně.
- Soukromí: vše zůstává na počítači. Ukládej jen **doménu** webu, nikdy celou URL. Anonymní okna prohlížeče se neměří ani nefotí. Favicony stahuj přímo z navštíveného webu, nikdy přes službu třetí strany. Žádná telemetrie.
- **Data jsou v `%AppData%\TimeTrek`** (`app.path().data_dir().join("TimeTrek")`). Generovaný odinstalátor Tauri maže jen složky podle identifikátoru, proto tuto složku při volbě „Delete the application data“ maže vlastní krok v `src-tauri/windows/hooks.nsh` (`installerHooks` v `tauri.conf.json`). Změníš-li název složky, změň ho i tam (`TT_DATA_DIRNAME`).
- Čas se ukládá v milisekundách od epochy; hranice dne (lokální půlnoc) se počítají v UI.
- Každé nové pravidlo měření má test v `tracker.rs`; každá oprava chyby má test, který chybu chytí.
- Drž styl okolního kódu. Nastavení, která uživatel může měnit, patří do dialogu Nastavení (`src/components/Dialogs.tsx`) a uplatní se bez restartu.

## Build a verze

- Vývojový běh: `npm run tauri dev`. Instalátor: `npx tauri build` -> `src-tauri/target/release/bundle/nsis/TimeTrek_<verze>_x64-setup.exe`.
- Ikony se generují z `assets/icon/timetrek.svg` příkazem `npm run icons`.
- U každého instalátoru předaného uživateli zvyš verzi (`package.json`, `src-tauri/Cargo.toml` a `src-tauri/tauri.conf.json` musí mít stejnou) a řekni, o jakou verzi jde.
- Když `node`, `cargo` nebo `npx` v novém shellu nejsou na PATH, obnov PATH ze systémového a uživatelského prostředí (instalovaly se přes winget).

## Ověření ve skutečné aplikaci

- Spusť vývojovou aplikaci, ovládej ji a podívej se na výsledek: `.claude/skills/drive-timetrek/` má skript, který vyfotí okno TimeTrek.
- Nic neoznačuj za ověřené, dokud to neběželo ve skutečné aplikaci. Věci, které potřebují uživatele (zamčení PC, spánek, menu v tray, autostart po novém přihlášení), uveď jako „neotestováno“ a předej je s instalátorem.
- Vývojová i nainstalovaná aplikace sdílejí stejný adresář s daty a smí běžet jen jedna instance, takže před testem zavři nainstalovaný TimeTrek. Zavřít nebo ukončit TimeTrek kvůli testu smíš bez ptaní. Adresář s daty uživatele nikdy nemaž.

## Jak pracovat

- Když něco nevíš nebo je to rozhodnutí uživatele, zeptej se. Nevymýšlej si přání uživatele.
- Žádné průběžné popisování kroků. Řekni jednu krátkou větu na začátku, ptej se jen když jsi zablokovaný a skonči shrnutím česky.
- Hlas výsledky přesně: co prošlo, co selhalo (s výstupem) a co se netestovalo.
- Soubory s apostrofy zapisuj nástrojem Write, ne přes bash heredoc.
