//! `POST /sync/batches`：冪等寫入 S3＋`sync_record`、裝置移除/帳號停用檢查（T043）。

use axum::extract::State;
use axum::Json;
use base64::Engine;
use serde::{Deserialize, Serialize};

use crate::api::ApiError;
use crate::auth::extractor::CurrentSession;
use crate::auth::session::SessionKind;
use crate::db::account_repo::AccountRepo;
use crate::db::device_repo::DeviceRepo;
use crate::db::sync_record_repo::SyncRecordRepo;
use crate::state::AppState;
use crate::storage::s3::ObjectStore;

/// 單批上限（FR-004）：超過由用戶端切分。
pub const MAX_BATCH_RECORDS: i32 = 500;
pub const MAX_BATCH_BYTES: usize = 5 * 1024 * 1024;

#[derive(Deserialize)]
pub struct SyncBatchRequest {
    pub dedup_key: String,
    /// `[start, end]`，RFC3339 字串。
    pub device_local_time_range: (String, String),
    pub record_count: i32,
    /// gzip(JSON) 之 base64 編碼。
    pub payload: String,
}

#[derive(Serialize)]
pub struct SyncBatchResponse {
    pub sync_record_id: String,
    pub server_received_at: String,
}

pub async fn upload_batch(
    State(state): State<AppState>,
    CurrentSession(session): CurrentSession,
    Json(req): Json<SyncBatchRequest>,
) -> Result<Json<SyncBatchResponse>, ApiError> {
    if session.kind != SessionKind::Desktop {
        return Err(ApiError::Forbidden {
            reason: "requires_desktop_session",
        });
    }
    let device_id = session.device_id.ok_or(ApiError::Forbidden {
        reason: "requires_desktop_session",
    })?;

    let device_repo = DeviceRepo::new(&state.pool);
    let device = device_repo
        .find_by_id(device_id)
        .await?
        .ok_or(ApiError::Forbidden {
            reason: "device_removed",
        })?;
    if device.removed_at.is_some() {
        return Err(ApiError::Forbidden {
            reason: "device_removed",
        });
    }
    let account = AccountRepo::new(&state.pool)
        .find_by_id(session.account_id)
        .await?
        .ok_or(ApiError::Forbidden {
            reason: "account_disabled",
        })?;
    if account.deleted_at.is_some() {
        return Err(ApiError::Forbidden {
            reason: "account_disabled",
        });
    }

    if req.record_count > MAX_BATCH_RECORDS {
        return Err(ApiError::BadRequest("單批筆數超過上限".into()));
    }

    let sync_repo = SyncRecordRepo::new(&state.pool);
    // 冪等：dedup_key 已存在時直接回傳先前結果，不重複寫入物件儲存。
    if let Some(existing) = sync_repo.find_by_dedup_key(&req.dedup_key).await? {
        return Ok(Json(SyncBatchResponse {
            sync_record_id: existing.id.to_string(),
            server_received_at: existing.server_received_at.to_rfc3339(),
        }));
    }

    let payload_bytes = base64::engine::general_purpose::STANDARD
        .decode(&req.payload)
        .map_err(|e| ApiError::BadRequest(format!("payload base64 解碼失敗：{e}")))?;
    if payload_bytes.len() > MAX_BATCH_BYTES {
        return Err(ApiError::BadRequest("單批大小超過上限".into()));
    }

    let range_start: jiff::Timestamp = req
        .device_local_time_range
        .0
        .parse()
        .map_err(|_| ApiError::BadRequest("device_local_time_range 起始時間格式錯誤".into()))?;
    let range_end: jiff::Timestamp = req
        .device_local_time_range
        .1
        .parse()
        .map_err(|_| ApiError::BadRequest("device_local_time_range 結束時間格式錯誤".into()))?;

    let sync_record_id = uuid::Uuid::new_v4();
    let date = range_end.to_zoned(jiff::tz::TimeZone::UTC).date();
    let object_key = ObjectStore::object_key(
        &account.id.to_string(),
        &device_id.to_string(),
        date,
        &sync_record_id.to_string(),
    );
    state
        .objects
        .put_object(&object_key, payload_bytes)
        .await
        .map_err(|e| ApiError::Internal(e.to_string()))?;

    let record = sync_repo
        .create(
            sync_record_id,
            device_id,
            &req.dedup_key,
            range_start,
            range_end,
            &object_key,
            req.record_count,
        )
        .await?;
    device_repo.touch_last_sync(device_id).await?;

    Ok(Json(SyncBatchResponse {
        sync_record_id: record.id.to_string(),
        server_received_at: record.server_received_at.to_rfc3339(),
    }))
}
