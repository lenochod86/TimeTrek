//! Záložka Den: časová osa, seznam záznamů a žebříček aplikací.

use super::colors::{color_for, parse_hex};
use super::fmt::*;
use super::timeline::{self, LaneItem, View};
use super::{rgba_image, slint_color, ymd, ChipD, Ctl, DayState, MainWindow, RankD, RowD, TagLabelD, TickD, Tip};
use crate::data::{self, RangeData, Segment};
use crate::db::KIND_ACTIVE;
use crate::i18n;
use chrono::NaiveDate;
use slint::{ComponentHandle, Image, ModelRc, SharedString, VecModel};
use std::collections::HashSet;
use std::rc::Rc;

const LANE_H: f64 = 50.0;
const OVERVIEW_H: f64 = 30.0;
const USAGE_ACTIVE: [u8; 3] = [0x4c, 0x9a, 0x1f];
const USAGE_IDLE: [u8; 3] = [0xd4, 0x18, 0x0c];
/// Nejmenší tažení myši, které se bere jako posun (jinak jde o klik).
const DRAG_PX: f64 = 3.0;

pub enum Drag {
    None,
    /// Výběr úseku pro značku.
    Tag { t0: i64, cur: i64 },
    Pan { x0: f64, view: View, moved: bool },
    Overview { len: i64, anchor: i64 },
}

pub struct DayData {
    pub date: NaiveDate,
    pub start: i64,
    pub end: i64,
    pub data: RangeData,
    /// Záznamy oříznuté na den (záznam může přesahovat půlnoc).
    pub clipped: Vec<Segment>,
    pub now_ms: i64,
    pub fit: bool,
    pub view: View,
    pub checked_apps: HashSet<i64>,
    pub checked_rows: HashSet<i64>,
    pub selected: Option<i64>,
    pub sort_key: i32,
    pub sort_dir: i32,
    pub scroll_token: i32,
    pub items: [Vec<LaneItem>; 4],
    pub usage: Vec<(i64, i64)>,
    /// Segmenty v seznamu (po filtru podle aplikací), seřazené podle zvoleného sloupce.
    pub rows: Vec<Segment>,
    pub drag: Drag,
}

impl DayData {
    pub fn new() -> Self {
        let date = today();
        let start = day_start_ms(date);
        let end = day_start_ms(add_days(date, 1));
        Self {
            date,
            start,
            end,
            data: RangeData::default(),
            clipped: Vec::new(),
            now_ms: crate::db::now_ms(),
            fit: true,
            view: View { start, end },
            checked_apps: HashSet::new(),
            checked_rows: HashSet::new(),
            selected: None,
            sort_key: 1,
            sort_dir: -1,
            scroll_token: 0,
            items: Default::default(),
            usage: Vec::new(),
            rows: Vec::new(),
            drag: Drag::None,
        }
    }

    pub fn is_today(&self) -> bool {
        self.date == today()
    }

    fn day_len(&self) -> i64 {
        (self.end - self.start).max(1)
    }

    fn active(&self) -> impl Iterator<Item = &Segment> {
        self.clipped.iter().filter(|s| s.kind == KIND_ACTIVE)
    }

    /// První a poslední aktivita dne.
    pub fn day_info(&self) -> Option<(i64, i64)> {
        let s = self.active().map(|s| s.start_ms).min()?;
        let e = self.active().map(|s| s.end_ms).max()?;
        Some((s, e))
    }

    /// Zobrazený úsek, který ukazuje celý pracovní den (okolo první a poslední aktivity).
    fn fit_view(&self) -> View {
        match self.day_info() {
            Some((s, e)) => {
                let end = e.max(if self.is_today() { self.now_ms } else { 0 });
                View { start: (s - 5 * MIN).max(self.start), end: (end + 5 * MIN).min(self.end) }
            }
            None => View { start: self.start + 8 * HOUR, end: self.start + 18 * HOUR },
        }
    }

