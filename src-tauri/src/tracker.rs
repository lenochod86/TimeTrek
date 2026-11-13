//! Background tracker: samples the focused window once per second and stores
//! contiguous "segments" (same app + window title + website) in SQLite.

use crate::capture::{self, CaptureRequest};
use crate::config::Config;
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

pub struct Tracker {
    stop: Arc<AtomicBool>,
    handles: Mutex<Vec<JoinHandle<()>>>,
}

impl Tracker {
    pub fn start(data_dir: PathBuf, db_path: PathBuf, config: Config) -> Tracker {
        let stop = Arc::new(AtomicBool::new(false));
        let (cap_tx, cap_handle) =
            capture::spawn_worker(data_dir.clone(), db_path.clone(), config.clone());
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

fn run(
    data_dir: PathBuf,
    db_path: PathBuf,
    cfg: Config,
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
    db::purge_old_screenshots(&conn, &data_dir, cfg.retention_days);

    let platform = platform::create();
    let mut app_ids: HashMap<String, i64> = HashMap::new();
    let mut requested_domains: HashSet<String> = HashSet::new();
    let mut cur: Option<Current> = None;
    let mut domain_cache: Option<(isize, String, i64, Option<String>)> = None; // (handle,title,checked_at,domain)
    let own_pid = std::process::id();
    let mut last_global_shot: i64 = 0;

    while !stop.load(Ordering::SeqCst) {
        let now = db::now_ms();

        // The machine slept / the clock jumped: close the open segment where it last was.
        if let Some(c) = &cur {
            if now - c.last_tick > 15_000 {
                cur = None;
            }
        }

        let idle = platform.idle_seconds();
        let is_idle = idle >= cfg.idle_timeout_secs;
        let fg = if is_idle { None } else { platform.foreground_window() };

        match fg {
            None => {
                // Idle, locked screen or no focused window.
                let idle_start = if is_idle {
                    now - (idle as i64) * 1000
                } else {
                    now
                };
                match &mut cur {
                    Some(c) if c.key.kind == KIND_IDLE => {
                        c.last_tick = now;
                        let _ = db::set_segment_end(&conn, c.id, now);
                    }
                    _ => {
                        if let Some(c) = cur.take() {
                            // The tail of the previous segment was actually idle time.
                            let end = idle_start.clamp(c.start, c.last_tick.max(c.start));
                            let _ = db::set_segment_end(&conn, c.id, end);
                        }
                        let start = idle_start;
                        if let Ok(id) =
                            db::insert_segment(&conn, KIND_IDLE, None, "", None, start, now)
                        {
                            cur = Some(Current {
                                id,
                                key: Key {
                                    kind: KIND_IDLE,
                                    exe: String::new(),
                                    title: String::new(),
                                    domain: None,
                                },
                                start,
                                last_tick: now,
                                handle: 0,
                                private: false,
                                last_shot: 0,
                            });
                        }
                    }
                }
            }
            Some(w) => {
                let (title, domain, private) =
                    describe_window(platform.as_ref(), &w, &mut domain_cache, now);
                let key = Key {
                    kind: KIND_ACTIVE,
                    exe: w.exe.clone(),
                    title,
                    domain,
                };

                let same = cur.as_ref().is_some_and(|c| c.key == key);
                if same {
                    let c = cur.as_mut().unwrap();
                    c.last_tick = now;
                    c.handle = w.handle;
                    let _ = db::set_segment_end(&conn, c.id, now);
                } else {
                    if let Some(c) = cur.take() {
                        let _ = db::set_segment_end(&conn, c.id, now);
                    }
                    let app_id = resolve_app(&conn, platform.as_ref(), &mut app_ids, &w.exe);
                    if let Some(id) = app_id.and_then(|app_id| {
                        db::insert_segment(
                            &conn,
                            KIND_ACTIVE,
                            Some(app_id),
                            &key.title,
                            key.domain.as_deref(),
                            now,
                            now,
                        )
                        .ok()
                    }) {
                        if let Some(d) = &key.domain {
                            if requested_domains.insert(d.clone()) {
                                let _ = fav_tx.send(d.clone());
                            }
                        }
                        cur = Some(Current {
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

                // Screenshots: shortly after a window comes up (but not more often than
                // every 4 s overall, titles can change rapidly), then periodically.
                if let Some(c) = &mut cur {
                    let due = if c.last_shot == 0 {
                        now - c.start >= 1_000 && now - last_global_shot >= 4_000
                    } else {
                        now - c.last_shot >= (cfg.screenshot_interval_secs as i64) * 1000
                    };
                    if cfg.screenshots_enabled && due && !c.private && w.pid != own_pid {
                        c.last_shot = now;
                        last_global_shot = now;
                        let _ = cap_tx.send(CaptureRequest {
                            segment_id: c.id,
                            handle: c.handle,
                            ts: now,
                        });
                    }
                }
            }
        }

        // Sleep ~1 s but react to shutdown quickly.
        for _ in 0..10 {
            if stop.load(Ordering::SeqCst) {
                break;
            }
            std::thread::sleep(Duration::from_millis(100));
        }
    }

    // Final flush so the last segment ends exactly when the app was closed.
    if let Some(c) = cur {
        let _ = db::set_segment_end(&conn, c.id, db::now_ms());
    }
    drop(cap_tx);
    drop(fav_tx);
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
        let domain = platform
            .browser_address(w)
            .and_then(|a| extract_domain(&a));
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
}
