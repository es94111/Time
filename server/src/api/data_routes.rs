//! `GET /data`：依 `server_received_at` 排序、session 授權中介層檢查（T046）；
//! 加入分享授權過濾邏輯（T049）；支援 `device_id=all` 彙總與單一裝置過濾（T059）。

use axum::extract::{Query, State};
use axum::Json;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::api::ApiError;
use crate::auth::extractor::CurrentSession;
use crate::db::device_repo::DeviceRepo;
use crate::db::share_grant_repo::ShareGrantRepo;
use crate::db::sync_record_repo::SyncRecordRepo;
use crate::state::AppState;

#[derive(Deserialize)]
pub struct DataQuery {
    /// 裝置 ID，或 `"all"`／省略表示彙總所有裝置。
    pub device_id: Option<String>,
    /// 若查詢的是被分享者取得唯讀存取權的擁有者資料，帶入擁有者帳號 ID。
    pub owner_id: Option<Uuid>,
}

#[derive(Serialize)]
pub struct DataRecord {
    pub sync_record_id: String,
    pub device_id: String,
    pub server_received_at: String,
    pub record_count: i32,
    pub status: String,
}

pub async fn get_data(
    State(state): State<AppState>,
    CurrentSession(session): CurrentSession,
    Query(query): Query<DataQuery>,
) -> Result<Json<Vec<DataRecord>>, ApiError> {
    let target_account_id =
        resolve_target_account(&state, session.account_id, query.owner_id).await?;

    let device_repo = DeviceRepo::new(&state.pool);
    let devices = device_repo.list_by_account(target_account_id).await?;

    let device_ids: Vec<Uuid> = match query.device_id.as_deref() {
        None | Some("all") => devices.iter().map(|d| d.id).collect(),
        Some(id_str) => {
            let id: Uuid = id_str
                .parse()
                .map_err(|_| ApiError::BadRequest("device_id 格式錯誤".into()))?;
            if !devices.iter().any(|d| d.id == id) {
                return Err(ApiError::Forbidden {
                    reason: "device_not_owned",
                });
            }
            vec![id]
        }
    };

    let sync_repo = SyncRecordRepo::new(&state.pool);
    let records = sync_repo.list_by_devices(&device_ids).await?;

    Ok(Json(
        records
            .into_iter()
            .map(|r| DataRecord {
                sync_record_id: r.id.to_string(),
                device_id: r.device_id.to_string(),
                server_received_at: r.server_received_at.to_rfc3339(),
                record_count: r.record_count,
                status: r.status,
            })
            .collect(),
    ))
}

/// 決定實際查詢目標帳號：本人資料，或經有效分享授權的擁有者資料（僅唯讀，FR-006）。
async fn resolve_target_account(
    state: &AppState,
    requester_account_id: Uuid,
    owner_id: Option<Uuid>,
) -> Result<Uuid, ApiError> {
    let Some(owner_id) = owner_id else {
        return Ok(requester_account_id);
    };
    if owner_id == requester_account_id {
        return Ok(requester_account_id);
    }
    let grants = ShareGrantRepo::new(&state.pool)
        .active_grants_for_grantee(requester_account_id)
        .await?;
    if grants.iter().any(|g| g.owner_account_id == owner_id) {
        Ok(owner_id)
    } else {
        Err(ApiError::Forbidden {
            reason: "no_share_grant",
        })
    }
}
