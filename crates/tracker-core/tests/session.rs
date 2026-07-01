//! 單元測試（T016）：工作階段狀態機。
//!
//! 涵蓋：active/idle、5 秒最短門檻、**已知操作時間下時長累計誤差 ≤ ±2%（SC-001）**、
//! 閒置回溯扣除。

use jiff::tz::{offset, TimeZone};
use tracker_core::model::AppIdentity;
use tracker_core::rules::Exclusions;
use tracker_core::session::{SessionConfig, SessionMachine, Tick};

fn vscode() -> AppIdentity {
    AppIdentity {
        display_name: "Visual Studio Code".into(),
        executable: "code.exe".into(),
        is_browser: false,
    }
}

fn cfg(idle_threshold_ms: i64) -> SessionConfig {
    SessionConfig {
        idle_threshold_ms,
        min_session_ms: 5_000,
        tz: TimeZone::fixed(offset(8)),
    }
}

fn active_tick(now_ms: i64, app: AppIdentity) -> Tick {
    Tick { now_ms, foreground: Some(app), website: None, idle_ms: 0, session_active: true }
}

/// 連續活躍 10 分鐘：記錄時長與已知操作時間誤差 ≤ ±2%（SC-001）。
#[test]
fn continuous_activity_within_two_percent() {
    let mut m = SessionMachine::new(cfg(300_000), Exclusions::default());
    let t0 = 1_700_000_000_000i64;
    let known_ms = 600_000i64; // 10 分鐘
    let step = 2_000i64;

    let mut t = t0;
    while t <= t0 + known_ms {
        let _ = m.on_tick(active_tick(t, vscode()));
        t += step;
    }
    let closed = m.force_close(t0 + known_ms).expect("應產生工作階段");

    let err = (closed.active_ms - known_ms).abs();
    assert!(
        err * 100 <= known_ms * 2,
        "誤差 {err}ms 超過 ±2%（active={}）",
        closed.active_ms
    );
    // 同一段未跨午夜，local_date 應等於起始瞬時之本機日期。
    assert_eq!(
        closed.local_date,
        tracker_core::time::local_date(t0, &TimeZone::fixed(offset(8)))
    );
}

/// 活躍 300 秒後閒置：閒置回溯扣除，最終僅計入活躍部分。
#[test]
fn idle_backout_excludes_inactive_time() {
    let threshold = 60_000i64;
    let mut m = SessionMachine::new(cfg(threshold), Exclusions::default());
    let t0 = 1_700_000_000_000i64;
    let active_for = 300_000i64;
    let step = 5_000i64;

    // 活躍階段：idle=0。
    let mut t = t0;
    while t <= t0 + active_for {
        let _ = m.on_tick(active_tick(t, vscode()));
        t += step;
    }
    // 閒置階段：idle 自最後輸入（t0+active_for）起累加。
    while t <= t0 + active_for + 300_000 {
        let idle = t - (t0 + active_for);
        let _ = m.on_tick(Tick {
            now_ms: t,
            foreground: Some(vscode()),
            website: None,
            idle_ms: idle,
            session_active: true,
        });
        t += step;
    }
    let closed = m.force_close(t).expect("應產生工作階段");
    assert_eq!(closed.active_ms, active_for, "閒置時間不應計入");
}

/// 不足 5 秒的前景停留不建立工作階段（FR-019）。
#[test]
fn below_minimum_threshold_is_dropped() {
    let mut m = SessionMachine::new(cfg(300_000), Exclusions::default());
    let t0 = 1_700_000_000_000i64;
    let _ = m.on_tick(active_tick(t0, vscode()));
    let _ = m.on_tick(active_tick(t0 + 2_000, vscode())); // 僅 2 秒活躍
    let closed = m.force_close(t0 + 2_000);
    assert!(closed.is_none(), "活躍 < 5 秒不應建立工作階段");
}

/// 鎖定／睡眠（session_active=false）期間不累計並結算。
#[test]
fn locked_does_not_accumulate() {
    let mut m = SessionMachine::new(cfg(300_000), Exclusions::default());
    let t0 = 1_700_000_000_000i64;
    // 活躍 1 分鐘。
    let mut t = t0;
    while t <= t0 + 60_000 {
        let _ = m.on_tick(active_tick(t, vscode()));
        t += 2_000;
    }
    // 鎖定：結算目前段。
    let closed = m.on_tick(Tick {
        now_ms: t,
        foreground: Some(vscode()),
        website: None,
        idle_ms: 0,
        session_active: false,
    });
    assert_eq!(closed.len(), 1, "鎖定應結算目前段");
    assert!(closed[0].active_ms >= 55_000 && closed[0].active_ms <= 62_000);
}

/// 切換 App 會關閉前段並開啟新段。
#[test]
fn switching_app_closes_previous() {
    let chrome = AppIdentity {
        display_name: "Google Chrome".into(),
        executable: "chrome.exe".into(),
        is_browser: true,
    };
    let mut m = SessionMachine::new(cfg(300_000), Exclusions::default());
    let t0 = 1_700_000_000_000i64;
    let mut t = t0;
    while t <= t0 + 60_000 {
        let _ = m.on_tick(active_tick(t, vscode()));
        t += 2_000;
    }
    // 切換到 Chrome。
    let closed = m.on_tick(active_tick(t, chrome.clone()));
    assert_eq!(closed.len(), 1);
    assert_eq!(closed[0].app.executable, "code.exe");
}
