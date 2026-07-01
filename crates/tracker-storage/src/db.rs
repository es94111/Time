//! 加密資料庫啟動與遷移（T011）。
//!
//! 以 SQLCipher 原始 256-bit 金鑰開啟，套用安全 PRAGMA 與 WAL（FR-010、FR-011、FR-016）。

use std::path::Path;

use rusqlite::Connection;

use crate::crypto::dek_to_sqlcipher_key;
use crate::error::{Result, StorageError};

pub use crate::error::StorageError as Error;

/// 資料庫遷移 SQL（對應 data-model.md）。
const MIGRATION_SQL: &str = r#"
CREATE TABLE IF NOT EXISTS application (
  id           INTEGER PRIMARY KEY,
  display_name TEXT NOT NULL,
  executable   TEXT NOT NULL UNIQUE,
  is_browser   INTEGER NOT NULL DEFAULT 0,
  excluded     INTEGER NOT NULL DEFAULT 0
);

CREATE TABLE IF NOT EXISTS website (
  id        INTEGER PRIMARY KEY,
  hostname  TEXT NOT NULL UNIQUE,
  excluded  INTEGER NOT NULL DEFAULT 0
);

CREATE TABLE IF NOT EXISTS session (
  id             INTEGER PRIMARY KEY,
  application_id INTEGER NOT NULL REFERENCES application(id),
  website_id     INTEGER REFERENCES website(id),
  start_utc      INTEGER NOT NULL,
  end_utc        INTEGER NOT NULL,
  active_ms      INTEGER NOT NULL,
  local_date     TEXT NOT NULL,
  CHECK (end_utc > start_utc),
  CHECK (active_ms >= 0 AND active_ms <= end_utc - start_utc)
);
CREATE INDEX IF NOT EXISTS idx_session_date      ON session(local_date);
CREATE INDEX IF NOT EXISTS idx_session_app_date  ON session(application_id, local_date);
CREATE INDEX IF NOT EXISTS idx_session_site_date ON session(website_id, local_date);

CREATE TABLE IF NOT EXISTS exclusion (
  id      INTEGER PRIMARY KEY,
  kind    TEXT NOT NULL CHECK (kind IN ('app','website')),
  pattern TEXT NOT NULL,
  UNIQUE(kind, pattern)
);

CREATE TABLE IF NOT EXISTS setting (
  key   TEXT PRIMARY KEY,
  value TEXT NOT NULL
);

-- 系統指標資料表（002-system-metrics，對應 data-model.md §3）。
CREATE TABLE IF NOT EXISTS metric_sample (
  id              INTEGER PRIMARY KEY,
  ts_utc          INTEGER NOT NULL,
  cpu_pct         REAL,
  mem_used_bytes  INTEGER,
  mem_total_bytes INTEGER
);
CREATE INDEX IF NOT EXISTS idx_metric_sample_ts ON metric_sample(ts_utc);

CREATE TABLE IF NOT EXISTS disk_device (
  id           INTEGER PRIMARY KEY,
  identifier   TEXT NOT NULL UNIQUE,
  display_name TEXT
);
CREATE TABLE IF NOT EXISTS disk_sample (
  sample_id INTEGER NOT NULL REFERENCES metric_sample(id) ON DELETE CASCADE,
  disk_id   INTEGER NOT NULL REFERENCES disk_device(id),
  read_bps  INTEGER,
  write_bps INTEGER,
  PRIMARY KEY (sample_id, disk_id)
);

CREATE TABLE IF NOT EXISTS network_interface (
  id           INTEGER PRIMARY KEY,
  identifier   TEXT NOT NULL UNIQUE,
  display_name TEXT
);
CREATE TABLE IF NOT EXISTS net_sample (
  sample_id INTEGER NOT NULL REFERENCES metric_sample(id) ON DELETE CASCADE,
  iface_id  INTEGER NOT NULL REFERENCES network_interface(id),
  rx_bps    INTEGER,
  tx_bps    INTEGER,
  PRIMARY KEY (sample_id, iface_id)
);

