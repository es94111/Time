//! `PATCH /account/settings`、`DELETE /account`（T067，FR-012/FR-013）。

use axum::extract::State;
use axum::http::StatusCode;
use axum::Json;
use serde::Deserialize;

use crate::api::ApiError;
use crate::auth::extractor::CurrentSession;
use crate::db::account_repo::AccountRepo;
use crate::state::AppState;

#[derive(Deserialize)]
pub struct UpdateSettingsRequest {
    pub retention_days: Option<i32>,
}

pub async fn update_settings(
    State(state): State<AppState>,
    CurrentSession(session): CurrentSession,
    Json(req): Json<UpdateSettingsRequest>,
) -> Result<StatusCode, ApiError> {
    AccountRepo::new(&state.pool)
        .update_retention_days(session.account_id, req.retention_days)
        .await?;
    Ok(StatusCode::OK)
}

/// 軟刪除帳號（`deleted_at = now()`），30 天後由 `jobs::retention_cleanup` 永久刪除（FR-012）。
pub async fn delete_account(
    State(state): State<AppState>,
    CurrentSession(session): CurrentSession,
) -> Result<StatusCode, ApiError> {
    AccountRepo::new(&state.pool)
        .soft_delete(session.account_id)
        .await?;
    Ok(StatusCode::NO_CONTENT)
}
