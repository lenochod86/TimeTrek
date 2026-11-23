//! Texty, které skládá Rust (menu v tray, dny v týdnu, období, štítky souhrnů).
//! Ostatní texty UI jsou ve `ui/texts.slint`.

pub const TRAY_TOOLTIP: &str = "TimeTrek";
pub const TRAY_OPEN: &str = "Otevřít";
pub const TRAY_QUIT: &str = "Zavřít";

pub const MONTHS: [&str; 12] = [
    "leden", "únor", "březen", "duben", "květen", "červen", "červenec", "srpen", "září", "říjen", "listopad", "prosinec",
];

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

pub fn update_available(new: &str, current: &str) -> String {
    format!("Je dostupná nová verze {new} (používáš {current}).")
}
pub const UPDATE_DOWNLOAD: &str = "Stáhnout a nainstalovat";
pub const UPDATE_OPEN_PAGE: &str = "Zobrazit vydání";
pub const UPDATE_DOWNLOADING: &str = "Stahuji…";
pub fn update_progress(percent: u64) -> String {
    format!("Stahuji novou verzi… {percent} %")
}
pub fn update_failed(e: &str) -> String {
    format!("Stažení se nepodařilo: {e}")
}
pub const UPDATE_OFF: &str = "Automatická kontrola je vypnutá.";
pub const UPDATE_CHECKING: &str = "Kontroluji…";
pub const UPDATE_CHECK_FAILED: &str = "Kontrola se nepodařila (zkontroluj připojení k internetu).";
pub fn update_up_to_date(v: &str) -> String {
    format!("Používáš nejnovější verzi ({v}).")
}
pub fn update_found(v: &str) -> String {
    format!("Nalezena verze {v}.")
}
pub fn update_tray_tooltip(v: &str) -> String {
    format!("TimeTrek: je dostupná nová verze {v}")
}
