//! Čtení a zápis dat pro UI. Čisté funkce nad spojením s databází, takže se dají testovat
//! bez okna. Čas je v milisekundách od epochy; hranice dne počítá volající (lokální půlnoc).

use crate::db;
use rusqlite::{params, Connection};
use std::collections::HashMap;

#[derive(Debug, Clone)]
pub struct Segment {
    pub id: i64,
    pub kind: i64,
    pub app_id: Option<i64>,
    pub title: String,
    pub domain: Option<String>,
    pub start_ms: i64,
    pub end_ms: i64,
}

#[derive(Debug, Clone)]
pub struct Tag {
    pub id: i64,
    pub name: String,
    pub color: String,
}

#[derive(Debug, Clone)]
pub struct TagRange {
    pub id: i64,
    pub tag_id: i64,
    pub start_ms: i64,
    pub end_ms: i64,
}

#[derive(Debug, Clone, Default)]
pub struct RangeData {
    pub segments: Vec<Segment>,
    pub apps: HashMap<i64, String>,
    pub tags: Vec<Tag>,
    pub tag_ranges: Vec<TagRange>,
}

pub fn get_range(conn: &Connection, from_ms: i64, to_ms: i64) -> rusqlite::Result<RangeData> {
    let segments = conn
        .prepare(
            "SELECT id, kind, app_id, title, domain, start_ms, end_ms FROM segments
             WHERE end_ms > ?1 AND start_ms < ?2 ORDER BY start_ms",
        )?
        .query_map(params![from_ms, to_ms], |r| {
            Ok(Segment {
                id: r.get(0)?,
                kind: r.get(1)?,
                app_id: r.get(2)?,
                title: r.get(3)?,
                domain: r.get(4)?,
                start_ms: r.get(5)?,
                end_ms: r.get(6)?,
            })
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;

    let apps = app_names(conn)?;

    let tags = conn
        .prepare("SELECT id, name, color FROM tags ORDER BY name")?
        .query_map([], |r| Ok(Tag { id: r.get(0)?, name: r.get(1)?, color: r.get(2)? }))?
        .collect::<rusqlite::Result<Vec<_>>>()?;

    let tag_ranges = conn
        .prepare(
            "SELECT id, tag_id, start_ms, end_ms FROM tag_ranges
             WHERE end_ms > ?1 AND start_ms < ?2 ORDER BY start_ms",
        )?
        .query_map(params![from_ms, to_ms], |r| {
            Ok(TagRange {
                id: r.get(0)?,
                tag_id: r.get(1)?,
                start_ms: r.get(2)?,
                end_ms: r.get(3)?,
            })
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;

    Ok(RangeData { segments, apps, tags, tag_ranges })
}

pub fn app_names(conn: &Connection) -> rusqlite::Result<HashMap<i64, String>> {
    conn.prepare("SELECT id, name FROM apps")?
        .query_map([], |r| Ok((r.get::<_, i64>(0)?, r.get::<_, String>(1)?)))?
        .collect()
}

/// Ikona aplikace jako PNG.
pub fn app_icon(conn: &Connection, id: i64) -> Option<Vec<u8>> {
    conn.query_row("SELECT icon FROM apps WHERE id = ?1", [id], |r| r.get(0))
        .unwrap_or(None)
}

/// Favicon webu jako PNG (stahuje ho tracker na pozadí).
pub fn favicon(conn: &Connection, domain: &str) -> Option<Vec<u8>> {
    conn.query_row("SELECT png FROM favicons WHERE domain = ?1", [domain], |r| r.get(0))
        .unwrap_or(None)
}

#[derive(Debug, Clone)]
pub struct Shot {
    pub id: i64,
    pub segment_id: i64,
    pub ts: i64,
}

fn shot_from(r: &rusqlite::Row) -> rusqlite::Result<Shot> {
    Ok(Shot { id: r.get(0)?, segment_id: r.get(1)?, ts: r.get(2)? })
}

pub fn segment_shots(conn: &Connection, segment_id: i64) -> rusqlite::Result<Vec<Shot>> {
    conn.prepare("SELECT id, segment_id, ts FROM screenshots WHERE segment_id = ?1 ORDER BY ts")?
        .query_map([segment_id], shot_from)?
        .collect()
}

pub fn range_shots(conn: &Connection, from_ms: i64, to_ms: i64) -> rusqlite::Result<Vec<Shot>> {
    conn.prepare("SELECT id, segment_id, ts FROM screenshots WHERE ts >= ?1 AND ts < ?2 ORDER BY ts")?
        .query_map(params![from_ms, to_ms], shot_from)?
        .collect()
}

/// Cesta k souboru snímku relativně k datové složce.
pub fn screenshot_path(conn: &Connection, id: i64) -> Option<String> {
    conn.query_row("SELECT path FROM screenshots WHERE id = ?1", [id], |r| r.get(0)).ok()
}

#[derive(Debug, Clone)]
pub struct DaySummary {
    pub from_ms: i64,
    pub start_ms: Option<i64>,
    pub end_ms: Option<i64>,
    pub active_ms: i64,
    /// Aktivní čas v každé hodině dne (24 přihrádek).
    pub hourly: [i64; 24],
    pub apps: Vec<(i64, i64)>,
    pub domains: Vec<(String, i64)>,
}

/// Souhrny pro dny zadané jako (od, do) v milisekundách.
pub fn get_summaries(conn: &Connection, days: &[(i64, i64)]) -> rusqlite::Result<Vec<DaySummary>> {
    let mut stmt = conn.prepare(
        "SELECT app_id, domain, start_ms, end_ms FROM segments
         WHERE kind = ?1 AND end_ms > ?2 AND start_ms < ?3 ORDER BY start_ms",
    )?;

    let mut out = Vec::new();
    for &(from, to) in days {
        let mut s = DaySummary {
            from_ms: from,
            start_ms: None,
            end_ms: None,
            active_ms: 0,
            hourly: [0; 24],
            apps: Vec::new(),
            domains: Vec::new(),
        };
        let mut apps: HashMap<i64, i64> = HashMap::new();
        let mut domains: HashMap<String, i64> = HashMap::new();

        let rows = stmt.query_map(params![db::KIND_ACTIVE, from, to], |r| {
            Ok((
                r.get::<_, Option<i64>>(0)?,
                r.get::<_, Option<String>>(1)?,
                r.get::<_, i64>(2)?,
                r.get::<_, i64>(3)?,
            ))
        })?;
        for (app_id, domain, start, end) in rows.flatten() {
            let (a, b) = (start.max(from), end.min(to));
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
            // Rozprostřít do hodinových přihrádek.
            let mut t = a;
            while t < b {
                let hour = ((t - from) / 3_600_000).clamp(0, 23);
                let bucket_end = from + (hour + 1) * 3_600_000;
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

/// Přidá značku na časový úsek (značka se podle názvu vytvoří, nebo se jí změní barva).
pub fn add_tag_range(
    conn: &Connection,
    name: &str,
    color: &str,
    start_ms: i64,
    end_ms: i64,
) -> rusqlite::Result<()> {
    let name = name.trim();
    if name.is_empty() || end_ms <= start_ms {
        return Err(rusqlite::Error::InvalidParameterName("invalid tag".into()));
    }
    conn.execute(
        "INSERT INTO tags(name, color) VALUES (?1, ?2)
         ON CONFLICT(name) DO UPDATE SET color = ?2",
        params![name, color],
    )?;
    let tag_id: i64 =
        conn.query_row("SELECT id FROM tags WHERE name = ?1", [name], |r| r.get(0))?;
    conn.execute(
        "INSERT INTO tag_ranges(tag_id, start_ms, end_ms) VALUES (?1, ?2, ?3)",
        params![tag_id, start_ms, end_ms],
    )?;
    Ok(())
}

pub fn delete_tag_range(conn: &Connection, id: i64) -> rusqlite::Result<()> {
    conn.execute("DELETE FROM tag_ranges WHERE id = ?1", [id])?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    const H: i64 = 3_600_000;

    fn conn() -> Connection {
        db::open_memory().unwrap()
    }

    #[test]
    fn summary_spreads_segment_over_hours_and_clips_to_day() {
        let c = conn();
        let app = db::upsert_app(&c, "a.exe", "A", None).unwrap();
        // Den 0..24 h; záznam z předchozího dne (-1 h) do 1:30 se ořízne na 0..1:30.
        db::insert_segment(&c, db::KIND_ACTIVE, Some(app), "t", Some("x.cz"), -H, H + H / 2).unwrap();
        // Nečinnost se nepočítá.
        db::insert_segment(&c, db::KIND_IDLE, None, "", None, 2 * H, 3 * H).unwrap();
        let s = &get_summaries(&c, &[(0, 24 * H)]).unwrap()[0];
        assert_eq!(s.active_ms, H + H / 2);
        assert_eq!(s.hourly[0], H);
        assert_eq!(s.hourly[1], H / 2);
        assert_eq!(s.hourly[2], 0);
        assert_eq!(s.start_ms, Some(0));
        assert_eq!(s.end_ms, Some(H + H / 2));
        assert_eq!(s.apps, vec![(app, H + H / 2)]);
        assert_eq!(s.domains, vec![("x.cz".to_string(), H + H / 2)]);
    }

    #[test]
    fn empty_day_has_no_start() {
        let c = conn();
        let s = &get_summaries(&c, &[(0, 24 * H)]).unwrap()[0];
        assert_eq!(s.active_ms, 0);
        assert_eq!(s.start_ms, None);
    }

    #[test]
    fn range_returns_only_overlapping_segments() {
        let c = conn();
        db::insert_segment(&c, db::KIND_ACTIVE, None, "před", None, 0, 100).unwrap();
        db::insert_segment(&c, db::KIND_ACTIVE, None, "uvnitř", None, 150, 250).unwrap();
        db::insert_segment(&c, db::KIND_ACTIVE, None, "za", None, 400, 500).unwrap();
        let d = get_range(&c, 100, 300).unwrap();
        assert_eq!(d.segments.len(), 1);
        assert_eq!(d.segments[0].title, "uvnitř");
    }

    #[test]
    fn tag_range_reuses_tag_and_updates_color() {
        let c = conn();
        add_tag_range(&c, "  Práce ", "#111111", 0, 10).unwrap();
        add_tag_range(&c, "Práce", "#222222", 20, 30).unwrap();
        let d = get_range(&c, 0, 100).unwrap();
        assert_eq!(d.tags.len(), 1);
        assert_eq!(d.tags[0].name, "Práce");
        assert_eq!(d.tags[0].color, "#222222");
        assert_eq!(d.tag_ranges.len(), 2);
        delete_tag_range(&c, d.tag_ranges[0].id).unwrap();
        assert_eq!(get_range(&c, 0, 100).unwrap().tag_ranges.len(), 1);
    }

    #[test]
    fn tag_range_rejects_empty_name_and_empty_range() {
        let c = conn();
        assert!(add_tag_range(&c, "  ", "#111111", 0, 10).is_err());
        assert!(add_tag_range(&c, "x", "#111111", 10, 10).is_err());
    }
}
