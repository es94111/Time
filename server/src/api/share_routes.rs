//! `POST /shares`、`DELETE /shares/{id}`：唯讀分享授權建立/撤銷（T048，FR-006）。

use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::Json;
use serde::Deserialize;
use uuid::Uuid;

use crate::api::ApiError;
use crate::auth::extractor::CurrentSession;
use crate::db::account_repo::AccountRepo;
use crate::db::share_grant_repo::ShareGrantRepo;
use crate::state::AppState;

#[derive(Deserialize)]
pub struct CreateShareRequest {
    pub grantee_identifier: String,
    pub scope: String,
}

pub async fn create_share(
    State(state): State<AppState>,
    CurrentSession(session): CurrentSession,
    Json(req): Json<CreateShareRequest>,
) -> Result<(StatusCode, Json<serde_json::Value>), ApiError> {
    let grantee = AccountRepo::new(&state.pool)
        .find_by_identifier(&req.grantee_identifier)
        .await?
        .ok_or(ApiError::BadRequest("找不到被分享帳號".into()))?;
    if grantee.id == session.account_id {
        return Err(ApiError::BadRequest("不可分享給自己".into()));
    }
    let grant = ShareGrantRepo::new(&state.pool)
        .create(session.account_id, grantee.id, &req.scope)
        .await?;
    Ok((
        StatusCode::CREATED,
        Json(serde_json::json!({ "id": grant.id })),
    ))
}

pub async fn delete_share(
    State(state): State<AppState>,
    CurrentSession(_session): CurrentSession,
    Path(id): Path<Uuid>,
) -> Result<StatusCode, ApiError> {
    ShareGrantRepo::new(&state.pool).revoke(id).await?;
    Ok(StatusCode::NO_CONTENT)
}
