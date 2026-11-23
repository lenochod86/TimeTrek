//! Formátování a parsování času pro UI. Čas se ukládá v ms od epochy, zobrazuje se lokálně.

use chrono::{DateTime, Datelike, Duration, Local, NaiveDate, TimeZone, Timelike};

pub const SEC: i64 = 1000;
pub const MIN: i64 = 60 * SEC;
pub const HOUR: i64 = 60 * MIN;

fn local(ms: i64) -> DateTime<Local> {
    Local.timestamp_millis_opt(ms).single().unwrap_or_else(|| Local.timestamp_millis_opt(0).unwrap())
}

/// Lokální půlnoc daného dne v ms (při přechodu na letní čas, kdy půlnoc neexistuje, první existující okamžik dne).
pub fn day_start_ms(date: NaiveDate) -> i64 {
    let naive = date.and_hms_opt(0, 0, 0).unwrap();
    match Local.from_local_datetime(&naive) {
        chrono::LocalResult::Single(d) => d.timestamp_millis(),
        chrono::LocalResult::Ambiguous(a, _) => a.timestamp_millis(),
        chrono::LocalResult::None => Local
            .from_local_datetime(&(naive + Duration::hours(1)))
            .earliest()
            .map(|d| d.timestamp_millis())
            .unwrap_or_else(|| naive.and_utc().timestamp_millis()),
    }
}

pub fn today() -> NaiveDate {
    Local::now().date_naive()
}

pub fn date_of(ms: i64) -> NaiveDate {
    local(ms).date_naive()
}

pub fn add_days(d: NaiveDate, n: i64) -> NaiveDate {
    d + Duration::days(n)
}

/// `DD.MM.YYYY`
pub fn fmt_date(d: NaiveDate) -> String {
    format!("{:02}.{:02}.{}", d.day(), d.month(), d.year())
}

/// Přijímá `D.M.YYYY` (mezery kolem teček jsou povolené) i `YYYY-MM-DD`.
pub fn parse_date(s: &str) -> Option<NaiveDate> {
    let s = s.trim();
    if let Ok(d) = NaiveDate::parse_from_str(s, "%Y-%m-%d") {
        return Some(d);
    }
    let parts: Vec<&str> = s.split('.').map(|p| p.trim()).filter(|p| !p.is_empty()).collect();
    if parts.len() != 3 {
        return None;
    }
    let (d, m, y) = (parts[0].parse().ok()?, parts[1].parse().ok()?, parts[2].parse().ok()?);
    if y < 1970 {
        return None;
    }
    NaiveDate::from_ymd_opt(y, m, d)
}

/// Hodina a minuta lokálního času.
pub fn hour_min(ms: i64) -> (u32, u32) {
    let d = local(ms);
    (d.hour(), d.minute())
}

/// `HH:MM:SS`
pub fn fmt_clock(ms: i64) -> String {
    let d = local(ms);
    format!("{:02}:{:02}:{:02}", d.hour(), d.minute(), d.second())
}

/// `H:MM`
pub fn fmt_hm(ms: i64) -> String {
    let d = local(ms);
    format!("{}:{:02}", d.hour(), d.minute())
}

/// Doba jako `HH:MM:SS` (hodin může být víc než 24).
pub fn fmt_duration(ms: i64) -> String {
    let s = ((ms.max(0) as f64) / 1000.0).round() as i64;
    format!("{:02}:{:02}:{:02}", s / 3600, (s % 3600) / 60, s % 60)
}

/// Doba jako `H:MM` (záhlaví dne).
pub fn fmt_duration_hm(ms: i64) -> String {
    let m = ((ms.max(0) as f64) / 60_000.0).round() as i64;
    format!("{}:{:02}", m / 60, m % 60)
}

/// Desetinné hodiny s českou čárkou, např. `4,36`.
pub fn fmt_hours(ms: i64) -> String {
    format!("{:.2}", ms as f64 / HOUR as f64).replace('.', ",")
}

/// Minuty od lokální půlnoci dne, který obsahuje `ms` (pro průměry začátků dne).
pub fn minutes_of_day(ms: i64) -> f64 {
    let d = local(ms);
    (d.hour() * 60 + d.minute()) as f64
}

