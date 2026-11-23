//! Životní cyklus aplikace: data, tracker, ikona v systray a okno, které existuje jen když je otevřené.
//!
//! Zavření okna ho celé zahodí (včetně UI stavu a spojení s databází), takže v systray zůstává
//! jen jádro v Rustu. Otevření z tray okno vytvoří znovu.

use crate::ui::{Ctl, Env, MainWindow};
use crate::{config, db, single_instance, tracker};
use slint::{CloseRequestResponse, ComponentHandle};
use slint::winit_030::WinitWindowAccessor;
use std::cell::RefCell;
use std::rc::Rc;
use std::sync::{Arc, RwLock};
use std::time::Duration;

struct Open {
    win: MainWindow,
    _ctl: Rc<Ctl>,
}

struct App {
    env: Env,
    open: RefCell<Option<Open>>,
}

thread_local! {
    /// Aplikace patří vláknu smyčky událostí; zpětná volání z jiných vláken se na něj přepošlou.
    static APP: RefCell<Option<Rc<App>>> = const { RefCell::new(None) };
}

/// Spustí `f` s aplikací ve vlákně smyčky událostí.
fn with_app(f: impl FnOnce(&Rc<App>) + Send + 'static) {
    let _ = slint::invoke_from_event_loop(move || {
        APP.with(|a| {
            if let Some(app) = a.borrow().as_ref() {
                f(app);
            }
        })
    });
}

impl App {
    /// Otevře okno, nebo přenese do popředí už otevřené.
    fn show(self: &Rc<Self>) {
        if let Some(o) = self.open.borrow().as_ref() {
            let _ = o.win.show();
            o.win.window().set_minimized(false);
            o.win.window().with_winit_window(|w| w.focus_window());
            return;
        }
        let win = match MainWindow::new() {
            Ok(w) => w,
            Err(e) => return log::error!("okno: {e}"),
        };
        let ctl = match Ctl::new(&win, self.env.clone()) {
            Ok(c) => c,
            Err(e) => return log::error!("UI: {e}"),
        };
        let weak = Rc::downgrade(self);
        win.window().on_close_requested(move || {
            // Okno se zahazuje až po návratu z tohoto zpětného volání.
            let weak = weak.clone();
            slint::Timer::single_shot(Duration::ZERO, move || {
                if let Some(app) = weak.upgrade() {
                    app.close();
                }
            });
            CloseRequestResponse::HideWindow
        });
        if let Err(e) = win.show() {
            return log::error!("zobrazení okna: {e}");
        }
        *self.open.borrow_mut() = Some(Open { win, _ctl: ctl });
    }

    fn close(&self) {
        self.open.borrow_mut().take();
        // Bez ikony v systray by se zavřením okna nedalo aplikaci znovu otevřít ani ukončit.
        if !cfg!(any(windows, target_os = "macos")) {
            let _ = slint::quit_event_loop();
        }
    }
}

pub fn run() {
    let data_dir = dirs::data_dir().expect("složka s daty uživatele").join("TimeTrek");
    std::fs::create_dir_all(&data_dir).expect("vytvoření datové složky");
    let hidden = std::env::args().any(|a| a == "--hidden");

    // Druhé spuštění jen přivolá okno už běžící instance.
    if !single_instance::acquire(|| with_app(|app| app.show())) {
        return;
    }

    // %AppData%\TimeTrek. Při volbě „smazat data“ ji maže odinstalátor vlastním krokem
    // v installer/hooks.nsh (názvy musí zůstat shodné).
    let db_path = data_dir.join("timetrek.db");
    if let Err(e) = db::open(&db_path) {
        eprintln!("databáze se nedá otevřít: {e}");
        return;
    }
    let cfg: config::SharedConfig = Arc::new(RwLock::new(config::Config::load(&data_dir)));
    let tracker = tracker::Tracker::start(data_dir.clone(), db_path.clone(), cfg.clone());

    let env = Env {
        data_dir,
        db_path,
        config: cfg,
        version: env!("CARGO_PKG_VERSION").to_string(),
        autostart: true,
    };
    let app = Rc::new(App { env, open: RefCell::new(None) });
    APP.with(|a| *a.borrow_mut() = Some(app.clone()));

    #[cfg(any(windows, target_os = "macos"))]
    let _tray = crate::tray::create(|| with_app(|a| a.show()), || {
        let _ = slint::quit_event_loop();
    })
    .map_err(|e| log::error!("ikona v systray: {e}"))
    .ok();

    // Při startu s `--hidden` (autostart) zůstane aplikace jen v systray.
    if !hidden || !cfg!(any(windows, target_os = "macos")) {
        app.show();
    }

    let _ = slint::run_event_loop_until_quit();

    // Zavření z tray: otevřený záznam se uzavře přesně v okamžiku ukončení.
    app.close_all();
    tracker.shutdown();
}

impl App {
    fn close_all(&self) {
        self.open.borrow_mut().take();
    }
}
