//! Dialogy: nová značka, potvrzení smazání, nastavení a o aplikaci.

use super::colors::{parse_hex, TAG_COLORS};
use super::fmt::*;
use super::{slint_color, AboutState, Confirm, Ctl, MainWindow, Nav, SettingsState, TagDlg};
use crate::{autostart, config, data, i18n};
use slint::{ComponentHandle, ModelRc, VecModel};
use std::rc::Rc;

#[derive(Default)]
pub struct DialogData {
    pub tag_range: Option<(i64, i64)>,
    /// Barva nové značky (volba z palety nebo barva existující značky).
    pub tag_color: String,
    pub delete_id: Option<i64>,
}

impl Ctl {
    pub(super) fn bind_dialogs(self: &Rc<Self>, ui: &MainWindow) {
        use super::on;
        let ctl = self.clone();
        let nav = ui.global::<Nav>();
        on!(ctl, nav, on_show_settings, |c| c.open_settings());
        on!(ctl, nav, on_show_about, |c| c.open_about());

        let s = ui.global::<SettingsState>();
        on!(ctl, s, on_close, |c| c.close_settings());
        on!(ctl, s, on_autostart_toggled, |c, v| c.autostart_toggled(v));
        on!(ctl, s, on_idle_edited, |c, v| c.idle_edited(v));
        let a = ui.global::<AboutState>();
        on!(ctl, a, on_close, |c| c.close_about());

        let t = ui.global::<TagDlg>();
        on!(ctl, t, on_save, |c| c.tag_save());
        on!(ctl, t, on_cancel, |c| c.tag_cancel());
        on!(ctl, t, on_chip_picked, |c, i| c.tag_chip_picked(i));
        on!(ctl, t, on_name_edited, |c, name| c.tag_name_edited(&name));
        let k = ui.global::<Confirm>();
        on!(ctl, k, on_yes, |c| c.confirm_yes());
        on!(ctl, k, on_no, |c| c.confirm_no());

        let colors: Vec<slint::Color> = TAG_COLORS.iter().map(|c| slint_color(parse_hex(c))).collect();
        t.set_colors(ModelRc::new(VecModel::from(colors)));
        if let Some(logo) = super::decode_image(include_bytes!("../../assets/icon/timetrek-128.png")) {
            a.set_logo(logo);
        }
    }

    // --- značky --------------------------------------------------------------------------------

    pub(super) fn open_tag_dialog(&self, start: i64, end: i64) {
        self.st.borrow_mut().dialogs.tag_range = Some((start, end));
        self.st.borrow_mut().dialogs.tag_color = TAG_COLORS[0].to_string();
        let ui = self.win();
        let g = ui.global::<TagDlg>();
        g.set_range_text(format!("{} - {} ({})", fmt_clock(start), fmt_clock(end), fmt_duration(end - start)).into());
        g.set_name("".into());
        g.set_color_index(0);
        g.set_chips(ModelRc::new(VecModel::from(self.tag_names())));
        g.set_open(true);
    }

    fn tag_name_edited(&self, name: &str) {
        let known = self.st.borrow().day.data.tags.iter().find(|t| t.name == name).map(|t| t.color.clone());
        if let Some(color) = known {
            self.set_tag_color(&color);
        }
    }

    fn tag_chip_picked(&self, index: i32) {
        let tag = self.st.borrow().day.data.tags.get(index as usize).cloned();
        if let Some(t) = tag {
            self.win().global::<TagDlg>().set_name(t.name.clone().into());
            self.set_tag_color(&t.color);
        }
    }

    fn set_tag_color(&self, color: &str) {
        self.st.borrow_mut().dialogs.tag_color = color.to_string();
        let idx = TAG_COLORS.iter().position(|c| c.eq_ignore_ascii_case(color)).map_or(-1, |i| i as i32);
        self.win().global::<TagDlg>().set_color_index(idx);
    }

    fn tag_save(&self) {
        let ui = self.win();
        let g = ui.global::<TagDlg>();
        let name = g.get_name().to_string();
        let (range, color) = {
            let st = self.st.borrow();
            let idx = g.get_color_index();
            let color = TAG_COLORS.get(idx.max(0) as usize).filter(|_| idx >= 0).map(|c| c.to_string());
            (st.dialogs.tag_range, color.unwrap_or_else(|| st.dialogs.tag_color.clone()))
        };
        let (Some((start, end)), false) = (range, name.trim().is_empty()) else { return };
        if let Err(e) = data::add_tag_range(&self.conn, &name, &color, start, end) {
            log::error!("uložení značky: {e}");
        }
        self.tag_cancel();
        self.reload_day();
    }

    pub(super) fn tag_cancel(&self) {
        self.st.borrow_mut().dialogs.tag_range = None;
        self.win().global::<TagDlg>().set_open(false);
    }

    pub(super) fn ask_delete_tag(&self, id: i64, name: &str) {
        self.st.borrow_mut().dialogs.delete_id = Some(id);
        let ui = self.win();
        let g = ui.global::<Confirm>();
        g.set_text(i18n::tag_delete_question(name).into());
        g.set_open(true);
    }

    fn confirm_yes(&self) {
        let id = self.st.borrow_mut().dialogs.delete_id.take();
        self.win().global::<Confirm>().set_open(false);
        if let Some(id) = id {
            if let Err(e) = data::delete_tag_range(&self.conn, id) {
                log::error!("smazání značky: {e}");
            }
            self.reload_day();
        }
    }

    pub(super) fn confirm_no(&self) {
        self.st.borrow_mut().dialogs.delete_id = None;
        self.win().global::<Confirm>().set_open(false);
    }

    // --- nastavení a o aplikaci ----------------------------------------------------------------

    fn open_settings(&self) {
        let ui = self.win();
        let g = ui.global::<SettingsState>();
        let minutes = self.env.config.read().map(|c| c.idle_timeout_secs / 60).unwrap_or(5);
        g.set_idle_minutes(minutes as i32);
        let enabled = self.env.autostart && autostart::is_enabled();
        g.set_autostart(enabled);
        g.set_autostart_ready(self.env.autostart);
        g.set_open(true);
    }

    pub(super) fn close_settings(&self) {
        self.win().global::<SettingsState>().set_open(false);
    }

    fn autostart_toggled(&self, on: bool) {
        let ui = self.win();
        let g = ui.global::<SettingsState>();
        match autostart::set_enabled(on) {
            Ok(()) => g.set_autostart(on),
            Err(e) => {
                log::error!("autostart: {e}");
                g.set_autostart(!on);
            }
        }
    }

    /// Interval nečinnosti se uplatní hned (tracker čte nastavení každou sekundu).
    fn idle_edited(&self, minutes: i32) {
        let secs = (minutes.max(1) as u64 * 60).clamp(config::MIN_IDLE_SECS, config::MAX_IDLE_SECS);
        if let Ok(mut cfg) = self.env.config.write() {
            cfg.idle_timeout_secs = secs;
            cfg.save(&self.env.data_dir);
        }
        self.win().global::<SettingsState>().set_idle_minutes((secs / 60) as i32);
    }

    fn open_about(&self) {
        let ui = self.win();
        let g = ui.global::<AboutState>();
        g.set_version(self.env.version.clone().into());
        g.set_data_dir(self.env.data_dir.to_string_lossy().to_string().into());
        g.set_open(true);
    }

    pub(super) fn close_about(&self) {
        self.win().global::<AboutState>().set_open(false);
    }
}
