//! Záložky Statistika a Přehledy: výběr období a souhrny po dnech.

use super::colors::color_for;
use super::fmt::*;
use super::{slint_color, BarRowD, Ctl, DayBarD, HeatD, MainWindow, Nav, RangeState, ReportsState, StatRowD, StatsState};
use crate::data::{self, DaySummary};
use crate::i18n;
use chrono::{Datelike, Duration, NaiveDate};
use slint::{ComponentHandle, ModelRc, VecModel};
use std::collections::HashMap;
use std::rc::Rc;

/// Pořadí odpovídá `i18n::PRESETS`.
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Preset {
    Today,
    ThisWeek,
    LastWeek,
    ThisMonth,
    LastMonth,
    Last7,
    Last30,
    Custom,
}

const PRESETS: [Preset; 8] = [
    Preset::Today,
    Preset::ThisWeek,
    Preset::LastWeek,
    Preset::ThisMonth,
    Preset::LastMonth,
    Preset::Last7,
    Preset::Last30,
    Preset::Custom,
];

/// Období; `to` je včetně.
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct Range {
    pub from: NaiveDate,
    pub to: NaiveDate,
    pub preset: Preset,
}

fn monday_of(d: NaiveDate) -> NaiveDate {
    d - Duration::days(weekday_index(d) as i64)
}

fn month_start(y: i32, m: i32) -> NaiveDate {
    // m může být mimo 1..=12 (předchozí / příští rok)
    let y = y + (m - 1).div_euclid(12);
    let m = (m - 1).rem_euclid(12) + 1;
    NaiveDate::from_ymd_opt(y, m as u32, 1).unwrap()
}

fn month_end(y: i32, m: i32) -> NaiveDate {
    month_start(y, m + 1) - Duration::days(1)
}

pub fn preset_range(p: Preset, today: NaiveDate) -> Range {
    let (y, m) = (today.year(), today.month() as i32);
    let (from, to) = match p {
        Preset::Today | Preset::Custom => (today, today),
        Preset::ThisWeek => (monday_of(today), monday_of(today) + Duration::days(6)),
        Preset::LastWeek => {
            let mon = monday_of(today) - Duration::days(7);
            (mon, mon + Duration::days(6))
        }
        Preset::ThisMonth => (month_start(y, m), month_end(y, m)),
        Preset::LastMonth => (month_start(y, m - 1), month_end(y, m - 1)),
        Preset::Last7 => (today - Duration::days(6), today),
        Preset::Last30 => (today - Duration::days(29), today),
    };
    Range { from, to, preset: p }
}

/// Posun o délku období (u celých měsíců o měsíc).
pub fn shift(r: Range, dir: i32) -> Range {
    if matches!(r.preset, Preset::ThisMonth | Preset::LastMonth) {
        let (y, m) = (r.from.year(), r.from.month() as i32 + dir);
        return Range { from: month_start(y, m), to: month_end(y, m), preset: Preset::Custom };
    }
    let len = (r.to - r.from).num_days() + 1;
    let d = Duration::days(len * dir as i64);
    Range { from: r.from + d, to: r.to + d, preset: Preset::Custom }
}

pub struct RangeData {
    pub range: Range,
    pub days: Vec<NaiveDate>,
    pub summaries: Vec<DaySummary>,
}

impl RangeData {
    pub fn new() -> Self {
        Self { range: preset_range(Preset::ThisWeek, today()), days: Vec::new(), summaries: Vec::new() }
    }
}

/// Dny období (nejvýše 400, aby špatně zadané období nezatížilo UI).
fn days_of(r: Range) -> Vec<NaiveDate> {
    let mut out = Vec::new();
    let mut d = r.from;
    while d <= r.to && out.len() < 400 {
        out.push(d);
        d += Duration::days(1);
    }
    out
}

impl Ctl {
    pub(super) fn bind_range(self: &Rc<Self>, ui: &MainWindow) {
        use super::on;
        let ctl = self.clone();
        let r = ui.global::<RangeState>();
        on!(ctl, r, on_from_edited, |c, t| c.range_edited(&t, true));
        on!(ctl, r, on_to_edited, |c, t| c.range_edited(&t, false));
        on!(ctl, r, on_shift, |c, dir| c.range_shift(dir));
        on!(ctl, r, on_preset_selected, |c, i| c.range_preset(i));
    }

