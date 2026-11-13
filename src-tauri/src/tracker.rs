//! Background tracker: samples the focused window once per second and stores
//! contiguous "segments" (same app + window title + website) in SQLite.
//!
//! The decision logic lives in [`Engine::step`], a pure function of
//! (time, idle seconds, lock state, focused window) so it can be unit tested
//! with simulated time; [`run`] only feeds it real samples.

use crate::capture::{self, CaptureRequest};
use crate::config::{Config, SharedConfig};
use crate::db::{self, KIND_ACTIVE, KIND_IDLE};
use crate::favicon;
use crate::platform::{self, PlatformApi, WindowInfo};
use rusqlite::Connection;
use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::Sender;
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;
use std::time::Duration;

pub const PRIVATE_TITLE: &str = "Anonymní okno";

/// If two samples are further apart than this, the machine was asleep /
/// hibernating / suspended and the gap must not be counted as any activity.
const SLEEP_GAP_MS: i64 = 15_000;

pub struct Tracker {
    stop: Arc<AtomicBool>,
    handles: Mutex<Vec<JoinHandle<()>>>,
}

impl Tracker {
    pub fn start(data_dir: PathBuf, db_path: PathBuf, config: SharedConfig) -> Tracker {
        let stop = Arc::new(AtomicBool::new(false));
        let snapshot = config.read().map(|c| c.clone()).unwrap_or_default();
        let (cap_tx, cap_handle) =
            capture::spawn_worker(data_dir.clone(), db_path.clone(), snapshot);
        let (fav_tx, fav_handle) = favicon::spawn_worker(db_path.clone());

        let stop2 = stop.clone();
        let main = std::thread::Builder::new()
            .name("timetrek-tracker".into())
            .spawn(move || run(data_dir, db_path, config, stop2, cap_tx, fav_tx))
            .expect("spawn tracker thread");

        Tracker {
            stop,
            handles: Mutex::new(vec![main, cap_handle, fav_handle]),
        }
    }

    /// Stops the loop and waits until the open segment has been flushed.
    pub fn shutdown(&self) {
        self.stop.store(true, Ordering::SeqCst);
        let handles: Vec<_> = self.handles.lock().unwrap().drain(..).collect();
        for h in handles {
            let _ = h.join();
        }
    }
}

fn run(
    data_dir: PathBuf,
    db_path: PathBuf,
    config: SharedConfig,
    stop: Arc<AtomicBool>,
    cap_tx: Sender<CaptureRequest>,
    fav_tx: Sender<String>,
) {
    let conn = match db::open(&db_path) {
        Ok(c) => c,
        Err(e) => {
            log::error!("tracker: cannot open database: {e}");
            return;
        }
    };
    let retention = config.read().map(|c| c.retention_days).unwrap_or(60);
    db::purge_old_screenshots(&conn, &data_dir, retention);

    let platform = platform::create();
    let mut engine = Engine::new(conn, db::now_ms(), Some(cap_tx), Some(fav_tx));

    while !stop.load(Ordering::SeqCst) {
        // Settings are re-read every tick, so changes apply immediately.
        let cfg = config.read().map(|c| c.clone()).unwrap_or_default();
        let idle_secs = platform.idle_seconds();
        let locked = platform.is_session_locked();
        let is_idle = idle_secs >= cfg.idle_timeout_secs;
        let fg = if locked || is_idle { None } else { platform.foreground_window() };

        engine.step(
            platform.as_ref(),
            &cfg,
            Input { now: db::now_ms(), idle_secs, locked, fg },
        );

        // Sleep ~1 s but react to shutdown quickly.
        for _ in 0..10 {
            if stop.load(Ordering::SeqCst) {
                break;
            }
            std::thread::sleep(Duration::from_millis(100));
        }
    }

    // Final flush so the last segment ends exactly when the app was closed.
    engine.finish(db::now_ms());
}

/// One sample of the world.
pub struct Input {
    pub now: i64,
    pub idle_secs: u64,
    /// Workstation locked (lock screen / secure desktop).
    pub locked: bool,
    pub fg: Option<WindowInfo>,
}

