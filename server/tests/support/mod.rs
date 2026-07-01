//! 契約測試共用輔助：以隨機埠位啟動完整 axum 應用，供各 `tests/*.rs` 呼叫真實 HTTP API。
//!
//! 需要可用的 PostgreSQL；`#[sqlx::test]` 自動建立/遷移臨時測試資料庫。
//! 物件儲存以測試用假 endpoint 建立（不涉及實際上傳的測試不會真的連線）。

use std::sync::Arc;

use server::api::auth_routes::CreateAccountRequest;
use server::config::Config;
use server::state::AppState;
use server::storage::s3::ObjectStore;

pub struct TestApp {
    pub base_url: String,
    pub client: reqwest::Client,
}

impl TestApp {
    pub fn url(&self, path: &str) -> String {
        format!("{}{}", self.base_url, path)
    }
}

fn test_config() -> Config {
    Config {
        database_url: String::new(), // 未使用：pool 由 sqlx::test 提供
        s3_endpoint: "http://127.0.0.1:9000".into(),
        s3_bucket: "test-bucket".into(),
        // 對應 docker-compose.yml 本機開發用 MinIO 憑證。
        s3_access_key: "minioadmin".into(),
        s3_secret_key: "minioadmin".into(),
        session_signing_key: "test-signing-key".into(),
        default_retention_days: 90,
        bind_addr: "127.0.0.1:0".into(),
    }
}

/// 啟動完整應用（以 `sqlx::test` 提供的臨時資料庫），回傳可發送真實 HTTP 請求的測試句柄。
pub async fn spawn_app(pool: sqlx::PgPool) -> TestApp {
    let _ = tracing_subscriber::fmt().with_test_writer().try_init();
    let config = test_config();
    let objects = ObjectStore::new(
        &config.s3_endpoint,
        &config.s3_bucket,
        &config.s3_access_key,
        &config.s3_secret_key,
    )
    .await;
    let state = AppState {
        pool,
        objects,
        config: Arc::new(config),
    };
    let app = server::build_router(state);

    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });

    TestApp {
        base_url: format!("http://{addr}"),
        client: reqwest::Client::builder()
            .cookie_store(true)
            .build()
            .unwrap(),
    }
}

pub struct TestAccount {
    pub id: String,
    pub identifier: String,
}

impl TestAccount {
    /// 透過 `POST /auth/account` 建立測試帳號。
    pub async fn create(app: &TestApp, identifier_prefix: &str, password: &str) -> Self {
        let identifier = format!("{identifier_prefix}-{}", uuid::Uuid::new_v4());
        let req = CreateAccountRequest {
            identifier: identifier.clone(),
            password: password.to_string(),
        };
        let res = app
            .client
            .post(app.url("/api/v1/auth/account"))
            .json(&req)
            .send()
            .await
            .unwrap();
        assert!(
            res.status().is_success(),
            "建立測試帳號失敗：{:?}",
            res.text().await
        );
        let body: serde_json::Value = res.json().await.unwrap();
        let id = body["id"].as_str().unwrap().to_string();
        Self { id, identifier }
    }
}

/// 以 `client="desktop"` 登入並回傳 `session_id`（供 `Authorization: Bearer` 使用）。
pub async fn login_desktop(
    app: &TestApp,
    identifier: &str,
    password: &str,
    device_fingerprint: &str,
) -> String {
    let res = app
        .client
        .post(app.url("/api/v1/auth/login"))
        .json(&serde_json::json!({
            "identifier": identifier,
            "password": password,
            "client": "desktop",
            "device_fingerprint": device_fingerprint,
            "device_name": "測試裝置",
        }))
        .send()
        .await
        .unwrap();
    assert!(
        res.status().is_success(),
        "桌面登入失敗：{:?}",
        res.text().await
    );
    let body: serde_json::Value = res.json().await.unwrap();
    body["session_id"].as_str().unwrap().to_string()
}

/// 以 `client="web"` 登入（走 Cookie，`client.cookie_store(true)` 已啟用）。
pub async fn login_web(app: &TestApp, identifier: &str, password: &str) {
    let res = app
        .client
        .post(app.url("/api/v1/auth/login"))
        .json(
            &serde_json::json!({ "identifier": identifier, "password": password, "client": "web" }),
        )
        .send()
        .await
        .unwrap();
    assert!(
        res.status().is_success(),
        "網頁登入失敗：{:?}",
        res.text().await
    );
}
