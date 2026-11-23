# TimeTrek

Automatický time tracker pro Windows (připravený na port na macOS). Běží na pozadí v systray,
měří, kolik času trávíš v jakých aplikacích (a na jakých webech), a ukládá snímky okna,
takže se později dá dohledat, co jsi dělal.

## Vlastnosti

- měření aktivního okna 1× za sekundu včetně nečinnosti (po nastaveném intervalu, výchozí 5 min,
  se čas přestane počítat a zobrazí se červeně); zamčený počítač se zapíše jako nečinnost,
  spánek a vypnutí se nezapisují vůbec
- snímky aktivního okna (při změně okna a pak každých 30 s)
- u Edge/Chrome/Brave/Vivaldi/Opera se rozpozná doména a zobrazí se favicon webu
  (anonymní okna se neměří a nefotí)
- záložky **Den** (timeline, seznam záznamů, žebříček aplikací, náhled snímku), **Statistika** a **Přehledy**
- zavření okna = okno se úplně zahodí a v systray zůstane jen měření (malá spotřeba paměti);
  levý klik na ikonu nebo pravé tlačítko → **Otevřít** / **Zavřít** (ukončí měření)
- tmavý a světlý režim (přepínač ikonou slunce / měsíce v horní liště)
- Nastavení → General → **Spustit při startu** a **Interval nečinnosti**

## Vývoj

Požadavky: Rust (MSVC toolchain) a Visual Studio Build Tools (C++). Rozhraní je ve Slintu (software
renderer, žádný prohlížeč ani WebView), takže Node.js není potřeba.

```powershell
cargo run                     # vývojový běh
pwsh installer/Build-Installer.ps1   # instalátor (NSIS) v target\release\bundle\nsis (potřebuje NSIS: winget install NSIS.NSIS)
cargo run --manifest-path tools/make-icons/Cargo.toml   # přegeneruje ikony z assets/icon/timetrek.svg
cargo test
```

Vzhled UI jde zkontrolovat i bez Windows: `cargo run --example preview -- --theme light --tab reports --out snimek.png`
vykreslí okno s vymyšlenými daty do PNG (volby viz začátek `examples/preview.rs`).

### CI/CD

Build a kontroly běží na serveru (GitHub Actions):

- `.github/workflows/ci.yml`: při každém pushi do `main` a při PR (verze, překlad, testy)
- `.github/workflows/release.yml`: sestaví instalátor a vydá GitHub Release při pushi tagu `vX.Y.Z`;
  ručně (Run workflow) jen sestaví instalátor jako artefakt
- `pwsh .github/scripts/Release.ps1`: vytvoří a pushne tag (po zvýšení verze v `Cargo.toml`), instalátor
  pak sestaví server
- `pwsh .github/scripts/Verify-Local.ps1`: stejné kontroly jako CI, lokálně

Data jsou v `%AppData%\TimeTrek` (`timetrek.db`, `screens/`, `settings.json`). Při odinstalaci se instalátor
zeptá, jestli je smazat (`installer/hooks.nsh`). Pokročilé volby (interval snímků, retence…) jsou v `settings.json`.

Architektura: viz [docs/architecture.md](docs/architecture.md). Instrukce pro Claude Code: [CLAUDE.md](CLAUDE.md).

## Licence

[MIT](LICENSE)