/// What identifies a segment; a change of any field starts a new one.
#[derive(PartialEq, Clone)]
struct Key {
    kind: i64,
    exe: String,
    title: String,
    domain: Option<String>,
}

struct Current {
    id: i64,
    key: Key,
    start: i64,
    last_tick: i64,
    handle: isize,
    private: bool,
    last_shot: i64,
}

pub struct Engine {
    conn: Connection,
    app_ids: HashMap<String, i64>,
    requested_domains: HashSet<String>,
    cur: Option<Current>,
    domain_cache: Option<(isize, String, i64, Option<String>)>, // (handle, title, checked_at, domain)
    /// Nothing before this instant may be recorded (app start / last wake-up).
    floor: i64,
    last_step: i64,
    last_global_shot: i64,
    /// Application of the most recent screenshot request.
    last_shot_exe: String,
    cap_tx: Option<Sender<CaptureRequest>>,
    fav_tx: Option<Sender<String>>,
}

impl Engine {
    pub fn new(
        conn: Connection,
        now: i64,
        cap_tx: Option<Sender<CaptureRequest>>,
        fav_tx: Option<Sender<String>>,
    ) -> Self {
        Engine {
            conn,
            app_ids: HashMap::new(),
            requested_domains: HashSet::new(),
            cur: None,
            domain_cache: None,
            floor: now,
            last_step: now,
            last_global_shot: 0,
            last_shot_exe: String::new(),
            cap_tx,
            fav_tx,
        }
    }

    pub fn finish(&mut self, now: i64) {
        if let Some(c) = self.cur.take() {
            let _ = db::set_segment_end(&self.conn, c.id, now.max(c.start));
        }
    }

    pub fn step(&mut self, platform: &dyn PlatformApi, cfg: &Config, inp: Input) {
        let now = inp.now;

        // The machine slept / hibernated / the clock jumped. The open segment keeps
        // the end it had at the last sample, and nothing before "now" may ever be
        // back-filled (Windows reports the whole sleep as idle time after waking).
        if now - self.last_step > SLEEP_GAP_MS {
            self.cur = None;
            self.floor = now;
        }
        self.last_step = now;

        let is_idle = inp.idle_secs >= cfg.idle_timeout_secs;
        // The Windows lock screen is sometimes visible as a normal window.
        let lock_window = inp.fg.as_ref().is_some_and(|w| is_lock_screen_exe(&w.exe));
        let locked = inp.locked || lock_window;
        let fg = if locked || is_idle { None } else { inp.fg };

        match fg {
            None => self.step_idle(now, is_idle, inp.idle_secs),
            Some(w) => self.step_active(platform, cfg, now, w),
        }
    }

    /// Idle (no input for the configured time), locked screen or no focused window.
    fn step_idle(&mut self, now: i64, is_idle: bool, idle_secs: u64) {
        // Inactivity is counted from the last input, not from the moment the timeout
        // fired - so the idle interval is subtracted from the working time. A locked
        // screen starts "now".
        let idle_start = if is_idle {
            (now - idle_secs as i64 * 1000).max(self.floor)
        } else {
            now
        };

        if let Some(c) = &mut self.cur {
            if c.key.kind == KIND_IDLE {
                c.last_tick = now;
                let _ = db::set_segment_end(&self.conn, c.id, now);
                return;
            }
        }
        if let Some(c) = self.cur.take() {
            let end = idle_start.clamp(c.start, now.max(c.start));
            let _ = db::set_segment_end(&self.conn, c.id, end);
        }
        let start = idle_start.min(now);
        if let Ok(id) = db::insert_segment(&self.conn, KIND_IDLE, None, "", None, start, now) {
            self.cur = Some(Current {
                id,
                key: Key { kind: KIND_IDLE, exe: String::new(), title: String::new(), domain: None },
                start,
                last_tick: now,
                handle: 0,
                private: false,
                last_shot: 0,
            });
        }
    }

