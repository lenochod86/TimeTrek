//! Downloads website favicons straight from the visited site (no third party
//! service, so the list of visited domains never leaves the machine except to
//! the site itself) and caches them as 32x32 PNG in the database.

use crate::db;
use image::imageops::FilterType;
use rusqlite::{params, Connection, OptionalExtension};
use std::io::Read;
use std::path::PathBuf;
use std::sync::mpsc::{channel, Sender};
use std::thread::JoinHandle;
use std::time::Duration;

const RETRY_FAILED_MS: i64 = 24 * 3600 * 1000;
const REFRESH_MS: i64 = 14 * 24 * 3600 * 1000;

pub fn spawn_worker(db_path: PathBuf) -> (Sender<String>, JoinHandle<()>) {
    let (tx, rx) = channel::<String>();
    let handle = std::thread::Builder::new()
        .name("timetrek-favicon".into())
        .spawn(move || {
            let Ok(conn) = db::open(&db_path) else { return };
            while let Ok(domain) = rx.recv() {
                if !needs_fetch(&conn, &domain) {
                    continue;
                }
                let png = fetch(&domain);
                let _ = conn.execute(
                    "INSERT INTO favicons(domain, png, fetched_ms) VALUES (?1, ?2, ?3)
                     ON CONFLICT(domain) DO UPDATE SET png = ?2, fetched_ms = ?3",
                    params![domain, png, db::now_ms()],
                );
            }
        })
        .expect("spawn favicon thread");
    (tx, handle)
}

fn needs_fetch(conn: &Connection, domain: &str) -> bool {
    let row: Option<(bool, i64)> = conn
        .query_row(
            "SELECT png IS NOT NULL, fetched_ms FROM favicons WHERE domain = ?1",
            [domain],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .optional()
        .unwrap_or(None);
    match row {
        None => true,
        Some((has, at)) => {
            let age = db::now_ms() - at;
            age > if has { REFRESH_MS } else { RETRY_FAILED_MS }
        }
    }
}

fn agent() -> ureq::Agent {
    ureq::AgentBuilder::new()
        .timeout(Duration::from_secs(6))
        .user_agent("Mozilla/5.0 (Windows NT 10.0; Win64; x64) TimeTrek")
        .build()
}

fn fetch(domain: &str) -> Option<Vec<u8>> {
    let agent = agent();
    let local = domain == "localhost";
    let schemes: &[&str] = if local { &["http", "https"] } else { &["https", "http"] };

    for scheme in schemes {
        let base = format!("{scheme}://{domain}/");
        let Some(html) = get_text(&agent, &base) else {
            continue;
        };
        let mut candidates = icon_links(&html, &base);
        candidates.push(format!("{base}favicon.ico"));
        for url in candidates {
            if let Some(png) = get_icon(&agent, &url) {
                return Some(png);
            }
        }
        return None;
    }
    None
}

fn get_text(agent: &ureq::Agent, url: &str) -> Option<String> {
    let resp = agent.get(url).call().ok()?;
    let mut buf = Vec::new();
    resp.into_reader().take(300_000).read_to_end(&mut buf).ok()?;
    Some(String::from_utf8_lossy(&buf).to_string())
}

fn get_icon(agent: &ureq::Agent, url: &str) -> Option<Vec<u8>> {
    let resp = agent.get(url).call().ok()?;
    let mut buf = Vec::new();
    resp.into_reader().take(1_000_000).read_to_end(&mut buf).ok()?;
    let img = image::load_from_memory(&buf).ok()?;
    let img = img.resize(32, 32, FilterType::Lanczos3).to_rgba8();
    let mut out = Vec::new();
    img.write_to(&mut std::io::Cursor::new(&mut out), image::ImageFormat::Png)
        .ok()?;
    Some(out)
}

/// Absolute URLs of `<link rel="...icon...">` entries (SVG skipped - can't be decoded).
fn icon_links(html: &str, base: &str) -> Vec<String> {
    let lower = html.to_lowercase();
    let base_url = url::Url::parse(base).ok();
    let mut out = Vec::new();
    let mut pos = 0;
    while let Some(i) = lower[pos..].find("<link") {
        let start = pos + i;
        let Some(end_rel) = lower[start..].find('>') else { break };
        let tag = &html[start..start + end_rel];
        pos = start + end_rel;
        let rel = attr(tag, "rel").unwrap_or_default().to_lowercase();
        if !rel.contains("icon") || rel.contains("mask-icon") {
            continue;
        }
        let Some(href) = attr(tag, "href") else { continue };
        if href.starts_with("data:") || href.to_lowercase().contains(".svg") {
            continue;
        }
        if let Some(abs) = base_url.as_ref().and_then(|b| b.join(&href).ok()) {
            out.push(abs.to_string());
        }
    }
    out
}

fn attr(tag: &str, name: &str) -> Option<String> {
    let lower = tag.to_lowercase();
    let mut search = 0;
    while let Some(i) = lower[search..].find(name) {
        let at = search + i;
        search = at + name.len();
        let before_ok = at == 0 || lower.as_bytes()[at - 1].is_ascii_whitespace();
        let rest = tag[at + name.len()..].trim_start();
        if !before_ok || !rest.starts_with('=') {
            continue;
        }
        let val = rest[1..].trim_start();
        return match val.chars().next()? {
            q @ ('"' | '\'') => val[1..].split(q).next().map(|s| s.to_string()),
            _ => val.split(|c: char| c.is_whitespace() || c == '>').next().map(|s| s.to_string()),
        };
    }
    None
}
