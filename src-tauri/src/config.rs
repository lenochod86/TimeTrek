use serde::{Deserialize, Serialize};
use std::path::Path;

/// Tracker settings persisted in `settings.json` inside the data directory.
/// The UI currently exposes only "start at login" (handled by the autostart
/// plugin); the rest can be tuned by editing the file.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Config {
    /// Seconds without keyboard/mouse input after which time stops counting.
    pub idle_timeout_secs: u64,
    pub screenshots_enabled: bool,
    /// Interval between screenshots while staying in the same window.
    pub screenshot_interval_secs: u64,
    pub screenshot_max_width: u32,
    pub jpeg_quality: u8,
    /// Screenshots older than this are deleted at startup.
    pub retention_days: i64,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            idle_timeout_secs: 300,
            screenshots_enabled: true,
            screenshot_interval_secs: 30,
            screenshot_max_width: 1280,
            jpeg_quality: 70,
            retention_days: 60,
        }
    }
}

impl Config {
    pub fn load(dir: &Path) -> Self {
        let path = dir.join("settings.json");
        match std::fs::read_to_string(&path) {
            Ok(s) => serde_json::from_str(&s).unwrap_or_default(),
            Err(_) => {
                let cfg = Self::default();
                if let Ok(s) = serde_json::to_string_pretty(&cfg) {
                    let _ = std::fs::write(&path, s);
                }
                cfg
            }
        }
    }
}