    fn app_name(&self, id: Option<i64>) -> String {
        id.and_then(|i| self.data.apps.get(&i)).cloned().unwrap_or_default()
    }

    /// Seznam záznamů: aktivní, po filtru podle aplikací, seřazený.
    fn build_rows(&mut self) {
        let filtered = !self.checked_apps.is_empty();
        let mut rows: Vec<Segment> = self
            .active()
            .filter(|s| !filtered || s.app_id.is_some_and(|a| self.checked_apps.contains(&a)))
            .cloned()
            .collect();
        let (key, dir) = (self.sort_key, self.sort_dir);
        rows.sort_by(|a, b| {
            let o = match key {
                0 => a.title.to_lowercase().cmp(&b.title.to_lowercase()),
                1 => a.start_ms.cmp(&b.start_ms),
                2 => a.end_ms.cmp(&b.end_ms),
                _ => (a.end_ms - a.start_ms).cmp(&(b.end_ms - b.start_ms)),
            };
            if dir < 0 { o.reverse() } else { o }
        });
        self.rows = rows;
    }

    fn build_items(&mut self) {
        let filtered = !self.checked_apps.is_empty();
        let active: Vec<&Segment> = self
            .active()
            .filter(|s| !filtered || s.app_id.is_some_and(|a| self.checked_apps.contains(&a)))
            .collect();

        let usage = self
            .clipped
            .iter()
            .map(|s| {
                let on = s.kind == KIND_ACTIVE;
                LaneItem {
                    id: s.id,
                    start: s.start_ms,
                    end: s.end_ms,
                    color: if on { USAGE_ACTIVE } else { USAGE_IDLE },
                    label: None,
                    tip: format!(
                        "{}  {} - {}",
                        if on { i18n::ACTIVE_LABEL } else { i18n::IDLE_LABEL },
                        fmt_clock(s.start_ms),
                        fmt_clock(s.end_ms)
                    ),
                }
            })
            .collect();
        let apps = active
            .iter()
            .map(|s| LaneItem {
                id: s.id,
                start: s.start_ms,
                end: s.end_ms,
                color: color_for(&format!("app:{}", s.app_id.map_or("None".to_string(), |a| a.to_string()))),
                label: None,
                tip: format!(
                    "{}\n{}\n{} - {} ({})",
                    self.app_name(s.app_id),
                    s.title,
                    fmt_clock(s.start_ms),
                    fmt_clock(s.end_ms),
                    fmt_duration(s.end_ms - s.start_ms)
                ),
            })
            .collect();
        let docs = active
            .iter()
            .map(|s| LaneItem {
                id: s.id,
                start: s.start_ms,
                end: s.end_ms,
                color: color_for(&format!(
                    "doc:{}:{}:{}",
                    s.app_id.map_or("None".to_string(), |a| a.to_string()),
                    s.domain.as_deref().unwrap_or(""),
                    s.title
                )),
                label: None,
                tip: format!(
                    "{}\n{} - {} ({})",
                    s.title,
                    fmt_clock(s.start_ms),
                    fmt_clock(s.end_ms),
                    fmt_duration(s.end_ms - s.start_ms)
                ),
            })
            .collect();
        let tags = self
            .data
            .tag_ranges
            .iter()
            .map(|r| {
                let tag = self.data.tags.iter().find(|t| t.id == r.tag_id);
                let name = tag.map(|t| t.name.clone()).unwrap_or_default();
                LaneItem {
                    id: r.id,
                    start: r.start_ms,
                    end: r.end_ms,
                    color: tag.map_or([0x88; 3], |t| parse_hex(&t.color)),
                    label: Some(name.clone()),
                    tip: format!("{}\n{} - {}", name, fmt_clock(r.start_ms), fmt_clock(r.end_ms)),
                }
            })
            .collect();
        self.items = [tags, usage, apps, docs];
        self.usage = self.clipped.iter().filter(|s| s.kind == KIND_ACTIVE).map(|s| (s.start_ms, s.end_ms)).collect();
    }
}

