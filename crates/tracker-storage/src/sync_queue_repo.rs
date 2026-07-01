//! 佇列 CRUD、磁碟空間檢查與捨棄最舊（`discarded_disk_full`）邏輯（T037，FR-004）。

use rusqlite::params;
use tracker_core::sync::UploadState;

use crate::db::Database;
use crate::error::Result;

#[derive(Debug, Clone)]
pub struct QueueRow {
    pub id: i64,
    pub payload: Vec<u8>,
    pub created_at_local: String,
    pub dedup_key: String,
}

pub struct SyncQueueRepo<'a> {
    db: &'a Database,
}

impl<'a> SyncQueueRepo<'a> {
    pub fn new(db: &'a Database) -> Self {
        Self { db }
    }

    /// 新增待同步項目；`dedup_key` 須為呼叫端保證的唯一鍵（重複視為同一筆，忽略）。
    pub fn enqueue(&self, payload: &[u8], created_at_local: &str, dedup_key: &str) -> Result<()> {
        self.db.conn().execute(
            "INSERT OR IGNORE INTO sync_queue (payload, created_at_local, dedup_key, upload_state)
             VALUES (?1, ?2, ?3, 'queued')",
            params![payload, created_at_local, dedup_key],
        )?;
        Ok(())
    }

    /// 依時間序（舊到新）取出「待上傳」項目，供批次切分。
    pub fn queued_items(&self, limit: usize) -> Result<Vec<QueueRow>> {
        let conn = self.db.conn();
        let mut stmt = conn.prepare(
            "SELECT id, payload, created_at_local, dedup_key FROM sync_queue
             WHERE upload_state = 'queued' ORDER BY created_at_local ASC LIMIT ?1",
        )?;
        let rows = stmt
            .query_map(params![limit as i64], |row| {
                Ok(QueueRow {
                    id: row.get(0)?,
                    payload: row.get(1)?,
                    created_at_local: row.get(2)?,
                    dedup_key: row.get(3)?,
                })
            })?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        Ok(rows)
    }

    pub fn set_state(&self, ids: &[i64], state: UploadState) -> Result<()> {
        for id in ids {
            self.db.conn().execute(
                "UPDATE sync_queue SET upload_state = ?2 WHERE id = ?1",
                params![id, state.as_str()],
            )?;
        }
        Ok(())
    }

    /// 上傳成功後可直接刪除已完成項目，避免佇列表無限成長。
    pub fn delete(&self, ids: &[i64]) -> Result<()> {
        for id in ids {
            self.db.conn().execute("DELETE FROM sync_queue WHERE id = ?1", params![id])?;
        }
        Ok(())
    }

    pub fn queued_count(&self) -> Result<i64> {
        self.db.conn().query_row(
            "SELECT COUNT(*) FROM sync_queue WHERE upload_state = 'queued'",
            [],
            |row| row.get(0),
        ).map_err(Into::into)
    }

    /// 待同步資料佔用位元組數（`queued`／`uploading`，供磁碟用量檢查）。
    pub fn pending_bytes(&self) -> Result<i64> {
        self.db
            .conn()
            .query_row(
                "SELECT COALESCE(SUM(LENGTH(payload)), 0) FROM sync_queue
                 WHERE upload_state IN ('queued', 'uploading')",
                [],
                |row| row.get(0),
            )
            .map_err(Into::into)
    }

    /// 若待同步資料量超過 `max_bytes`，依 `created_at_local` 由舊到新捨棄 `queued` 列直到低於門檻，
    /// 並標記為 `discarded_disk_full`（FR-004）。回傳是否有捨棄任何項目。
    pub fn discard_oldest_if_over(&self, max_bytes: i64) -> Result<bool> {
        let mut discarded_any = false;
        loop {
            let current = self.pending_bytes()?;
            if current <= max_bytes {
                break;
            }
            let oldest: Option<i64> = self
                .db
                .conn()
                .query_row(
                    "SELECT id FROM sync_queue WHERE upload_state = 'queued'
                     ORDER BY created_at_local ASC LIMIT 1",
                    [],
                    |row| row.get(0),
                )
                .ok();
            let Some(id) = oldest else { break };
            self.db.conn().execute(
                "UPDATE sync_queue SET upload_state = 'discarded_disk_full' WHERE id = ?1",
                params![id],
            )?;
            discarded_any = true;
        }
        Ok(discarded_any)
    }
}
