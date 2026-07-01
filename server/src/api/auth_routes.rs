//! `POST /auth/login`、`/auth/logout`、`/auth/account`、`/auth/change-password`
//! （T027、T056、T065）。

use axum::extract::State;
use axum::http::{header, HeaderMap, HeaderValue, StatusCode};
use axum::response::IntoResponse;
use axum::Json;
use serde::{Deserialize, Serialize};

use crate::api::ApiError;
use crate::auth::extractor::{CurrentSession, SESSION_COOKIE_NAME};
use crate::auth::session::{SessionKind, SessionService};
use crate::auth::{lockout, password};
use crate::db::account_repo::AccountRepo;
use crate::jobs::device_merge;
use crate::state::AppState;

#[derive(Deserialize)]
pub struct LoginRequest {
    pub identifier: String,
    pub password: String,
    pub client: String, // "web" | "desktop"
    pub device_fingerprint: Option<String>,
    pub device_name: Option<String>,
}

#[derive(Serialize)]
pub struct LoginResponse {
    pub session_id: String,
    pub expires_at: String,
}

pub async fn login(
    State(state): State<AppState>,
    Json(req): Json<LoginRequest>,
) -> Result<impl IntoResponse, ApiError> {
    let kind = match req.client.as_str() {
        "desktop" => SessionKind::Desktop,
        _ => SessionKind::Web,
    };
    if kind == SessionKind::Desktop && req.device_fingerprint.is_none() {
        return Err(ApiError::BadRequest(
            "desktop client 需提供 device_fingerprint".into(),
        ));
    }

    let account_repo = AccountRepo::new(&state.pool);
    let account = account_repo
        .find_by_identifier(&req.identifier)
        .await?
        .ok_or(ApiError::Unauthorized)?;

    if let Some(retry_after_seconds) = lockout::remaining_lock_seconds(&account) {
        return Err(ApiError::Locked {
            retry_after_seconds,
        });
    }

    if !password::verify_password(&req.password, &account.password_hash) {
        lockout::record_failure(&state.pool, account.id).await?;
        return Err(ApiError::Unauthorized);
    }
    lockout::record_success(&state.pool, account.id).await?;

    let device_id = if kind == SessionKind::Desktop {
        let fingerprint = req.device_fingerprint.as_deref().unwrap();
        let name = req.device_name.as_deref().unwrap_or("我的裝置");
        let device =
            device_merge::resolve_or_create_device(&state.pool, account.id, fingerprint, name)
                .await?;
        Some(device.id)
    } else {
        None
    };

    let session = SessionService::new(&state.pool)
        .create(account.id, kind, device_id)
        .await?;

    let mut headers = HeaderMap::new();
    if kind == SessionKind::Web {
        let cookie = format!(
            "{SESSION_COOKIE_NAME}={}; HttpOnly; Path=/; SameSite=Lax",
            session.id
        );
        headers.insert(header::SET_COOKIE, HeaderValue::from_str(&cookie).unwrap());
    }

    let expires_at = jiff::Timestamp::now().to_string();
    Ok((
        headers,
        Json(LoginResponse {
            session_id: session.id,
            expires_at,
        }),
    ))
}

pub async fn logout(
    State(state): State<AppState>,
    CurrentSession(session): CurrentSession,
) -> Result<StatusCode, ApiError> {
    SessionService::new(&state.pool).revoke(&session.id).await?;
    Ok(StatusCode::NO_CONTENT)
}

#[derive(Serialize, Deserialize)]
pub struct CreateAccountRequest {
    pub identifier: String,
    pub password: String,
}

pub async fn create_account(
    State(state): State<AppState>,
    Json(req): Json<CreateAccountRequest>,
) -> Result<impl IntoResponse, ApiError> {
    if !password::validate_password_len(&req.password) {
        return Err(ApiError::BadRequest(format!(
            "密碼長度至少須 {} 字元",
            password::MIN_PASSWORD_LEN
        )));
    }
    let hash = password::hash_password(&req.password).map_err(ApiError::Internal)?;
    let account = AccountRepo::new(&state.pool)
        .create(&req.identifier, &hash)
        .await?;
    Ok((
        StatusCode::CREATED,
        Json(serde_json::json!({ "id": account.id })),
    ))
}

#[derive(Deserialize)]
pub struct ChangePasswordRequest {
    pub current_password: String,
    pub new_password: String,
}

pub async fn change_password(
    State(state): State<AppState>,
    CurrentSession(session): CurrentSession,
    Json(req): Json<ChangePasswordRequest>,
) -> Result<StatusCode, ApiError> {
    if !password::validate_password_len(&req.new_password) {
        return Err(ApiError::BadRequest(format!(
            "密碼長度至少須 {} 字元",
            password::MIN_PASSWORD_LEN
        )));
    }
    let account_repo = AccountRepo::new(&state.pool);
    let account = account_repo
        .find_by_id(session.account_id)
        .await?
        .ok_or(ApiError::Unauthorized)?;
    if !password::verify_password(&req.current_password, &account.password_hash) {
        return Err(ApiError::BadRequest("目前密碼不正確".into()));
    }
    let new_hash = password::hash_password(&req.new_password).map_err(ApiError::Internal)?;
    account_repo
        .update_password_hash(account.id, &new_hash)
        .await?;

    // 變更密碼後撤銷該帳號所有其他 Session（FR-009）。
    SessionService::new(&state.pool)
        .revoke_all_others(account.id, &session.id)
        .await?;
    Ok(StatusCode::OK)
}