    fn step_active(&mut self, platform: &dyn PlatformApi, cfg: &Config, now: i64, w: WindowInfo) {
        let (title, domain, private) = describe_window(platform, &w, &mut self.domain_cache, now);
        let key = Key { kind: KIND_ACTIVE, exe: w.exe.clone(), title, domain };

        if self.cur.as_ref().is_some_and(|c| c.key == key) {
            let c = self.cur.as_mut().unwrap();
            c.last_tick = now;
            c.handle = w.handle;
            let _ = db::set_segment_end(&self.conn, c.id, now);
        } else {
            if let Some(c) = self.cur.take() {
                let _ = db::set_segment_end(&self.conn, c.id, now.max(c.start));
            }
            let app_id = resolve_app(&self.conn, platform, &mut self.app_ids, &w.exe);
            let inserted = app_id.and_then(|app_id| {
                db::insert_segment(
                    &self.conn,
                    KIND_ACTIVE,
                    Some(app_id),
                    &key.title,
                    key.domain.as_deref(),
                    now,
                    now,
                )
                .ok()
            });
            if let Some(id) = inserted {
                if let (Some(d), Some(tx)) = (&key.domain, &self.fav_tx) {
                    if self.requested_domains.insert(d.clone()) {
                        let _ = tx.send(d.clone());
                    }
                }
                self.cur = Some(Current {
                    id,
                    key,
                    start: now,
                    last_tick: now,
                    handle: w.handle,
                    private,
                    last_shot: 0,
                });
            }
        }

        // Screenshots: IMMEDIATELY when the application changes (the same sample in
        // which the new segment starts), then periodically. A title change inside the
        // same application is photographed too, but at most every 2 s, because some
        // apps change their title constantly.
        if let Some(c) = &mut self.cur {
            let due = if c.last_shot == 0 {
                self.last_shot_exe != c.key.exe || now - self.last_global_shot >= 2_000
            } else {
                now - c.last_shot >= (cfg.screenshot_interval_secs as i64) * 1000
            };
            if cfg.screenshots_enabled && due && !c.private {
                c.last_shot = now;
                self.last_global_shot = now;
                self.last_shot_exe = c.key.exe.clone();
                if let Some(tx) = &self.cap_tx {
                    let _ = tx.send(CaptureRequest { segment_id: c.id, handle: c.handle, ts: now });
                }
            }
        }
    }
}

fn is_lock_screen_exe(exe: &str) -> bool {
    let name = exe.rsplit(['\\', '/']).next().unwrap_or("").to_lowercase();
    matches!(name.as_str(), "lockapp.exe" | "logonui.exe")
}

fn resolve_app(
    conn: &Connection,
    platform: &dyn PlatformApi,
    cache: &mut HashMap<String, i64>,
    exe: &str,
) -> Option<i64> {
    let key = exe.to_lowercase();
    if let Some(id) = cache.get(&key) {
        return Some(*id);
    }
    let name = platform.app_name(exe);
    let icon = platform.app_icon_png(exe);
    let id = db::upsert_app(conn, &key, &name, icon.as_deref()).ok()?;
    cache.insert(key, id);
    Some(id)
}

/// Returns (title to store, website domain, is-private-window).
fn describe_window(
    platform: &dyn PlatformApi,
    w: &WindowInfo,
    cache: &mut Option<(isize, String, i64, Option<String>)>,
    now: i64,
) -> (String, Option<String>, bool) {
    if !is_browser_exe(&w.exe) {
        return (normalize_title(&w.title), None, false);
    }
    if looks_private(&w.title) {
        return (PRIVATE_TITLE.to_string(), None, true);
    }
    // Re-read the address bar when the window/title changed or every 5 s.
    let fresh = matches!(cache, Some((h, t, at, _)) if *h == w.handle && *t == w.title && now - *at < 5_000);
    if !fresh {
        let domain = platform.browser_address(w).and_then(|a| extract_domain(&a));
        *cache = Some((w.handle, w.title.clone(), now, domain));
    }
    let domain = cache.as_ref().and_then(|c| c.3.clone());
    (normalize_title(&w.title), domain, false)
}

/// Terminals and some apps animate a spinner glyph at the start of the title
/// (e.g. "◐ TimeTrek"). Dropping leading symbol characters keeps that from
/// creating a new segment every second.
fn normalize_title(title: &str) -> String {
    let is_symbol = |c: char| {
        matches!(c as u32,
            0x2190..=0x2BFF   // arrows, geometric shapes, dingbats, braille spinners
            | 0x1F300..=0x1FAFF // emoji
        ) || c.is_whitespace()
    };
    let trimmed = title.trim_start_matches(is_symbol);
    if trimmed.is_empty() {
        title.to_string()
    } else {
        trimmed.to_string()
    }
}

