---
name: drive-timetrek
description: Spustí vývojovou aplikaci TimeTrek a vyfotí její okno, aby se změna UI nebo chování ověřila ve skutečné aplikaci
---

# Ovládání TimeTrek

1. Zavři běžící TimeTrek (nainstalovaný i vývojový); smí běžet jen jedna instance:
   `Get-Process timetrek -ErrorAction SilentlyContinue | Stop-Process -Force`
2. Spusť vývojovou aplikaci na pozadí: `cargo run` (první start se několik minut kompiluje; počkej na `Running target\debug\timetrek.exe` ve výstupu).
3. Vyfoť okno:
   `pwsh .claude/skills/drive-timetrek/screenshot.ps1 -Out <scratchpad>\win.png`
   a podívej se na snímek nástrojem Read. Skript okno nejdřív přenese do popředí.
4. Měření vyzkoušíš přepínáním do jiných oken (aplikace zapisuje aktivní okno); záložka Den se obnovuje každé 3 sekundy.
5. Až skončíš, zastav vývojovou aplikaci (`Stop-Process -Name timetrek`).

Poznámky:
- Okno zavřené křížkem se zahodí (zůstane jen tray) a skript ho nevyfotí; otevřeš ho z tray, nebo aplikaci spusť znovu (druhé spuštění okno přivolá).
- Aplikace spuštěná s `--hidden` okno nevytvoří.
- Bez Windows (např. cloudové prostředí) jde vzhled zkontrolovat `cargo run --example preview` (vykreslí UI do PNG, viz README).
- Snímky uživatelovy obrazovky nikdy nepatří do repozitáře; ukládej je do scratchpadu.