    pub(super) fn init_range(&self, ui: &MainWindow) {
        let labels: Vec<slint::SharedString> = i18n::PRESETS.iter().map(|s| (*s).into()).collect();
        ui.global::<RangeState>().set_preset_labels(ModelRc::new(VecModel::from(labels)));
        let wd: Vec<slint::SharedString> = i18n::WEEKDAYS.iter().map(|s| (*s).into()).collect();
        ui.global::<ReportsState>().set_weekdays(ModelRc::new(VecModel::from(wd)));
        self.push_range_bar();
    }

    fn set_range(&self, r: Range) {
        self.st.borrow_mut().range.range = r;
        self.push_range_bar();
        self.refresh_range();
    }

    fn range_edited(&self, text: &str, is_from: bool) {
        let cur = self.st.borrow().range.range;
        match parse_date(text) {
            Some(d) if is_from => self.set_range(Range { from: d, to: cur.to.max(d), preset: Preset::Custom }),
            Some(d) => self.set_range(Range { from: cur.from.min(d), to: d, preset: Preset::Custom }),
            None => self.push_range_bar(),
        }
    }

    fn range_shift(&self, dir: i32) {
        let cur = self.st.borrow().range.range;
        self.set_range(shift(cur, dir));
    }

    fn range_preset(&self, index: i32) {
        let p = PRESETS.get(index as usize).copied().unwrap_or(Preset::Custom);
        let cur = self.st.borrow().range.range;
        if p == Preset::Custom {
            self.set_range(Range { preset: Preset::Custom, ..cur });
        } else {
            self.set_range(preset_range(p, today()));
        }
    }

    fn push_range_bar(&self) {
        let r = self.st.borrow().range.range;
        let ui = self.win();
        let g = ui.global::<RangeState>();
        g.set_from_text(fmt_date(r.from).into());
        g.set_to_text(fmt_date(r.to).into());
        g.set_preset_index(PRESETS.iter().position(|p| *p == r.preset).unwrap_or(7) as i32);
        g.set_hint(format!("{} – {}", fmt_date(r.from), fmt_date(r.to)).into());
    }

    /// Načte souhrny pro zvolené období a naplní aktuální záložku (Statistika nebo Přehledy).
    pub(super) fn refresh_range(&self) {
        let tab = self.win().global::<Nav>().get_tab();
        if tab == 0 {
            return;
        }
        let r = self.st.borrow().range.range;
        let days = days_of(r);
        let bounds: Vec<(i64, i64)> =
            days.iter().map(|d| (day_start_ms(*d), day_start_ms(*d + Duration::days(1)))).collect();
        let summaries = data::get_summaries(&self.conn, &bounds).unwrap_or_default();
        {
            let mut st = self.st.borrow_mut();
            st.range.days = days;
            st.range.summaries = summaries;
        }
        if tab == 1 {
            self.push_stats();
        } else {
            self.push_reports();
        }
    }

    fn push_stats(&self) {
        let st = self.st.borrow();
        let (days, sums) = (&st.range.days, &st.range.summaries);

        struct Row {
            date: NaiveDate,
            length: i64,
            start: Option<i64>,
            end: Option<i64>,
        }
        let rows: Vec<Row> = days
            .iter()
            .zip(sums)
            .map(|(d, s)| match (s.start_ms, s.end_ms) {
                // „Délka“ je rozpětí pracovního dne (od první po poslední aktivitu), jako v záhlaví dne.
                (Some(a), Some(b)) => Row { date: *d, length: b - a, start: Some(a), end: Some(b) },
                _ => Row { date: *d, length: 0, start: None, end: None },
            })
            .collect();
        let worked: Vec<&Row> = rows.iter().filter(|r| r.start.is_some()).collect();
        let n = worked.len();
        let sum: i64 = worked.iter().map(|r| r.length).sum();

        let mut out: Vec<StatRowD> = rows
            .iter()
            .map(|r| StatRowD {
                kind: 0,
                from: fmt_date(r.date).into(),
                to: fmt_date(r.date).into(),
                length: format!("{} {}", fmt_hours(r.length), i18n::HOURS_SHORT).into(),
                start: r.start.map(fmt_hm).unwrap_or_default().into(),
                end: r.end.map(fmt_hm).unwrap_or_default().into(),
            })
            .collect();
        out.push(StatRowD {
            kind: 1,
            from: "".into(),
            to: i18n::ROW_TOTAL.into(),
            length: format!("{} {}", fmt_hours(sum), i18n::HOURS_SHORT).into(),
            start: "".into(),
            end: "".into(),
        });
        if n > 0 {
            let agg = |f: &dyn Fn(&Row) -> f64, pick: u8| -> f64 {
                let v: Vec<f64> = worked.iter().map(|r| f(r)).collect();
                match pick {
                    0 => v.iter().sum::<f64>() / n as f64,
                    1 => v.iter().cloned().fold(f64::INFINITY, f64::min),
                    _ => v.iter().cloned().fold(f64::NEG_INFINITY, f64::max),
                }
            };
            for (label, pick) in [(i18n::ROW_AVG, 0u8), (i18n::ROW_MIN, 1), (i18n::ROW_MAX, 2)] {
                let len = agg(&|r| r.length as f64, pick);
                let st_ = agg(&|r| minutes_of_day(r.start.unwrap()), pick);
                let en = agg(&|r| minutes_of_day(r.end.unwrap()), pick);
                out.push(StatRowD {
                    kind: 2,
                    from: "".into(),
                    to: label.into(),
                    length: format!("{} {}", fmt_hours(len.round() as i64), i18n::HOURS_SHORT).into(),
                    start: fmt_minutes_of_day(st_).into(),
                    end: fmt_minutes_of_day(en).into(),
                });
            }
        }
        self.win().global::<StatsState>().set_rows(ModelRc::new(VecModel::from(out)));
    }

