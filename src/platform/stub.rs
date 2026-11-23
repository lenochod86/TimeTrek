//! Zástupná platforma pro systémy bez podpory (např. Linux): vývoj UI a testy bez měření.
use super::{PlatformApi, WindowInfo};
use image::RgbaImage;

pub struct StubPlatform;

pub fn create() -> Box<dyn PlatformApi> {
    Box::new(StubPlatform)
}

impl PlatformApi for StubPlatform {
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
    fn app_icon_png(&self, _exe: &str) -> Option<Vec<u8>> {
        None
    }
    fn browser_address(&self, _window: &WindowInfo) -> Option<String> {
        None
    }
    fn capture_window(&self, _handle: isize) -> Option<RgbaImage> {
        None
    }
    fn open_url(&self, url: &str) {
        let _ = std::process::Command::new("xdg-open").arg(url).spawn();
    }
}
