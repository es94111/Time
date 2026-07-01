//! 路由骨架與統一錯誤處理（T017）。

pub mod account_routes;
pub mod auth_routes;
pub mod data_routes;
pub mod device_routes;
pub mod share_routes;
pub mod sync_routes;

use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::Json;
use serde_json::json;

/// API 統一錯誤，實作 `IntoResponse` 轉為一致的 JSON 回應。
#[derive(Debug)]
pub enum ApiError {
    /// 401：未登入或 Session 失效。
    Unauthorized,
    /// 403：無權限（裝置已移除／帳號停用／非本人資料）。
    Forbidden { reason: &'static str },
    /// 423：帳號鎖定中。
    Locked { retry_after_seconds: i64 },
    /// 400：請求驗證失敗（如密碼長度不足）。
    BadRequest(String),
    /// 404：資源不存在。
    NotFound,
    /// 500：內部錯誤（資料庫、物件儲存等）。
    Internal(String),
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let (status, body) = match self {
            ApiError::Unauthorized => {
                (StatusCode::UNAUTHORIZED, json!({ "error": "unauthorized" }))
            }
            ApiError::Forbidden { reason } => (StatusCode::FORBIDDEN, json!({ "reason": reason })),
            ApiError::Locked {
                retry_after_seconds,
            } => (
                StatusCode::LOCKED,
                json!({ "retry_after_seconds": retry_after_seconds }),
            ),
            ApiError::BadRequest(msg) => (StatusCode::BAD_REQUEST, json!({ "error": msg })),
            ApiError::NotFound => (StatusCode::NOT_FOUND, json!({ "error": "not_found" })),
            ApiError::Internal(msg) => {
                tracing::error!(%msg, "internal error");
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    json!({ "error": "internal_error" }),
                )
            }
        };
        (status, Json(body)).into_response()
    }
}

impl From<sqlx::Error> for ApiError {
    fn from(e: sqlx::Error) -> Self {
        ApiError::Internal(e.to_string())
    }
}
