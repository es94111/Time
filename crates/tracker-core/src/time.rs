//! 時間工具（T007）：以 `jiff` 做時區感知運算。
//!
//! 工作階段以 UTC 瞬時（epoch 毫秒）記錄；彙總時換算本機時區，
//! 並於本機午夜切分（FR-006）。`jiff` 嚴謹處理 DST 與時鐘變更（research.md R6）。

use jiff::{civil, tz::TimeZone, Timestamp};

/// 現在的 epoch 毫秒（UTC）。
pub fn now_ms() -> i64 {
    Timestamp::now().as_millisecond()
}

/// 系統本機時區；取得失敗時退回 UTC（不致 panic）。
pub fn system_tz() -> TimeZone {
    TimeZone::try_system().unwrap_or(TimeZone::UTC)
}

fn to_timestamp(ms: i64) -> Timestamp {
    Timestamp::from_millisecond(ms).unwrap_or(Timestamp::UNIX_EPOCH)
}

/// 將 epoch 毫秒換算為指定時區的本機日期字串（YYYY-MM-DD）。
pub fn local_date(ms: i64, tz: &TimeZone) -> String {
    let zoned = to_timestamp(ms).to_zoned(tz.clone());
    zoned.date().to_string()
}

/// 給定瞬時所在本機日的「次日午夜」epoch 毫秒（用於跨午夜切分）。
///
/// 正確處理 DST：以民用時間 00:00 在該時區解析回瞬時。
pub fn next_local_midnight_ms(ms: i64, tz: &TimeZone) -> i64 {
    let zoned = to_timestamp(ms).to_zoned(tz.clone());
    let tomorrow = match zoned.date().tomorrow() {
        Ok(d) => d,
        Err(_) => return ms, // 已達可表示日期上限：保守退回原值
    };
    let midnight = tomorrow.to_datetime(civil::Time::MIN);
    match midnight.to_zoned(tz.clone()) {
        Ok(z) => z.timestamp().as_millisecond(),
        Err(_) => ms,
    }
}

/// 兩個瞬時是否落在同一本機日期。
pub fn same_local_date(a_ms: i64, b_ms: i64, tz: &TimeZone) -> bool {
    local_date(a_ms, tz) == local_date(b_ms, tz)
}