/// Podpis dat dne: rozliší, jestli se od minulého načtení něco změnilo.
fn data_signature(d: &RangeData) -> (usize, Option<(i64, i64)>, usize, usize, usize) {
    (
        d.segments.len(),
        d.segments.last().map(|s| (s.id, s.end_ms)),
        d.tag_ranges.len(),
        d.tags.len(),
        d.apps.len(),
    )
}

impl Ctl {
    pub(super) fn bind_day(self: &Rc<Self>, ui: &MainWindow) {
        use super::on;
        let ctl = self.clone();
        let d = ui.global::<DayState>();
        on!(ctl, d, on_prev_day, |c| c.set_date(add_days(c.st.borrow().day.date, -1)));
        on!(ctl, d, on_next_day, |c| c.set_date(add_days(c.st.borrow().day.date, 1)));
        on!(ctl, d, on_go_today, |c| c.set_date(today()));
        on!(ctl, d, on_date_edited, |c, t| c.date_edited(&t));
        on!(ctl, d, on_lane_width_changed, |c, w| c.lane_width_changed(w as f64));
        on!(ctl, d, on_lane_pressed, |c, lane, x| c.lane_pressed(lane as usize, x as f64));
        on!(ctl, d, on_lane_moved, |c, lane, x| c.lane_moved(lane as usize, x as f64));
        on!(ctl, d, on_lane_released, |c, lane, x| c.lane_released(lane as usize, x as f64));
        on!(ctl, d, on_lane_hover, |c, lane, x, ax, ay| c.lane_hover(lane as usize, x as f64, ax as f32, ay as f32));
        on!(ctl, d, on_lane_leave, |c| c.lane_leave());
        on!(ctl, d, on_lane_context, |c, lane, x| c.lane_context(lane as usize, x as f64));
        on!(ctl, d, on_lane_scroll, |c, _lane, x, dy| c.lane_scroll(x as f64, dy));
        on!(ctl, d, on_overview_pressed, |c, x| c.overview_pressed(x as f64));
        on!(ctl, d, on_overview_moved, |c, x| c.overview_moved(x as f64));
        on!(ctl, d, on_overview_released, |c| c.st.borrow_mut().day.drag = Drag::None);
        on!(ctl, d, on_fit_clicked, |c| c.fit_clicked());
        on!(ctl, d, on_row_clicked, |c, id| c.row_clicked(id as i64));
        on!(ctl, d, on_row_toggled, |c, id| c.row_toggled(id as i64));
        on!(ctl, d, on_toggle_all, |c| c.toggle_all());
        on!(ctl, d, on_sort_clicked, |c, k| c.sort_clicked(k));
        on!(ctl, d, on_rank_toggled, |c, id| c.rank_toggled(id as i64));
    }

    pub(super) fn init_day(&self) {
        self.set_date(today());
    }

    // --- načtení dat ------------------------------------------------------------------------

    fn set_date(&self, date: NaiveDate) {
        {
            let mut st = self.st.borrow_mut();
            let d = &mut st.day;
            d.date = date;
            d.start = day_start_ms(date);
            d.end = day_start_ms(add_days(date, 1));
            d.data = RangeData::default();
            d.fit = true;
            d.checked_apps.clear();
            d.checked_rows.clear();
            d.selected = None;
            d.drag = Drag::None;
        }
        self.preview_reset();
        self.reload_day();
    }

    fn date_edited(&self, text: &str) {
        match parse_date(text) {
            Some(d) => self.set_date(d),
            None => {
                let d = self.st.borrow().day.date;
                self.win().global::<DayState>().set_date_text(fmt_date(d).into());
            }
        }
    }

    /// Znovu načte den z databáze a překreslí všechno.
    pub(super) fn reload_day(&self) {
        self.load_day(true);
    }

