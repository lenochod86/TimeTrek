//! Vykreslí UI do PNG bez okna a bez Windows (software renderer), s vymyšlenými daty.
//! Slouží ke kontrole vzhledu v obou režimech:
//!
//!   cargo run --example preview -- --theme dark --tab day --out den.png
//!   cargo run --example preview -- --theme light --tab reports --out prehledy.png
//!
//! Volby: --theme dark|light, --tab day|stats|reports, --size 1400x860, --out soubor.png,
//!        --scenario none|preview|locked|viewer|tag|confirm|settings|about|zoom

use image::codecs::jpeg::JpegEncoder;
use slint::platform::software_renderer::{MinimalSoftwareWindow, RepaintBufferType};
use slint::platform::{Platform, WindowAdapter};
use slint::{ComponentHandle, PhysicalSize};
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::sync::{Arc, RwLock};
use timetrek_lib::ui::{Ctl, DayState, Env, MainWindow, Nav, Preview, RangeState, TagDlg};
use timetrek_lib::{config, data, db};

struct P(Rc<MinimalSoftwareWindow>, std::time::Instant);
impl Platform for P {
    fn create_window_adapter(&self) -> Result<Rc<dyn WindowAdapter>, slint::PlatformError> {
        Ok(self.0.clone())
    }
    fn duration_since_start(&self) -> std::time::Duration {
        self.1.elapsed()
    }
}

fn arg(name: &str, default: &str) -> String {
    let args: Vec<String> = std::env::args().collect();
    args.iter().position(|a| a == name).and_then(|i| args.get(i + 1)).cloned().unwrap_or_else(|| default.into())
}

fn icon_png(rgb: [u8; 3]) -> Vec<u8> {
    let img = image::RgbaImage::from_fn(32, 32, |x, y| {
        let edge = x < 3 || y < 3 || x > 28 || y > 28;
        image::Rgba([if edge { 255 } else { rgb[0] }, if edge { 255 } else { rgb[1] }, rgb[2], 255])
    });
    let mut out = Vec::new();
    image::DynamicImage::ImageRgba8(img)
        .write_to(&mut std::io::Cursor::new(&mut out), image::ImageFormat::Png)
        .unwrap();
    out
}

fn shot_jpeg(seed: u8) -> Vec<u8> {
    let img = image::RgbImage::from_fn(640, 360, |x, y| {
        image::Rgb([(x / 3) as u8 ^ seed, (y / 2) as u8, 120u8.wrapping_add(seed)])
    });
    let mut out = Vec::new();
    JpegEncoder::new_with_quality(&mut out, 70).encode_image(&img).unwrap();
    out
}

