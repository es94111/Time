//! 指標儲存整合測試（T010、T025）：寫入與歸戶、缺值 NULL、降取樣桶聚合。

use std::path::PathBuf;

use tracker_core::metrics::{DiskReading, MetricSnapshot};
use tracker_core::ports::{PortError, SecretStore};
use tracker_storage::metrics_repo::MetricsRepo;
use tracker_storage::{crypto, Database};

/// 測試用機密保護（恆等）。
struct FakeSecret;
impl SecretStore for FakeSecret {
    fn protect(&self, p: &[u8]) -> Result<Vec<u8>, PortError> {
        Ok(p.to_vec())
    }
    fn unprotect(&self, c: &[u8]) -> Result<Vec<u8>, PortError> {
        Ok(c.to_vec())
    }
}

fn unique_temp_dir() -> PathBuf {
    let mut d = std::env::temp_dir();
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    d.push(format!("tracker_metrics_test_{nanos}"));
    std::fs::create_dir_all(&d).unwrap();
    d
}

fn open_db(dir: &std::path::Path) -> Database {
    let key = dir.join("key.json");
    let db_path = dir.join("activity.db");
    let dek = crypto::load_or_create_dek(&key, &FakeSecret, None).unwrap();
    Database::open(&db_path, &dek).unwrap()
}

fn disk(identifier: &str, name: &str, r: Option<i64>, w: Option<i64>) -> DiskReading {
    DiskReading {
        id: 0,
        identifier: identifier.into(),
        name: Some(name.into()),
        read_bps: r,
        write_bps: w,
    }
}

fn snapshot(ts: i64, cpu: Option<f64>, disks: Vec<DiskReading>) -> MetricSnapshot {
    MetricSnapshot {
        ts_utc: ts,
        cpu_pct: cpu,
        mem_used_bytes: Some(8_000_000_000),
        mem_total_bytes: Some(16_000_000_000),
        disks,
        nets: Vec::new(),
        gpus: Vec::new(),
    }
}

#[test]
fn insert_snapshot_with_children_and_device_attribution() {
    let dir = unique_temp_dir();
    let db = open_db(&dir);
    let repo = MetricsRepo::new(&db);

    // 兩次取樣，同一硬碟 identifier → 應歸戶為同一 disk_device（不重複）。
    repo.insert_snapshot(&snapshot(
        1_000,
        Some(25.0),
        vec![
            disk("0 C:", "系統碟", Some(1000), Some(500)),
            disk("1 D:", "資料碟", Some(0), None), // write 缺值 → NULL
        ],
    ))
    .unwrap();
    repo.insert_snapshot(&snapshot(2_000, None, vec![disk("0 C:", "系統碟", Some(2000), Some(0))]))
        .unwrap();

    // 裝置歸戶：同 identifier 不重複，共 2 顆硬碟。
    let devices = repo.list_devices().unwrap();
    assert_eq!(devices.disks.len(), 2);

    // 最新快照為 ts=2000，含 CPU 缺值（None）與單一硬碟子列。
    let latest = repo.get_latest_snapshot().unwrap().expect("應有樣本");
    assert_eq!(latest.ts_utc, 2_000);
    assert!(latest.cpu_pct.is_none(), "CPU 缺值應為 None（NULL）");
    assert_eq!(latest.disks.len(), 1);
    assert_eq!(latest.disks[0].read_bps, Some(2000));
    assert_eq!(latest.mem_used_bytes, Some(8_000_000_000));
}

#[test]
fn missing_values_are_stored_as_null() {
    let dir = unique_temp_dir();
    let db = open_db(&dir);
    let repo = MetricsRepo::new(&db);

    // D: 於 ts=1000 write 缺值；歷史查詢應反映 NULL（MAX 忽略 NULL）。
    repo.insert_snapshot(&snapshot(1_000, Some(10.0), vec![disk("1 D:", "資料碟", None, None)]))
        .unwrap();
    let series = repo.get_metrics_range(0, 5_000).unwrap();
    assert_eq!(series.bucket_ms, 0, "短區間為原始逐筆");
    assert_eq!(series.points.len(), 1);
    let d = &series.points[0].disks[0];
    assert!(d.read_max.is_none());
    assert!(d.write_max.is_none());
}

#[test]
fn downsampling_avg_max_and_empty_buckets() {
    let dir = unique_temp_dir();
    let db = open_db(&dir);
    let repo = MetricsRepo::new(&db);

    // 於同一分鐘桶內放兩筆、下一分鐘桶放一筆，中間留一整分鐘空缺。
    // 選一段 > 2 小時的區間以觸發分鐘桶（span 落在 (2h, 2d]）。
    let base = 100_000_000; // 任意基準（epoch ms）
    let minute = 60_000;
    // 桶 A（base 所屬分鐘）：cpu 20、40 → avg 30、max 40。
    repo.insert_snapshot(&snapshot(base + 1_000, Some(20.0), vec![])).unwrap();
    repo.insert_snapshot(&snapshot(base + 2_000, Some(40.0), vec![])).unwrap();
    // 桶 C（base + 2 分鐘）：cpu 50。中間桶 B 無資料 → 不應產生點。
    repo.insert_snapshot(&snapshot(base + 2 * minute + 1_000, Some(50.0), vec![])).unwrap();

    // 區間長度 3 小時 → 觸發分鐘桶（bucket_ms = 60_000）。
    let from = base - 3_600_000;
    let to = base + 3 * 3_600_000;
    let series = repo.get_metrics_range(from, to).unwrap();
    assert_eq!(series.bucket_ms, 60_000, "應採分鐘桶");

    // 只有兩個有資料的桶（A 與 C），空缺桶 B 不產生點（不內插）。
    assert_eq!(series.points.len(), 2);

    let a = &series.points[0];
    assert_eq!(a.cpu_avg, Some(30.0));
    assert_eq!(a.cpu_max, Some(40.0));

    let c = &series.points[1];
    assert_eq!(c.cpu_avg, Some(50.0));
    assert_eq!(c.cpu_max, Some(50.0));

    // 兩桶時間差應為 2 分鐘（中間空缺桶被略過）。
    assert_eq!(c.ts_utc - a.ts_utc, 2 * minute);
}

#[test]
fn purge_and_clear_cascade_children() {
    let dir = unique_temp_dir();
    let db = open_db(&dir);
    let repo = MetricsRepo::new(&db);

    let day = 86_400_000i64;
    let now = 100 * day;
    // 一筆很舊（40 天前）、一筆很新。
    repo.insert_snapshot(&snapshot(now - 40 * day, Some(1.0), vec![disk("0 C:", "系統碟", Some(1), Some(1))]))
        .unwrap();
    repo.insert_snapshot(&snapshot(now, Some(2.0), vec![disk("0 C:", "系統碟", Some(2), Some(2))]))
        .unwrap();

    // 保留 30 天 → 應刪除 1 筆過期樣本（CASCADE 清子列）。
    let purged = repo.purge_expired(now, 30).unwrap();
    assert_eq!(purged, 1);

    // 全部清除 → 刪除剩餘 1 筆。
    let cleared = repo.clear_metrics(None).unwrap();
    assert_eq!(cleared, 1);
    assert!(repo.get_latest_snapshot().unwrap().is_none());
}
