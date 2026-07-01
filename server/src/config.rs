//! 環境變數設定（T012）：DB／S3／Session 簽章金鑰／預設保留天數。

/// 伺服器執行期設定，全部由環境變數載入。
#[derive(Debug, Clone)]
pub struct Config {
    pub database_url: String,
    pub s3_endpoint: String,
    pub s3_bucket: String,
    pub s3_access_key: String,
    pub s3_secret_key: String,
    /// Session Cookie 簽章金鑰（`tower-sessions` Cookie 私密簽章用）。
    pub session_signing_key: String,
    /// 使用者未自訂 `retention_days` 時的系統預設保留天數（FR-013）。
    pub default_retention_days: i32,
    pub bind_addr: String,
}

impl Config {
    /// 由環境變數載入設定；缺少必要值時直接 panic（啟動即失敗，避免以錯誤設定運作）。
    pub fn from_env() -> Self {
        Self {
            database_url: require_env("DATABASE_URL"),
            s3_endpoint: require_env("S3_ENDPOINT"),
            s3_bucket: require_env("S3_BUCKET"),
            s3_access_key: require_env("S3_ACCESS_KEY"),
            s3_secret_key: require_env("S3_SECRET_KEY"),
            session_signing_key: require_env("SESSION_SIGNING_KEY"),
            default_retention_days: std::env::var("DEFAULT_RETENTION_DAYS")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(90),
            bind_addr: std::env::var("BIND_ADDR").unwrap_or_else(|_| "0.0.0.0:8080".to_string()),
        }
    }
}

fn require_env(key: &str) -> String {
    std::env::var(key).unwrap_or_else(|_| panic!("缺少必要環境變數：{key}"))
}