CREATE TABLE IF NOT EXISTS gpu_device (
  id           INTEGER PRIMARY KEY,
  identifier   TEXT NOT NULL UNIQUE,
  display_name TEXT
);
CREATE TABLE IF NOT EXISTS gpu_sample (
  sample_id       INTEGER NOT NULL REFERENCES metric_sample(id) ON DELETE CASCADE,
  gpu_id          INTEGER NOT NULL REFERENCES gpu_device(id),
  util_pct        REAL,
  mem_used_bytes  INTEGER,
  mem_total_bytes INTEGER,
  PRIMARY KEY (sample_id, gpu_id)
);

-- 待同步佇列（003-remote-sync-web-login，data-model.md 用戶端本機小節）。
CREATE TABLE IF NOT EXISTS sync_queue (
  id               INTEGER PRIMARY KEY,
  payload          BLOB NOT NULL,
  created_at_local TEXT NOT NULL,
  dedup_key        TEXT NOT NULL UNIQUE,
  upload_state     TEXT NOT NULL DEFAULT 'queued'
    CHECK (upload_state IN ('queued','uploading','uploaded','discarded_disk_full'))
);
CREATE INDEX IF NOT EXISTS idx_sync_queue_state_created ON sync_queue(upload_state, created_at_local);

-- 裝置身分快取（單列，登入時寫入伺服器回傳的 device.id）。
CREATE TABLE IF NOT EXISTS device_identity (
  hardware_fingerprint TEXT PRIMARY KEY,
  server_device_id     TEXT
);

-- 登入憑證快取（單列；refresh_token 以 DPAPI 保護後存放）。
CREATE TABLE IF NOT EXISTS auth_cache (
  id                          INTEGER PRIMARY KEY CHECK (id = 1),
  refresh_token               BLOB,
  account_hint                TEXT,
  desktop_session_expires_at  TEXT
);

-- 預設設定（僅在不存在時插入）。
INSERT OR IGNORE INTO setting(key, value) VALUES
  ('idle_threshold_sec', '300'),
  ('tracking_paused',    'false'),
  ('autostart_enabled',  'true'),
  ('master_password_set','false'),
  ('ui_language',        'zh-TW'),
  ('metrics_sample_interval_sec', '1'),
  ('metrics_retention_days',      '30'),
  ('metrics_enabled',             'true');
"#;

/// 加密 SQLite 連線封裝。
pub struct Database {
    pub(crate) conn: Connection,
}

impl Database {
    /// 以原始 DEK 開啟（或建立）加密資料庫並完成遷移。
    pub fn open(db_path: &Path, dek: &[u8]) -> Result<Self> {
        if let Some(parent) = db_path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let conn = Connection::open(db_path)?;

        // 1) 套用 SQLCipher 金鑰（必須在任何其他操作前）。
        let key = dek_to_sqlcipher_key(dek);
        conn.execute_batch(&format!("PRAGMA key = \"{}\";", &*key))?;

        // 2) 安全與耐用性 PRAGMA。
        conn.execute_batch(
            "PRAGMA journal_mode = WAL;\n\
             PRAGMA synchronous = NORMAL;\n\
             PRAGMA foreign_keys = ON;\n\
             PRAGMA busy_timeout = 5000;",
        )?;

        // 3) 驗證金鑰正確（金鑰錯誤時讀取 schema 會失敗）。
        conn.query_row("SELECT count(*) FROM sqlite_master;", [], |r| r.get::<_, i64>(0))
            .map_err(|_| StorageError::KeyProtection("資料庫金鑰錯誤或檔案損毀".into()))?;

        let db = Self { conn };
        db.conn.execute_batch(MIGRATION_SQL)?;
        Ok(db)
    }

    /// 取得底層連線（同 crate 內部使用）。
    pub(crate) fn conn(&self) -> &Connection {
        &self.conn
    }

    /// 強制 WAL checkpoint（截斷），降低當機資料損失（SC-005）。
    pub fn checkpoint(&self) -> Result<()> {
        self.conn.execute_batch("PRAGMA wal_checkpoint(TRUNCATE);")?;
        Ok(())
    }
}
