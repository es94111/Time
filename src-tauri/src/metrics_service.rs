//! 背景指標服務（T017、T020、T031）：串接 platform → core → storage 的常駐取樣迴圈。
//!
//! 每個取樣週期呼叫 [`tracker_platform::MetricsSampler`] 取整機快照、寫入加密資料庫，
//! 並 emit `metrics://sample` 事件供即時檢視（SC-003）。停用時暫停取樣（FR-015），
//! 每日觸發一次保留清理（FR-011）。錯誤不中斷迴圈（原則 III：據實紀錄、不中斷整體）。

use std::sync::atomic::Ordering;
use std::sync::Arc;
use std::thread;
use std::time::Duration;

use jiff::tz::TimeZone;
use tauri::{AppHandle, Emitter, Manager};
use tracker_storage::metrics_repo::MetricsRepo;

use crate::state::{AppState, MetricsControl, SharedDb};

/// 取樣週期界限（秒）。
const MIN_INTERVAL_SEC: i64 = 1;
const MAX_INTERVAL_SEC: i64 = 3600;

/// 由目前設定啟動指標取樣執行緒（僅一次）。須於資料庫已開啟後呼叫。
pub fn start(app: &AppHandle) {
    let (db, control, tz) = {
        let state = app.state::<AppState>();
        // 由資料庫載入指標設定到控制旗標。
        if let Ok(guard) = state.db.lock() {
            if let Some(dbref) = guard.as_ref() {
                if let Ok(s) = MetricsRepo::new(dbref).get_metrics_settings() {
                    state.metrics.interval_sec.store(
                        s.sample_interval_sec.clamp(MIN_INTERVAL_SEC, MAX_INTERVAL_SEC),
                        Ordering::SeqCst,
                    );
                    state.metrics.retention_days.store(s.retention_days.max(1), Ordering::SeqCst);
                    state.metrics.enabled.store(s.enabled, Ordering::SeqCst);
                }
            }
        }
        (state.db.clone(), state.metrics.clone(), state.tz.clone())
    };

    if control.started.swap(true, Ordering::SeqCst) {
        return;
    }
    let app = app.clone();
    thread::Builder::new()
        .name("metrics-service".into())
        .spawn(move || run_loop(app, db, control, tz))
        .ok();
}

fn run_loop(app: AppHandle, db: SharedDb, control: Arc<MetricsControl>, tz: TimeZone) {
    let mut sampler = tracker_platform::default_sampler();
    let mut last_purge_date = String::new();

    loop {
        let interval = control
            .interval_sec
            .load(Ordering::SeqCst)
            .clamp(MIN_INTERVAL_SEC, MAX_INTERVAL_SEC);
        thread::sleep(Duration::from_secs(interval as u64));

        // 停用時暫停取樣與寫入（FR-015）；既有歷史不受影響。
        if !control.enabled.load(Ordering::SeqCst) {
            continue;
        }

        let snapshot = sampler.sample();

        let guard = match db.lock() {
            Ok(g) => g,
            Err(_) => continue,
        };
        let dbref = match guard.as_ref() {
            Some(d) => d,
            None => continue,
        };
        let repo = MetricsRepo::new(dbref);

        // 寫入樣本；失敗僅略過本次（不中斷迴圈）。
        if repo.insert_snapshot(&snapshot).is_ok() {
            if let Ok(Some(full)) = repo.get_latest_snapshot() {
                if let Ok(mut last) = control.last.lock() {
                    *last = Some(full.clone());
                }
                // emit 事件供即時檢視（SC-003）；前端可退回輪詢。
                let _ = app.emit("metrics://sample", &full);
            }
        }

        // 每日一次保留清理（FR-011）。
        let now = tracker_core::time::now_ms();
        let today = tracker_core::time::local_date(now, &tz);
        if today != last_purge_date {
            last_purge_date = today;
            let days = control.retention_days.load(Ordering::SeqCst);
            let _ = repo.purge_expired(now, days);
        }
    }
}

/// 立即觸發一次保留清理（變更保留天數後由命令層呼叫）。
pub fn purge_now(state: &AppState) {
    if let Ok(guard) = state.db.lock() {
        if let Some(dbref) = guard.as_ref() {
            let now = tracker_core::time::now_ms();
            let days = state.metrics.retention_days.load(Ordering::SeqCst);
            let _ = MetricsRepo::new(dbref).purge_expired(now, days);
        }
    }
}
