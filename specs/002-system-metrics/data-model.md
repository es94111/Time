# Phase 1 資料模型：電腦資訊監控紀錄（System Metrics）

**日期**：2026-07-01 ｜ **語言**：zh-TW ｜ **儲存**：沿用 001 之 SQLCipher 加密資料庫 `%APPDATA%\ActivityTracker\activity.db`（FR-013）

本檔由規格「關鍵實體」與 FR 導出領域型別、驗證規則與 SQL 結構。時間一律以 **UTC 瞬時（epoch ms）** 儲存，呈現時換算本機時區（沿用 001 慣例）。速率單位一律 **位元組／秒（bytes/sec）**；記憶體與 VRAM 一律 **位元組**。缺值以 **NULL** 表示（FR-007）。

---

## 1. 領域實體

### MetricSample（指標樣本）— 整機純量指標與取樣時間錨
| 欄位 | 型別 | 說明／驗證 |
|------|------|------------|
| id | i64（PK） | 內部識別，各裝置子樣本以此關聯 |
| ts_utc | int（epoch ms） | 取樣瞬時；不可空、單調遞增建索引 |
| cpu_pct | real（nullable） | CPU 整體使用率 0–100（FR-001）；讀取失敗為 NULL |
| mem_used_bytes | int（nullable） | 已用實體記憶體位元組（FR-002） |
| mem_total_bytes | int（nullable） | 實體記憶體總量位元組；記憶體使用率 = used/total |

- 一個 MetricSample 對應「某一時間點對整機的一次量測快照」（規格關鍵實體）。
- 記憶體使用率不另存百分比，由 used/total 推算，避免冗餘與不一致。

### DiskDevice（硬碟裝置）
| 欄位 | 型別 | 說明／驗證 |
|------|------|------------|
| id | i64（PK） | 內部識別 |
| identifier | text | 穩定識別鍵（PDH PhysicalDisk instance，如 `0 C:`）；不可空、唯一 |
| display_name | text（nullable） | 易讀名稱（型號或磁碟區標籤，若可得） |

### DiskSample（硬碟樣本，逐顆）
| 欄位 | 型別 | 說明／驗證 |
|------|------|------------|
| sample_id | i64（FK→MetricSample，ON DELETE CASCADE） | 不可空 |
| disk_id | i64（FK→DiskDevice） | 不可空 |
| read_bps | int（nullable） | 讀取速度 bytes/sec ≥ 0（FR-003） |
| write_bps | int（nullable） | 寫入速度 bytes/sec ≥ 0（FR-003） |

- 主鍵 `(sample_id, disk_id)`；每顆硬碟於每個取樣週期一列，可辨識所屬硬碟。

### NetworkInterface（網路介面）
| 欄位 | 型別 | 說明／驗證 |
|------|------|------------|
| id | i64（PK） | 內部識別 |
| identifier | text | 穩定識別鍵（PDH Network Interface instance）；不可空、唯一 |
| display_name | text（nullable） | 友善名稱（若可由 IP Helper 取得） |

### NetSample（網路樣本，逐介面）
| 欄位 | 型別 | 說明／驗證 |
|------|------|------------|
| sample_id | i64（FK→MetricSample，ON DELETE CASCADE） | 不可空 |
| iface_id | i64（FK→NetworkInterface） | 不可空 |
| rx_bps | int（nullable） | 接收（下載）bytes/sec ≥ 0（FR-004） |
| tx_bps | int（nullable） | 傳輸（上傳）bytes/sec ≥ 0（FR-004） |

- 主鍵 `(sample_id, iface_id)`。整機合計流量 = 同一 sample 各介面 rx/tx 加總（FR-004），呈現層計算、不另存。

### GpuDevice（GPU 裝置）
| 欄位 | 型別 | 說明／驗證 |
|------|------|------------|
| id | i64（PK） | 內部識別 |
| identifier | text | 穩定識別鍵（GPU LUID 字串）；不可空、唯一 |
| display_name | text（nullable） | 介面卡名稱（DXGI `AdapterDesc.Description`） |

### GpuSample（GPU 樣本，逐張）
| 欄位 | 型別 | 說明／驗證 |
|------|------|------------|
| sample_id | i64（FK→MetricSample，ON DELETE CASCADE） | 不可空 |
| gpu_id | i64（FK→GpuDevice） | 不可空 |
| util_pct | real（nullable） | GPU 使用率 0–100（FR-005，各 engine 取最大） |
| mem_used_bytes | int（nullable） | GPU 記憶體已用位元組（DXGI CurrentUsage） |
| mem_total_bytes | int（nullable） | GPU 記憶體總量位元組（DedicatedVideoMemory）；使用率由 used/total 推算（FR-005） |

- 主鍵 `(sample_id, gpu_id)`。GPU 記憶體同時保存已用量與總量（FR-005 澄清）。

### Setting（設定）— 沿用 001 既有 `setting` 表，新增鍵
| 鍵 | 預設 | 說明／驗證 |
|----|------|------------|
| metrics_sample_interval_sec | 1 | 取樣週期秒數（FR-012）；整數，1 ≤ 值 ≤ 3600，超出拒絕並提示 |
| metrics_retention_days | 30 | 保留天數（FR-011）；整數，1 ≤ 值 ≤ 3650，超出拒絕並提示 |
| metrics_enabled | true | 是否啟用背景指標紀錄（FR-015）；停用時暫停取樣與寫入，既有歷史不受影響 |

---

## 2. 缺值與裝置生命週期（呼應原則 III）

