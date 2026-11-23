//! macOS backend placeholder - not implemented yet. See `platform/mod.rs`.
use super::{PlatformApi, WindowInfo};
use image::RgbaImage;

pub struct MacPlatform;

pub fn create() -> Box<dyn PlatformApi> {
    Box::new(MacPlatform)
}

impl PlatformApi for MacPlatform {
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
}
