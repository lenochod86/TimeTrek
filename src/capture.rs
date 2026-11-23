//! Screenshot worker. Capturing can be slow or block on hung windows, so it
//! runs on its own thread and never delays the tracker loop.

use crate::config::Config;
use crate::db;
use crate::platform;
use chrono::{Local, TimeZone};
use image::codecs::jpeg::JpegEncoder;
use image::imageops::FilterType;
use image::DynamicImage;
use rusqlite::params;
use std::path::PathBuf;
use std::sync::mpsc::{channel, Sender};
use std::thread::JoinHandle;

pub struct CaptureRequest {
    pub segment_id: i64,
    pub handle: isize,
    pub ts: i64,
}

pub fn spawn_worker(
    data_dir: PathBuf,
    db_path: PathBuf,
    cfg: Config,
) -> (Sender<CaptureRequest>, JoinHandle<()>) {
    let (tx, rx) = channel::<CaptureRequest>();
    let handle = std::thread::Builder::new()
        .name("timetrek-capture".into())
        .spawn(move || {
            let Ok(conn) = db::open(&db_path) else { return };
            let platform = platform::create();
            // Ends when the tracker drops the sender.
            while let Ok(req) = rx.recv() {
                let Some(img) = platform.capture_window(req.handle) else {
                    continue;
                };
                let mut img = DynamicImage::ImageRgba8(img);
                if img.width() > cfg.screenshot_max_width {
                    let h = (img.height() as f64 * cfg.screenshot_max_width as f64
                        / img.width() as f64) as u32;
                    img = img.resize_exact(cfg.screenshot_max_width, h.max(1), FilterType::Triangle);
                }
                let rgb = img.to_rgb8();

                let when = Local
                    .timestamp_millis_opt(req.ts)
                    .single()
                    .unwrap_or_else(Local::now);
                let rel_dir = format!("screens/{}", when.format("%Y-%m-%d"));
                let rel = format!("{}/{}_{}.jpg", rel_dir, when.format("%H%M%S"), req.segment_id);
                let abs = data_dir.join(&rel);
                if let Some(parent) = abs.parent() {
                    let _ = std::fs::create_dir_all(parent);
                }
                let mut buf = Vec::new();
                let encoder = JpegEncoder::new_with_quality(&mut buf, cfg.jpeg_quality);
                if rgb.write_with_encoder(encoder).is_err() {
                    continue;
                }
                if std::fs::write(&abs, &buf).is_err() {
                    continue;
                }
                let _ = conn.execute(
                    "INSERT INTO screenshots(segment_id, ts, path) VALUES (?1, ?2, ?3)",
                    params![req.segment_id, req.ts, rel],
                );
            }
        })
        .expect("spawn capture thread");
    (tx, handle)
}
