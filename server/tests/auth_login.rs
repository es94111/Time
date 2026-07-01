//! 契約測試 `POST /auth/login`（T021）：帳密正確成功、帳密錯誤 401、鎖定中 423。
//!
//! 需要可用的 PostgreSQL（`sqlx::test` 會自動建立/遷移臨時測試資料庫，
//! 依 `DATABASE_URL` 所指向的伺服器；本機開發請先 `docker compose up -d postgres`）。

mod support;

use axum::http::StatusCode;
use serde_json::json;
use support::{spawn_app, TestAccount};

#[sqlx::test(migrations = "./migrations")]
async fn login_succeeds_with_correct_credentials(pool: sqlx::PgPool) {
    let app = spawn_app(pool).await;
    let account = TestAccount::create(&app, "alice", "correct-horse-battery").await;

    let res = app
        .client
        .post(app.url("/api/v1/auth/login"))
        .json(&json!({ "identifier": account.identifier, "password": "correct-horse-battery", "client": "web" }))
        .send()
        .await
        .unwrap();

    assert_eq!(res.status(), StatusCode::OK);
    let body: serde_json::Value = res.json().await.unwrap();
    assert!(body["session_id"].is_string());
}

#[sqlx::test(migrations = "./migrations")]
async fn login_fails_with_wrong_password(pool: sqlx::PgPool) {
    let app = spawn_app(pool).await;
    let account = TestAccount::create(&app, "bob", "correct-horse-battery").await;

    let res = app
        .client
        .post(app.url("/api/v1/auth/login"))
        .json(&json!({ "identifier": account.identifier, "password": "wrong-password", "client": "web" }))
        .send()
        .await
        .unwrap();

    assert_eq!(res.status(), StatusCode::UNAUTHORIZED);
}

#[sqlx::test(migrations = "./migrations")]
async fn login_rejects_when_account_locked(pool: sqlx::PgPool) {
    let app = spawn_app(pool).await;
    let account = TestAccount::create(&app, "carol", "correct-horse-battery").await;

    // 連續 10 次錯誤密碼觸發鎖定（FR-010）。
    for _ in 0..10 {
        let _ = app
            .client
            .post(app.url("/api/v1/auth/login"))
            .json(
                &json!({ "identifier": account.identifier, "password": "wrong", "client": "web" }),
            )
            .send()
            .await
            .unwrap();
    }

    let res = app
        .client
        .post(app.url("/api/v1/auth/login"))
        .json(&json!({ "identifier": account.identifier, "password": "correct-horse-battery", "client": "web" }))
        .send()
        .await
        .unwrap();

    assert_eq!(res.status(), StatusCode::LOCKED);
    let body: serde_json::Value = res.json().await.unwrap();
    assert!(body["retry_after_seconds"].as_i64().unwrap() > 0);
}
