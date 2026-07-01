//! Session 授權中介：自 Cookie（網頁）或 `Authorization: Bearer`（Windows 用戶端）取出 Session ID，
//! 驗證通過後供路由處理常式使用；失敗回傳 401（T017、T027）。

use axum::extract::FromRequestParts;
use axum::http::request::Parts;
use axum_extra::extract::CookieJar;

use crate::api::ApiError;
use crate::auth::session::{AuthSession, SessionService};
use crate::state::AppState;

pub const SESSION_COOKIE_NAME: &str = "session_id";

fn extract_session_id(parts: &Parts) -> Option<String> {
    if let Some(auth) = parts.headers.get(axum::http::header::AUTHORIZATION) {
        if let Ok(s) = auth.to_str() {
            if let Some(token) = s.strip_prefix("Bearer ") {
                return Some(token.to_string());
            }
        }
    }
    let jar = CookieJar::from_headers(&parts.headers);
    jar.get(SESSION_COOKIE_NAME).map(|c| c.value().to_string())
}

/// 通過驗證的目前 Session（授權中介，未通過回傳 401）。
pub struct CurrentSession(pub AuthSession);

impl FromRequestParts<AppState> for CurrentSession {
    type Rejection = ApiError;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &AppState,
    ) -> Result<Self, Self::Rejection> {
        let session_id = extract_session_id(parts).ok_or(ApiError::Unauthorized)?;
        let session = SessionService::new(&state.pool)
            .validate_and_touch(&session_id)
            .await
            .map_err(|e| ApiError::Internal(e.to_string()))?
            .ok_or(ApiError::Unauthorized)?;
        Ok(CurrentSession(session))
    }
}
