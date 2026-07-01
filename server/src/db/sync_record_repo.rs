//! `sync_record` CRUD／依 `batch_dedup_key` 冪等查詢（T042）。

use jiff::Timestamp;
use sqlx::PgPool;
use uuid::Uuid;

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct SyncRecord {
    pub id: Uuid,
    pub device_id: Uuid,
    pub batch_dedup_key: String,
    pub server_received_at: chrono::DateTime<chrono::Utc>,
    pub status: String,
    pub object_storage_key: Option<String>,
    pub record_count: i32,
}

pub struct SyncRecordRepo<'a> {
    pool: &'a PgPool,
}

impl<'a> SyncRecordRepo<'a> {
    pub fn new(pool: &'a PgPool) -> Self {
        Self { pool }
    }

    pub async fn find_by_dedup_key(
        &self,
        dedup_key: &str,
    ) -> Result<Option<SyncRecord>, sqlx::Error> {
        sqlx::query_as::<_, SyncRecord>(
            r#"SELECT id, device_id, batch_dedup_key, server_received_at, status, object_storage_key, record_count
               FROM sync_record WHERE batch_dedup_key = $1"#,
        )
        .bind(dedup_key)
        .fetch_optional(self.pool)
        .await
    }

    /// 建立同步紀錄；`batch_dedup_key` 重複時交由呼叫端先以 `find_by_dedup_key` 檢查冪等（FR-004）。
    /// `id` 由呼叫端先行產生，以便與物件儲存鍵值（含 sync_record_id）保持一致。
    #[allow(clippy::too_many_arguments)]
    pub async fn create(
        &self,
        id: Uuid,
        device_id: Uuid,
        batch_dedup_key: &str,
        range_start: Timestamp,
        range_end: Timestamp,
        object_storage_key: &str,
        record_count: i32,
    ) -> Result<SyncRecord, sqlx::Error> {
        sqlx::query_as::<_, SyncRecord>(
            r#"INSERT INTO sync_record
                 (id, device_id, batch_dedup_key, device_local_time_range, object_storage_key, record_count, status)
               VALUES ($1, $2, $3, tstzrange($4, $5, '[]'), $6, $7, 'completed')
               RETURNING id, device_id, batch_dedup_key, server_received_at, status, object_storage_key, record_count"#,
        )
        .bind(id)
        .bind(device_id)
        .bind(batch_dedup_key)
        .bind(to_chrono(range_start))
        .bind(to_chrono(range_end))
        .bind(object_storage_key)
        .bind(record_count)
        .fetch_one(self.pool)
        .await
    }

    /// 依裝置清單查詢資料（`device_id=all` 由呼叫端展開為多筆查詢或 `IN`；此處為單裝置查詢，T059 擴充彙總）。
    pub async fn list_by_device(&self, device_id: Uuid) -> Result<Vec<SyncRecord>, sqlx::Error> {
        sqlx::query_as::<_, SyncRecord>(
            r#"SELECT id, device_id, batch_dedup_key, server_received_at, status, object_storage_key, record_count
               FROM sync_record WHERE device_id = $1 ORDER BY server_received_at"#,
        )
        .bind(device_id)
        .fetch_all(self.pool)
        .await
    }

    pub async fn list_by_devices(
        &self,
        device_ids: &[Uuid],
    ) -> Result<Vec<SyncRecord>, sqlx::Error> {
        sqlx::query_as::<_, SyncRecord>(
            r#"SELECT id, device_id, batch_dedup_key, server_received_at, status, object_storage_key, record_count
               FROM sync_record WHERE device_id = ANY($1) ORDER BY server_received_at"#,
        )
        .bind(device_ids)
        .fetch_all(self.pool)
        .await
    }
}

fn to_chrono(ts: Timestamp) -> chrono::DateTime<chrono::Utc> {
    chrono::DateTime::from_timestamp(ts.as_second(), ts.subsec_nanosecond() as u32)
        .unwrap_or_else(chrono::Utc::now)
}