/// Vymyšlený den (datum zadané lokálně jako ms půlnoci).
fn seed(dir: &Path, midnight: i64) -> PathBuf {
    std::fs::create_dir_all(dir.join("screens")).unwrap();
    let path = dir.join("timetrek.db");
    let _ = std::fs::remove_file(&path);
    let conn = db::open(&path).unwrap();
    let mk = |exe: &str, name: &str, c: [u8; 3]| db::upsert_app(&conn, exe, name, Some(&icon_png(c))).unwrap();
    let code = mk("code.exe", "Visual Studio Code", [0x20, 0x70, 0xd0]);
    let edge = mk("msedge.exe", "Microsoft Edge", [0x20, 0xa0, 0x70]);
    let term = mk("wt.exe", "Windows Terminal", [0x50, 0x50, 0x50]);
    let mail = mk("outlook.exe", "Outlook", [0x30, 0x60, 0xc0]);
    let h = 3_600_000i64;
    let m = 60_000i64;
    type Plan<'a> = (i64, i64, Option<i64>, &'a str, Option<&'a str>);
    let plan: &[Plan] = &[
        (8 * h + 3 * m, 8 * h + 41 * m, Some(mail), "Doručená pošta - Outlook", None),
        (8 * h + 41 * m, 9 * h + 30 * m, Some(code), "tracker.rs - TimeTrek - Visual Studio Code", None),
        (9 * h + 30 * m, 9 * h + 52 * m, Some(edge), "Slint docs - Microsoft Edge", Some("slint.dev")),
        (9 * h + 52 * m, 11 * h + 10 * m, Some(code), "day.slint - TimeTrek - Visual Studio Code", None),
        (11 * h + 10 * m, 11 * h + 14 * m, Some(term), "cargo test", None),
        (11 * h + 14 * m, 12 * h + 20 * m, Some(code), "timeline.rs - TimeTrek - Visual Studio Code", None),
        (13 * h + 5 * m, 13 * h + 40 * m, Some(edge), "GitHub - lenochod86/TimeTrek", Some("github.com")),
        (13 * h + 40 * m, 15 * h + 55 * m, Some(code), "range.rs - TimeTrek - Visual Studio Code", None),
        (15 * h + 55 * m, 16 * h + 30 * m, Some(mail), "Doručená pošta - Outlook", None),
    ];
    let mut first_seg = 0;
    for (i, (s, e, app, title, dom)) in plan.iter().enumerate() {
        let id = db::insert_segment(&conn, db::KIND_ACTIVE, *app, title, *dom, midnight + s, midnight + e).unwrap();
        if i == 1 {
            first_seg = id;
        }
        // Snímek uprostřed záznamu.
        let rel = format!("screens/{id}.jpg");
        std::fs::write(dir.join(&rel), shot_jpeg(id as u8 * 17)).unwrap();
        conn.execute(
            "INSERT INTO screenshots(segment_id, ts, path) VALUES (?1, ?2, ?3)",
            rusqlite::params![id, midnight + (s + e) / 2, rel],
        )
        .unwrap();
    }
    // Nečinnost přes oběd.
    db::insert_segment(&conn, db::KIND_IDLE, None, "", None, midnight + 12 * h + 20 * m, midnight + 13 * h + 5 * m)
        .unwrap();
    data::add_tag_range(&conn, "Projekt TimeTrek", "#7a6bc4", midnight + 8 * h + 41 * m, midnight + 12 * h + 20 * m).unwrap();
    data::add_tag_range(&conn, "Schůzky", "#c47a1a", midnight + 13 * h + 5 * m, midnight + 13 * h + 40 * m).unwrap();
    // Předchozí dny pro Statistiku a Přehledy.
    for day in 1..=9i64 {
        let base = midnight - day * 24 * h;
        let start = 8 * h + (day * 11 % 50) * m;
        let len = (4 + day % 4) * h + day * 7 * m;
        db::insert_segment(&conn, db::KIND_ACTIVE, Some(if day % 2 == 0 { code } else { edge }), "x", if day % 2 == 0 { None } else { Some("github.com") }, base + start, base + start + len).unwrap();
        db::insert_segment(&conn, db::KIND_ACTIVE, Some(mail), "mail", None, base + start + len, base + start + len + h).unwrap();
    }
    conn.close().ok();
    let _ = first_seg;
    path
}

