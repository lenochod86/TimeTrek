//! Kontrola nové verze podle posledního vydání na GitHubu.
//!
//! Jediné síťové spojení kromě faviconů: jeden GET na veřejné API GitHubu (bez přihlášení, bez
//! jakýchkoli údajů o uživateli) po spuštění a pak nejvýš 1× za den. Jde vypnout v Nastavení.
//! Nová verze se jen nabídne; stažení spustí až uživatel a instalátor spouští jeho klik.

use crate::config::{Config, SharedConfig};
use crate::db::now_ms;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::mpsc::{channel, RecvTimeoutError, Sender};
use std::sync::{Arc, Mutex};
use std::time::Duration;

const API_URL: &str = "https://api.github.com/repos/lenochod86/TimeTrek/releases/latest";
/// Stahovat se smí jen z vydání tohoto repozitáře.
const DOWNLOAD_PREFIX: &str = "https://github.com/lenochod86/TimeTrek/releases/download/";
const DAY_MS: i64 = 24 * 3600 * 1000;
const HOUR_MS: i64 = 3600 * 1000;
const MAX_DOWNLOAD: u64 = 200 * 1024 * 1024;

#[derive(Debug, Clone, PartialEq)]
pub struct Release {
    /// Bez úvodního `v`, např. `0.2.0`.
    pub version: String,
    pub page_url: String,
    /// Instalátor (`TimeTrek_<verze>_x64-setup.exe`), pokud je u vydání.
    pub installer_url: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Status {
    /// Kontrola je vypnutá nebo ještě neproběhla.
    Idle,
    Checking,
    UpToDate,
    Failed,
    Available,
}

#[derive(Debug, Clone)]
pub struct Snapshot {
    pub status: Status,
    pub release: Option<Release>,
    /// Má se nabídka ukázat (novější verze, která není přeskočená ani odložená).
    pub offer: bool,
}

/// `v1.2.3` / `1.2.3` -> (1, 2, 3); přípony jako `-beta` se ignorují.
pub fn parse_version(s: &str) -> Option<(u64, u64, u64)> {
    let core = s.trim().trim_start_matches('v').split(['-', '+']).next()?;
    let mut it = core.split('.');
    let v = (it.next()?.parse().ok()?, it.next().unwrap_or("0").parse().ok()?, it.next().unwrap_or("0").parse().ok()?);
    it.next().is_none().then_some(v)
}

pub fn is_newer(latest: &str, current: &str) -> bool {
    match (parse_version(latest), parse_version(current)) {
        (Some(l), Some(c)) => l > c,
        _ => false,
    }
}

/// Nabídne se jen novější verze, kterou uživatel nepřeskočil a která není odložená.
pub fn should_offer(release: &Release, current: &str, cfg: &Config, now: i64) -> bool {
    is_newer(&release.version, current)
        && parse_version(&cfg.skipped_version) != parse_version(&release.version)
        && now >= cfg.update_snooze_until_ms
}

/// Z JSONu `releases/latest` (nebo chyba, když to není použitelné vydání).
pub fn parse_release(json: &str) -> Result<Release, String> {
    let v: serde_json::Value = serde_json::from_str(json).map_err(|e| e.to_string())?;
    let tag = v["tag_name"].as_str().ok_or("chybí tag_name")?;
    let version = tag.trim_start_matches('v').to_string();
    parse_version(&version).ok_or("neplatná verze")?;
    let page_url = v["html_url"].as_str().unwrap_or("").to_string();
    let installer_url = v["assets"].as_array().and_then(|a| {
        a.iter().find_map(|x| {
            let name = x["name"].as_str()?;
            let url = x["browser_download_url"].as_str()?;
            (name.starts_with("TimeTrek_") && name.ends_with("-setup.exe") && url.starts_with(DOWNLOAD_PREFIX))
                .then(|| url.to_string())
        })
    });
    Ok(Release { version, page_url, installer_url })
}

fn agent() -> ureq::Agent {
    ureq::AgentBuilder::new().timeout(Duration::from_secs(20)).user_agent("TimeTrek-update-check").build()
}

fn fetch_latest() -> Result<Release, String> {
    let body = agent()
        .get(API_URL)
        .set("Accept", "application/vnd.github+json")
        .call()
        .map_err(|e| e.to_string())?
        .into_string()
        .map_err(|e| e.to_string())?;
    parse_release(&body)
}

enum Msg {
    CheckNow,
    Stop,
}

type Fetcher = Box<dyn Fn() -> Result<Release, String> + Send + Sync>;

struct Inner {
    cfg: SharedConfig,
    data_dir: PathBuf,
    current: String,
    state: Mutex<(Status, Option<Release>)>,
    notify: Box<dyn Fn() + Send + Sync>,
    fetch: Fetcher,
}

/// Kontrola aktualizací běžící na vlastním vlákně (stejně jako snímky a favicony).
pub struct Updater {
    inner: Arc<Inner>,
    tx: Sender<Msg>,
}

impl Updater {
    /// `notify` se zavolá z vlákna kontroly, kdykoli se změní stav nabídky.
    pub fn start(
        cfg: SharedConfig,
        data_dir: PathBuf,
        current: String,
        notify: impl Fn() + Send + Sync + 'static,
    ) -> Updater {
        Self::start_with(cfg, data_dir, current, Box::new(fetch_latest), Box::new(notify))
    }

