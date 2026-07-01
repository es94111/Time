//! 單元測試（T009）：速率換算、缺值規則、桶粒度選擇邊界。

use tracker_core::metrics::{
    bucket_ms_for_span, compute_rate_bps, ratio_pct, MetricSnapshot, BUCKET_DAY_MS,
    BUCKET_HOUR_MS, BUCKET_MINUTE_MS, HOUR_MAX_MS, MINUTE_MAX_MS, RAW_MAX_MS,
};

#[test]
fn rate_is_delta_over_time_in_bytes_per_sec() {
    // 1 秒內累計增加 1_000_000 bytes → 1_000_000 bytes/sec。
    assert_eq!(compute_rate_bps(0, 1_000_000, 0, 1_000), Some(1_000_000));
    // 2 秒內增加 1_000_000 bytes → 500_000 bytes/sec。
    assert_eq!(compute_rate_bps(500_000, 1_500_000, 0, 2_000), Some(500_000));
}

#[test]
fn rate_rejects_non_advancing_time_and_counter_reset() {
    // 時間未前進 → None。
    assert_eq!(compute_rate_bps(0, 100, 1_000, 1_000), None);
    assert_eq!(compute_rate_bps(0, 100, 2_000, 1_000), None);
    // 計數器回繞／重置（curr < prev）→ None，不產生負速率（原則 III）。
    assert_eq!(compute_rate_bps(1_000, 10, 0, 1_000), None);
}

#[test]
fn missing_value_rule_keeps_other_fields() {
    // 單項失敗記 None，其餘照常（FR-007）。
    let snap = MetricSnapshot {
        ts_utc: 1_000,
        cpu_pct: None,               // CPU 讀取失敗
        mem_used_bytes: Some(8),
        mem_total_bytes: Some(16),
        ..Default::default()
    };
    assert!(snap.cpu_pct.is_none());
    assert_eq!(snap.sample().mem_used_bytes, Some(8));
    // 使用率由 used/total 推算；缺總量則為 None。
    assert_eq!(ratio_pct(Some(8), Some(16)), Some(50.0));
    assert_eq!(ratio_pct(Some(8), None), None);
    assert_eq!(ratio_pct(Some(8), Some(0)), None);
}

#[test]
fn bucket_granularity_boundaries() {
    // 邊界皆採「≤」，等於門檻時仍屬較細粒度。
    assert_eq!(bucket_ms_for_span(RAW_MAX_MS), 0); // 2h 內：原始
    assert_eq!(bucket_ms_for_span(RAW_MAX_MS + 1), BUCKET_MINUTE_MS);
    assert_eq!(bucket_ms_for_span(MINUTE_MAX_MS), BUCKET_MINUTE_MS); // 2d：分鐘
    assert_eq!(bucket_ms_for_span(MINUTE_MAX_MS + 1), BUCKET_HOUR_MS);
    assert_eq!(bucket_ms_for_span(HOUR_MAX_MS), BUCKET_HOUR_MS); // 2mo：小時
    assert_eq!(bucket_ms_for_span(HOUR_MAX_MS + 1), BUCKET_DAY_MS);
    assert_eq!(bucket_ms_for_span(i64::MAX / 2), BUCKET_DAY_MS); // 更長：日
}
