//! 待同步批次切分、去重鍵產生、佇列狀態純邏輯（T018，可單元測試）。
//!
//! 對應 FR-004（固定批次補傳、磁碟滿捨棄最舊）。純邏輯不涉及實際 I/O，
//! 由 `tracker-sync`／`tracker-storage` 呼叫並注入真實資料。

use serde::{Deserialize, Serialize};

/// 單批上限（FR-004）：≤500 筆或 ≤5MB。
#[derive(Debug, Clone, Copy)]
pub struct BatchLimits {
    pub max_records: usize,
    pub max_bytes: usize,
}

impl Default for BatchLimits {
    fn default() -> Self {
        Self { max_records: 500, max_bytes: 5 * 1024 * 1024 }
    }
}

/// 待同步佇列項目摘要（僅供批次切分計算，不含實際內容）。
#[derive(Debug, Clone, Copy)]
pub struct QueueItem {
    pub id: i64,
    pub payload_bytes: usize,
    pub created_at_local_ms: i64,
}

/// 依「筆數上限、位元組上限」將**依時間序**排列的佇列項目切分為多個批次（各批依 id 清單表示）。
///
/// 假設輸入已依 `created_at_local_ms` 由舊到新排序；單一項目若超過 `max_bytes`
/// 仍自成一批（呼叫端須另行處理過大項目，此函式不會遺漏任何項目）。
pub fn split_into_batches(items: &[QueueItem], limits: &BatchLimits) -> Vec<Vec<i64>> {
    let mut batches = Vec::new();
    let mut current: Vec<i64> = Vec::new();
    let mut current_bytes = 0usize;

    for item in items {
        let would_exceed_count = current.len() + 1 > limits.max_records;
        let would_exceed_bytes = !current.is_empty() && current_bytes + item.payload_bytes > limits.max_bytes;
        if (would_exceed_count || would_exceed_bytes) && !current.is_empty() {
            batches.push(std::mem::take(&mut current));
            current_bytes = 0;
        }
        current.push(item.id);
        current_bytes += item.payload_bytes;
    }
    if !current.is_empty() {
        batches.push(current);
    }
    batches
}

/// 產生批次去重鍵（裝置指紋 + 本機序號），保證同批次重試上傳冪等（對應伺服器 `sync_record.batch_dedup_key`）。
pub fn make_dedup_key(device_fingerprint: &str, local_batch_seq: u64) -> String {
    format!("{device_fingerprint}:{local_batch_seq}")
}

/// 佇列項目上傳狀態（對應 data-model.md `sync_queue.upload_state`）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[allow(clippy::enum_variant_names)]
pub enum UploadState {
    Queued,
    Uploading,
    Uploaded,
    /// 磁碟空間接近上限時捨棄之舊資料（FR-004）。
    DiscardedDiskFull,
}

impl UploadState {
    pub fn as_str(self) -> &'static str {
        match self {
            UploadState::Queued => "queued",
            UploadState::Uploading => "uploading",
            UploadState::Uploaded => "uploaded",
            UploadState::DiscardedDiskFull => "discarded_disk_full",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "queued" => Some(UploadState::Queued),
            "uploading" => Some(UploadState::Uploading),
            "uploaded" => Some(UploadState::Uploaded),
            "discarded_disk_full" => Some(UploadState::DiscardedDiskFull),
            _ => None,
        }
    }
}