    fn start_with(
        cfg: SharedConfig,
        data_dir: PathBuf,
        current: String,
        fetch: Fetcher,
        notify: Box<dyn Fn() + Send + Sync>,
    ) -> Updater {
        let inner = Arc::new(Inner { cfg, data_dir, current, state: Mutex::new((Status::Idle, None)), notify, fetch });
        let (tx, rx) = channel();
        let worker = inner.clone();
        std::thread::Builder::new()
            .name("timetrek-updates".into())
            .spawn(move || {
                // Po spuštění se kontroluje hned, jen ne častěji než 1× za hodinu (rychlé restarty).
                let last = worker.cfg.read().map(|c| c.last_update_check_ms).unwrap_or(0);
                let mut next_due = now_ms().max(last + HOUR_MS);
                let mut last_offer = false;
                let mut first = true;
                loop {
                    // První průchod (kontrola po spuštění) nečeká celých 30 s.
                    let wait = if std::mem::take(&mut first) { Duration::from_millis(500) } else { Duration::from_secs(30) };
                    let forced = match rx.recv_timeout(wait) {
                        Ok(Msg::Stop) | Err(RecvTimeoutError::Disconnected) => break,
                        Ok(Msg::CheckNow) => true,
                        Err(RecvTimeoutError::Timeout) => false,
                    };
                    let enabled = worker.cfg.read().map(|c| c.update_check).unwrap_or(false);
                    if !enabled {
                        // Vypnuto: zahodí se i už nalezená nabídka.
                        let had = std::mem::replace(&mut *worker.state.lock().unwrap(), (Status::Idle, None));
                        if had.0 != Status::Idle {
                            (worker.notify)();
                        }
                        last_offer = false;
                        continue;
                    }
                    let now = now_ms();
                    if forced || now >= next_due {
                        next_due = worker.check(now);
                    }
                    // Odložení vyprší samo: nabídka se ukáže, aniž by se musela znovu stahovat.
                    let offer = worker.snapshot().offer;
                    if offer != last_offer {
                        last_offer = offer;
                        (worker.notify)();
                    }
                }
            })
            .expect("spawn updates thread");
        Updater { inner, tx }
    }

    pub fn snapshot(&self) -> Snapshot {
        self.inner.snapshot()
    }

    /// Ruční kontrola z Nastavení.
    pub fn check_now(&self) {
        let _ = self.tx.send(Msg::CheckNow);
    }

    /// Odloží nabídku o den.
    pub fn snooze(&self) {
        self.edit_config(|c| c.update_snooze_until_ms = now_ms() + DAY_MS);
    }

    /// Přeskočí aktuálně nabízenou verzi (nabídne se až další novější).
    pub fn skip(&self) {
        let version = self.inner.state.lock().unwrap().1.as_ref().map(|r| r.version.clone());
        if let Some(v) = version {
            self.edit_config(|c| c.skipped_version = v);
        }
    }

