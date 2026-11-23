//! Náhled snímku u kurzoru nad řádkem seznamu (F11 zamkne) a prohlížeč snímků celého dne (F12).

use super::fmt::*;
use super::{Ctl, MainWindow, Preview, SettingsState, AboutState, Confirm, TagDlg};
use crate::data::{self, Segment, Shot};
use slint::{ComponentHandle, Image};
use std::rc::Rc;
use std::time::Duration;

#[derive(Default)]
pub struct PreviewData {
    /// Řádek pod kurzorem a poloha kurzoru v okně.
    pub hover: Option<(i64, f32, f32)>,
    /// Zamčený náhled (F11).
    pub locked: Option<i64>,
    /// Záznam, jehož náhled je právě zobrazený.
    pub shown: Option<i64>,
    pub viewer_open: bool,
    pub shots: Vec<Shot>,
    pub index: usize,
}

impl Ctl {
    pub(super) fn bind_preview(self: &Rc<Self>, ui: &MainWindow) {
        use super::on;
        let ctl = self.clone();
        let d = ui.global::<super::DayState>();
        on!(ctl, d, on_row_hover, |c, id, x, y| c.row_hover(id as i64, x as f32, y as f32));
        on!(ctl, d, on_row_leave, |c| c.row_leave());
        let p = ui.global::<Preview>();
        on!(ctl, p, on_toggle_lock, |c| c.toggle_lock());
        on!(ctl, p, on_toggle_viewer, |c| c.toggle_viewer());
        on!(ctl, p, on_viewer_step, |c, delta| c.viewer_step(delta as i64));
        on!(ctl, p, on_escape, |c| c.escape());
    }

    /// Načte snímek z disku (relativní cesta je v databázi).
    fn load_shot(&self, id: i64) -> Option<Image> {
        let rel = data::screenshot_path(&self.conn, id)?;
        let bytes = std::fs::read(self.env.data_dir.join(rel)).ok()?;
        super::decode_image(&bytes)
    }

    fn segment(&self, id: i64) -> Option<Segment> {
        self.st.borrow().day.data.segments.iter().find(|s| s.id == id).cloned()
    }

    // --- náhled u kurzoru ---------------------------------------------------------------------

    fn row_hover(&self, id: i64, x: f32, y: f32) {
        let same_shown = {
            let mut st = self.st.borrow_mut();
            let p = &mut st.preview;
            p.hover = Some((id, x, y));
            if p.locked.is_some() {
                return;
            }
            p.shown == Some(id)
        };
        if same_shown {
            let ui = self.win();
            let g = ui.global::<Preview>();
            g.set_x(x);
            g.set_y(y);
            return;
        }
        // Náhled se ukáže až po chvíli, aby nebliká při přejíždění seznamem.
        let weak = self.me.clone();
        self.hover_timer.start(slint::TimerMode::SingleShot, Duration::from_millis(200), move || {
            if let Some(c) = weak.upgrade() {
                c.show_hover();
            }
        });
    }

    fn row_leave(&self) {
        self.hover_timer.stop();
        let locked = {
            let mut st = self.st.borrow_mut();
            st.preview.hover = None;
            st.preview.locked.is_some()
        };
        if !locked {
            self.hide_preview();
        }
    }

    fn show_hover(&self) {
        let hover = self.st.borrow().preview.hover;
        if let Some((id, x, y)) = hover {
            self.show_preview(id, x, y);
        }
    }

    fn show_preview(&self, id: i64, x: f32, y: f32) {
        let Some(seg) = self.segment(id) else { return };
        let app = self.st.borrow().day.data.apps.get(&seg.app_id.unwrap_or(-1)).cloned().unwrap_or_default();
        let icon = self.icon_for(&seg);
        let ui = self.win();
        let g = ui.global::<Preview>();
        g.set_app(app.into());
        g.set_domain(seg.domain.clone().unwrap_or_default().into());
        g.set_title(seg.title.clone().into());
        g.set_has_icon(icon.is_some());
        g.set_icon(icon.unwrap_or_default());
        g.set_x(x);
        g.set_y(y);
        // Jen snímek, který k tomuto záznamu opravdu patří; žádné náhradní snímky.
        let shots = data::segment_shots(&self.conn, seg.id).unwrap_or_default();
        let img = if shots.is_empty() { None } else { self.load_shot(shots[shots.len() / 2].id) };
        match img {
            Some(i) => {
                g.set_img(i);
                g.set_state(1);
            }
            None => g.set_state(2),
        }
        g.set_visible(true);
        self.st.borrow_mut().preview.shown = Some(id);
    }

    fn hide_preview(&self) {
        self.st.borrow_mut().preview.shown = None;
        let ui = self.win();
        let g = ui.global::<Preview>();
        g.set_visible(false);
        g.set_img(Image::default());
    }