    /// Pravidelné obnovení: jen dnešek se mění; seznam se přestaví jen když se data opravdu změnila.
    pub(super) fn refresh_day(&self) {
        if self.st.borrow().day.is_today() {
            self.load_day(false);
        }
    }

    fn load_day(&self, force: bool) {
        let (from, to) = {
            let st = self.st.borrow();
            (st.day.start, st.day.end)
        };
        let data = data::get_range(&self.conn, from, to).unwrap_or_else(|e| {
            log::error!("načtení dne: {e}");
            RangeData::default()
        });
        let icons_changed = {
            let mut st = self.st.borrow_mut();
            let d = &mut st.day;
            d.now_ms = crate::db::now_ms();
            let changed = force || data_signature(&data) != data_signature(&d.data);
            if changed {
                d.clipped = data
                    .segments
                    .iter()
                    .map(|s| Segment { start_ms: s.start_ms.max(from), end_ms: s.end_ms.min(to), ..s.clone() })
                    .collect();
                d.data = data;
            }
            changed
        };
        let new_icons = self.ensure_icons();
        if icons_changed || new_icons {
            self.rebuild_day();
        } else {
            // Beze změny dat se posune jen značka „teď“ a případně zobrazený úsek.
            {
                let mut st = self.st.borrow_mut();
                if st.day.fit {
                    st.day.view = st.day.fit_view();
                }
            }
            self.push_lanes();
        }
    }

    /// Dotáhne ikony aplikací a faviconů, které ještě nejsou v paměti.
    fn ensure_icons(&self) -> bool {
        let mut added = false;
        let mut st = self.st.borrow_mut();
        let st = &mut *st;
        for id in st.day.data.apps.keys() {
            if !st.icons.apps.contains_key(id) {
                if let Some(img) = data::app_icon(&self.conn, *id).and_then(|b| super::decode_image(&b)) {
                    st.icons.apps.insert(*id, img);
                    added = true;
                }
            }
        }
        let domains: HashSet<&String> = st.day.data.segments.iter().filter_map(|s| s.domain.as_ref()).collect();
        for d in domains {
            if !st.icons.sites.contains_key(d) {
                if let Some(img) = data::favicon(&self.conn, d).and_then(|b| super::decode_image(&b)) {
                    st.icons.sites.insert(d.clone(), img);
                    added = true;
                }
            }
        }
        added
    }

    pub(super) fn icon_for(&self, seg: &Segment) -> Option<Image> {
        let st = self.st.borrow();
        seg.domain
            .as_ref()
            .and_then(|d| st.icons.sites.get(d))
            .or_else(|| seg.app_id.and_then(|a| st.icons.apps.get(&a)))
            .cloned()
    }

    // --- překreslení ------------------------------------------------------------------------

    fn rebuild_day(&self) {
        {
            let mut st = self.st.borrow_mut();
            let d = &mut st.day;
            d.build_items();
            d.build_rows();
            if d.fit {
                d.view = d.fit_view();
            }
        }
        self.push_header();
        self.push_lanes();
        self.push_lists();
    }

    fn push_header(&self) {
        let ui = self.win();
        let st = self.st.borrow();
        let d = &st.day;
        let g = ui.global::<DayState>();
        // Pole s datem se nepřepisuje, když se do něj právě píše.
        let text = fmt_date(d.date);
        if g.get_date_text() != text.as_str() {
            g.set_date_text(text.into());
        }
        g.set_date_ymd(ymd(d.date));
        g.set_is_today(d.is_today());
        match d.day_info() {
            Some((s, e)) => {
                g.set_day_start(fmt_hm(s).into());
                g.set_day_end(fmt_hm(e).into());
                g.set_day_length(fmt_duration_hm(e - s).into());
            }
            None => {
                g.set_day_start("–".into());
                g.set_day_end("–".into());
                g.set_day_length("–".into());
            }
        }
        g.set_fit(d.fit);
    }

