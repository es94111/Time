//! 單元測試（T039）：跨多日之區間彙總（含跨午夜段歸屬）。

use tracker_core::aggregation::summarize_range;
use tracker_core::model::{AppIdentity, ClosedSession};

fn sess(app_name: &str, website: Option<&str>, active_ms: i64, date: &str) -> ClosedSession {
    ClosedSession {
        app: AppIdentity { display_name: app_name.into(), executable: "x.exe".into(), is_browser: false },
        website: website.map(|s| s.to_string()),
        start_utc_ms: 0,
        end_utc_ms: active_ms.max(1),
        active_ms,
        local_date: date.into(),
    }
}

#[test]
fn range_includes_endpoints_and_excludes_outside() {
    let sessions = vec![
        sess("A", None, 100, "2026-06-23"), // 區間外（前）
        sess("A", None, 200, "2026-06-24"), // from（含）
        sess("B", Some("b.com"), 300, "2026-06-27"),
        sess("A", None, 400, "2026-06-30"), // to（含）
        sess("A", None, 500, "2026-07-01"), // 區間外（後）
    ];
    let s = summarize_range(&sessions, "2026-06-24", "2026-06-30");

    assert_eq!(s.from, "2026-06-24");
    assert_eq!(s.to, "2026-06-30");
    assert_eq!(s.total_active_ms, 200 + 300 + 400);
    // App A 跨多日累加：200 + 400 = 600，應排第一。
    assert_eq!(s.apps[0].name, "A");
    assert_eq!(s.apps[0].active_ms, 600);
    assert_eq!(s.websites.len(), 1);
    assert_eq!(s.websites[0].name, "b.com");
}

#[test]
fn reversed_bounds_are_normalized() {
    let sessions = vec![sess("A", None, 100, "2026-06-25")];
    let s = summarize_range(&sessions, "2026-06-30", "2026-06-20");
    assert_eq!(s.from, "2026-06-20");
    assert_eq!(s.to, "2026-06-30");
    assert_eq!(s.total_active_ms, 100);
}
