//! tracker-platform：Windows 原廠 API 整合（windows crate）。
//!
//! 各模組實作 `tracker-core::ports` 之 trait，使服務層可注入真實的 Windows 來源；
//! 於非 Windows 平台僅提供占位，邏輯測試以核心的假實作進行。

#[cfg(windows)]
pub mod autostart;
#[cfg(windows)]
pub mod browser_url;
/// 裝置指紋（跨平台可測試：trait + 假來源恆可用，真實登錄檔實作於 Windows 限定，T019/T022）。
pub mod device_id;
#[cfg(windows)]
pub mod foreground;
#[cfg(windows)]
pub mod idle;
#[cfg(windows)]
pub mod metrics_gpu;
#[cfg(windows)]
pub mod metrics_mem;
#[cfg(windows)]
pub mod metrics_pdh;
#[cfg(windows)]
pub mod process;
#[cfg(windows)]
pub mod secret;
#[cfg(windows)]
pub mod session_power;

use tracker_core::metrics::MetricSnapshot;

/// 平台指標採集抽象（T007）：回傳單次整機快照。
///
/// 供 Windows 原廠實作與非 Windows 假實作共用，使服務層與 CI 可跨平台編譯／測試
/// （呼應 plan.md 測試策略）。
pub trait MetricsSampler: Send {
    /// 採集一次整機指標快照（含當下時間戳）。
    fn sample(&mut self) -> MetricSnapshot;
}

/// 建立本平台預設取樣器；Windows 上為原廠 PDH/DXGI 實作，其餘為假實作。
pub fn default_sampler() -> Box<dyn MetricsSampler> {
    #[cfg(windows)]
    {
        Box::new(win_metrics::WindowsSampler::new())
    }
    #[cfg(not(windows))]
    {
        Box::new(FakeSampler::new())
    }
}

/// 非 Windows 假實作：回傳僅含時間戳的空快照（供 CI 與非平台環境）。
#[cfg(not(windows))]
pub struct FakeSampler;

#[cfg(not(windows))]
impl FakeSampler {
    /// 建立假取樣器。
    pub fn new() -> Self {
        Self
    }
}

#[cfg(not(windows))]
impl Default for FakeSampler {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(not(windows))]
impl MetricsSampler for FakeSampler {
    fn sample(&mut self) -> MetricSnapshot {
        MetricSnapshot { ts_utc: tracker_core::time::now_ms(), ..Default::default() }
    }
}

/// Windows 原廠指標採集（T015）：整合 PDH（CPU／磁碟／網路／GPU Engine）、
/// `GlobalMemoryStatusEx`（記憶體）與 DXGI（VRAM）為單次 [`MetricSnapshot`]。
#[cfg(windows)]
mod win_metrics {
    use std::collections::HashMap;

    use tracker_core::metrics::{DiskReading, GpuReading, MetricSnapshot, NetReading};

    use crate::metrics_gpu::GpuSampler;
    use crate::metrics_pdh::PdhSampler;
    use crate::{metrics_mem, MetricsSampler};

    /// 將 bytes/sec 之 f64 讀數轉為非負 i64。
    fn bps(v: f64) -> Option<i64> {
        if v.is_finite() && v >= 0.0 {
            Some(v.round() as i64)
        } else {
            None
        }
    }

    /// 將使用率 f64（0–100）夾到合理範圍。
    fn pct(v: f64) -> Option<f64> {
        if v.is_finite() {
            Some(v.clamp(0.0, 100.0))
        } else {
            None
        }
    }

    pub struct WindowsSampler {
        pdh: Option<PdhSampler>,
        gpu: Option<GpuSampler>,
    }

    impl WindowsSampler {
        pub fn new() -> Self {
            Self { pdh: PdhSampler::new(), gpu: GpuSampler::new() }
        }
    }

    impl MetricsSampler for WindowsSampler {
        fn sample(&mut self) -> MetricSnapshot {
            let ts_utc = tracker_core::time::now_ms();
            let (mem_used_bytes, mem_total_bytes) = metrics_mem::read();

            let readings = self.pdh.as_mut().map(|p| p.collect()).unwrap_or_default();

            // 磁碟：以實例名合併讀／寫。
            let mut disk_map: HashMap<String, (Option<i64>, Option<i64>)> = HashMap::new();
            for (name, v) in readings.disk_read {
                disk_map.entry(name).or_default().0 = bps(v);
            }
            for (name, v) in readings.disk_write {
                disk_map.entry(name).or_default().1 = bps(v);
            }
            let mut disks: Vec<DiskReading> = disk_map
                .into_iter()
                .map(|(identifier, (read_bps, write_bps))| DiskReading {
                    id: 0,
                    name: Some(identifier.clone()),
                    identifier,
                    read_bps,
                    write_bps,
                })
                .collect();
            disks.sort_by(|a, b| a.identifier.cmp(&b.identifier));

            // 網路：以實例名合併收／發。
            let mut net_map: HashMap<String, (Option<i64>, Option<i64>)> = HashMap::new();
            for (name, v) in readings.net_rx {
                net_map.entry(name).or_default().0 = bps(v);
            }
            for (name, v) in readings.net_tx {
                net_map.entry(name).or_default().1 = bps(v);
            }
            let mut nets: Vec<NetReading> = net_map
                .into_iter()
                .map(|(identifier, (rx_bps, tx_bps))| NetReading {
                    id: 0,
                    name: Some(identifier.clone()),
                    identifier,
                    rx_bps,
                    tx_bps,
                })
                .collect();
            nets.sort_by(|a, b| a.identifier.cmp(&b.identifier));

            // GPU：以 DXGI 列舉為主，GPU Engine 使用率依 LUID 歸戶（各 engine 取最大）。
            let gpu_infos = self.gpu.as_ref().map(|g| g.read()).unwrap_or_default();
            let gpus: Vec<GpuReading> = gpu_infos
                .into_iter()
                .map(|info| {
                    let key = info.luid_key.to_ascii_lowercase();
                    let util = readings
                        .gpu_util
                        .iter()
                        .filter(|(inst, _)| inst.to_ascii_lowercase().contains(&key))
                        .map(|(_, v)| *v)
                        .fold(None::<f64>, |acc, v| Some(acc.map_or(v, |a| a.max(v))))
                        .and_then(pct);
                    GpuReading {
                        id: 0,
                        identifier: info.luid_key,
                        name: Some(info.name),
                        util_pct: util,
                        mem_used_bytes: info.mem_used_bytes,
                        mem_total_bytes: info.mem_total_bytes,
                    }
                })
                .collect();

            MetricSnapshot {
                ts_utc,
                cpu_pct: readings.cpu_pct.and_then(pct),
                mem_used_bytes,
                mem_total_bytes,
                disks,
                nets,
                gpus,
            }
        }
    }
}