    /// Bitmapy pruhů, osa a okno přehledu (po změně zobrazeného úseku, výběru, tažení nebo šířky).
    pub(super) fn push_lanes(&self) {
        let ui = self.win();
        let scale = ui.window().scale_factor();
        let lane_w = self.lane_w.get();
        let st = self.st.borrow();
        let d = &st.day;
        let g = ui.global::<DayState>();

        let img_w = ((lane_w - 2.0).max(1.0) * scale as f64).round() as u32;
        let img_h = (LANE_H * scale as f64).round() as u32;
        let lane = |i: usize, sel: Option<i64>, drag: Option<(i64, i64)>| {
            let b = timeline::render_lane(&d.items[i], img_w, img_h, d.view, sel, drag, scale);
            rgba_image(b.w, b.h, &b.rgba)
        };
        let drag = match d.drag {
            Drag::Tag { t0, cur } => Some((t0, cur)),
            _ => None,
        };
        g.set_lane_tags(lane(0, None, drag));
        g.set_lane_usage(lane(1, None, None));
        g.set_lane_apps(lane(2, d.selected, None));
        g.set_lane_docs(lane(3, d.selected, None));

        let ov_w = (lane_w * scale as f64).round() as u32;
        let ov = timeline::render_overview(&d.usage, ov_w, (OVERVIEW_H * scale as f64).round() as u32, d.start, d.day_len(), scale);
        g.set_overview(rgba_image(ov.w, ov.h, &ov.rgba));

        let span = d.view.span() as f64;
        // Popisky těsně u okraje by se ořízly.
        let ticks: Vec<TickD> = timeline::ticks(d.view, d.start, lane_w, hour_min)
            .into_iter()
            .filter(|t| t.frac * lane_w >= 16.0 && t.frac * lane_w <= lane_w - 16.0)
            .map(|t| TickD { frac: t.frac as f32, label: t.label.into(), major: t.major })
            .collect();
        g.set_ticks(ModelRc::new(VecModel::from(ticks)));

        let labels: Vec<TagLabelD> = d.items[0]
            .iter()
            .filter(|it| it.end >= d.view.start && it.start <= d.view.end)
            .filter_map(|it| {
                let vs = it.start.max(d.view.start);
                let ve = it.end.min(d.view.end);
                let width = (ve - vs) as f64 / span;
                (width * lane_w > 40.0).then(|| TagLabelD {
                    left: ((vs - d.view.start) as f64 / span) as f32,
                    width: width as f32,
                    text: it.label.clone().unwrap_or_default().into(),
                })
            })
            .collect();
        g.set_tag_labels(ModelRc::new(VecModel::from(labels)));

        let now = d.now_ms;
        g.set_now_frac(if d.is_today() && now >= d.view.start && now <= d.view.end {
            ((now - d.view.start) as f64 / span) as f32
        } else {
            -1.0
        });
        g.set_view_left(((d.view.start - d.start) as f64 / d.day_len() as f64) as f32);
        g.set_view_width((span / d.day_len() as f64) as f32);
        g.set_fit(d.fit);
    }

