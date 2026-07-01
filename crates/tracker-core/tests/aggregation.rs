//! 單元測試（T017）：午夜切分與今日彙總排序。

use jiff::tz::{offset, TimeZone};
use tracker_core::aggregation::summarize_day;
use tracker_core::model::{AppIdentity, ClosedSession};
use tracker_core::rules::Exclusions;
use tracker_core::session::{SessionConfig, SessionMachine, Tick};

fn app(name: &str, exe: &str) -> AppIdentity {
    AppIdentity { display_name: name.into(), executable: exe.into(), is_browser: false }
}

fn sess(app_name: &str, website: Option<&str>, active_ms: i64, date: &str) -> ClosedSession {
    ClosedSession {
        app: app(app_name, "x.exe"),
        website: website.map(|s| s.to_string()),
        start_utc_ms: 0,
        end_utc_ms: active_ms.max(1),
        active_ms,
        local_date: date.into(),
    }
}

/// 各 App／網站皆依活躍時間由大到小排序；總計為各 App 之和。
#[test]
fn day_summary_is_sorted_desc() {
    let date = "2026-06-30";
    let sessions = vec![
        sess("App A", None, 100, date),
        sess("App B", Some("b.com"), 300, date),
        sess("App C", Some("c.com"), 200, date),
        sess("App B", Some("b.com"), 50, date),  // 同 App/網站累加
        sess("App X", None, 999, "2026-06-29"), // 他日，不應計入
    ];
    let s = summarize_day(&sessions, date);

    assert_eq!(s.total_active_ms, 100 + 300 + 200 + 50);
    assert_eq!(s.apps.iter().map(|a| a.name.as_str()).collect::<Vec<_>>(), vec!["App B", "App C", "App A"]);
    assert_eq!(s.apps[0].active_ms, 350);
    assert_eq!(s.websites[0].name, "b.com");
    assert_eq!(s.websites[0].active_ms, 350);
}

/// 跨本機午夜的停留切成兩筆，分屬前後兩日（FR-006）。
#[test]
fn crossing_midnight_splits_into_two_days() {
    let tz = TimeZone::fixed(offset(8));
    let cfg = SessionConfig { idle_threshold_ms: 600_000, min_session_ms: 5_000, tz: tz.clone() };
    let mut m = SessionMachine::new(cfg, Exclusions::default());

    // 2026-06-30 23:50（+08:00）起，連續活躍至 2026-07-01 00:10。
    let start = "2026-06-30T23:50:00"
        .parse::<jiff::civil::DateTime>()
        .unwrap()
        .to_zoned(tz.clone())
        .unwrap()
        .timestamp()
        .as_millisecond();

    let mut closed = Vec::new();
    let mut t = start;
    let end = start + 20 * 60_000; // 20 分鐘後
    while t <= end {
        closed.extend(m.on_tick(Tick {
            now_ms: t,
            foreground: Some(app("Editor", "editor.exe")),
            website: None,
            idle_ms: 0,
            session_active: true,
        }));
        t += 60_000;
    }
    if let Some(last) = m.force_close(end) {
        closed.push(last);
    }

    assert_eq!(closed.len(), 2, "跨午夜應切為兩段");
    assert_eq!(closed[0].local_date, "2026-06-30");
    assert_eq!(closed[1].local_date, "2026-07-01");
    // 前段約 10 分鐘（23:50→00:00），後段約 10 分鐘（00:00→00:10）。
    assert!((closed[0].active_ms - 600_000).abs() <= 60_000);
    assert!((closed[1].active_ms - 600_000).abs() <= 60_000);
}
