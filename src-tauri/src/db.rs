use rusqlite::{params, Connection, OptionalExtension};
use std::path::Path;

pub const KIND_ACTIVE: i64 = 0;
pub const KIND_IDLE: i64 = 1;

pub fn open(path: &Path) -> rusqlite::Result<Connection> {
    let conn = Connection::open(path)?;
    conn.busy_timeout(std::time::Duration::from_secs(5))?;
    conn.execute_batch(
        "PRAGMA journal_mode=WAL; PRAGMA synchronous=NORMAL; PRAGMA foreign_keys=ON;",
    )?;
    migrate(&conn)?;
    Ok(conn)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn shot(conn: &Connection, seg: i64, ts: i64) -> i64 {
        conn.execute(
            "INSERT INTO screenshots(segment_id, ts, path) VALUES (?1, ?2, 'x.jpg')",
            params![seg, ts],
        )
        .unwrap();
        conn.last_insert_rowid()
    }

    #[test]
    fn preview_never_uses_another_application() {
        let conn = open_memory().unwrap();
        let editor = upsert_app(&conn, "editor.exe", "Editor", None).unwrap();
        let term = upsert_app(&conn, "term.exe", "Terminal", None).unwrap();
        let a = insert_segment(&conn, KIND_ACTIVE, Some(editor), "doc", None, 0, 60_000).unwrap();
        let shot_a = shot(&conn, a, 30_000);
        // short segments without a snapshot of their own
        let same_app = insert_segment(&conn, KIND_ACTIVE, Some(editor), "other doc", None, 61_000, 62_000).unwrap();
        let other_app = insert_segment(&conn, KIND_ACTIVE, Some(term), "shell", None, 62_000, 63_000).unwrap();

        let own = preview_shot(&conn, a).unwrap().unwrap();
        assert_eq!((own.id, own.exact), (shot_a, true));

        let approx = preview_shot(&conn, same_app).unwrap().unwrap();
        assert_eq!((approx.id, approx.exact), (shot_a, false));

        // the terminal has no snapshot at all; the editor's one must NOT be shown
        assert!(preview_shot(&conn, other_app).unwrap().is_none());
    }

    #[test]
    fn preview_prefers_same_website_and_ignores_far_away_shots() {
        let conn = open_memory().unwrap();
        let browser = upsert_app(&conn, "edge.exe", "Edge", None).unwrap();
        let gh = insert_segment(&conn, KIND_ACTIVE, Some(browser), "GitHub", Some("github.com"), 0, 10_000).unwrap();
        let news = insert_segment(&conn, KIND_ACTIVE, Some(browser), "News", Some("news.cz"), 20_000, 30_000).unwrap();
        let _gh_shot = shot(&conn, gh, 5_000);
        let news_shot = shot(&conn, news, 25_000);
        let target = insert_segment(&conn, KIND_ACTIVE, Some(browser), "Other news", Some("news.cz"), 11_000, 12_000).unwrap();
        // github shot is closer in time, but the same website wins
        assert_eq!(preview_shot(&conn, target).unwrap().unwrap().id, news_shot);

        // more than 10 minutes away -> nothing
        let late = insert_segment(&conn, KIND_ACTIVE, Some(browser), "Late", Some("news.cz"), 3_000_000, 3_001_000).unwrap();
        assert!(preview_shot(&conn, late).unwrap().is_none());
    }
}

#[cfg(test)]
pub fn open_memory() -> rusqlite::Result<Connection> {
    let conn = Connection::open_in_memory()?;
    migrate(&conn)?;
    Ok(conn)
}

fn migrate(conn: &Connection) -> rusqlite::Result<()> {
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS apps (
            id INTEGER PRIMARY KEY,
            exe TEXT NOT NULL UNIQUE,
            name TEXT NOT NULL,
            icon BLOB
        );
        CREATE TABLE IF NOT EXISTS segments (
            id INTEGER PRIMARY KEY,
            kind INTEGER NOT NULL,
            app_id INTEGER REFERENCES apps(id),
            title TEXT NOT NULL DEFAULT '',
            domain TEXT,
            start_ms INTEGER NOT NULL,
            end_ms INTEGER NOT NULL
        );
        CREATE INDEX IF NOT EXISTS idx_segments_start ON segments(start_ms);
        CREATE TABLE IF NOT EXISTS screenshots (
            id INTEGER PRIMARY KEY,
            segment_id INTEGER NOT NULL REFERENCES segments(id),
            ts INTEGER NOT NULL,
            path TEXT NOT NULL
        );
        CREATE INDEX IF NOT EXISTS idx_shots_segment ON screenshots(segment_id);
        CREATE INDEX IF NOT EXISTS idx_shots_ts ON screenshots(ts);
        CREATE TABLE IF NOT EXISTS favicons (
            domain TEXT PRIMARY KEY,
            png BLOB,
            fetched_ms INTEGER NOT NULL
        );
        CREATE TABLE IF NOT EXISTS tags (
            id INTEGER PRIMARY KEY,
            name TEXT NOT NULL UNIQUE,
            color TEXT NOT NULL
        );
        CREATE TABLE IF NOT EXISTS tag_ranges (
            id INTEGER PRIMARY KEY,
            tag_id INTEGER NOT NULL REFERENCES tags(id),
            start_ms INTEGER NOT NULL,
            end_ms INTEGER NOT NULL
        );
        CREATE INDEX IF NOT EXISTS idx_tag_ranges_start ON tag_ranges(start_ms);",
    )
}