    /// Seznam záznamů a žebříček aplikací.
    pub(super) fn push_lists(&self) {
        let ui = self.win();
        let st = self.st.borrow();
        let d = &st.day;
        let g = ui.global::<DayState>();

        let rows: Vec<RowD> = d
            .rows
            .iter()
            .map(|s| {
                let icon = s
                    .domain
                    .as_ref()
                    .and_then(|dom| st.icons.sites.get(dom))
                    .or_else(|| s.app_id.and_then(|a| st.icons.apps.get(&a)));
                RowD {
                    id: s.id as i32,
                    icon: icon.cloned().unwrap_or_default(),
                    has_icon: icon.is_some(),
                    title: if s.title.is_empty() { d.app_name(s.app_id) } else { s.title.clone() }.into(),
                    start: fmt_clock(s.start_ms).into(),
                    end: fmt_clock(s.end_ms).into(),
                    duration: fmt_duration(s.end_ms - s.start_ms).into(),
                    checked: d.checked_rows.contains(&s.id),
                    selected: d.selected == Some(s.id),
                }
            })
            .collect();
        let sum: i64 = d.rows.iter().filter(|s| d.checked_rows.contains(&s.id)).map(|s| s.end_ms - s.start_ms).sum();
        let checked = d.rows.iter().filter(|s| d.checked_rows.contains(&s.id)).count();
        g.set_all_checked(!d.rows.is_empty() && checked == d.rows.len());
        g.set_checked_count(d.checked_rows.len() as i32);
        g.set_checked_sum(fmt_duration(sum).into());
        g.set_sort_key(d.sort_key);
        g.set_sort_dir(d.sort_dir);
        g.set_rows(ModelRc::new(VecModel::from(rows)));

        // Žebříček aplikací (ze všech aktivních záznamů dne, bez filtru).
        let mut sum_by_app: std::collections::HashMap<i64, i64> = Default::default();
        for s in d.active() {
            if let Some(a) = s.app_id {
                *sum_by_app.entry(a).or_default() += s.end_ms - s.start_ms;
            }
        }
        let mut ranking: Vec<(i64, i64)> = sum_by_app.into_iter().collect();
        ranking.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));
        let all_total: i64 = ranking.iter().map(|r| r.1).sum();
        let filtered = !d.checked_apps.is_empty();
        let shown_total: i64 =
            if filtered { ranking.iter().filter(|r| d.checked_apps.contains(&r.0)).map(|r| r.1).sum() } else { all_total };
        let rank: Vec<RankD> = ranking
            .iter()
            .map(|&(id, ms)| {
                let pct = if shown_total > 0 { ms as f64 / shown_total as f64 * 100.0 } else { 0.0 };
                let icon = st.icons.apps.get(&id);
                RankD {
                    id: id as i32,
                    icon: icon.cloned().unwrap_or_default(),
                    has_icon: icon.is_some(),
                    name: d.data.apps.get(&id).cloned().unwrap_or_else(|| i18n::SHOT_UNKNOWN_APP.into()).into(),
                    time: fmt_duration(ms).into(),
                    percent: format!("{pct:.1}%").into(),
                    ratio: (pct / 100.0).clamp(0.0, 1.0) as f32,
                    color: slint_color(color_for(&format!("app:{id}"))),
                    checked: d.checked_apps.contains(&id),
                }
            })
            .collect();
        g.set_rank(ModelRc::new(VecModel::from(rank)));
        g.set_rank_total(fmt_duration(shown_total).into());
    }

    /// Po změně filtru nebo řazení.
    fn rebuild_lists_and_lanes(&self) {
        {
            let mut st = self.st.borrow_mut();
            st.day.build_items();
            st.day.build_rows();
        }
        self.push_lanes();
        self.push_lists();
    }

    // --- ovládání seznamů ------------------------------------------------------------------

    fn row_clicked(&self, id: i64) {
        self.st.borrow_mut().day.selected = Some(id);
        self.push_lanes();
        self.push_lists();
    }

    fn row_toggled(&self, id: i64) {
        {
            let mut st = self.st.borrow_mut();
            let set = &mut st.day.checked_rows;
            if !set.remove(&id) {
                set.insert(id);
            }
        }
        self.push_lists();
    }

    fn toggle_all(&self) {
        {
            let mut st = self.st.borrow_mut();
            let d = &mut st.day;
            let all = !d.rows.is_empty() && d.rows.iter().all(|s| d.checked_rows.contains(&s.id));
            if all {
                d.checked_rows.clear();
            } else {
                d.checked_rows = d.rows.iter().map(|s| s.id).collect();
            }
        }
        self.push_lists();
    }

    fn sort_clicked(&self, key: i32) {
        {
            let mut st = self.st.borrow_mut();
            let d = &mut st.day;
            if d.sort_key == key {
                d.sort_dir = -d.sort_dir;
            } else {
                d.sort_key = key;
                d.sort_dir = if key == 0 { 1 } else { -1 };
            }
            d.build_rows();
        }
        self.push_lists();
    }

    fn rank_toggled(&self, id: i64) {
        {
            let mut st = self.st.borrow_mut();
            let set = &mut st.day.checked_apps;
            if !set.remove(&id) {
                set.insert(id);
            }
        }
        self.rebuild_lists_and_lanes();
    }

    // --- časová osa ------------------------------------------------------------------------

    fn lane_width_changed(&self, w: f64) {
        if w > 1.0 && (w - self.lane_w.get()).abs() > 0.5 {
            self.lane_w.set(w);
            self.push_lanes();
        }
    }

    /// Čas pod vodorovnou polohou v pruhu (obrázek pruhu je uvnitř 1px ohraničení).
    fn time_at(&self, x: f64) -> i64 {
        let st = self.st.borrow();
        let v = st.day.view;
        let w = (self.lane_w.get() - 2.0).max(1.0);
        v.start + (((x - 1.0) / w).clamp(0.0, 1.0) * v.span() as f64) as i64
    }

    fn set_view(&self, start: i64, end: i64) {
        {
            let mut st = self.st.borrow_mut();
            let d = &mut st.day;
            d.view = timeline::clamp_view(d.start, d.end, start, end);
            d.fit = false;
        }
        self.push_lanes();
    }

    fn lane_pressed(&self, lane: usize, x: f64) {
        let t = self.time_at(x);
        {
            let mut st = self.st.borrow_mut();
            let view = st.day.view;
            st.day.drag = if lane == 0 { Drag::Tag { t0: t, cur: t } } else { Drag::Pan { x0: x, view, moved: false } };
        }
        self.win().global::<Tip>().set_text(SharedString::new());
        if lane == 0 {
            self.push_lanes();
        }
    }

    fn lane_moved(&self, lane: usize, x: f64) {
        let t = self.time_at(x);
        let lane_w = (self.lane_w.get() - 2.0).max(1.0);
        let mut pan = None;
        {
            let mut st = self.st.borrow_mut();
            match &mut st.day.drag {
                Drag::Tag { cur, .. } if lane == 0 => *cur = t,
                Drag::Pan { x0, view, moved } => {
                    if (x - *x0).abs() > DRAG_PX {
                        *moved = true;
                    }
                    if *moved {
                        let dt = ((x - *x0) / lane_w * view.span() as f64) as i64;
                        pan = Some((view.start - dt, view.end - dt));
                    }
                }
                _ => return,
            }
        }
        match pan {
            Some((s, e)) => self.set_view(s, e),
            None => self.push_lanes(),
        }
    }

    fn lane_released(&self, lane: usize, x: f64) {
        let t = self.time_at(x);
        let drag = std::mem::replace(&mut self.st.borrow_mut().day.drag, Drag::None);
        match drag {
            Drag::Tag { t0, .. } => {
                self.push_lanes();
                if (t - t0).abs() > 30_000 {
                    self.open_tag_dialog(t0.min(t), t0.max(t));
                }
            }
            Drag::Pan { moved: false, .. } => {
                let pick = {
                    let st = self.st.borrow();
                    let d = &st.day;
                    let span = d.view.span() as f64;
                    timeline::hit(&d.items[lane], t as f64, span, self.lane_w.get()).map(|it| it.id)
                };
                if let Some(id) = pick {
                    if lane >= 2 {
                        self.pick_from_timeline(id);
                    }
                }
            }
            _ => {}
        }
    }

    /// Klik na úsek v pruhu: vybere záznam a seznam na něj odroluje.
    fn pick_from_timeline(&self, id: i64) {
        let row = {
            let mut st = self.st.borrow_mut();
            let d = &mut st.day;
            d.selected = Some(id);
            d.scroll_token += 1;
            d.rows.iter().position(|s| s.id == id)
        };
        self.push_lanes();
        self.push_lists();
        let st = self.st.borrow();
        let ui = self.win();
        let g = ui.global::<DayState>();
        g.set_scroll_row(row.map_or(0, |r| r as i32));
        g.set_scroll_token(st.day.scroll_token);
    }

    fn lane_hover(&self, lane: usize, x: f64, ax: f32, ay: f32) {
        let tip = {
            let st = self.st.borrow();
            let d = &st.day;
            if !matches!(d.drag, Drag::None) {
                String::new()
            } else {
                let t = {
                    let w = (self.lane_w.get() - 2.0).max(1.0);
                    d.view.start as f64 + ((x - 1.0) / w).clamp(0.0, 1.0) * d.view.span() as f64
                };
                timeline::hit(&d.items[lane], t, d.view.span() as f64, self.lane_w.get())
                    .map(|it| it.tip.clone())
                    .unwrap_or_default()
            }
        };
        let ui = self.win();
        let g = ui.global::<Tip>();
        g.set_text(tip.into());
        g.set_x(ax);
        g.set_y(ay);
    }

    pub(super) fn lane_leave(&self) {
        self.win().global::<Tip>().set_text(SharedString::new());
    }

    fn lane_scroll(&self, x: f64, dy: f32) {
        if dy == 0.0 {
            return;
        }
        let (view, ds, de) = {
            let st = self.st.borrow();
            (st.day.view, st.day.start, st.day.end)
        };
        let w = (self.lane_w.get() - 2.0).max(1.0);
        let frac = ((x - 1.0) / w).clamp(0.0, 1.0);
        let z = timeline::zoom(view, ds, de, frac, dy < 0.0);
        self.set_view(z.start, z.end);
    }

    fn overview_time(&self, x: f64) -> i64 {
        let st = self.st.borrow();
        let d = &st.day;
        d.start + ((x / self.lane_w.get().max(1.0)).clamp(0.0, 1.0) * d.day_len() as f64) as i64
    }

    fn overview_pressed(&self, x: f64) {
        let t = self.overview_time(x);
        {
            let mut st = self.st.borrow_mut();
            let v = st.day.view;
            let len = v.span();
            let inside = t >= v.start && t <= v.end;
            st.day.drag = Drag::Overview { len, anchor: if inside { t - v.start - len / 2 } else { 0 } };
        }
        self.overview_moved(x);
    }

    fn overview_moved(&self, x: f64) {
        let t = self.overview_time(x);
        let (len, anchor) = match self.st.borrow().day.drag {
            Drag::Overview { len, anchor } => (len, anchor),
            _ => return,
        };
        let c = t - anchor;
        self.set_view(c - len / 2, c + len / 2);
    }

    fn fit_clicked(&self) {
        {
            let mut st = self.st.borrow_mut();
            st.day.fit = true;
            st.day.view = st.day.fit_view();
        }
        self.push_lanes();
    }

    fn lane_context(&self, lane: usize, x: f64) {
        if lane != 0 {
            return;
        }
        let t = self.time_at(x);
        let found = {
            let st = self.st.borrow();
            let d = &st.day;
            timeline::hit(&d.items[0], t as f64, d.view.span() as f64, self.lane_w.get())
                .map(|it| (it.id, it.label.clone().unwrap_or_default()))
        };
        if let Some((id, name)) = found {
            self.ask_delete_tag(id, &name);
        }
    }

    // --- značky -----------------------------------------------------------------------------

    pub(super) fn tag_names(&self) -> Vec<ChipD> {
        let st = self.st.borrow();
        st.day
            .data
            .tags
            .iter()
            .map(|t| ChipD { name: t.name.clone().into(), color: slint_color(parse_hex(&t.color)) })
            .collect()
    }
}
