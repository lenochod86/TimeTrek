# TimeTrek

Automatický time tracker pro Windows (připravený na port na macOS). Běží na pozadí v systray,
měří, kolik času trávíš v jakých aplikacích (a na jakých webech), a ukládá snímky okna,
takže se později dá dohledat, co jsi dělal.

## Vlastnosti

- měření aktivního okna 1× za sekundu, včetně nečinnosti (po 5 min bez vstupu se čas nepočítá)
- screenshoty aktivního okna (při změně okna a pak každých 30 s)
- u Edge/Chrome/Brave/Vivaldi/Opera se rozpozná doména a zobrazí se favicon webu
  (anonymní okna se neměří a nefotí)
- záložky **Den** (timeline, seznam záznamů, žebříček aplikací, náhled snímku), **Statistika** a **Přehledy**
- zavření okna = schování do systray; pravé tlačítko na ikonu → **Otevřít** / **Zavřít** (ukončí měření)
- Nastavení → General → **Spustit při startu**

## Vývoj

Požadavky: Node.js LTS, Rust (MSVC toolchain) a Visual Studio Build Tools (C++).

```powershell
npm install
npm run tauri dev      # vývojový režim
npm run tauri build    # instalátor (NSIS) v src-tauri/target/release/bundle
npm run icons          # přegeneruje ikony z assets/icon/timetrek.svg
```

Data jsou v `%AppData%\TimeTrek` (`timetrek.db`, `screens/`, `settings.json`).
Pokročilé volby (nečinnost, interval snímků, retence…) jsou v `settings.json`.

Architektura: viz [docs/architecture.md](docs/architecture.md).
