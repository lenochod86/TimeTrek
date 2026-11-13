# TimeTrek architecture

```
UI (React + TypeScript, Vite)          src/
  pages/      DayPage, StatsPage, ReportsPage
  components/ Timeline, SegmentList, AppRanking, Shots, RangeBar, Dialogs, TitleBar
  i18n/       UI string dictionaries (cs for now)
        |  Tauri commands (invoke)
Core (Rust)                            src-tauri/src/
  tracker.rs   loop once per second -> Engine::step: focused window + idle + lock -> segments in SQLite
  capture.rs   screenshot thread (never delays the tracker)
  favicon.rs   favicon download thread (straight from the site, cached in the DB)
  db.rs        SQLite (WAL), schema, migration
  commands.rs  commands called from the UI
  tray.rs      system tray: Otevřít / Zavřít
  config.rs    settings.json, shared live with the tracker
  platform/    the ONLY place that depends on the OS
```

## Tracking

A record (*segment*) is a continuous stretch in the same window (application + title + domain).
When the title changes, a new segment starts, as in the list of the original design.
Leading symbol characters of a title are ignored, because terminals animate a spinner there.

`Engine::step` is a pure function of (time, idle seconds, lock state, focused window), so the
behavior is covered by tests with simulated time:

- **Inactivity** (no input for the configured interval, default 5 min) is an `idle` segment (red).
  It starts at the moment of the last input, so the idle interval is subtracted from the work time.
- **Locked workstation** is detected directly (`OpenInputDesktop` fails on the secure desktop; the
  `LockApp.exe` window counts as locked too) and is idle from the moment of locking.
- **Sleep / hibernation / power-off**: a gap between two samples longer than 15 s closes the open
  segment where it was and raises a "floor": nothing before the wake-up (or the app start) is ever
  back-filled, even though Windows reports the whole sleep as idle time after waking up.
- Quitting from the tray flushes the open segment.

## Portability (macOS)

Everything OS specific is behind the trait `platform::PlatformApi`:
`foreground_window`, `idle_seconds`, `is_session_locked`, `app_name`, `app_icon_png`,
`browser_address`, `capture_window`. The Windows implementation (`platform/windows.rs`) uses Win32 and
UI Automation. For macOS, fill in `platform/macos.rs` (NSWorkspace, CGWindowList, Accessibility API);
the tracker, the database and the UI do not change.

## Websites and favicons

The address of the active tab is read from the address bar through UI Automation (the Chromium
omnibox has the class name `OmniboxViewViews` regardless of the UI language). Only the **domain**
is stored. The favicon is downloaded straight from the visited site (`<link rel=icon>` ->
`/favicon.ico`), scaled to 32x32 PNG and stored in the `favicons` table. No third party service.
Private windows (InPrivate / Incognito / "Anonymní") are neither stored nor photographed.

## Database

`apps`, `segments`, `screenshots`, `favicons`, `tags`, `tag_ranges`; see `db.rs`.
Time is in milliseconds since the epoch; day boundaries are computed in the UI (local midnight).

## Known limits

- UWP applications run under `ApplicationFrameHost.exe` (the concrete application is not resolved yet)
- Firefox: the domain is not detected (Chromium browsers only)
- no screenshot of a minimized window
- a UAC prompt runs on the secure desktop and is recorded as inactivity for its duration
- verified in the real app: tracking, favicons, hide to tray on close, screenshots. Not verified
  in the real app (only by tests): lock, sleep, the tray menu, autostart.