    pub fn set_enabled(&self, on: bool) {
        self.edit_config(|c| c.update_check = on);
        if on {
            self.check_now();
        } else {
            self.inner.clear();
        }
    }

    fn edit_config(&self, f: impl FnOnce(&mut Config)) {
        if let Ok(mut c) = self.inner.cfg.write() {
            f(&mut c);
            c.save(&self.inner.data_dir);
        }
        (self.inner.notify)();
    }

    /// Vlákno se jen požádá o konec; nečeká se na rozběhnutý dotaz (ukončení aplikace nemá zdržet síť).
    pub fn shutdown(&self) {
        let _ = self.tx.send(Msg::Stop);
    }

    /// Jen pro náhled UI: vloží hotový výsledek kontroly.
    pub fn preview(release: Release, cfg: SharedConfig, data_dir: PathBuf, current: &str) -> Updater {
        let u = Self::start_with(
            cfg,
            data_dir,
            current.to_string(),
            Box::new(move || Ok(release.clone())),
            Box::new(|| {}),
        );
        u.check_now();
        u
    }
}

impl Inner {
    fn snapshot(&self) -> Snapshot {
        let (status, release) = self.state.lock().unwrap().clone();
        let offer = match (&status, &release, self.cfg.read()) {
            (Status::Available, Some(r), Ok(c)) => should_offer(r, &self.current, &c, now_ms()),
            _ => false,
        };
        Snapshot { status, release, offer }
    }

    fn clear(&self) {
        *self.state.lock().unwrap() = (Status::Idle, None);
        (self.notify)();
    }

