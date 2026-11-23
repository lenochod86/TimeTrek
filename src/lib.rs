pub mod app;
pub mod autostart;
mod capture;
pub mod config;
pub mod data;
pub mod db;
mod favicon;
pub mod i18n;
pub mod platform;
pub mod single_instance;
mod tracker;
#[cfg(any(windows, target_os = "macos"))]
mod tray;
pub mod ui;

pub use app::run;
