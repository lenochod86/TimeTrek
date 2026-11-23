//! Nabídka nové verze: banner, nastavení kontroly a stažení instalátoru.

use super::{Ctl, MainWindow, UpdateState};
use crate::updates::{self, Status};
use crate::{i18n, platform};
use slint::ComponentHandle;
use std::rc::Rc;

impl Ctl {
    pub(super) fn bind_update(self: &Rc<Self>, ui: &MainWindow) {
        use super::on;
        let ctl = self.clone();
        let u = ui.global::<UpdateState>();
        on!(ctl, u, on_download, |c| c.update_download());
        on!(ctl, u, on_snooze, |c| c.env.updater.snooze());
        on!(ctl, u, on_skip, |c| c.env.updater.skip());
        on!(ctl, u, on_check_toggled, |c, on| c.env.updater.set_enabled(on));
        on!(ctl, u, on_check_now, |c| c.env.updater.check_now());
        self.push_update();
    }

    /// Promítne stav kontroly do banneru a nastavení (volá se i z vlákna kontroly přes smyčku událostí).
    pub fn push_update(&self) {
        let snap = self.env.updater.snapshot();
        let enabled = self.env.config.read().map(|c| c.update_check).unwrap_or(true);
        let ui = self.win();
        let g = ui.global::<UpdateState>();
        g.set_check_enabled(enabled);
        g.set_checking(snap.status == Status::Checking);
        g.set_offer(snap.offer);
        let status = match (&snap.status, &snap.release) {
            _ if !enabled => i18n::UPDATE_OFF.to_string(),
            (Status::Checking, _) => i18n::UPDATE_CHECKING.to_string(),
            (Status::Failed, _) => i18n::UPDATE_CHECK_FAILED.to_string(),
            (Status::UpToDate, _) => i18n::update_up_to_date(&self.env.version),
            (Status::Available, Some(r)) => i18n::update_found(&r.version),
            _ => String::new(),
        };
        g.set_status(status.into());
        if let Some(r) = &snap.release {
            g.set_text(i18n::update_available(&r.version, &self.env.version).into());
            let label = if r.installer_url.is_some() { i18n::UPDATE_DOWNLOAD } else { i18n::UPDATE_OPEN_PAGE };
            g.set_download_label(label.into());
        }
        if !snap.offer {
            g.set_busy(false);
            g.set_progress("".into());
        }
    }

    /// Stáhne instalátor na pozadí; po dokončení se aplikace ukončí a instalátor se spustí
    /// (až po uložení měření, viz `app::run`). Bez instalátoru u vydání se jen otevře jeho stránka.
    fn update_download(&self) {
        let Some(rel) = self.env.updater.snapshot().release else { return };
        if rel.installer_url.is_none() {
            if rel.page_url.starts_with("https://github.com/lenochod86/TimeTrek/") {
                platform::create().open_url(&rel.page_url);
            }
            return;
        }
        let ui = self.win();
        let g = ui.global::<UpdateState>();
        g.set_busy(true);
        g.set_progress(i18n::UPDATE_DOWNLOADING.into());

        let weak = self.ui.clone();
        std::thread::spawn(move || {
            let last = std::cell::Cell::new(u64::MAX);
            let progress_ui = weak.clone();
            let result = updates::download_installer(&rel, |done, total| {
                let Some(total) = total.filter(|t| *t > 0) else { return };
                let pct = done * 100 / total;
                if pct != last.get() {
                    last.set(pct);
                    let _ = progress_ui.upgrade_in_event_loop(move |ui| {
                        ui.global::<UpdateState>().set_progress(i18n::update_progress(pct).into());
                    });
                }
            });
            match result {
                Ok(path) => {
                    updates::set_pending_installer(path);
                    let _ = slint::invoke_from_event_loop(|| {
                        let _ = slint::quit_event_loop();
                    });
                }
                Err(e) => {
                    let _ = weak.upgrade_in_event_loop(move |ui| {
                        let g = ui.global::<UpdateState>();
                        g.set_busy(false);
                        g.set_progress(i18n::update_failed(&e).into());
                    });
                }
            }
        });
    }
}
