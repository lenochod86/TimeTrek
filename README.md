# TimeTrek

Automatic time tracker for Windows (prepared for a macOS port). It runs in the system tray,
measures how long you work in which application (and on which website) and stores
screenshots of the window, so you can later find out what you were doing.

## Features

- tracks the active window once per second, including inactivity (after the configured
  idle interval, 5 minutes by default, the time stops counting and is shown in red);
  a locked screen is recorded as inactivity, sleep and power-off are not recorded at all
- screenshots of the active window (when the window changes, then every 30 s)
- for Edge/Chrome/Brave/Vivaldi/Opera the website domain is detected and the site favicon is shown
  (private/incognito windows are neither recorded nor photographed)
- tabs **Day** (timeline, list of records, application ranking, screenshot preview),
  **Statistics** and **Reports**
- closing the window hides it to the tray; right click on the tray icon -> **Otevřít** (open) /
  **Zavřít** (quit, which stops the tracking)
- Settings -> General -> **Spustit při startu** (start at login) and the **idle interval**

The UI is in Czech.

## Development

Requirements: Node.js LTS, Rust (MSVC toolchain) and Visual Studio Build Tools (C++).

```powershell
npm install
npm run tauri dev      # development mode
npm run tauri build    # installer (NSIS) in src-tauri/target/release/bundle
npm run icons          # regenerates the icons from assets/icon/timetrek.svg
cd src-tauri; cargo test
```

Data is stored in `%AppData%\TimeTrek` (`timetrek.db`, `screens/`, `settings.json`).
Advanced options (screenshot interval, retention, ...) are in `settings.json`.

Architecture: see [docs/architecture.md](docs/architecture.md). Instructions for Claude Code: [CLAUDE.md](CLAUDE.md).
