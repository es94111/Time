//! 背景追蹤服務（T014、T027）：串接 platform → core → storage 的常駐迴圈。

use std::sync::atomic::Ordering;
use std::sync::Arc;
use std::thread;
use std::time::Duration;

use jiff::tz::TimeZone;
use tracker_core::model::ClosedSession;
use tracker_core::ports::IdlePort;
use tracker_core::rules::Exclusions;
use tracker_core::session::{SessionConfig, SessionMachine, Tick};
use tracker_core::{browsers, hostname, time};
use tracker_platform::browser_url::WinBrowserUrl;
use tracker_platform::foreground::WinForeground;
use tracker_platform::idle::WinIdle;
use tracker_platform::session_power;
use tracker_storage::{crypto, Database, Repository};

use crate::state::{AppState, SharedControl, SharedDb};

/// tick 間隔（毫秒）。事件驅動的鎖定/睡眠另由事件執行緒處理。
const TICK_MS: u64 = 2_000;
/// 每 N 次 tick 做一次 WAL checkpoint（約 60 秒，SC-005）。
const FLUSH_EVERY_TICKS: u32 = 30;

/// 以（可選）主密碼解開 DEK、開啟加密 DB、載入設定，並在首次成功時啟動追蹤執行緒。
pub fn open_and_start(state: &AppState, password: Option<&str>) -> tracker_storage::Result<()> {
    let dek = crypto::load_or_create_dek(&state.paths.key, &state.secret, password)?;
    let database = Database::open(&state.paths.db, &dek)?;

    // 載入設定與排除規則到共享控制。
    {
        let repo = Repository::new(&database);
        let settings = repo.get_settings()?;
        state
            .control
            .idle_threshold_ms
            .store(settings.idle_threshold_sec.max(1) * 1000, Ordering::SeqCst);
        state.control.paused.store(settings.tracking_paused, Ordering::SeqCst);
        let (apps, sites) = repo.exclusion_patterns()?;
        *state.control.exclusions.lock().unwrap() = Exclusions::new(apps, sites);
        state.control.exclusions_version.fetch_add(1, Ordering::SeqCst);
    }
    *state.db.lock().unwrap() = Some(database);

    // 僅啟動一次追蹤執行緒。
    if !state.control.started.swap(true, Ordering::SeqCst) {
        spawn_loop(state.db.clone(), state.control.clone(), state.tz.clone());
    }
    Ok(())
}

fn spawn_loop(db: SharedDb, control: Arc<SharedControl>, tz: TimeZone) {
    thread::Builder::new()
        .name("tracker-service".into())
        .spawn(move || run_loop(db, control, tz))
        .ok();
}

fn run_loop(db: SharedDb, control: Arc<SharedControl>, tz: TimeZone) {
    let signal = session_power::start();
    let foreground = WinForeground::new();
    let idle = WinIdle::new();
    let browser = WinBrowserUrl::new();

    let mut cfg = SessionConfig::with_tz(tz);
    cfg.idle_threshold_ms = control.idle_threshold_ms.load(Ordering::SeqCst);
    let exclusions = control.exclusions.lock().unwrap().clone();
    let mut machine = SessionMachine::new(cfg, exclusions);
    let mut last_excl_version = control.exclusions_version.load(Ordering::SeqCst);

    let mut was_paused = false;
    let mut tick_count: u32 = 0;

    loop {
        thread::sleep(Duration::from_millis(TICK_MS));
        let now = time::now_ms();

        // 重載可變設定。
        machine.set_idle_threshold_ms(control.idle_threshold_ms.load(Ordering::SeqCst));
        let version = control.exclusions_version.load(Ordering::SeqCst);
        if version != last_excl_version {
            last_excl_version = version;
            let ex = control.exclusions.lock().unwrap().clone();
            machine.set_exclusions(ex);
        }

        // 暫停：結算目前段並停止累計。
        if control.paused.load(Ordering::SeqCst) {
            if !was_paused {
                if let Some(closed) = machine.force_close(now) {
                    write_session(&db, &closed);
                }
                *control.current.lock().unwrap() = None;
                was_paused = true;
            }
            continue;
        }
        was_paused = false;

        // 蒐集觀測。
        let fg = foreground.current_full();
        let (identity, is_browser, title) = match fg {
            Some(f) => {
                let is_browser = f.identity.is_browser;
                (Some(f.identity), is_browser, f.title)
            }
            None => (None, false, String::new()),
        };
        let website = if is_browser && !browsers::is_incognito_title(&title) {
            browser
                .url_for_foreground()
                .and_then(|url| hostname::hostname_from_url(&url))
        } else {
            None
        };

        let tick = Tick {
            now_ms: now,
            foreground: identity,
            website,
            idle_ms: idle.idle_ms(),
            session_active: signal.is_active(),
        };
        for closed in machine.on_tick(tick) {
            write_session(&db, &closed);
        }
        *control.current.lock().unwrap() = machine.current();

        tick_count += 1;
        if tick_count >= FLUSH_EVERY_TICKS {
            tick_count = 0;
            if let Ok(guard) = db.lock() {
                if let Some(dbref) = guard.as_ref() {
                    let _ = dbref.checkpoint();
                }
            }
        }
    }
}

fn write_session(db: &SharedDb, closed: &ClosedSession) {
    if let Ok(guard) = db.lock() {
        if let Some(dbref) = guard.as_ref() {
            let repo = Repository::new(dbref);
            let _ = repo.insert_session(closed);
        }
    }
}
