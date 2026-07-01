//! 指標儲存（T008、T016、T021、T026、T030、T033）：設定、寫入樣本、
//! 裝置歸戶、即時快照、區間降取樣查詢、保留清理與清除。
//!
//! 沿用既有加密資料庫（同一 SQLCipher 連線，FR-013）。時間為 epoch ms；
//! 速率 bytes/sec；缺值以 NULL 儲存（原則 III）。

use rusqlite::{params, OptionalExtension};
use serde::{Deserialize, Serialize};
use tracker_core::metrics::{
    bucket_ms_for_span, DiskReading, GpuReading, MetricSnapshot, NetReading,
};

use crate::db::Database;
use crate::error::Result;

/// 指標設定（contracts §4）；沿用既有 `setting` 表之鍵。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MetricsSettings {
    pub sample_interval_sec: i64,
    pub retention_days: i64,
    pub enabled: bool,
}

/// 指標設定部分更新（僅傳要修改的欄位）。
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MetricsSettingsPatch {
    pub sample_interval_sec: Option<i64>,
    pub retention_days: Option<i64>,
    pub enabled: Option<bool>,
}

/// 單一裝置的識別（供裝置清單與圖例，contracts §3）。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DeviceInfo {
    pub id: i64,
    pub name: Option<String>,
}

/// 裝置清單（含已移除，FR-008 / SC-004）。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DeviceLists {
    pub disks: Vec<DeviceInfo>,
    pub nets: Vec<DeviceInfo>,
    pub gpus: Vec<DeviceInfo>,
}

