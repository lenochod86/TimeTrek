//! The only place where the operating system matters.
//!
//! Everything platform specific hides behind [`PlatformApi`]; the tracker, the
//! database and the UI never call OS APIs directly. To port TimeTrek to macOS,
//! implement this trait in `macos.rs` (NSWorkspace / CGWindowList / Accessibility).

use image::RgbaImage;

#[cfg(windows)]
mod windows;
#[cfg(windows)]
pub use self::windows::create;

#[cfg(target_os = "macos")]
mod macos;
#[cfg(target_os = "macos")]
pub use self::macos::create;

#[derive(Debug, Clone)]
pub struct WindowInfo {
    /// Opaque native handle (HWND on Windows, window id on macOS).
    pub handle: isize,
    pub pid: u32,
    /// Full path of the executable.
    pub exe: String,
    pub title: String,
}

pub trait PlatformApi {
    /// Currently focused top-level window, if any.
    fn foreground_window(&self) -> Option<WindowInfo>;
    /// Seconds since the last keyboard / mouse input.
    fn idle_seconds(&self) -> u64;
    /// Human readable application name (e.g. "Microsoft Edge").
    fn app_name(&self, exe: &str) -> String;
    /// Application icon as PNG bytes.
    fn app_icon_png(&self, exe: &str) -> Option<Vec<u8>>;
    /// Address-bar text of the active browser tab for supported browsers.
    fn browser_address(&self, window: &WindowInfo) -> Option<String>;
    /// Grabs the content of a single window.
    fn capture_window(&self, handle: isize) -> Option<RgbaImage>;
}
