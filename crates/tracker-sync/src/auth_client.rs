//! 呼叫伺服器登入/登出 API、憑證以 DPAPI 保護寫入本機 `auth_cache`（T028）。

use serde::{Deserialize, Serialize};
use tracker_core::ports::SecretStore;
use tracker_storage::auth_cache_repo::{AuthCache, AuthCacheRepo};
use tracker_storage::Database;

#[derive(Debug)]
pub enum LoginError {
    InvalidCredentials,
    AccountLocked { retry_after_seconds: i64 },
    NetworkError(String),
}

#[derive(Debug, Serialize)]
struct LoginRequestBody<'a> {
    identifier: &'a str,
    password: &'a str,
    client: &'a str,
    device_fingerprint: Option<&'a str>,
    device_name: Option<&'a str>,
}

#[derive(Debug, Deserialize)]
struct LoginResponseBody {
    session_id: String,
    expires_at: String,
}

#[derive(Debug, Deserialize)]
struct LockedBody {
    retry_after_seconds: i64,
}

pub struct LoginResult {
    pub session_id: String,
    pub expires_at: String,
}

/// 登入：呼叫 `POST /auth/login`（`client="desktop"`），成功後將 refresh token
/// 以 DPAPI 保護寫入本機 `auth_cache`（FR-002）。
#[allow(clippy::too_many_arguments)]
pub async fn login(
    server_base_url: &str,
    db: &Database,
    secret: &dyn SecretStore,
    identifier: &str,
    password: &str,
    device_fingerprint: &str,
    device_name: &str,
) -> Result<LoginResult, LoginError> {
    let client = reqwest::Client::new();
    let body = LoginRequestBody {
        identifier,
        password,
        client: "desktop",
        device_fingerprint: Some(device_fingerprint),
        device_name: Some(device_name),
    };

    let res = client
        .post(format!("{server_base_url}/api/v1/auth/login"))
        .json(&body)
        .send()
        .await
        .map_err(|e| LoginError::NetworkError(e.to_string()))?;

    match res.status().as_u16() {
        200 => {}
        401 => return Err(LoginError::InvalidCredentials),
        423 => {
            let locked: LockedBody =
                res.json().await.map_err(|e| LoginError::NetworkError(e.to_string()))?;
            return Err(LoginError::AccountLocked { retry_after_seconds: locked.retry_after_seconds });
        }
        _ => return Err(LoginError::NetworkError(format!("未預期的伺服器回應狀態：{}", res.status()))),
    }

    let parsed: LoginResponseBody = res.json().await.map_err(|e| LoginError::NetworkError(e.to_string()))?;

    let protected = secret
        .protect(parsed.session_id.as_bytes())
        .map_err(|e| LoginError::NetworkError(format!("憑證保護失敗：{e}")))?;
    let cache = AuthCache {
        refresh_token: protected,
        account_hint: identifier.to_string(),
        desktop_session_expires_at: parsed.expires_at.clone(),
    };
    AuthCacheRepo::new(db)
        .save(&cache)
        .map_err(|e| LoginError::NetworkError(format!("寫入本機憑證快取失敗：{e}")))?;

    Ok(LoginResult { session_id: parsed.session_id, expires_at: parsed.expires_at })
}

/// 登出：呼叫 `POST /auth/logout`，清除本機 `auth_cache`（FR-008）。
pub async fn logout(server_base_url: &str, db: &Database, secret: &dyn SecretStore) -> Result<(), LoginError> {
    if let Some(session_id) = current_session_id(db, secret) {
        let client = reqwest::Client::new();
        let _ = client
            .post(format!("{server_base_url}/api/v1/auth/logout"))
            .bearer_auth(session_id)
            .send()
            .await;
    }
    AuthCacheRepo::new(db)
        .clear()
        .map_err(|e| LoginError::NetworkError(format!("清除本機憑證快取失敗：{e}")))?;
    Ok(())
}

/// 讀取並解保護目前快取的 Session ID（供 `uploader` 呼叫同步 API 使用）。
pub fn current_session_id(db: &Database, secret: &dyn SecretStore) -> Option<String> {
    let cache = AuthCacheRepo::new(db).load().ok().flatten()?;
    let plaintext = secret.unprotect(&cache.refresh_token).ok()?;
    String::from_utf8(plaintext).ok()
}