fn is_browser_exe(exe: &str) -> bool {
    let name = exe.rsplit(['\\', '/']).next().unwrap_or("").to_lowercase();
    matches!(
        name.as_str(),
        "chrome.exe" | "msedge.exe" | "brave.exe" | "vivaldi.exe" | "opera.exe"
    )
}

fn looks_private(title: &str) -> bool {
    let t = title.to_lowercase();
    ["inprivate", "incognito", "anonymní", "anonymous"]
        .iter()
        .any(|k| t.contains(k))
}

/// Reduces what is shown in the address bar to a bare host name.
/// Returns `None` for search text typed in the omnibox or internal pages.
pub fn extract_domain(address: &str) -> Option<String> {
    let a = address.trim();
    if a.is_empty() || a.contains(' ') {
        return None;
    }
    let rest = match a.split_once("://") {
        Some((scheme, rest)) => {
            if !matches!(scheme, "http" | "https") {
                return None; // edge://, chrome://, file://, ...
            }
            rest
        }
        None => a,
    };
    let host = rest.split(['/', '?', '#']).next()?;
    let host = host.rsplit('@').next()?; // strip user:pass@
    let host = host.split(':').next()?; // strip port
    let host = host.trim_start_matches("www.").to_lowercase();
    let valid = !host.is_empty()
        && (host.contains('.') || host == "localhost")
        && host
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '.' || c == '-' || c == '_');
    valid.then_some(host)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn domains() {
        assert_eq!(extract_domain("https://www.github.com/a/b?x=1").as_deref(), Some("github.com"));
        assert_eq!(extract_domain("github.com/foo").as_deref(), Some("github.com"));
        assert_eq!(extract_domain("localhost:5173/x").as_deref(), Some("localhost"));
        assert_eq!(extract_domain("edge://settings"), None);
        assert_eq!(extract_domain("jak se pocasi"), None);
        assert_eq!(extract_domain("intranet"), None);
    }

    // --- simulated-time tests of the idle / lock / sleep behaviour ----------

    struct Fake;
    impl PlatformApi for Fake {
        fn foreground_window(&self) -> Option<WindowInfo> {
            None
        }
        fn idle_seconds(&self) -> u64 {
            0
        }
        fn is_session_locked(&self) -> bool {
            false
        }
        fn app_name(&self, exe: &str) -> String {
            exe.to_string()
        }
        fn app_icon_png(&self, _: &str) -> Option<Vec<u8>> {
            None
        }
        fn browser_address(&self, _: &WindowInfo) -> Option<String> {
            None
        }
        fn capture_window(&self, _: isize) -> Option<image::RgbaImage> {
            None
        }
    }

    const T0: i64 = 1_000_000_000_000;
    const S: i64 = 1000;

    fn editor() -> Option<WindowInfo> {
        Some(WindowInfo { handle: 1, exe: "C:\\app\\editor.exe".into(), title: "doc".into() })
    }

    fn cfg() -> Config {
        Config { idle_timeout_secs: 300, screenshots_enabled: false, ..Config::default() }
    }

    fn engine() -> Engine {
        Engine::new(db::open_memory().unwrap(), T0, None, None)
    }

    /// Feeds one sample per second from `from` to `to` (seconds after T0).
    /// `last_input` is when the user last touched keyboard/mouse (seconds after T0).
    fn run(e: &mut Engine, from: i64, to: i64, last_input: i64, locked: bool) {
        for t in from..=to {
            let idle = (t - last_input).max(0) as u64;
            let is_idle = idle >= cfg().idle_timeout_secs;
            let fg = if locked || is_idle { None } else { editor() };
            e.step(&Fake, &cfg(), Input { now: T0 + t * S, idle_secs: idle, locked, fg });
        }
    }

    /// (kind, start_s, end_s) relative to T0, ordered by start.
    fn segments(e: &Engine) -> Vec<(i64, i64, i64)> {
        e.conn
            .prepare("SELECT kind, start_ms, end_ms FROM segments ORDER BY start_ms")
            .unwrap()
            .query_map([], |r| {
                Ok((r.get::<_, i64>(0)?, (r.get::<_, i64>(1)? - T0) / S, (r.get::<_, i64>(2)? - T0) / S))
            })
            .unwrap()
            .flatten()
            .collect()
    }

    /// The real OS probe must not report "locked" on an ordinary unlocked desktop,
    /// otherwise every moment would be recorded as idle.
    #[cfg(windows)]
    #[test]
    fn real_lock_probe_is_false_on_unlocked_desktop() {
        assert!(!platform::create().is_session_locked());
    }

    fn window(exe: &str, title: &str) -> Option<WindowInfo> {
        Some(WindowInfo { handle: 1, exe: exe.into(), title: title.into() })
    }

    /// Screenshot requests as (segment id, time in seconds after T0), in order.
    fn shot_requests(rx: &std::sync::mpsc::Receiver<CaptureRequest>) -> Vec<(i64, i64)> {
        rx.try_iter().map(|r| (r.segment_id, (r.ts - T0) / S)).collect()
    }

    #[test]
    fn screenshot_is_taken_in_the_same_sample_as_the_application_change() {
        let (tx, rx) = std::sync::mpsc::channel();
        let mut e = Engine::new(db::open_memory().unwrap(), T0, Some(tx), None);
        let cfg = Config { screenshots_enabled: true, ..Config::default() };
        let step = |e: &mut Engine, t: i64, w: Option<WindowInfo>| {
            e.step(&Fake, &cfg, Input { now: T0 + t * S, idle_secs: 0, locked: false, fg: w });
        };

        step(&mut e, 1, window("a.exe", "doc")); // first window ever -> photographed at once
        step(&mut e, 2, window("b.exe", "shell")); // application changed -> at once
        step(&mut e, 3, window("a.exe", "doc")); // and back -> at once
        let reqs = shot_requests(&rx);
        assert_eq!(reqs.iter().map(|r| r.1).collect::<Vec<_>>(), vec![1, 2, 3], "{reqs:?}");
        // each one belongs to its own, new segment
        let ids: std::collections::HashSet<_> = reqs.iter().map(|r| r.0).collect();
        assert_eq!(ids.len(), 3);
    }

    #[test]
    fn title_flapping_in_one_application_is_throttled() {
        let (tx, rx) = std::sync::mpsc::channel();
        let mut e = Engine::new(db::open_memory().unwrap(), T0, Some(tx), None);
        let cfg = Config { screenshots_enabled: true, ..Config::default() };
        for t in 1..=6 {
            let title = format!("title {t}");
            e.step(&Fake, &cfg, Input { now: T0 + t * S, idle_secs: 0, locked: false, fg: window("a.exe", &title) });
        }
        let secs: Vec<i64> = shot_requests(&rx).iter().map(|r| r.1).collect();
        assert_eq!(secs, vec![1, 3, 5], "at most one per 2 s inside one app");
    }

    #[test]
    fn private_window_is_never_photographed() {
        let (tx, rx) = std::sync::mpsc::channel();
        let mut e = Engine::new(db::open_memory().unwrap(), T0, Some(tx), None);
        let cfg = Config { screenshots_enabled: true, ..Config::default() };
        e.step(&Fake, &cfg, Input { now: T0 + S, idle_secs: 0, locked: false, fg: window("C:\\x\\msedge.exe", "Tab - InPrivate - Edge") });
        assert!(shot_requests(&rx).is_empty());
    }

    #[test]
    fn idle_interval_is_subtracted_from_work_time() {
        let mut e = engine();
        // Works for 10 minutes, then walks away; the timeout (300 s) fires at t=900.
        run(&mut e, 1, 600, 600, false);
        run(&mut e, 601, 1000, 600, false);
        e.finish(T0 + 1000 * S);
        let segs = segments(&e);
        assert_eq!(segs.len(), 2, "{segs:?}");
        // Active work ends at the last input (600), NOT at the moment the timeout fired.
        assert_eq!(segs[0], (KIND_ACTIVE, 1, 600));
        // Inactivity starts at the last input and is shown as idle (red).
        assert_eq!(segs[1], (KIND_IDLE, 600, 1000));
    }

    #[test]
    fn work_resumes_after_idle() {
        let mut e = engine();
        run(&mut e, 1, 100, 100, false);
        run(&mut e, 101, 500, 100, false); // idle since 100, flagged at 400
        run(&mut e, 501, 520, 501, false); // user is back
        e.finish(T0 + 520 * S);
        let segs = segments(&e);
        assert_eq!(segs[0], (KIND_ACTIVE, 1, 100));
        assert_eq!(segs[1].0, KIND_IDLE);
        assert_eq!(segs[1].1, 100);
        assert_eq!(segs[2].0, KIND_ACTIVE);
        assert_eq!(segs[2].1, 501);
        // no overlap, no gap between idle and the new active segment
        assert_eq!(segs[1].2, segs[2].1);
    }

    #[test]
    fn locked_screen_is_idle_immediately() {
        let mut e = engine();
        run(&mut e, 1, 60, 60, false); // working, last input at 60
        run(&mut e, 61, 120, 60, true); // Win+L at t=61 (only 1 s since input)
        e.finish(T0 + 120 * S);
        let segs = segments(&e);
        assert_eq!(segs.len(), 2, "{segs:?}");
        assert_eq!(segs[0].0, KIND_ACTIVE);
        assert_eq!(segs[1].0, KIND_IDLE);
        // starts at the lock, not after the 5 minute timeout; no hole in between
        assert_eq!(segs[1].1, 61);
        assert_eq!(segs[0].2, segs[1].1);
        assert_eq!(segs[1].2, 120);
    }

    #[test]
    fn lock_screen_window_counts_as_locked_not_as_app() {
        let mut e = engine();
        run(&mut e, 1, 30, 30, false);
        for t in 31..=60 {
            let lock = Some(WindowInfo { handle: 2, exe: "C:\\Windows\\SystemApps\\LockApp.exe".into(), title: "Lock".into() });
            e.step(&Fake, &cfg(), Input { now: T0 + t * S, idle_secs: 1, locked: false, fg: lock });
        }
        e.finish(T0 + 60 * S);
        let segs = segments(&e);
        assert_eq!(segs.iter().filter(|s| s.0 == KIND_ACTIVE).count(), 1, "{segs:?}");
        assert_eq!(segs.last().unwrap().0, KIND_IDLE);
    }

    #[test]
    fn sleep_is_not_recorded_even_when_windows_reports_it_as_idle() {
        let mut e = engine();
        run(&mut e, 1, 100, 100, false);
        // Laptop sleeps for 2 hours; on wake Windows says "idle for 2 hours".
        let wake = 100 + 2 * 3600;
        for t in wake..=wake + 5 {
            e.step(&Fake, &cfg(), Input { now: T0 + t * S, idle_secs: (t - 100) as u64, locked: true, fg: None });
        }
        // The user unlocks and works again.
        run(&mut e, wake + 6, wake + 60, wake + 60, false);
        e.finish(T0 + (wake + 60) * S);
        let segs = segments(&e);
        // Nothing may cover the sleep (100 .. wake).
        for (_, s, en) in &segs {
            assert!(*en <= 100 || *s >= wake, "segment {s}-{en} overlaps sleep: {segs:?}");
        }
        assert_eq!(segs[0], (KIND_ACTIVE, 1, 100));
    }

    #[test]
    fn sleep_with_no_lock_is_not_back_filled_as_idle() {
        let mut e = engine();
        run(&mut e, 1, 100, 100, false);
        let wake = 100 + 3 * 3600;
        // Wake-up: GetLastInputInfo still reports 3 hours of idle.
        e.step(&Fake, &cfg(), Input { now: T0 + wake * S, idle_secs: (wake - 100) as u64, locked: false, fg: None });
        e.finish(T0 + (wake + 1) * S);
        let segs = segments(&e);
        for (_, s, en) in &segs {
            assert!(*en <= 100 || *s >= wake, "segment {s}-{en} overlaps sleep: {segs:?}");
        }
    }
}
