//! 系統指標純領域邏輯（T006、T011）。
//!
//! 本模組不依賴任何作業系統 API：領域型別、速率換算、缺值約定、
//! 降取樣桶粒度選擇皆可於任意平台單元測試（呼應憲章原則 III）。
//!
//! 單位約定（對應 data-model.md §1、contracts）：
//! - 時間：UTC 瞬時 epoch 毫秒（`ts_utc`）。
//! - 速率：位元組／秒（bytes/sec）。
//! - 記憶體／VRAM：位元組（bytes）。
//! - 缺值：一律以 `Option::None`（持久化為 NULL），不以 0 假冒（FR-007）。

use serde::{Deserialize, Serialize};

/// 整機純量指標（對應 `metric_sample` 資料列）。
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MetricSample {
    /// 取樣瞬時（epoch ms）。
    pub ts_utc: i64,
    /// CPU 整體使用率 0–100（FR-001）；讀取失敗為 `None`。
    pub cpu_pct: Option<f64>,
    /// 已用實體記憶體位元組（FR-002）。
    pub mem_used_bytes: Option<i64>,
    /// 實體記憶體總量位元組。
    pub mem_total_bytes: Option<i64>,
}

/// 單顆硬碟的即時／某時點讀寫速度（FR-003）。
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DiskReading {
    /// 對應 `disk_device.id`；平台採集當下尚未歸戶時為 0。
    pub id: i64,
    /// 穩定識別鍵（PDH PhysicalDisk instance）；不對前端序列化。
    #[serde(skip)]
    pub identifier: String,
    /// 易讀名稱。
    pub name: Option<String>,
    /// 讀取速度 bytes/sec ≥ 0。
    pub read_bps: Option<i64>,
    /// 寫入速度 bytes/sec ≥ 0。
    pub write_bps: Option<i64>,
}

/// 單一網路介面的即時收發速度（FR-004）。
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NetReading {
    /// 對應 `network_interface.id`。
    pub id: i64,
    /// 穩定識別鍵（PDH Network Interface instance）；不對前端序列化。
    #[serde(skip)]
    pub identifier: String,
    /// 友善名稱。
    pub name: Option<String>,
    /// 接收（下載）bytes/sec ≥ 0。
    pub rx_bps: Option<i64>,
    /// 傳輸（上傳）bytes/sec ≥ 0。
    pub tx_bps: Option<i64>,
}

/// 單張 GPU 的即時使用率與 VRAM（FR-005）。
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GpuReading {
    /// 對應 `gpu_device.id`。
    pub id: i64,
    /// 穩定識別鍵（GPU LUID 字串）；不對前端序列化。
    #[serde(skip)]
    pub identifier: String,
    /// 介面卡名稱（DXGI `AdapterDesc.Description`）。
    pub name: Option<String>,
    /// GPU 使用率 0–100（各 engine 取最大）。
    pub util_pct: Option<f64>,
    /// GPU 記憶體已用位元組（DXGI CurrentUsage）。
    pub mem_used_bytes: Option<i64>,
    /// GPU 記憶體總量位元組（DedicatedVideoMemory）。
    pub mem_total_bytes: Option<i64>,
}

/// 某一取樣時點的整機快照（platform 採集輸出＝IPC 即時檢視酬載）。
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MetricSnapshot {
    /// 取樣瞬時（epoch ms）。
    pub ts_utc: i64,
    pub cpu_pct: Option<f64>,
    pub mem_used_bytes: Option<i64>,
    pub mem_total_bytes: Option<i64>,
    /// 逐顆硬碟。
    pub disks: Vec<DiskReading>,
    /// 逐介面。
    pub nets: Vec<NetReading>,
    /// 逐張 GPU（空陣列＝無 GPU，呈現層顯示「不適用」）。
    pub gpus: Vec<GpuReading>,
}

impl MetricSnapshot {
    /// 取出整機純量部分。
    pub fn sample(&self) -> MetricSample {
        MetricSample {
            ts_utc: self.ts_utc,
            cpu_pct: self.cpu_pct,
            mem_used_bytes: self.mem_used_bytes,
            mem_total_bytes: self.mem_total_bytes,
        }
    }
}

// ---- 速率換算（FR-003/004） ----

/// 由前後兩次累計計數器（bytes）與其時間戳（epoch ms）換算平均速率（bytes/sec）。
///
/// - `dt <= 0`：時間未前進 → `None`（無法換算）。
/// - `curr < prev`：計數器重置／回繞 → `None`（不產生負值或假數據，原則 III）。
pub fn compute_rate_bps(prev: u64, curr: u64, prev_ms: i64, curr_ms: i64) -> Option<i64> {
    let dt = curr_ms - prev_ms;
    if dt <= 0 {
        return None;
    }
    if curr < prev {
        return None;
    }
    let delta = (curr - prev) as i128;
    Some((delta * 1000 / dt as i128) as i64)
}

/// 由已用量與總量推算使用率百分比（0–100）；任一缺值或總量為 0 → `None`。
pub fn ratio_pct(used: Option<i64>, total: Option<i64>) -> Option<f64> {
    match (used, total) {
        (Some(u), Some(t)) if t > 0 => Some((u as f64 / t as f64) * 100.0),
        _ => None,
    }
}

// ---- 降取樣桶粒度（FR-014、data-model.md §4） ----

/// 原始逐筆上限：2 小時。
pub const RAW_MAX_MS: i64 = 7_200_000;
/// 分鐘桶上限：2 天。
pub const MINUTE_MAX_MS: i64 = 172_800_000;
/// 小時桶上限：2 月（以 60 日計）。
pub const HOUR_MAX_MS: i64 = 5_184_000_000;

/// 分鐘桶寬（ms）。
pub const BUCKET_MINUTE_MS: i64 = 60_000;
/// 小時桶寬（ms）。
pub const BUCKET_HOUR_MS: i64 = 3_600_000;
/// 日桶寬（ms）。
pub const BUCKET_DAY_MS: i64 = 86_400_000;

/// 依區間長度 `span_ms`（= toUtc − fromUtc）選擇桶寬（ms）；`0` 表示原始逐筆。
pub fn bucket_ms_for_span(span_ms: i64) -> i64 {
    if span_ms <= RAW_MAX_MS {
        0
    } else if span_ms <= MINUTE_MAX_MS {
        BUCKET_MINUTE_MS
    } else if span_ms <= HOUR_MAX_MS {
        BUCKET_HOUR_MS
    } else {
        BUCKET_DAY_MS
    }
}