- **缺值（FR-007）**：任一指標當次讀取失敗 → 對應欄位寫 NULL，其餘照常寫入；呈現時標示「無資料」，不以 0 假冒。
- **裝置新增（FR-008）**：偵測到新 instance／LUID → 於對應 Device 表新增列，後續樣本開始引用。
- **裝置移除（FR-008、US1 邊界）**：Device 表列與其歷史樣本**保留不動**；後續樣本不再產生該裝置子列。Device 表**不因移除而刪除**。
- **時間軸空缺（US3 邊界）**：睡眠／休眠期間無樣本；相鄰 `ts_utc` 差 ≫ 取樣週期即為空缺，呈現層以「無資料」表示，不內插。

---

## 3. SQL 結構（SQLCipher；於既有資料庫以遷移新增）

```sql
PRAGMA journal_mode = WAL;   -- 沿用 001；降低當機損失
PRAGMA foreign_keys = ON;

CREATE TABLE metric_sample (
  id              INTEGER PRIMARY KEY,
  ts_utc          INTEGER NOT NULL,
  cpu_pct         REAL,
  mem_used_bytes  INTEGER,
  mem_total_bytes INTEGER
);
CREATE INDEX idx_metric_sample_ts ON metric_sample(ts_utc);

CREATE TABLE disk_device (
  id           INTEGER PRIMARY KEY,
  identifier   TEXT NOT NULL UNIQUE,
  display_name TEXT
);
CREATE TABLE disk_sample (
  sample_id INTEGER NOT NULL REFERENCES metric_sample(id) ON DELETE CASCADE,
  disk_id   INTEGER NOT NULL REFERENCES disk_device(id),
  read_bps  INTEGER,
  write_bps INTEGER,
  PRIMARY KEY (sample_id, disk_id)
);

CREATE TABLE network_interface (
  id           INTEGER PRIMARY KEY,
  identifier   TEXT NOT NULL UNIQUE,
  display_name TEXT
);
CREATE TABLE net_sample (
  sample_id INTEGER NOT NULL REFERENCES metric_sample(id) ON DELETE CASCADE,
  iface_id  INTEGER NOT NULL REFERENCES network_interface(id),
  rx_bps    INTEGER,
  tx_bps    INTEGER,
  PRIMARY KEY (sample_id, iface_id)
);

CREATE TABLE gpu_device (
  id           INTEGER PRIMARY KEY,
  identifier   TEXT NOT NULL UNIQUE,
  display_name TEXT
);
CREATE TABLE gpu_sample (
  sample_id       INTEGER NOT NULL REFERENCES metric_sample(id) ON DELETE CASCADE,
  gpu_id          INTEGER NOT NULL REFERENCES gpu_device(id),
  util_pct        REAL,
  mem_used_bytes  INTEGER,
  mem_total_bytes INTEGER,
  PRIMARY KEY (sample_id, gpu_id)
);
```

- `metric_sample(ts_utc)` 索引使區間查詢與降取樣桶聚合高效。
- 子表以 `sample_id` 為外鍵並 `ON DELETE CASCADE`，保留清理只需刪 `metric_sample` 過期列即連帶清除子列（FR-011、R12）。
- 各裝置子表 PK 含 `sample_id`，天然依時間叢集，利於「某裝置某區間」查詢。

---

## 4. 長區間降取樣查詢（FR-014、R10）

呈現層依區間長度選擇桶粒度，對每桶以 `AVG`（趨勢線）與 `MAX`（保留尖峰）聚合。原始樣本不受影響。範例（每小時桶）：

```sql
SELECT (ts_utc / 3600000) AS bucket_h,
       AVG(cpu_pct)  AS cpu_avg,
       MAX(cpu_pct)  AS cpu_max
FROM   metric_sample
WHERE  ts_utc BETWEEN :from AND :to
GROUP  BY bucket_h
ORDER  BY bucket_h;
```

桶粒度依區間長度 `span = toUtc − fromUtc`（毫秒）以固定門檻選擇，避免「月」定義歧義（此處 1 月固定以 30 日計）：

| 條件（毫秒） | 桶粒度 | 桶寬（ms） |
|--------------|--------|-----------|
| `span ≤ 7_200_000`（2 小時） | 原始逐筆 | 0 |
| `span ≤ 172_800_000`（2 天） | 分鐘 | 60_000 |
| `span ≤ 5_184_000_000`（2 月＝60 日） | 小時 | 3_600_000 |
| 更長 | 日 | 86_400_000 |

---

## 5. 對應規格之驗證

| 規格 | 資料模型對應 |
|------|--------------|
| FR-001 CPU 使用率 | `metric_sample.cpu_pct` |
| FR-002 記憶體使用率 | `metric_sample.mem_used_bytes` / `mem_total_bytes` |
| FR-003 逐顆硬碟讀寫 | `disk_device` + `disk_sample(read_bps, write_bps)` |
| FR-004 逐介面收發、合計可加總 | `network_interface` + `net_sample(rx_bps, tx_bps)` |
| FR-005 逐張 GPU 使用率＋VRAM 已用/總量 | `gpu_device` + `gpu_sample(util_pct, mem_used_bytes, mem_total_bytes)` |
| FR-006 時間戳記＋持久化 | `metric_sample.ts_utc` + SQLCipher 持久化 |
| FR-007 缺值不中斷 | 各指標欄位 nullable，NULL 表缺值 |
| FR-008 裝置增減 | Device 表新增列／保留舊列，不刪除 |
| FR-011 保留天數 1–3650、自動清除 | `setting.metrics_retention_days` + CASCADE 清理 |
| FR-012 取樣週期 1–3600 | `setting.metrics_sample_interval_sec` |
| FR-013 沿用加密儲存 | 同一 SQLCipher 資料庫、同一 DPAPI 金鑰 |
| FR-014 長區間降取樣 | 呈現層桶聚合查詢；原始樣本保留 |
| FR-015 啟用／停用開關 | `setting.metrics_enabled`；停用時服務暫停取樣 |
