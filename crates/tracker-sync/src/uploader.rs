//! `reqwest` 呼叫 `POST /sync/batches`，固定批次（≤500 筆／≤5MB）＋節流補傳（T039）。

use std::io::Write;

use base64::Engine;
use flate2::write::GzEncoder;
use flate2::Compression;
use serde::Serialize;
use tracker_core::ports::SecretStore;
use tracker_core::sync::{make_dedup_key, split_into_batches, BatchLimits, QueueItem};
use tracker_storage::Database;

use crate::auth_client;
use crate::queue_repo;

#[derive(Debug)]
pub enum UploadError {
    NotLoggedIn,
    /// Session 已失效（登出或變更密碼致其他工作階段被撤銷，FR-008/FR-009）：
    /// 本機憑證快取已被清除，呼叫端須提示使用者重新登入。
    SessionExpired,
    DeviceRemoved,
    AccountDisabled,
    Network(String),
}

#[derive(Debug, Default)]
pub struct UploadSummary {
    pub batches_uploaded: usize,
}

#[derive(Serialize)]
struct BatchRequest {
    dedup_key: String,
    device_local_time_range: (String, String),
    record_count: i32,
    payload: String,
}

/// 讀取本機待同步佇列，依固定批次補傳；一旦遇到 `device_removed`／`account_disabled`
/// 即停止並回傳錯誤，交由呼叫端（`agent`）處理（如提示重新登入）。
pub async fn upload_pending(
    server_base_url: &str,
    db: &Database,
    secret: &dyn SecretStore,
    device_fingerprint: &str,
) -> Result<UploadSummary, UploadError> {
    let session_id = auth_client::current_session_id(db, secret).ok_or(UploadError::NotLoggedIn)?;

    let rows = queue_repo::queued_items(db, 10_000).map_err(|e| UploadError::Network(e.to_string()))?;
    if rows.is_empty() {
        return Ok(UploadSummary::default());
    }

    let core_items: Vec<QueueItem> = rows
        .iter()
        .map(|r| QueueItem {
            id: r.id,
            payload_bytes: r.payload.len(),
            created_at_local_ms: parse_local_ms(&r.created_at_local),
        })
        .collect();
    let batches = split_into_batches(&core_items, &BatchLimits::default());

    let client = reqwest::Client::new();
    let mut summary = UploadSummary::default();

    for (seq, id_batch) in batches.iter().enumerate() {
        let batch_rows: Vec<_> = rows.iter().filter(|r| id_batch.contains(&r.id)).collect();
        let dedup_key = make_dedup_key(device_fingerprint, first_id(id_batch) as u64 * 1000 + seq as u64);

        let combined: Vec<String> = batch_rows
            .iter()
            .map(|r| base64::engine::general_purpose::STANDARD.encode(&r.payload))
            .collect();
        let json_bytes =
            serde_json::to_vec(&combined).map_err(|e| UploadError::Network(format!("序列化失敗：{e}")))?;
        let gzipped = gzip(&json_bytes).map_err(|e| UploadError::Network(format!("壓縮失敗：{e}")))?;
        let payload_b64 = base64::engine::general_purpose::STANDARD.encode(&gzipped);

        let times: Vec<&str> = batch_rows.iter().map(|r| r.created_at_local.as_str()).collect();
        let range_start = times.iter().min().copied().unwrap_or_default().to_string();
        let range_end = times.iter().max().copied().unwrap_or_default().to_string();

        let body = BatchRequest {
            dedup_key,
            device_local_time_range: (range_start, range_end),
            record_count: batch_rows.len() as i32,
            payload: payload_b64,
        };

        queue_repo::mark_uploading(db, id_batch).map_err(|e| UploadError::Network(e.to_string()))?;

        let res = client
            .post(format!("{server_base_url}/api/v1/sync/batches"))
            .bearer_auth(&session_id)
            .json(&body)
            .send()
            .await
            .map_err(|e| UploadError::Network(e.to_string()))?;

        match res.status().as_u16() {
            200 | 201 => {
                queue_repo::mark_uploaded_and_remove(db, id_batch)
                    .map_err(|e| UploadError::Network(e.to_string()))?;
                summary.batches_uploaded += 1;
            }
            401 => {
                // Session 已失效（登出／變更密碼撤銷）：清除本機憑證快取並停止本輪補傳（FR-008/FR-009）。
                let _ = tracker_storage::auth_cache_repo::AuthCacheRepo::new(db).clear();
                return Err(UploadError::SessionExpired);
            }
            403 => {
                let reason = res
                    .json::<serde_json::Value>()
                    .await
                    .ok()
                    .and_then(|v| v.get("reason").and_then(|r| r.as_str()).map(str::to_string))
                    .unwrap_or_default();
                return Err(if reason == "device_removed" {
                    UploadError::DeviceRemoved
                } else {
                    UploadError::AccountDisabled
                });
            }
            status => return Err(UploadError::Network(format!("未預期的伺服器回應狀態：{status}"))),
        }
    }

    Ok(summary)
}

fn first_id(ids: &[i64]) -> i64 {
    ids.first().copied().unwrap_or(0)
}

fn parse_local_ms(s: &str) -> i64 {
    s.parse::<jiff::Timestamp>().map(|t| t.as_millisecond()).unwrap_or(0)
}

fn gzip(data: &[u8]) -> std::io::Result<Vec<u8>> {
    let mut encoder = GzEncoder::new(Vec::new(), Compression::default());
    encoder.write_all(data)?;
    encoder.finish()
}