pub fn fmt_minutes_of_day(min: f64) -> String {
    let m = min.round() as i64;
    format!("{}:{:02}", m / 60, m % 60)
}

/// Mřížka kalendáře 6×7 (týden od pondělí): čísla dnů, 0 = prázdné políčko.
pub fn month_grid(year: i32, month: u32) -> Vec<i32> {
    let mut out = vec![0; 42];
    let Some(first) = NaiveDate::from_ymd_opt(year, month, 1) else { return out };
    let lead = weekday_index(first);
    let mut d = first;
    while d.month() == month && lead + (d.day() as usize) <= 42 {
        out[lead + d.day() as usize - 1] = d.day() as i32;
        d += Duration::days(1);
    }
    out
}

/// Pondělí = 0.
pub fn weekday_index(d: NaiveDate) -> usize {
    d.weekday().num_days_from_monday() as usize
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ms(y: i32, m: u32, d: u32, h: u32, mi: u32, s: u32) -> i64 {
        Local.with_ymd_and_hms(y, m, d, h, mi, s).unwrap().timestamp_millis()
    }

    #[test]
    fn clock_and_duration() {
        let t = ms(2026, 3, 5, 9, 4, 7);
        assert_eq!(fmt_clock(t), "09:04:07");
        assert_eq!(fmt_hm(t), "9:04");
        assert_eq!(fmt_duration(3_661_000), "01:01:01");
        assert_eq!(fmt_duration(100 * HOUR), "100:00:00");
        assert_eq!(fmt_duration(-5), "00:00:00");
        assert_eq!(fmt_duration_hm(90 * MIN), "1:30");
    }

    #[test]
    fn hours_use_czech_comma() {
        assert_eq!(fmt_hours(HOUR + 30 * MIN), "1,50");
        assert_eq!(fmt_hours(0), "0,00");
    }

    #[test]
    fn minutes_of_day_roundtrip() {
        let t = ms(2026, 3, 5, 8, 30, 59);
        assert_eq!(minutes_of_day(t), 510.0);
        assert_eq!(fmt_minutes_of_day(510.4), "8:30");
    }

    #[test]
    fn parses_czech_and_iso_dates() {
        let d = NaiveDate::from_ymd_opt(2026, 3, 5).unwrap();
        assert_eq!(parse_date("5.3.2026"), Some(d));
        assert_eq!(parse_date(" 05. 03. 2026 "), Some(d));
        assert_eq!(parse_date("2026-03-05"), Some(d));
        assert_eq!(parse_date("32.1.2026"), None);
        assert_eq!(parse_date("abc"), None);
        assert_eq!(fmt_date(d), "05.03.2026");
        assert_eq!(parse_date(&fmt_date(d)), Some(d));
    }

    #[test]
    fn month_grid_starts_on_monday() {
        // Listopad 2026 začíná v neděli: šest prázdných políček, pak 1..30.
        let g = month_grid(2026, 11);
        assert_eq!(g.len(), 42);
        assert_eq!(&g[..7], &[0, 0, 0, 0, 0, 0, 1]);
        assert_eq!(g[6 + 29], 30);
        assert_eq!(g[6 + 30], 0);
        // Únor 2028 (přestupný) začíná v úterý a má 29 dní.
        let g = month_grid(2028, 2);
        assert_eq!(g[0], 0);
        assert_eq!(g[1], 1);
        assert_eq!(g.iter().filter(|d| **d > 0).count(), 29);
        // Měsíc začínající v pondělí a s 31 dny se vejde.
        let g = month_grid(2026, 6);
        assert_eq!(g[0], 1);
        assert_eq!(g.iter().max(), Some(&30));
        assert!(month_grid(2026, 13).iter().all(|d| *d == 0));
    }

    #[test]
    fn day_boundaries() {
        let d = NaiveDate::from_ymd_opt(2026, 3, 5).unwrap();
        let start = day_start_ms(d);
        assert_eq!(date_of(start), d);
        assert_eq!(date_of(start - 1), add_days(d, -1));
        assert_eq!(day_start_ms(add_days(d, 1)) - start, 24 * HOUR);
        assert_eq!(weekday_index(NaiveDate::from_ymd_opt(2026, 3, 2).unwrap()), 0); // pondělí
    }
}