pub fn now_ms() -> i64 {
    chrono::Utc::now().timestamp_millis()
}

/// Returns the app id for an executable, inserting it (with icon) when new.
pub fn upsert_app(
    conn: &Connection,
    exe: &str,
    name: &str,
    icon: Option<&[u8]>,
) -> rusqlite::Result<i64> {
    if let Some(id) = conn
        .query_row("SELECT id FROM apps WHERE exe = ?1", [exe], |r| r.get(0))
        .optional()?
    {
        return Ok(id);
    }
    conn.execute(
        "INSERT INTO apps(exe, name, icon) VALUES (?1, ?2, ?3)",
        params![exe, name, icon],
    )?;
    Ok(conn.last_insert_rowid())
}

pub fn insert_segment(
    conn: &Connection,
    kind: i64,
    app_id: Option<i64>,
    title: &str,
    domain: Option<&str>,
    start: i64,
    end: i64,
) -> rusqlite::Result<i64> {
    conn.execute(
        "INSERT INTO segments(kind, app_id, title, domain, start_ms, end_ms)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
        params![kind, app_id, title, domain, start, end],
    )?;
    Ok(conn.last_insert_rowid())
}

pub fn set_segment_end(conn: &Connection, id: i64, end: i64) -> rusqlite::Result<()> {
    conn.execute(
        "UPDATE segments SET end_ms = ?2 WHERE id = ?1",
        params![id, end],
    )?;
    Ok(())
}

pub struct PreviewShot {
    pub id: i64,
    /// `true`: the snapshot belongs to this very segment; `false`: the nearest one of the same app.
    pub exact: bool,
}

/// Picks the screenshot to preview for a segment. Short segments have none of their
/// own, so the nearest one (within 10 minutes) of the SAME application is used -
/// preferring the same website and window title. Never a snapshot of another app.
pub fn preview_shot(conn: &Connection, segment_id: i64) -> rusqlite::Result<Option<PreviewShot>> {
    let own: Vec<i64> = conn
        .prepare("SELECT id FROM screenshots WHERE segment_id = ?1 ORDER BY ts")?
        .query_map([segment_id], |r| r.get(0))?
        .flatten()
        .collect();
    if !own.is_empty() {
        return Ok(Some(PreviewShot { id: own[own.len() / 2], exact: true }));
    }

    let seg: Option<(Option<i64>, String, Option<String>, i64, i64)> = conn
        .query_row(
            "SELECT app_id, title, domain, start_ms, end_ms FROM segments WHERE id = ?1",
            [segment_id],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?)),
        )
        .optional()?;
    let Some((Some(app_id), title, domain, start, end)) = seg else {
        return Ok(None);
    };
    let mid = (start + end) / 2;
    let near = conn
        .query_row(
            "SELECT s.id FROM screenshots s JOIN segments g ON g.id = s.segment_id
             WHERE g.app_id = ?1 AND s.ts BETWEEN ?2 - 600000 AND ?2 + 600000
             ORDER BY (g.domain IS NOT ?3), (g.title IS NOT ?4), ABS(s.ts - ?2)
             LIMIT 1",
            params![app_id, mid, domain, title],
            |r| r.get(0),
        )
        .optional()?;
    Ok(near.map(|id| PreviewShot { id, exact: false }))
}

/// Deletes screenshots (rows and files) older than `retention_days`.
pub fn purge_old_screenshots(conn: &Connection, data_dir: &Path, retention_days: i64) {
    if retention_days <= 0 {
        return;
    }
    let cutoff = now_ms() - retention_days * 24 * 3600 * 1000;
    let Ok(mut stmt) = conn.prepare("SELECT id, path FROM screenshots WHERE ts < ?1") else {
        return;
    };
    let old: Vec<(i64, String)> = stmt
        .query_map([cutoff], |r| Ok((r.get(0)?, r.get(1)?)))
        .map(|rows| rows.flatten().collect())
        .unwrap_or_default();
    for (id, path) in old {
        let _ = std::fs::remove_file(data_dir.join(&path));
        let _ = conn.execute("DELETE FROM screenshots WHERE id = ?1", [id]);
    }
}
