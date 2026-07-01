//! 契約測試 `POST /sync/batches`（T034）：冪等、裝置移除 403、帳號停用 403。
//!
//! 需要可用的 PostgreSQL 與 S3 相容物件儲存（`docker compose up -d`）。

mod support;

use axum::http::StatusCode;
use base64::Engine;
use serde_json::json;
use support::{login_desktop, login_web, spawn_app, TestAccount};

fn sample_body(dedup_key: &str) -> serde_json::Value {
    let payload = base64::engine::general_purpose::STANDARD.encode(b"[]");
    json!({
        "dedup_key": dedup_key,
        "device_local_time_range": ["2026-07-01T00:00:00Z", "2026-07-01T00:01:00Z"],
        "record_count": 0,
        "payload": payload,
    })
}

#[sqlx::test(migrations = "./migrations")]
async fn duplicate_dedup_key_is_idempotent(pool: sqlx::PgPool) {
    let app = spawn_app(pool).await;
    let account = TestAccount::create(&app, "dave", "correct-horse-battery").await;
    let session_id = login_desktop(
        &app,
        &account.identifier,
        "correct-horse-battery",
        "fingerprint-1",
    )
    .await;

    let body = sample_body("fingerprint-1:1");
    let first = app
        .client
        .post(app.url("/api/v1/sync/batches"))
        .bearer_auth(&session_id)
        .json(&body)
        .send()
        .await
        .unwrap();
    assert!(first.status().is_success(), "{:?}", first.text().await);
    let first_json: serde_json::Value = first.json().await.unwrap();

    let second = app
        .client
        .post(app.url("/api/v1/sync/batches"))
        .bearer_auth(&session_id)
        .json(&body)
        .send()
        .await
        .unwrap();
    assert!(second.status().is_success());
    let second_json: serde_json::Value = second.json().await.unwrap();

    // 冪等：同一 dedup_key 重複上傳回傳同一筆 sync_record_id，不重複寫入。
    assert_eq!(first_json["sync_record_id"], second_json["sync_record_id"]);
}

#[sqlx::test(migrations = "./migrations")]
async fn removed_device_is_rejected(pool: sqlx::PgPool) {
    let app = spawn_app(pool).await;
    let account = TestAccount::create(&app, "erin", "correct-horse-battery").await;
    let session_id = login_desktop(
        &app,
        &account.identifier,
        "correct-horse-battery",
        "fingerprint-2",
    )
    .await;

    // 透過網頁登入取得裝置清單並移除該裝置。
    login_web(&app, &account.identifier, "correct-horse-battery").await;
    let devices: serde_json::Value = app
        .client
        .get(app.url("/api/v1/devices"))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let device_id = devices[0]["id"].as_str().unwrap();
    let del = app
        .client
        .delete(app.url(&format!("/api/v1/devices/{device_id}")))
        .send()
        .await
        .unwrap();
    assert_eq!(del.status(), StatusCode::NO_CONTENT);

    let res = app
        .client
        .post(app.url("/api/v1/sync/batches"))
        .bearer_auth(&session_id)
        .json(&sample_body("fingerprint-2:1"))
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::FORBIDDEN);
    let body: serde_json::Value = res.json().await.unwrap();
    assert_eq!(body["reason"], "device_removed");
}

#[sqlx::test(migrations = "./migrations")]
async fn disabled_account_is_rejected(pool: sqlx::PgPool) {
    let app = spawn_app(pool).await;
    let account = TestAccount::create(&app, "frank", "correct-horse-battery").await;
    let session_id = login_desktop(
        &app,
        &account.identifier,
        "correct-horse-battery",
        "fingerprint-3",
    )
    .await;

    login_web(&app, &account.identifier, "correct-horse-battery").await;
    let del = app
        .client
        .delete(app.url("/api/v1/account"))
        .send()
        .await
        .unwrap();
    assert_eq!(del.status(), StatusCode::NO_CONTENT);

    let res = app
        .client
        .post(app.url("/api/v1/sync/batches"))
        .bearer_auth(&session_id)
        .json(&sample_body("fingerprint-3:1"))
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::FORBIDDEN);
    let body: serde_json::Value = res.json().await.unwrap();
    assert_eq!(body["reason"], "account_disabled");
}
