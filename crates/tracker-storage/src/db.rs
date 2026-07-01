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

-- 預設設定（僅在不存在時插入）。
INSERT OR IGNORE INTO setting(key, value) VALUES
  ('idle_threshold_sec', '300'),
  ('tracking_paused',    'false'),
  ('autostart_enabled',  'true'),
  ('master_password_set','false'),
  ('ui_language',        'zh-TW');
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
