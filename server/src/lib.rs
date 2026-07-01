//! 伺服器函式庫：組裝 Router 與啟動邏輯（供 `main.rs` 與整合測試共用，T015）。

pub mod api;
pub mod auth;
pub mod config;
pub mod db;
pub mod jobs;
pub mod state;
pub mod storage;
pub mod web;

use std::sync::Arc;

use axum::routing::{delete, get, patch, post};
use axum::Router;

use config::Config;
use state::AppState;
use storage::s3::ObjectStore;

/// 依環境變數啟動伺服器（正式進入點，供 `main.rs` 呼叫）。
pub async fn run() {
    tracing_subscriber::fmt::try_init().ok();

    let config = Config::from_env();
    let pool = db::connect_and_migrate(&config.database_url)
        .await
        .expect("資料庫連線/遷移失敗");
    let objects = ObjectStore::new(
        &config.s3_endpoint,
        &config.s3_bucket,
        &config.s3_access_key,
        &config.s3_secret_key,
    )
    .await;

    jobs::retention_cleanup::spawn(pool.clone(), objects.clone(), config.default_retention_days);

    let bind_addr = config.bind_addr.clone();
    let state = AppState {
        pool,
        objects,
        config: Arc::new(config),
    };

    let app = build_router(state);

    let listener = tokio::net::TcpListener::bind(&bind_addr)
        .await
        .expect("監聽埠位失敗");
    tracing::info!(%bind_addr, "伺服器啟動");
    axum::serve(listener, app).await.expect("伺服器執行失敗");
}

/// 組裝完整路由（網頁 + `/api/v1` REST API）；供 `run()` 與整合測試共用。
pub fn build_router(state: AppState) -> Router {
    let api_v1 = Router::new()
        .route("/auth/login", post(api::auth_routes::login))
        .route("/auth/logout", post(api::auth_routes::logout))
        .route("/auth/account", post(api::auth_routes::create_account))
        .route(
            "/auth/change-password",
            post(api::auth_routes::change_password),
        )
        .route("/sync/batches", post(api::sync_routes::upload_batch))
        .route("/devices", get(api::device_routes::list_devices))
        .route("/devices/{id}", delete(api::device_routes::remove_device))
        .route("/data", get(api::data_routes::get_data))
        .route("/shares", post(api::share_routes::create_share))
        .route("/shares/{id}", delete(api::share_routes::delete_share))
        .route(
            "/account/settings",
            patch(api::account_routes::update_settings),
        )
        .route("/account", delete(api::account_routes::delete_account));

    Router::new()
        .route("/healthz", get(|| async { "ok" }))
        .route(
            "/login",
            get(web::login::login_page).post(web::login::login_submit),
        )
        .route("/dashboard", get(web::dashboard::dashboard_page))
        .route("/devices", get(web::devices::devices_page))
        .route(
            "/devices/{id}/remove",
            post(web::devices::remove_device_submit),
        )
        .nest("/api/v1", api_v1)
        .with_state(state)
}
