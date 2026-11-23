//! Uživatelské rozhraní (Slint). `Ctl` drží stav jednoho otevřeného okna a převádí data z databáze
//! na modely UI; veškerá logika UI je tady a v podmodulech, `ui/*.slint` jen kreslí.
//!
//! Okno existuje, jen když je otevřené: po zavření se `Ctl` i okno zahodí a uvolní se všechna paměť.

use crate::config::SharedConfig;
use rusqlite::Connection;
use slint::{ComponentHandle, Image, Rgba8Pixel, SharedPixelBuffer, Timer, TimerMode, Weak};
use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::path::PathBuf;
use std::rc::Rc;
use std::time::Duration;

pub mod colors;
mod day;
mod dialogs;
pub mod fmt;
mod preview;
mod range;
pub mod timeline;

slint::include_modules!();

/// Co UI potřebuje zvenku.
#[derive(Clone)]
pub struct Env {
    pub data_dir: PathBuf,
    pub db_path: PathBuf,
    pub config: SharedConfig,
    pub version: String,
    /// Zapíná se změna spuštění po přihlášení (v náhledu bez dotyku na systém vypnuto).
    pub autostart: bool,
}

/// Ikony aplikací a webů v paměti. Favicony stahuje tracker na pozadí, takže se chybějící
/// ptáme znovu při dalším obnovení.
#[derive(Default)]
pub struct Icons {
    pub apps: HashMap<i64, Image>,
    pub sites: HashMap<String, Image>,
}

pub struct State {
    pub day: day::DayData,
    pub preview: preview::PreviewData,
    pub range: range::RangeData,
    pub dialogs: dialogs::DialogData,
    pub icons: Icons,
}

pub struct Ctl {
    pub ui: Weak<MainWindow>,
    pub env: Env,
    pub conn: Connection,
    pub st: RefCell<State>,
    /// Logická šířka oblasti pruhů (z UI).
    pub lane_w: Cell<f64>,
    /// Slabý odkaz na sebe, aby si metody mohly vyrobit zpětné volání pro časovač.
    pub me: std::rc::Weak<Ctl>,
    refresh: Timer,
    pub hover_timer: Timer,
}

/// Napojí zpětné volání Slintu na metodu `Ctl`; zpětné volání drží jen slabý odkaz.
macro_rules! on {
    ($ctl:ident, $obj:expr, $method:ident, |$c:ident $(, $a:ident)*| $body:expr) => {{
        let weak = std::rc::Rc::downgrade(&$ctl);
        $obj.$method(move |$($a),*| {
            if let Some($c) = weak.upgrade() {
                let _ = $body;
            }
        });
    }};
}
pub(crate) use on;

pub fn rgba_image(w: u32, h: u32, rgba: &[u8]) -> Image {
    let mut buf = SharedPixelBuffer::<Rgba8Pixel>::new(w.max(1), h.max(1));
    let dst = buf.make_mut_bytes();
    let n = dst.len().min(rgba.len());
    dst[..n].copy_from_slice(&rgba[..n]);
    Image::from_rgba8(buf)
}

/// PNG / JPEG z paměti na obrázek UI.
pub fn decode_image(bytes: &[u8]) -> Option<Image> {
    let img = image::load_from_memory(bytes).ok()?.into_rgba8();
    Some(rgba_image(img.width(), img.height(), img.as_raw()))
}

pub fn slint_color(c: [u8; 3]) -> slint::Color {
    slint::Color::from_rgb_u8(c[0], c[1], c[2])
}

impl Ctl {
    pub fn new(ui: &MainWindow, env: Env) -> Result<Rc<Ctl>, String> {
        let conn = crate::db::open(&env.db_path).map_err(|e| e.to_string())?;
        let ctl = Rc::new_cyclic(|me| Ctl {
            me: me.clone(),
            ui: ui.as_weak(),
            conn,
            st: RefCell::new(State {
                day: day::DayData::new(),
                preview: preview::PreviewData::default(),
                range: range::RangeData::new(),
                dialogs: dialogs::DialogData::default(),
                icons: Icons::default(),
            }),
            lane_w: Cell::new(800.0),
            refresh: Timer::default(),
            hover_timer: Timer::default(),
            env,
        });

        ui.set_dark(!ctl.env.config.read().map(|c| c.light_theme).unwrap_or(false));
        ctl.bind_nav(ui);
        ctl.bind_day(ui);
        ctl.bind_preview(ui);
        ctl.bind_range(ui);
        ctl.bind_dialogs(ui);

        ctl.init_day();
        ctl.init_range(ui);

        // Obnovení dat: dnešek každé 3 s, souhrny každých ~9 s; minimalizované okno se neobnovuje.
        let weak = Rc::downgrade(&ctl);
        let tick = Cell::new(0u32);
        ctl.refresh.start(TimerMode::Repeated, Duration::from_secs(3), move || {
            let Some(c) = weak.upgrade() else { return };
            let Some(ui) = c.ui.upgrade() else { return };
            if ui.window().is_minimized() {
                return;
            }
            tick.set(tick.get().wrapping_add(1));
            match ui.global::<Nav>().get_tab() {
                0 => c.refresh_day(),
                _ if tick.get().is_multiple_of(3) => c.refresh_range(),
                _ => {}
            }
        });
        Ok(ctl)
    }

    pub fn win(&self) -> MainWindow {
        self.ui.upgrade().expect("okno už neexistuje")
    }

    fn bind_nav(self: &Rc<Self>, ui: &MainWindow) {
        let ctl = self.clone();
        let nav = ui.global::<Nav>();
        on!(ctl, nav, on_toggle_theme, |c| c.toggle_theme());
        on!(ctl, nav, on_tab_changed, |c, tab| c.tab_changed(tab));
    }

    fn toggle_theme(&self) {
        let ui = self.win();
        let dark = !ui.get_dark();
        ui.set_dark(dark);
        if let Ok(mut cfg) = self.env.config.write() {
            cfg.light_theme = !dark;
            cfg.save(&self.env.data_dir);
        }
        // Bitmapy pruhů se kreslí bez pozadí, takže přebarvení motivu je nepotřebuje.
    }

    fn tab_changed(&self, tab: i32) {
        match tab {
            0 => self.refresh_day(),
            _ => self.refresh_range(),
        }
    }
}
