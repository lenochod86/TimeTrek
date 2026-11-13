use serde::{Deserialize, Serialize};
use std::path::Path;
use std::sync::{Arc, RwLock};

/// Tracker settings persisted in `settings.json` inside the data directory.
/// The tracker re-reads them every second, so changes apply immediately.
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

pub type SharedConfig = Arc<RwLock<Config>>;

pub const MIN_IDLE_SECS: u64 = 60;
pub const MAX_IDLE_SECS: u64 = 120 * 60;

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
        let mut cfg = match std::fs::read_to_string(&path) {
            Ok(s) => serde_json::from_str(&s).unwrap_or_default(),
            Err(_) => {
                let cfg = Self::default();
                cfg.save(dir);
                cfg
            }
        };
        cfg.idle_timeout_secs = cfg.idle_timeout_secs.clamp(MIN_IDLE_SECS, MAX_IDLE_SECS);
        cfg
    }

    pub fn save(&self, dir: &Path) {
        if let Ok(s) = serde_json::to_string_pretty(self) {
            let _ = std::fs::write(dir.join("settings.json"), s);
        }
    }
}
