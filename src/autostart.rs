//! Spuštění po přihlášení. Na Windows záznam v `HKCU\...\Run` (stejný název jako dřív,
//! takže se zachová volba z předchozích verzí), na macOS LaunchAgent.

use auto_launch::AutoLaunchBuilder;

fn builder() -> Result<auto_launch::AutoLaunch, String> {
    let exe = std::env::current_exe().map_err(|e| e.to_string())?;
    AutoLaunchBuilder::new()
        .set_app_name("TimeTrek")
        .set_app_path(&exe.to_string_lossy())
        .set_args(&["--hidden"])
        .build()
        .map_err(|e| e.to_string())
}

pub fn is_enabled() -> bool {
    builder().and_then(|a| a.is_enabled().map_err(|e| e.to_string())).unwrap_or(false)
}

pub fn set_enabled(on: bool) -> Result<(), String> {
    let a = builder()?;
    if on { a.enable() } else { a.disable() }.map_err(|e| e.to_string())
}
