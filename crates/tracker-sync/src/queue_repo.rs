//! 封裝佇列讀寫（呼叫 `tracker-storage::sync_queue_repo`，T038）。

use tracker_core::sync::UploadState;
use tracker_storage::sync_queue_repo::{QueueRow, SyncQueueRepo};
use tracker_storage::{Database, Result};

/// 本機佇列磁碟用量上限（超過時捨棄最舊，FR-004）。
pub const MAX_QUEUE_BYTES: i64 = 50 * 1024 * 1024;

pub fn enqueue(db: &Database, payload: &[u8], created_at_local: &str, dedup_key: &str) -> Result<()> {
    let repo = SyncQueueRepo::new(db);
    repo.enqueue(payload, created_at_local, dedup_key)?;
    repo.discard_oldest_if_over(MAX_QUEUE_BYTES)?;
    Ok(())
}

pub fn queued_items(db: &Database, limit: usize) -> Result<Vec<QueueRow>> {
    SyncQueueRepo::new(db).queued_items(limit)
}

pub fn mark_uploaded_and_remove(db: &Database, ids: &[i64]) -> Result<()> {
    let repo = SyncQueueRepo::new(db);
    repo.set_state(ids, UploadState::Uploaded)?;
    repo.delete(ids)
}

pub fn mark_uploading(db: &Database, ids: &[i64]) -> Result<()> {
    SyncQueueRepo::new(db).set_state(ids, UploadState::Uploading)
}

pub fn queued_count(db: &Database) -> Result<i64> {
    SyncQueueRepo::new(db).queued_count()
}
