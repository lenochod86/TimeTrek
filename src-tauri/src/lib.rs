mod capture;
mod commands;
mod config;
mod db;
mod favicon;
mod platform;
mod tracker;
mod tray;

use rusqlite::Connection;
use std::path::PathBuf;
use std::sync::{Arc, Mutex, RwLock};
use tauri::{Manager, RunEvent, WindowEvent};
use tauri_plugin_autostart::MacosLauncher;

/// Shared state for UI commands (the tracker threads own their own connections).
pub struct AppState {
    pub conn: Mutex<Connection>,
    pub data_dir: PathBuf,
    pub config: config::SharedConfig,
}

pub fn run() {
    let app = tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
            // A second launch just brings the running instance to the front.
            tray::show_main(app);
        }))
        .plugin(tauri_plugin_autostart::init(
            MacosLauncher::LaunchAgent,
            Some(vec!["--hidden"]),
        ))
        .setup(|app| {
            let data_dir = app.path().data_dir()?.join("TimeTrek");
            std::fs::create_dir_all(&data_dir)?;
            let db_path = data_dir.join("timetrek.db");
            let conn = db::open(&db_path)?;
            let cfg: config::SharedConfig =
                Arc::new(RwLock::new(config::Config::load(&data_dir)));

            let tracker = tracker::Tracker::start(data_dir.clone(), db_path, cfg.clone());
            app.manage(tracker);
            app.manage(AppState { conn: Mutex::new(conn), data_dir, config: cfg });

            tray::create(app.handle())?;

            // Autostart launches with --hidden: stay in the tray.
            let hidden = std::env::args().any(|a| a == "--hidden");
            if !hidden {
                tray::show_main(app.handle());
            }
            Ok(())
        })
        .on_window_event(|window, event| {
            // Closing the window only hides it; tracking continues in the tray.
            if let WindowEvent::CloseRequested { api, .. } = event {
                api.prevent_close();
                let _ = window.hide();
            }
        })
        .invoke_handler(tauri::generate_handler![
            commands::get_range,
            commands::get_app_icons,
            commands::get_favicons,
            commands::get_screenshots,
            commands::get_screenshot,
            commands::get_summaries,
            commands::add_tag_range,
            commands::delete_tag_range,
            commands::get_app_info,
            commands::get_settings,
            commands::set_idle_timeout_minutes,
        ])
        .build(tauri::generate_context!())
        .expect("error while building TimeTrek");

    app.run(|app_handle, event| {
        if let RunEvent::Exit = event {
            if let Some(tracker) = app_handle.try_state::<tracker::Tracker>() {
                tracker.shutdown();
            }
        }
    });
}