    fn push_reports(&self) {
        // Ikony nejpoužívanějších aplikací a webů.
        let (apps, sites) = {
            let st = self.st.borrow();
            let mut a: HashMap<i64, i64> = HashMap::new();
            let mut s: HashMap<String, i64> = HashMap::new();
            for d in &st.range.summaries {
                for (id, ms) in &d.apps {
                    *a.entry(*id).or_default() += ms;
                }
                for (dom, ms) in &d.domains {
                    *s.entry(dom.clone()).or_default() += ms;
                }
            }
            let mut a: Vec<(i64, i64)> = a.into_iter().collect();
            a.sort_by(|x, y| y.1.cmp(&x.1).then(x.0.cmp(&y.0)));
            a.truncate(12);
            let mut s: Vec<(String, i64)> = s.into_iter().collect();
            s.sort_by(|x, y| y.1.cmp(&x.1).then(x.0.cmp(&y.0)));
            s.truncate(12);
            (a, s)
        };
        let names = data::app_names(&self.conn).unwrap_or_default();
        {
            let mut st = self.st.borrow_mut();
            for (id, _) in &apps {
                if !st.icons.apps.contains_key(id) {
                    if let Some(img) = data::app_icon(&self.conn, *id).and_then(|b| super::decode_image(&b)) {
                        st.icons.apps.insert(*id, img);
                    }
                }
            }
            for (dom, _) in &sites {
                if !st.icons.sites.contains_key(dom) {
                    if let Some(img) = data::favicon(&self.conn, dom).and_then(|b| super::decode_image(&b)) {
                        st.icons.sites.insert(dom.clone(), img);
                    }
                }
            }
        }

        let st = self.st.borrow();
        let (days, sums) = (&st.range.days, &st.range.summaries);
        let ui = self.win();
        let g = ui.global::<ReportsState>();

        let max_day = sums.iter().map(|s| s.active_ms).max().unwrap_or(0).max(1);
        let daily: Vec<DayBarD> = days
            .iter()
            .zip(sums)
            .map(|(d, s)| DayBarD {
                value: if s.active_ms > 0 { fmt_hours(s.active_ms) } else { String::new() }.into(),
                label: format!("{}.{}.", d.day(), d.month()).into(),
                ratio: (s.active_ms as f64 / max_day as f64) as f32,
            })
            .collect();
        g.set_daily(ModelRc::new(VecModel::from(daily)));
        g.set_daily_labels(days.len() <= 31);
        g.set_empty(sums.iter().all(|s| s.active_ms == 0));

        let max_app = apps.iter().map(|a| a.1).max().unwrap_or(0).max(1);
        let app_rows: Vec<BarRowD> = apps
            .iter()
            .map(|(id, ms)| {
                let icon = st.icons.apps.get(id);
                BarRowD {
                    icon: icon.cloned().unwrap_or_default(),
                    has_icon: icon.is_some(),
                    name: names.get(id).cloned().unwrap_or_else(|| i18n::SHOT_UNKNOWN_APP.into()).into(),
                    time: fmt_duration(*ms).into(),
                    ratio: (*ms as f64 / max_app as f64) as f32,
                    color: slint_color(color_for(&format!("app:{id}"))),
                }
            })
            .collect();
        g.set_apps(ModelRc::new(VecModel::from(app_rows)));

        let max_site = sites.iter().map(|a| a.1).max().unwrap_or(0).max(1);
        let site_rows: Vec<BarRowD> = sites
            .iter()
            .map(|(dom, ms)| {
                let icon = st.icons.sites.get(dom);
                BarRowD {
                    icon: icon.cloned().unwrap_or_default(),
                    has_icon: icon.is_some(),
                    name: dom.clone().into(),
                    time: fmt_duration(*ms).into(),
                    ratio: (*ms as f64 / max_site as f64) as f32,
                    color: slint_color(color_for(&format!("site:{dom}"))),
                }
            })
            .collect();
        g.set_sites(ModelRc::new(VecModel::from(site_rows)));

        // Den v týdnu (po = 0) × hodina: průměr aktivních minut na jeden takový den.
        let mut sum = [[0i64; 24]; 7];
        let mut count = [0i64; 7];
        for (d, s) in days.iter().zip(sums) {
            let wd = weekday_index(*d);
            count[wd] += 1;
            for (acc, ms) in sum[wd].iter_mut().zip(s.hourly) {
                *acc += ms;
            }
        }
        let mut avg = [[0f64; 24]; 7];
        let mut max_heat = 1f64;
        for wd in 0..7 {
            for h in 0..24 {
                if count[wd] > 0 {
                    avg[wd][h] = sum[wd][h] as f64 / count[wd] as f64;
                    max_heat = max_heat.max(avg[wd][h]);
                }
            }
        }
        let scale = max_heat.min(HOUR as f64);
        let heat: Vec<HeatD> = (0..7 * 24)
            .map(|i| {
                let (wd, h) = (i / 24, i % 24);
                let ms = avg[wd][h];
                HeatD {
                    intensity: if ms > 0.0 { (ms / scale).min(1.0) as f32 } else { 0.0 },
                    tip: format!("{} {}:00 – {} {}", i18n::WEEKDAYS[wd], h, (ms / 60_000.0).round() as i64, i18n::MINUTES_SHORT)
                        .into(),
                }
            })
            .collect();
        g.set_heat(ModelRc::new(VecModel::from(heat)));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn d(y: i32, m: u32, day: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(y, m, day).unwrap()
    }

    #[test]
    fn week_presets_start_on_monday() {
        let wed = d(2026, 3, 4);
        let r = preset_range(Preset::ThisWeek, wed);
        assert_eq!((r.from, r.to), (d(2026, 3, 2), d(2026, 3, 8)));
        let r = preset_range(Preset::LastWeek, wed);
        assert_eq!((r.from, r.to), (d(2026, 2, 23), d(2026, 3, 1)));
    }

    #[test]
    fn month_presets_handle_year_boundaries() {
        let jan = d(2026, 1, 15);
        let r = preset_range(Preset::LastMonth, jan);
        assert_eq!((r.from, r.to), (d(2025, 12, 1), d(2025, 12, 31)));
        let r = preset_range(Preset::ThisMonth, d(2026, 2, 10));
        assert_eq!((r.from, r.to), (d(2026, 2, 1), d(2026, 2, 28)));
    }

    #[test]
    fn rolling_presets() {
        let t = d(2026, 3, 4);
        assert_eq!(preset_range(Preset::Last7, t).from, d(2026, 2, 26));
        assert_eq!(preset_range(Preset::Last30, t).from, d(2026, 2, 3));
    }

    #[test]
    fn shift_moves_by_length_or_whole_month() {
        let r = preset_range(Preset::ThisWeek, d(2026, 3, 4));
        let n = shift(r, 1);
        assert_eq!((n.from, n.to), (d(2026, 3, 9), d(2026, 3, 15)));
        assert_eq!(n.preset, Preset::Custom);
        let m = shift(preset_range(Preset::ThisMonth, d(2026, 1, 20)), -1);
        assert_eq!((m.from, m.to), (d(2025, 12, 1), d(2025, 12, 31)));
        let m = shift(Range { preset: Preset::ThisMonth, ..preset_range(Preset::ThisMonth, d(2026, 12, 5)) }, 1);
        assert_eq!((m.from, m.to), (d(2027, 1, 1), d(2027, 1, 31)));
    }

    #[test]
    fn days_are_capped() {
        let r = Range { from: d(2000, 1, 1), to: d(2026, 1, 1), preset: Preset::Custom };
        assert_eq!(days_of(r).len(), 400);
    }
}