    fn toggle_lock(&self) {
        let (now_locked, hover) = {
            let mut st = self.st.borrow_mut();
            let p = &mut st.preview;
            if p.locked.is_some() {
                p.locked = None;
            } else {
                p.locked = p.hover.map(|h| h.0);
            }
            (p.locked.is_some(), p.hover)
        };
        self.win().global::<Preview>().set_locked(now_locked);
        match (now_locked, hover) {
            (true, Some((id, x, y))) => self.show_preview(id, x, y),
            (false, None) => self.hide_preview(),
            _ => {}
        }
    }

    /// Po změně dne se zahodí náhled i prohlížeč.
    pub(super) fn preview_reset(&self) {
        self.hover_timer.stop();
        {
            let mut st = self.st.borrow_mut();
            st.preview = PreviewData::default();
        }
        let ui = self.win();
        let g = ui.global::<Preview>();
        g.set_locked(false);
        g.set_viewer_open(false);
        g.set_viewer_img(Image::default());
        self.hide_preview();
    }

    // --- prohlížeč snímků ----------------------------------------------------------------------

    fn toggle_viewer(&self) {
        if self.st.borrow().preview.viewer_open {
            self.close_viewer();
            return;
        }
        let (from, to, focus_id) = {
            let st = self.st.borrow();
            let focus = st.preview.locked.or(st.preview.hover.map(|h| h.0)).or(st.day.selected);
            (st.day.start, st.day.end, focus)
        };
        let shots = data::range_shots(&self.conn, from, to).unwrap_or_default();
        let focus = focus_id.and_then(|id| self.segment(id));
        let index = focus.map_or(0, |f| {
            shots
                .iter()
                .position(|s| s.segment_id == f.id)
                // nejbližší snímek k začátku záznamu
                .or_else(|| {
                    shots.iter().enumerate().min_by_key(|(_, s)| (s.ts - f.start_ms).abs()).map(|(i, _)| i)
                })
                .unwrap_or(0)
        });
        {
            let mut st = self.st.borrow_mut();
            st.preview.shots = shots;
            st.preview.index = index;
            st.preview.viewer_open = true;
        }
        self.hover_timer.stop();
        self.win().global::<Preview>().set_viewer_open(true);
        self.show_viewer_shot();
    }

    fn close_viewer(&self) {
        {
            let mut st = self.st.borrow_mut();
            st.preview.viewer_open = false;
            st.preview.shots.clear();
        }
        let ui = self.win();
        let g = ui.global::<Preview>();
        g.set_viewer_open(false);
        g.set_viewer_img(Image::default());
        g.set_viewer_has_img(false);
    }

    fn viewer_step(&self, delta: i64) {
        {
            let mut st = self.st.borrow_mut();
            let p = &mut st.preview;
            if !p.viewer_open || p.shots.is_empty() {
                return;
            }
            p.index = (p.index as i64 + delta).clamp(0, p.shots.len() as i64 - 1) as usize;
        }
        self.show_viewer_shot();
    }

    fn show_viewer_shot(&self) {
        let (shot, n, index) = {
            let st = self.st.borrow();
            (st.preview.shots.get(st.preview.index).cloned(), st.preview.shots.len(), st.preview.index)
        };
        let ui = self.win();
        let g = ui.global::<Preview>();
        g.set_viewer_empty(shot.is_none());
        g.set_viewer_can_prev(index > 0);
        g.set_viewer_can_next(index + 1 < n);
        let Some(shot) = shot else { return };
        let seg = self.segment(shot.segment_id);
        let mut info = fmt_clock(shot.ts);
        if let Some(s) = &seg {
            if let Some(name) = self.st.borrow().day.data.apps.get(&s.app_id.unwrap_or(-1)) {
                info += &format!(" · {name}");
            }
            if let Some(d) = &s.domain {
                info += &format!(" · {d}");
            }
        }
        info += &format!(" · {} / {}", index + 1, n);
        g.set_viewer_info(info.into());
        g.set_viewer_title(seg.map(|s| s.title).unwrap_or_default().into());
        match self.load_shot(shot.id) {
            Some(img) => {
                g.set_viewer_img(img);
                g.set_viewer_has_img(true);
            }
            None => {
                g.set_viewer_img(Image::default());
                g.set_viewer_has_img(false);
            }
        }
    }

    // --- Escape ------------------------------------------------------------------------------

    /// Zavře nejhořejší vrstvu: prohlížeč, dialog, a nakonec odemkne náhled.
    fn escape(&self) {
        let ui = self.win();
        if ui.global::<Preview>().get_viewer_open() {
            return self.close_viewer();
        }
        if ui.global::<TagDlg>().get_open() {
            return self.tag_cancel();
        }
        if ui.global::<Confirm>().get_open() {
            return self.confirm_no();
        }
        if ui.global::<SettingsState>().get_open() {
            return self.close_settings();
        }
        if ui.global::<AboutState>().get_open() {
            return self.close_about();
        }
        let locked = self.st.borrow().preview.locked.is_some();
        if locked {
            self.toggle_lock();
        }
    }
}