    /// Provede kontrolu; vrací, kdy je dál na řadě (za den po úspěchu, za hodinu po chybě).
    fn check(&self, now: i64) -> i64 {
        self.state.lock().unwrap().0 = Status::Checking;
        (self.notify)();
        let result = (self.fetch)();
        let next = match result {
            Ok(rel) => {
                let status = if is_newer(&rel.version, &self.current) { Status::Available } else { Status::UpToDate };
                *self.state.lock().unwrap() = (status, Some(rel));
                if let Ok(mut c) = self.cfg.write() {
                    c.last_update_check_ms = now;
                    c.save(&self.data_dir);
                }
                now + DAY_MS
            }
            Err(e) => {
                log::warn!("kontrola aktualizací: {e}");
                let prev = self.state.lock().unwrap().1.clone();
                *self.state.lock().unwrap() = (Status::Failed, prev);
                now + HOUR_MS
            }
        };
        (self.notify)();
        next
    }
}

/// Instalátor stažený k dokončení po ukončení aplikace (spustí se až po uložení měření).
static PENDING_INSTALLER: Mutex<Option<PathBuf>> = Mutex::new(None);

pub fn set_pending_installer(path: PathBuf) {
    *PENDING_INSTALLER.lock().unwrap() = Some(path);
}

pub fn launch_pending_installer() {
    if let Some(p) = PENDING_INSTALLER.lock().unwrap().take() {
        if let Err(e) = std::process::Command::new(&p).spawn() {
            log::error!("spuštění instalátoru {}: {e}", p.display());
        }
    }
}

/// Stáhne instalátor do dočasné složky. `progress(stažené, celkem)`; blokuje, volat z vlákna.
pub fn download_installer(rel: &Release, progress: impl Fn(u64, Option<u64>)) -> Result<PathBuf, String> {
    let url = rel.installer_url.as_deref().ok_or("vydání nemá instalátor")?;
    if !url.starts_with(DOWNLOAD_PREFIX) {
        return Err("nedůvěryhodná adresa stažení".into());
    }
    if parse_version(&rel.version).is_none() {
        return Err("neplatná verze".into());
    }
    let resp = ureq::AgentBuilder::new()
        .timeout_connect(Duration::from_secs(15))
        .timeout_read(Duration::from_secs(30))
        .user_agent("TimeTrek-update-check")
        .build()
        .get(url)
        .call()
        .map_err(|e| e.to_string())?;
    let total = resp.header("Content-Length").and_then(|v| v.parse::<u64>().ok());
    if total.is_some_and(|t| t > MAX_DOWNLOAD) {
        return Err("instalátor je nečekaně velký".into());
    }
    let path = std::env::temp_dir().join(format!("TimeTrek_{}_x64-setup.exe", rel.version));
    let tmp = path.with_extension("part");
    let mut reader = resp.into_reader().take(MAX_DOWNLOAD + 1);
    let result = (|| -> Result<(), String> {
        let mut file = std::fs::File::create(&tmp).map_err(|e| e.to_string())?;
        let (mut buf, mut done) = (vec![0u8; 64 * 1024], 0u64);
        loop {
            let n = reader.read(&mut buf).map_err(|e| e.to_string())?;
            if n == 0 {
                break;
            }
            done += n as u64;
            if done > MAX_DOWNLOAD {
                return Err("instalátor je nečekaně velký".into());
            }
            file.write_all(&buf[..n]).map_err(|e| e.to_string())?;
            progress(done, total);
        }
        file.flush().map_err(|e| e.to_string())?;
        if total.is_some_and(|t| t != done) {
            return Err("stahování skončilo předčasně".into());
        }
        Ok(())
    })();
    if let Err(e) = result {
        let _ = std::fs::remove_file(&tmp);
        return Err(e);
    }
    replace_file(&tmp, &path)?;
    Ok(path)
}

fn replace_file(from: &Path, to: &Path) -> Result<(), String> {
    let _ = std::fs::remove_file(to);
    std::fs::rename(from, to).map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::RwLock;

    fn rel(v: &str) -> Release {
        Release { version: v.into(), page_url: String::new(), installer_url: None }
    }

    #[test]
    fn versions_parse_and_compare() {
        assert_eq!(parse_version("v0.2.10"), Some((0, 2, 10)));
        assert_eq!(parse_version("1.0"), Some((1, 0, 0)));
        assert_eq!(parse_version("0.3.0-beta.1"), Some((0, 3, 0)));
        assert_eq!(parse_version("1.2.3.4"), None);
        assert_eq!(parse_version("abc"), None);
        assert!(is_newer("v0.2.0", "0.1.5"));
        assert!(is_newer("0.10.0", "0.9.9")); // číselně, ne textově
        assert!(!is_newer("0.1.5", "0.1.5"));
        assert!(!is_newer("0.1.4", "0.1.5"));
        assert!(!is_newer("nesmysl", "0.1.5"));
    }

    #[test]
    fn offer_respects_skip_and_snooze() {
        let mut c = Config::default();
        let r = rel("0.2.0");
        assert!(should_offer(&r, "0.1.5", &c, 1000));
        assert!(!should_offer(&r, "0.2.0", &c, 1000)); // už mám
        c.update_snooze_until_ms = 5000;
        assert!(!should_offer(&r, "0.1.5", &c, 1000)); // odloženo
        assert!(should_offer(&r, "0.1.5", &c, 5000)); // odložení vypršelo
        c.skipped_version = "0.2.0".into();
        assert!(!should_offer(&r, "0.1.5", &c, 9999)); // přeskočeno
        assert!(should_offer(&rel("0.2.1"), "0.1.5", &c, 9999)); // novější než přeskočená
    }

    #[test]
    fn parses_github_release() {
        let json = r#"{"tag_name":"v0.2.0","html_url":"https://github.com/lenochod86/TimeTrek/releases/tag/v0.2.0",
          "assets":[{"name":"notes.txt","browser_download_url":"https://github.com/lenochod86/TimeTrek/releases/download/v0.2.0/notes.txt"},
          {"name":"TimeTrek_0.2.0_x64-setup.exe","browser_download_url":"https://github.com/lenochod86/TimeTrek/releases/download/v0.2.0/TimeTrek_0.2.0_x64-setup.exe"}]}"#;
        let r = parse_release(json).unwrap();
        assert_eq!(r.version, "0.2.0");
        assert!(r.installer_url.unwrap().ends_with("TimeTrek_0.2.0_x64-setup.exe"));
    }

