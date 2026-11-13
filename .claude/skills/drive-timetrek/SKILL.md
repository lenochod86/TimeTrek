---
name: drive-timetrek
description: Start the TimeTrek dev app and take a screenshot of its window to verify a UI or behavior change in the real app
---

# Drive TimeTrek

1. Close any running TimeTrek (installed or dev); only one instance may run:
   `Get-Process timetrek -ErrorAction SilentlyContinue | Stop-Process -Force`
2. Start the dev app in the background: `npm run tauri dev` (first start compiles for a few minutes; wait for `Running target\debug\timetrek.exe` in the output).
3. Take a screenshot of the window:
   `pwsh .claude/skills/drive-timetrek/screenshot.ps1 -Out <scratchpad>\win.png`
   and look at it with the Read tool. The script brings the window to the front first.
4. To exercise tracking, switch to other windows (the app records the focused window); the Day tab refreshes every 3 seconds.
5. When done, stop the dev app (`Stop-Process -Name timetrek`) and the `npm`/`node` dev server it started.

Notes:
- Windows may show a firewall prompt for Node.js (Vite dev server); it can be cancelled.
- A window closed with the X is only hidden (tray); the screenshot script cannot show it. Restart the app instead.
- Screenshots of the user's screen never go into the repository; save them to the scratchpad.
