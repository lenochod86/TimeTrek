# TimeTrek

A small hobby time tracker for Windows (macOS port possible later): it runs in the system tray, measures how long the user works in which application and on which website, takes screenshots of the active window and shows day / statistics / report views. Stack: Tauri 2 (Rust core) + React/TypeScript UI.

These instructions apply in every environment. Work the same way everywhere.

## Language

- **Talk to the user in Czech.**
- **Everything that goes into the repository is in English:** code, identifiers, comments, log messages, documentation and commit messages.
- Exception: the **UI texts are Czech** (the user's design is Czech). They all live in `src/i18n/cs.ts` (plus the tray menu in `src-tauri/src/tray.rs` and the few stored placeholder titles in the tracker), never inline in components, so another language is just another dictionary.

## Documents

- `docs/architecture.md`: architecture, decisions, known limits. Read it before changing the architecture and keep it up to date.
- `README.md`: features, development and build.

## Git (small project: no issues, no pull requests)

Repository: `lenochod86/TimeTrek`, branch `main`.

- Commit straight to `main`. No issues, no branches, no pull requests, no labels.
- **After every finished piece of work commit and push to `origin/main` without asking.**
- Small, self-contained commits. Imperative subject line of at most 72 characters; the body says why and what.
- Before every commit run the checks below. Never commit a red state.
- Never force push and never rewrite history that was already pushed.
- At the start of a session run `git status` and `git fetch`. Never overwrite or delete uncommitted changes; ask first.
- Never put into the repository: passwords, tokens, the user's data (`%AppData%\TimeTrek`: database, screenshots, settings), or screenshots of the user's screen. Generated files (`node_modules/`, `dist/`, `src-tauri/target/`, `src-tauri/gen/`) stay in `.gitignore`.
- If GitHub is not accessible, tell the user. Never silently skip a push.

## Checks before a commit

- `npx tsc --noEmit` (frontend types)
- `cd src-tauri && cargo test` (Rust tests; no warnings allowed)
- Anything that changes behavior is also run in the real app (see "Verify in the real app").

## Architecture and code

- Rust core (`src-tauri/src`): tracking, storage, screenshots, favicons, tray, commands. React UI (`src`) only displays data and calls Tauri commands.
- **All OS specific code lives behind `platform::PlatformApi`** (`platform/windows.rs`, `platform/macos.rs` placeholder). The tracker, database and UI never call OS APIs directly. A new OS capability means a new trait method and an implementation for each platform.
- Tracking decisions live in `Engine::step` (`tracker.rs`), a pure function of time, idle seconds, lock state and the focused window. Keep it that way, so it can be tested with simulated time.
- Never block the tracker loop: slow work (screenshots, favicon downloads) runs on its own thread.
- Privacy: everything stays on the machine. Store only the **domain** of a website, never the full URL. Private / incognito browser windows are neither recorded nor photographed. Fetch favicons straight from the visited site, never through a third party service. No telemetry.
- Time is stored as milliseconds since the epoch; day boundaries (local midnight) are computed in the UI.
- Every new tracking rule has a test in `tracker.rs`; every bug fix has a test that catches the bug.
- Follow the style of the surrounding code. Settings that the user can change go to the Settings dialog (`src/components/Dialogs.tsx`) and are applied without a restart.

## Build and versions

- Dev run: `npm run tauri dev`. Installer: `npx tauri build` -> `src-tauri/target/release/bundle/nsis/TimeTrek_<version>_x64-setup.exe`.
- Icons are generated from `assets/icon/timetrek.svg` with `npm run icons`.
- For every installer handed to the user bump the version (keep `package.json`, `src-tauri/Cargo.toml` and `src-tauri/tauri.conf.json` equal) and say which version it is.
- If `node`, `cargo` or `npx` are not on PATH in a new shell, refresh PATH from the machine and user environment (they were installed with winget).

## Verify in the real app

- Run the dev app, drive it and look at the result: `.claude/skills/drive-timetrek/` has a script that takes a screenshot of the TimeTrek window.
- Do not mark anything as verified until it has run in the real app. Things that need the user (locking the PC, sleep, the tray menu, autostart after a re-login) are listed as "not tested" and handed over with the installer.
- The dev app and the installed app share the same data directory and only one instance may run, so close the installed TimeTrek before testing. Closing or killing TimeTrek for a test is allowed without asking. Never delete the user's data directory.

## How to work

- If you do not know something, or it is the user's decision, ask. Do not make up the user's wishes.
- No step by step narration. Say one short line when you start, ask only when blocked, and finish with a summary in Czech.
- Report results precisely: what passed, what failed (with output) and what was not tested.
- Write files that contain apostrophes with the Write tool, not with bash heredocs.