// ---- 降取樣趨勢（contracts §2） ----

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TrendDisk {
    pub id: i64,
    pub name: Option<String>,
    pub read_avg: Option<f64>,
    pub read_max: Option<i64>,
    pub write_avg: Option<f64>,
    pub write_max: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TrendNet {
    pub id: i64,
    pub name: Option<String>,
    pub rx_avg: Option<f64>,
    pub rx_max: Option<i64>,
    pub tx_avg: Option<f64>,
    pub tx_max: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TrendGpu {
    pub id: i64,
    pub name: Option<String>,
    pub util_avg: Option<f64>,
    pub util_max: Option<f64>,
    pub mem_used_avg: Option<f64>,
    pub mem_total_bytes: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TrendPoint {
    pub ts_utc: i64,
    pub cpu_avg: Option<f64>,
    pub cpu_max: Option<f64>,
    pub mem_used_avg: Option<f64>,
    pub mem_total_bytes: Option<i64>,
    pub disks: Vec<TrendDisk>,
    pub nets: Vec<TrendNet>,
    pub gpus: Vec<TrendGpu>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TrendSeries {
    /// 實際採用之桶粒度（ms）；0＝原始逐筆。
    pub bucket_ms: i64,
    pub points: Vec<TrendPoint>,
}

/// 指標 Repository：對既有 [`Database`] 的指標查詢與寫入。
pub struct MetricsRepo<'a> {
    db: &'a Database,
}

impl<'a> MetricsRepo<'a> {
    /// 建立指標 Repository。
    pub fn new(db: &'a Database) -> Self {
        Self { db }
    }

    // ---- 設定（T008、FR-011/012/015） ----

    /// 讀取指標設定（含預設回退）。
    pub fn get_metrics_settings(&self) -> Result<MetricsSettings> {
        Ok(MetricsSettings {
            sample_interval_sec: self.get_int("metrics_sample_interval_sec", 1),
            retention_days: self.get_int("metrics_retention_days", 30),
            enabled: self.get_str("metrics_enabled")? != "false",
        })
    }

    /// 套用指標設定部分更新（不含界限驗證；驗證於命令層依 contracts 進行）。
    pub fn set_metrics_settings(&self, patch: &MetricsSettingsPatch) -> Result<MetricsSettings> {
        if let Some(v) = patch.sample_interval_sec {
            self.set("metrics_sample_interval_sec", &v.to_string())?;
        }
        if let Some(v) = patch.retention_days {
            self.set("metrics_retention_days", &v.to_string())?;
        }
        if let Some(v) = patch.enabled {
            self.set("metrics_enabled", &v.to_string())?;
        }
        self.get_metrics_settings()
    }

    fn get_str(&self, key: &str) -> Result<String> {
        let v = self
            .db
            .conn()
            .query_row("SELECT value FROM setting WHERE key = ?1", params![key], |r| {
                r.get::<_, String>(0)
            })
            .optional()?
            .unwrap_or_default();
        Ok(v)
    }

    fn get_int(&self, key: &str, default: i64) -> i64 {
        self.get_str(key).ok().and_then(|s| s.parse().ok()).unwrap_or(default)
    }

    fn set(&self, key: &str, value: &str) -> Result<()> {
        self.db.conn().execute(
            "INSERT INTO setting(key, value) VALUES(?1, ?2)\n\
             ON CONFLICT(key) DO UPDATE SET value = ?2",
            params![key, value],
        )?;
        Ok(())
    }

    // ---- 裝置 upsert（依 identifier 歸戶，FR-008） ----

    fn upsert_device(&self, table: &str, identifier: &str, name: Option<&str>) -> Result<i64> {
        let conn = self.db.conn();
        // display_name 僅在提供時更新，避免以 NULL 覆蓋既有名稱。
        let sql = format!(
            "INSERT INTO {table}(identifier, display_name) VALUES(?1, ?2)\n\
             ON CONFLICT(identifier) DO UPDATE SET display_name = COALESCE(?2, display_name)"
        );
        conn.execute(&sql, params![identifier, name])?;
        let id = conn.query_row(
            &format!("SELECT id FROM {table} WHERE identifier = ?1"),
            params![identifier],
            |r| r.get::<_, i64>(0),
        )?;
        Ok(id)
    }

    // ---- 寫入樣本（T016、FR-006/007/008） ----

    /// 寫入一次整機快照（`metric_sample` + 各裝置子列），缺值寫 NULL。回傳 sample_id。
    pub fn insert_snapshot(&self, snap: &MetricSnapshot) -> Result<i64> {
        let conn = self.db.conn();
        conn.execute(
            "INSERT INTO metric_sample(ts_utc, cpu_pct, mem_used_bytes, mem_total_bytes)\n\
             VALUES(?1, ?2, ?3, ?4)",
            params![snap.ts_utc, snap.cpu_pct, snap.mem_used_bytes, snap.mem_total_bytes],
        )?;
        let sample_id = conn.last_insert_rowid();

        for d in &snap.disks {
            let disk_id = self.upsert_device("disk_device", &d.identifier, d.name.as_deref())?;
            conn.execute(
                "INSERT OR IGNORE INTO disk_sample(sample_id, disk_id, read_bps, write_bps)\n\
                 VALUES(?1, ?2, ?3, ?4)",
                params![sample_id, disk_id, d.read_bps, d.write_bps],
            )?;
        }
        for n in &snap.nets {
            let iface_id =
                self.upsert_device("network_interface", &n.identifier, n.name.as_deref())?;
            conn.execute(
                "INSERT OR IGNORE INTO net_sample(sample_id, iface_id, rx_bps, tx_bps)\n\
                 VALUES(?1, ?2, ?3, ?4)",
                params![sample_id, iface_id, n.rx_bps, n.tx_bps],
            )?;
        }
        for g in &snap.gpus {
            let gpu_id = self.upsert_device("gpu_device", &g.identifier, g.name.as_deref())?;
            conn.execute(
                "INSERT OR IGNORE INTO gpu_sample(sample_id, gpu_id, util_pct, mem_used_bytes, mem_total_bytes)\n\
                 VALUES(?1, ?2, ?3, ?4, ?5)",
                params![sample_id, gpu_id, g.util_pct, g.mem_used_bytes, g.mem_total_bytes],
            )?;
        }
        Ok(sample_id)
    }

    // ---- 即時快照（US2 / FR-009） ----

    /// 讀取最近一次取樣的整機快照（含裝置 id 與名稱）；尚無樣本時為 `None`。
    pub fn get_latest_snapshot(&self) -> Result<Option<MetricSnapshot>> {
        let conn = self.db.conn();
        let head = conn
            .query_row(
                "SELECT id, ts_utc, cpu_pct, mem_used_bytes, mem_total_bytes\n\
                 FROM metric_sample ORDER BY ts_utc DESC, id DESC LIMIT 1",
                [],
                |r| {
                    Ok((
                        r.get::<_, i64>(0)?,
                        r.get::<_, i64>(1)?,
                        r.get::<_, Option<f64>>(2)?,
                        r.get::<_, Option<i64>>(3)?,
                        r.get::<_, Option<i64>>(4)?,
                    ))
                },
            )
            .optional()?;
        let (sample_id, ts_utc, cpu_pct, mem_used, mem_total) = match head {
            Some(v) => v,
            None => return Ok(None),
        };

        let mut disks = Vec::new();
        {
            let mut stmt = conn.prepare(
                "SELECT d.id, d.display_name, s.read_bps, s.write_bps\n\
                 FROM disk_sample s JOIN disk_device d ON d.id = s.disk_id\n\
                 WHERE s.sample_id = ?1 ORDER BY d.id",
            )?;
            let rows = stmt.query_map(params![sample_id], |r| {
                Ok(DiskReading {
                    id: r.get(0)?,
                    identifier: String::new(),
                    name: r.get(1)?,
                    read_bps: r.get(2)?,
                    write_bps: r.get(3)?,
                })
            })?;
            for row in rows {
                disks.push(row?);
            }
        }

        let mut nets = Vec::new();
        {
            let mut stmt = conn.prepare(
                "SELECT i.id, i.display_name, s.rx_bps, s.tx_bps\n\
                 FROM net_sample s JOIN network_interface i ON i.id = s.iface_id\n\
                 WHERE s.sample_id = ?1 ORDER BY i.id",
            )?;
            let rows = stmt.query_map(params![sample_id], |r| {
                Ok(NetReading {
                    id: r.get(0)?,
                    identifier: String::new(),
                    name: r.get(1)?,
                    rx_bps: r.get(2)?,
                    tx_bps: r.get(3)?,
                })
            })?;
            for row in rows {
                nets.push(row?);
            }
        }

        let mut gpus = Vec::new();
        {
            let mut stmt = conn.prepare(
                "SELECT g.id, g.display_name, s.util_pct, s.mem_used_bytes, s.mem_total_bytes\n\
                 FROM gpu_sample s JOIN gpu_device g ON g.id = s.gpu_id\n\
                 WHERE s.sample_id = ?1 ORDER BY g.id",
            )?;
            let rows = stmt.query_map(params![sample_id], |r| {
                Ok(GpuReading {
                    id: r.get(0)?,
                    identifier: String::new(),
                    name: r.get(1)?,
                    util_pct: r.get(2)?,
                    mem_used_bytes: r.get(3)?,
                    mem_total_bytes: r.get(4)?,
                })
            })?;
            for row in rows {
                gpus.push(row?);
            }
        }

        Ok(Some(MetricSnapshot {
            ts_utc,
            cpu_pct,
            mem_used_bytes: mem_used,
            mem_total_bytes: mem_total,
            disks,
            nets,
            gpus,
        }))
    }

    // ---- 裝置清單（T021、FR-008） ----

    /// 列出歷史中出現過的所有裝置（含已移除）。
    pub fn list_devices(&self) -> Result<DeviceLists> {
        Ok(DeviceLists {
            disks: self.device_table("disk_device")?,
            nets: self.device_table("network_interface")?,
            gpus: self.device_table("gpu_device")?,
        })
    }

    fn device_table(&self, table: &str) -> Result<Vec<DeviceInfo>> {
        let conn = self.db.conn();
        let mut stmt =
            conn.prepare(&format!("SELECT id, display_name FROM {table} ORDER BY id"))?;
        let rows = stmt
            .query_map([], |r| Ok(DeviceInfo { id: r.get(0)?, name: r.get(1)? }))?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        Ok(rows)
    }

    // ---- 區間降取樣查詢（T026、FR-010/014） ----

    /// 依時間區間回傳趨勢；長區間自動降取樣（AVG 趨勢＋MAX 尖峰），空缺桶不補點。
    pub fn get_metrics_range(&self, from_utc: i64, to_utc: i64) -> Result<TrendSeries> {
        let span = to_utc - from_utc;
        let bucket_ms = bucket_ms_for_span(span);
        // 原始逐筆時以 1ms 分桶，等效於每筆自成一桶（回報 bucket_ms=0）。
        let gbw = if bucket_ms == 0 { 1 } else { bucket_ms };
        let conn = self.db.conn();

        use std::collections::BTreeMap;
        let mut points: BTreeMap<i64, TrendPoint> = BTreeMap::new();

        // 整機純量。
        {
            let mut stmt = conn.prepare(
                "SELECT ts_utc / ?3 AS b, AVG(cpu_pct), MAX(cpu_pct),\n\
                        AVG(mem_used_bytes), MAX(mem_total_bytes)\n\
                 FROM metric_sample WHERE ts_utc BETWEEN ?1 AND ?2\n\
                 GROUP BY b ORDER BY b",
            )?;
            let rows = stmt.query_map(params![from_utc, to_utc, gbw], |r| {
                Ok((
                    r.get::<_, i64>(0)?,
                    r.get::<_, Option<f64>>(1)?,
                    r.get::<_, Option<f64>>(2)?,
                    r.get::<_, Option<f64>>(3)?,
                    r.get::<_, Option<i64>>(4)?,
                ))
            })?;
            for row in rows {
                let (b, cpu_avg, cpu_max, mem_used_avg, mem_total) = row?;
                points.insert(
                    b,
                    TrendPoint {
                        ts_utc: b * gbw,
                        cpu_avg,
                        cpu_max,
                        mem_used_avg,
                        mem_total_bytes: mem_total,
                        disks: Vec::new(),
                        nets: Vec::new(),
                        gpus: Vec::new(),
                    },
                );
            }
        }

        // 硬碟。
        {
            let mut stmt = conn.prepare(
                "SELECT ms.ts_utc / ?3 AS b, d.id, d.display_name,\n\
                        AVG(s.read_bps), MAX(s.read_bps), AVG(s.write_bps), MAX(s.write_bps)\n\
                 FROM disk_sample s\n\
                 JOIN metric_sample ms ON ms.id = s.sample_id\n\
                 JOIN disk_device d ON d.id = s.disk_id\n\
                 WHERE ms.ts_utc BETWEEN ?1 AND ?2\n\
                 GROUP BY b, d.id ORDER BY b, d.id",
            )?;
            let rows = stmt.query_map(params![from_utc, to_utc, gbw], |r| {
                Ok((
                    r.get::<_, i64>(0)?,
                    TrendDisk {
                        id: r.get(1)?,
                        name: r.get(2)?,
                        read_avg: r.get(3)?,
                        read_max: r.get(4)?,
                        write_avg: r.get(5)?,
                        write_max: r.get(6)?,
                    },
                ))
            })?;
            for row in rows {
                let (b, disk) = row?;
                if let Some(p) = points.get_mut(&b) {
                    p.disks.push(disk);
                }
            }
        }

        // 網路。
        {
            let mut stmt = conn.prepare(
                "SELECT ms.ts_utc / ?3 AS b, i.id, i.display_name,\n\
                        AVG(s.rx_bps), MAX(s.rx_bps), AVG(s.tx_bps), MAX(s.tx_bps)\n\
                 FROM net_sample s\n\
                 JOIN metric_sample ms ON ms.id = s.sample_id\n\
                 JOIN network_interface i ON i.id = s.iface_id\n\
                 WHERE ms.ts_utc BETWEEN ?1 AND ?2\n\
                 GROUP BY b, i.id ORDER BY b, i.id",
            )?;
            let rows = stmt.query_map(params![from_utc, to_utc, gbw], |r| {
                Ok((
                    r.get::<_, i64>(0)?,
                    TrendNet {
                        id: r.get(1)?,
                        name: r.get(2)?,
                        rx_avg: r.get(3)?,
                        rx_max: r.get(4)?,
                        tx_avg: r.get(5)?,
                        tx_max: r.get(6)?,
                    },
                ))
            })?;
            for row in rows {
                let (b, net) = row?;
                if let Some(p) = points.get_mut(&b) {
                    p.nets.push(net);
                }
            }
        }

        // GPU。
        {
            let mut stmt = conn.prepare(
                "SELECT ms.ts_utc / ?3 AS b, g.id, g.display_name,\n\
                        AVG(s.util_pct), MAX(s.util_pct), AVG(s.mem_used_bytes), MAX(s.mem_total_bytes)\n\
                 FROM gpu_sample s\n\
                 JOIN metric_sample ms ON ms.id = s.sample_id\n\
                 JOIN gpu_device g ON g.id = s.gpu_id\n\
                 WHERE ms.ts_utc BETWEEN ?1 AND ?2\n\
                 GROUP BY b, g.id ORDER BY b, g.id",
            )?;
            let rows = stmt.query_map(params![from_utc, to_utc, gbw], |r| {
                Ok((
                    r.get::<_, i64>(0)?,
                    TrendGpu {
                        id: r.get(1)?,
                        name: r.get(2)?,
                        util_avg: r.get(3)?,
                        util_max: r.get(4)?,
                        mem_used_avg: r.get(5)?,
                        mem_total_bytes: r.get(6)?,
                    },
                ))
            })?;
            for row in rows {
                let (b, gpu) = row?;
                if let Some(p) = points.get_mut(&b) {
                    p.gpus.push(gpu);
                }
            }
        }

        Ok(TrendSeries { bucket_ms, points: points.into_values().collect() })
    }

    // ---- 保留清理與清除（T030、T033、FR-011） ----

    /// 依保留天數刪除過期樣本；CASCADE 連帶清除子列。回傳刪除的 `metric_sample` 列數。
    pub fn purge_expired(&self, now_ms: i64, retention_days: i64) -> Result<u64> {
        let cutoff = now_ms - retention_days.max(1) * 86_400_000;
        let deleted = self
            .db
            .conn()
            .execute("DELETE FROM metric_sample WHERE ts_utc < ?1", params![cutoff])?;
        Ok(deleted as u64)
    }

    /// 清除指標樣本（`before_utc` 省略＝全刪；提供＝刪除該時點前）。回傳刪除列數。
    pub fn clear_metrics(&self, before_utc: Option<i64>) -> Result<u64> {
        let conn = self.db.conn();
        let deleted = match before_utc {
            Some(before) => {
                conn.execute("DELETE FROM metric_sample WHERE ts_utc < ?1", params![before])?
            }
            None => conn.execute("DELETE FROM metric_sample", [])?,
        };
        self.db.checkpoint()?;
        Ok(deleted as u64)
    }
}