    #[test]
    fn rejects_installer_from_foreign_host_and_bad_json() {
        let json = r#"{"tag_name":"v0.2.0","assets":[{"name":"TimeTrek_0.2.0_x64-setup.exe",
          "browser_download_url":"https://evil.example/TimeTrek_0.2.0_x64-setup.exe"}]}"#;
        assert_eq!(parse_release(json).unwrap().installer_url, None);
        assert!(parse_release("{}").is_err());
        assert!(parse_release("nejson").is_err());
        assert!(parse_release(r#"{"tag_name":"latest"}"#).is_err());
    }

    fn updater(fetch: Fetcher, cfg: Config) -> (Updater, Arc<AtomicUsize>, SharedConfig) {
        let notified = Arc::new(AtomicUsize::new(0));
        let n = notified.clone();
        let shared: SharedConfig = Arc::new(RwLock::new(cfg));
        let dir = std::env::temp_dir().join(format!("timetrek-upd-{}", now_ms()));
        std::fs::create_dir_all(&dir).unwrap();
        let u = Updater::start_with(shared.clone(), dir, "0.1.5".into(), fetch, Box::new(move || {
            n.fetch_add(1, Ordering::SeqCst);
        }));
        (u, notified, shared)
    }

    fn wait_for(u: &Updater, want: Status) {
        for _ in 0..200 {
            if u.snapshot().status == want {
                return;
            }
            std::thread::sleep(Duration::from_millis(10));
        }
        panic!("stav {:?} nenastal, je {:?}", want, u.snapshot().status);
    }

    #[test]
    fn manual_check_finds_update_and_skip_hides_it() {
        let cfg = Config { last_update_check_ms: now_ms(), ..Default::default() };
        let (u, notified, shared) = updater(Box::new(|| Ok(rel("0.2.0"))), cfg);
        u.check_now();
        wait_for(&u, Status::Available);
        assert!(u.snapshot().offer);
        assert!(shared.read().unwrap().last_update_check_ms > 0);
        u.skip();
        assert!(!u.snapshot().offer);
        assert_eq!(shared.read().unwrap().skipped_version, "0.2.0");
        assert!(notified.load(Ordering::SeqCst) >= 2);
        u.shutdown();
    }

    #[test]
    fn snooze_hides_offer_for_a_day() {
        let cfg = Config { last_update_check_ms: now_ms(), ..Default::default() };
        let (u, _, shared) = updater(Box::new(|| Ok(rel("0.2.0"))), cfg);
        u.check_now();
        wait_for(&u, Status::Available);
        u.snooze();
        assert!(!u.snapshot().offer);
        assert!(shared.read().unwrap().update_snooze_until_ms > now_ms() + DAY_MS - 5000);
        u.shutdown();
    }

    #[test]
    fn failure_is_reported_and_same_version_is_up_to_date() {
        let cfg = Config { last_update_check_ms: now_ms(), ..Default::default() };
        let (u, _, _) = updater(Box::new(|| Err("bez sítě".into())), cfg);
        u.check_now();
        wait_for(&u, Status::Failed);
        assert!(!u.snapshot().offer);
        u.shutdown();
        let cfg = Config { last_update_check_ms: now_ms(), ..Default::default() };
        let (u, _, _) = updater(Box::new(|| Ok(rel("0.1.5"))), cfg);
        u.check_now();
        wait_for(&u, Status::UpToDate);
        u.shutdown();
    }

    #[test]
    fn disabled_check_does_nothing_and_clears_offer() {
        let cfg = Config { update_check: false, ..Default::default() };
        let calls = Arc::new(AtomicUsize::new(0));
        let c = calls.clone();
        let (u, _, _) = updater(
            Box::new(move || {
                c.fetch_add(1, Ordering::SeqCst);
                Ok(rel("0.2.0"))
            }),
            cfg,
        );
        u.check_now();
        std::thread::sleep(Duration::from_millis(150));
        assert_eq!(calls.load(Ordering::SeqCst), 0);
        assert_eq!(u.snapshot().status, Status::Idle);
        u.shutdown();
    }
}
