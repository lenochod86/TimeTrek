//! Tauri commands called from the React UI.

use crate::config;
use crate::db;
use crate::AppState;
use base64::Engine;
use rusqlite::params;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use tauri::State;

type CmdResult<T> = Result<T, String>;

fn err<E: std::fmt::Display>(e: E) -> String {
    e.to_string()
}

fn data_url(mime: &str, bytes: &[u8]) -> String {
    format!(
        "data:{mime};base64,{}",
        base64::engine::general_purpose::STANDARD.encode(bytes)
    )
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SegmentDto {
    id: i64,
    kind: i64,
    app_id: Option<i64>,
    title: String,
    domain: Option<String>,
    start_ms: i64,
    end_ms: i64,
}

#[derive(Serialize)]
pub struct AppDto {
    id: i64,
    name: String,
}

#[derive(Serialize)]
pub struct TagDto {
    id: i64,
    name: String,
    color: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TagRangeDto {
    id: i64,
    tag_id: i64,
    start_ms: i64,
    end_ms: i64,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RangeData {
    segments: Vec<SegmentDto>,
    apps: Vec<AppDto>,
    tags: Vec<TagDto>,
    tag_ranges: Vec<TagRangeDto>,
}

#[tauri::command]
pub fn get_range(state: State<AppState>, from_ms: i64, to_ms: i64) -> CmdResult<RangeData> {
    let conn = state.conn.lock().map_err(err)?;

    let segments = conn
        .prepare(
            "SELECT id, kind, app_id, title, domain, start_ms, end_ms FROM segments
             WHERE end_ms > ?1 AND start_ms < ?2 ORDER BY start_ms",
        )
        .map_err(err)?
        .query_map(params![from_ms, to_ms], |r| {
            Ok(SegmentDto {
                id: r.get(0)?,
                kind: r.get(1)?,
                app_id: r.get(2)?,
                title: r.get(3)?,
                domain: r.get(4)?,
                start_ms: r.get(5)?,
                end_ms: r.get(6)?,
            })
        })
        .map_err(err)?
        .flatten()
        .collect::<Vec<_>>();

    let apps = conn
        .prepare("SELECT id, name FROM apps")
        .map_err(err)?
        .query_map([], |r| Ok(AppDto { id: r.get(0)?, name: r.get(1)? }))
        .map_err(err)?
        .flatten()
        .collect();

    let tags = conn
        .prepare("SELECT id, name, color FROM tags ORDER BY name")
        .map_err(err)?
        .query_map([], |r| {
            Ok(TagDto { id: r.get(0)?, name: r.get(1)?, color: r.get(2)? })
        })
        .map_err(err)?
        .flatten()
        .collect();

    let tag_ranges = conn
        .prepare(
            "SELECT id, tag_id, start_ms, end_ms FROM tag_ranges
             WHERE end_ms > ?1 AND start_ms < ?2 ORDER BY start_ms",
        )
        .map_err(err)?
        .query_map(params![from_ms, to_ms], |r| {
            Ok(TagRangeDto {
                id: r.get(0)?,
                tag_id: r.get(1)?,
                start_ms: r.get(2)?,
                end_ms: r.get(3)?,
            })
        })
        .map_err(err)?
        .flatten()
        .collect();

    Ok(RangeData { segments, apps, tags, tag_ranges })
}

#[tauri::command]
pub fn get_app_icons(state: State<AppState>, ids: Vec<i64>) -> CmdResult<HashMap<i64, String>> {
    let conn = state.conn.lock().map_err(err)?;
    let mut out = HashMap::new();
    for id in ids {
        let icon: Option<Vec<u8>> = conn
            .query_row("SELECT icon FROM apps WHERE id = ?1", [id], |r| r.get(0))
            .unwrap_or(None);
        if let Some(bytes) = icon {
            out.insert(id, data_url("image/png", &bytes));
        }
    }
    Ok(out)
}

#[tauri::command]
pub fn get_favicons(
    state: State<AppState>,
    domains: Vec<String>,
) -> CmdResult<HashMap<String, String>> {
    let conn = state.conn.lock().map_err(err)?;
    let mut out = HashMap::new();
    for d in domains {
        let png: Option<Vec<u8>> = conn
            .query_row("SELECT png FROM favicons WHERE domain = ?1", [&d], |r| r.get(0))
            .unwrap_or(None);
        if let Some(bytes) = png {
            out.insert(d, data_url("image/png", &bytes));
        }
    }
    Ok(out)
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ShotDto {
    id: i64,
    segment_id: i64,
    ts: i64,
}

/// Screenshots of one segment (`segment_id`) or of a whole time range.
#[tauri::command]
pub fn get_screenshots(
    state: State<AppState>,
    segment_id: Option<i64>,
    from_ms: i64,
    to_ms: i64,
) -> CmdResult<Vec<ShotDto>> {
    let conn = state.conn.lock().map_err(err)?;
    let map = |r: &rusqlite::Row| -> rusqlite::Result<ShotDto> {
        Ok(ShotDto { id: r.get(0)?, segment_id: r.get(1)?, ts: r.get(2)? })
    };
    let rows: Vec<ShotDto> = if let Some(seg) = segment_id {
        conn.prepare("SELECT id, segment_id, ts FROM screenshots WHERE segment_id = ?1 ORDER BY ts")
            .map_err(err)?
            .query_map([seg], map)
            .map_err(err)?
            .flatten()
            .collect()
    } else {
        conn.prepare(
            "SELECT id, segment_id, ts FROM screenshots WHERE ts >= ?1 AND ts < ?2 ORDER BY ts",
        )
        .map_err(err)?
        .query_map(params![from_ms, to_ms], map)
        .map_err(err)?
        .flatten()
        .collect()
    };
    Ok(rows)
}

#[derive(Serialize)]
pub struct PreviewDto {
    id: i64,
    /// `false` = nearest snapshot of the same application, not of this very record.
    exact: bool,
}

#[tauri::command]
pub fn get_preview_shot(state: State<AppState>, segment_id: i64) -> CmdResult<Option<PreviewDto>> {
    let conn = state.conn.lock().map_err(err)?;
    let shot = db::preview_shot(&conn, segment_id).map_err(err)?;
    Ok(shot.map(|s| PreviewDto { id: s.id, exact: s.exact }))
}

#[tauri::command]
pub fn get_screenshot(state: State<AppState>, id: i64) -> CmdResult<Option<String>> {
    let rel: Option<String> = {
        let conn = state.conn.lock().map_err(err)?;
        conn.query_row("SELECT path FROM screenshots WHERE id = ?1", [id], |r| r.get(0))
            .ok()
    };
    let Some(rel) = rel else { return Ok(None) };
    match std::fs::read(state.data_dir.join(rel)) {
        Ok(bytes) => Ok(Some(data_url("image/jpeg", &bytes))),
        Err(_) => Ok(None),
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DayRange {
    from_ms: i64,
    to_ms: i64,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DaySummary {
    from_ms: i64,
    start_ms: Option<i64>,
    end_ms: Option<i64>,
    active_ms: i64,
    /// Active milliseconds per hour of the day (24 buckets).
    hourly: Vec<i64>,
    apps: Vec<(i64, i64)>,
    domains: Vec<(String, i64)>,
}

#[tauri::command]
pub fn get_summaries(state: State<AppState>, days: Vec<DayRange>) -> CmdResult<Vec<DaySummary>> {
    let conn = state.conn.lock().map_err(err)?;
    let mut stmt = conn
        .prepare(
            "SELECT app_id, domain, start_ms, end_ms FROM segments
             WHERE kind = ?1 AND end_ms > ?2 AND start_ms < ?3 ORDER BY start_ms",
        )
        .map_err(err)?;

    let mut out = Vec::new();
    for day in days {
        let mut s = DaySummary {
            from_ms: day.from_ms,
            start_ms: None,
            end_ms: None,
            active_ms: 0,
            hourly: vec![0; 24],
            apps: Vec::new(),
            domains: Vec::new(),
        };
        let mut apps: HashMap<i64, i64> = HashMap::new();
        let mut domains: HashMap<String, i64> = HashMap::new();

        let rows = stmt
            .query_map(params![db::KIND_ACTIVE, day.from_ms, day.to_ms], |r| {
                Ok((
                    r.get::<_, Option<i64>>(0)?,
                    r.get::<_, Option<String>>(1)?,
                    r.get::<_, i64>(2)?,
                    r.get::<_, i64>(3)?,
                ))
            })
            .map_err(err)?;
        for (app_id, domain, start, end) in rows.flatten() {
            let (a, b) = (start.max(day.from_ms), end.min(day.to_ms));
            if b <= a {
                continue;
            }
            s.start_ms = Some(s.start_ms.map_or(a, |v| v.min(a)));
            s.end_ms = Some(s.end_ms.map_or(b, |v| v.max(b)));
            s.active_ms += b - a;
            if let Some(id) = app_id {
                *apps.entry(id).or_default() += b - a;
            }
            if let Some(d) = domain {
                *domains.entry(d).or_default() += b - a;
            }
            // Spread over hourly buckets.
            let mut t = a;
            while t < b {
                let hour = ((t - day.from_ms) / 3_600_000).clamp(0, 23);
                let bucket_end = day.from_ms + (hour + 1) * 3_600_000;
                let upto = b.min(bucket_end);
                s.hourly[hour as usize] += upto - t;
                t = upto;
            }
        }
        s.apps = apps.into_iter().collect();
        s.domains = domains.into_iter().collect();
        out.push(s);
    }
    Ok(out)
}

#[tauri::command]
pub fn add_tag_range(
    state: State<AppState>,
    name: String,
    color: String,
    start_ms: i64,
    end_ms: i64,
) -> CmdResult<()> {
    let conn = state.conn.lock().map_err(err)?;
    let name = name.trim().to_string();
    if name.is_empty() || end_ms <= start_ms {
        return Err("invalid tag".into());
    }
    conn.execute(
        "INSERT INTO tags(name, color) VALUES (?1, ?2)
         ON CONFLICT(name) DO UPDATE SET color = ?2",
        params![name, color],
    )
    .map_err(err)?;
    let tag_id: i64 = conn
        .query_row("SELECT id FROM tags WHERE name = ?1", [&name], |r| r.get(0))
        .map_err(err)?;
    conn.execute(
        "INSERT INTO tag_ranges(tag_id, start_ms, end_ms) VALUES (?1, ?2, ?3)",
        params![tag_id, start_ms, end_ms],
    )
    .map_err(err)?;
    Ok(())
}

#[tauri::command]
pub fn delete_tag_range(state: State<AppState>, id: i64) -> CmdResult<()> {
    let conn = state.conn.lock().map_err(err)?;
    conn.execute("DELETE FROM tag_ranges WHERE id = ?1", [id]).map_err(err)?;
    Ok(())
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SettingsDto {
    idle_timeout_minutes: u64,
}

#[tauri::command]
pub fn get_settings(state: State<AppState>) -> CmdResult<SettingsDto> {
    let cfg = state.config.read().map_err(err)?;
    Ok(SettingsDto { idle_timeout_minutes: cfg.idle_timeout_secs / 60 })
}

/// Changes the inactivity interval; the running tracker picks it up on its next tick.
#[tauri::command]
pub fn set_idle_timeout_minutes(state: State<AppState>, minutes: u64) -> CmdResult<SettingsDto> {
    let secs = (minutes * 60).clamp(config::MIN_IDLE_SECS, config::MAX_IDLE_SECS);
    let mut cfg = state.config.write().map_err(err)?;
    cfg.idle_timeout_secs = secs;
    cfg.save(&state.data_dir);
    Ok(SettingsDto { idle_timeout_minutes: secs / 60 })
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AppInfo {
    version: String,
    data_dir: String,
}

#[tauri::command]
pub fn get_app_info(app: tauri::AppHandle, state: State<AppState>) -> AppInfo {
    AppInfo {
        version: app.package_info().version.to_string(),
        data_dir: state.data_dir.to_string_lossy().to_string(),
    }
}
