//! `GET /devices`、`DELETE /devices/{id}`（T057，FR-007）。

use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::Json;
use serde::Serialize;
use uuid::Uuid;

use crate::api::ApiError;
use crate::auth::extractor::CurrentSession;
use crate::db::device_repo::DeviceRepo;
use crate::state::AppState;

#[derive(Serialize)]
pub struct DeviceView {
    pub id: String,
    pub display_name: String,
    pub last_sync_at: Option<String>,
    pub removed: bool,
}

pub async fn list_devices(
    State(state): State<AppState>,
    CurrentSession(session): CurrentSession,
) -> Result<Json<Vec<DeviceView>>, ApiError> {
    let devices = DeviceRepo::new(&state.pool)
        .list_by_account(session.account_id)
        .await?;
    Ok(Json(
        devices
            .into_iter()
            .filter(|d| d.removed_at.is_none())
            .map(|d| DeviceView {
                id: d.id.to_string(),
                display_name: d.display_name,
                last_sync_at: d.last_sync_at.map(|t| t.to_rfc3339()),
                removed: false,
            })
            .collect(),
    ))
}

pub async fn remove_device(
    State(state): State<AppState>,
    CurrentSession(session): CurrentSession,
    Path(id): Path<Uuid>,
) -> Result<StatusCode, ApiError> {
    let repo = DeviceRepo::new(&state.pool);
    let device = repo.find_by_id(id).await?.ok_or(ApiError::NotFound)?;
    if device.account_id != session.account_id {
        return Err(ApiError::Forbidden {
            reason: "not_owner",
        });
    }
    repo.mark_removed(id).await?;
    Ok(StatusCode::NO_CONTENT)
}