fn main() {
    let theme = arg("--theme", "dark");
    let tab = arg("--tab", "day");
    let scenario = arg("--scenario", "none");
    let out = arg("--out", "preview.png");
    let size = arg("--size", "1400x860");
    let (w, h) = size.split_once('x').map(|(a, b)| (a.parse().unwrap(), b.parse().unwrap())).unwrap();

    let win = MinimalSoftwareWindow::new(RepaintBufferType::NewBuffer);
    slint::platform::set_platform(Box::new(P(win.clone(), std::time::Instant::now()))).unwrap();

    // Pevné datum, aby byl snímek stejný při každém spuštění.
    let date = chrono::NaiveDate::from_ymd_opt(2026, 11, 20).unwrap();
    let midnight = {
        use chrono::TimeZone;
        chrono::Local.from_local_datetime(&date.and_hms_opt(0, 0, 0).unwrap()).unwrap().timestamp_millis()
    };
    let dir = std::env::temp_dir().join(format!("timetrek-preview-{}", std::process::id()));
    let db_path = seed(&dir, midnight);

    let cfg = config::Config { light_theme: theme == "light", ..Default::default() };
    let cfg = Arc::new(RwLock::new(cfg));
    let release = timetrek_lib::updates::Release {
        version: "0.2.0".into(),
        page_url: String::new(),
        installer_url: Some("https://github.com/lenochod86/TimeTrek/releases/download/v0.2.0/TimeTrek_0.2.0_x64-setup.exe".into()),
    };
    let updater = Arc::new(timetrek_lib::updates::Updater::preview(release, cfg.clone(), dir.clone(), env!("CARGO_PKG_VERSION")));
    let env = Env {
        updater,
        data_dir: dir.clone(),
        db_path,
        config: cfg,
        version: env!("CARGO_PKG_VERSION").to_string(),
        autostart: false,
    };

    let ui = MainWindow::new().unwrap();
    let _ctl = Ctl::new(&ui, env).unwrap();
    win.set_size(PhysicalSize::new(w, h));
    ui.show().unwrap();

    let day = ui.global::<DayState>();
    day.invoke_date_edited("20.11.2026".into());
    let tab_i = match tab.as_str() { "stats" => 1, "reports" => 2, _ => 0 };
    let range = ui.global::<RangeState>();
    range.invoke_from_edited("12.11.2026".into());
    range.invoke_to_edited("20.11.2026".into());
    ui.global::<Nav>().set_tab(tab_i);
    ui.global::<Nav>().invoke_tab_changed(tab_i);

    let pump = |ms: u64| {
        let end = std::time::Instant::now() + std::time::Duration::from_millis(ms);
        while std::time::Instant::now() < end {
            slint::platform::update_timers_and_animations();
            win.draw_if_needed(|r| {
                let mut buf = vec![slint::Rgb8Pixel { r: 0, g: 0, b: 0 }; (w * h) as usize];
                r.render(&mut buf, w as usize);
            });
            std::thread::sleep(std::time::Duration::from_millis(20));
        }
    };
    pump(300);

    match scenario.as_str() {
        "preview" | "locked" => {
            day.invoke_row_hover(2, 300.0, 600.0);
            pump(400);
            if scenario == "locked" {
                ui.global::<Preview>().invoke_toggle_lock();
                pump(100);
            }
        }
        "viewer" => {
            ui.global::<Preview>().invoke_toggle_viewer();
            pump(200);
        }
        "tag" => {
            day.invoke_lane_pressed(0, 300.0);
            day.invoke_lane_moved(0, 600.0);
            day.invoke_lane_released(0, 600.0);
            ui.global::<TagDlg>().set_name("Návrh".into());
            pump(200);
        }
        "confirm" => {
            day.invoke_lane_context(0, 400.0);
            pump(200);
        }
        "settings" => {
            ui.global::<Nav>().invoke_show_settings();
            pump(200);
        }
        "update" | "update-settings" => {
            // Kontrola poběží na vlákně; počkáme, až se nabídka objeví.
            pump(1200);
            if scenario == "update-settings" {
                ui.global::<Nav>().invoke_show_settings();
            }
            pump(200);
        }
        "datepicker" => {
            pump(1200);
            // Klik na tlačítko kalendáře vedle pole s datem.
            use slint::platform::{PointerEventButton, WindowEvent};
            let pos = slint::LogicalPosition::new(144.0, 108.0);
            win.dispatch_event(WindowEvent::PointerMoved { position: pos });
            win.dispatch_event(WindowEvent::PointerPressed { position: pos, button: PointerEventButton::Left });
            win.dispatch_event(WindowEvent::PointerReleased { position: pos, button: PointerEventButton::Left });
            pump(400);
        }
        "about" => {
            ui.global::<Nav>().invoke_show_about();
            pump(200);
        }
        "zoom" => {
            day.invoke_lane_scroll(2, 500.0, 120.0);
            day.invoke_lane_scroll(2, 500.0, 120.0);
            day.invoke_lane_hover(2, 300.0, 400.0, 250.0);
            pump(200);
        }
        _ => {}
    }
    pump(200);

    let mut buf = vec![slint::Rgb8Pixel { r: 0, g: 0, b: 0 }; (w * h) as usize];
    win.request_redraw();
    win.draw_if_needed(|r| {
        r.render(&mut buf, w as usize);
    });
    let raw: Vec<u8> = buf.iter().flat_map(|p| [p.r, p.g, p.b]).collect();
    image::save_buffer(&out, &raw, w, h, image::ColorType::Rgb8).unwrap();
    println!("uloženo: {out}");
    let _ = std::fs::remove_dir_all(dir);
}
