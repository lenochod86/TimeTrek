//! Texty, které skládá Rust (menu v tray, dny v týdnu, období, štítky souhrnů).
//! Ostatní texty UI jsou ve `ui/texts.slint`.

pub const TRAY_TOOLTIP: &str = "TimeTrek";
pub const TRAY_OPEN: &str = "Otevřít";
pub const TRAY_QUIT: &str = "Zavřít";

pub const WEEKDAYS: [&str; 7] = ["Po", "Út", "St", "Čt", "Pá", "So", "Ne"];

pub const ROW_TOTAL: &str = "Celkem";
pub const ROW_AVG: &str = "Průměr";
pub const ROW_MIN: &str = "Minimum";
pub const ROW_MAX: &str = "Maximum";
pub const HOURS_SHORT: &str = "h";
pub const MINUTES_SHORT: &str = "min";

pub const ACTIVE_LABEL: &str = "Aktivní práce";
pub const IDLE_LABEL: &str = "Nečinnost / zamčeno";
pub const SHOT_UNKNOWN_APP: &str = "?";

/// Vybrané období (hodnoty odpovídají `ui::range::Preset`).
pub const PRESETS: [&str; 8] = [
    "Dnes",
    "Tento týden",
    "Minulý týden",
    "Tento měsíc",
    "Minulý měsíc",
    "Posledních 7 dní",
    "Posledních 30 dní",
    "Vlastní",
];

pub fn tag_delete_question(name: &str) -> String {
    format!("Smazat značku „{name}“?")
}
