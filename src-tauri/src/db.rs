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
