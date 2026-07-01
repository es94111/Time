//! 契約測試 `GET /data`（T045）：未登入 401、僅回傳本人資料。

mod support;

use axum::http::StatusCode;
use support::{login_web, spawn_app, TestAccount};

#[sqlx::test(migrations = "./migrations")]
async fn requires_login(pool: sqlx::PgPool) {
    let app = spawn_app(pool).await;
    let res = app
        .client
        .get(app.url("/api/v1/data"))
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::UNAUTHORIZED);
}

#[sqlx::test(migrations = "./migrations")]
async fn only_returns_own_data_not_other_accounts(pool: sqlx::PgPool) {
    let app = spawn_app(pool).await;
    let alice = TestAccount::create(&app, "alice-data", "correct-horse-battery").await;
    let _bob = TestAccount::create(&app, "bob-data", "correct-horse-battery").await;

    login_web(&app, &alice.identifier, "correct-horse-battery").await;
    let res = app
        .client
        .get(app.url("/api/v1/data"))
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    // 新帳號尚無上傳資料，應為空陣列而非他人資料。
    let body: serde_json::Value = res.json().await.unwrap();
    assert_eq!(body.as_array().unwrap().len(), 0);
}

#[sqlx::test(migrations = "./migrations")]
async fn cannot_access_others_data_without_share_grant(pool: sqlx::PgPool) {
    let app = spawn_app(pool).await;
    let alice = TestAccount::create(&app, "alice-priv", "correct-horse-battery").await;
    let bob = TestAccount::create(&app, "bob-priv", "correct-horse-battery").await;

    login_web(&app, &bob.identifier, "correct-horse-battery").await;

    // 無分享授權下，bob 不應能以 owner_id 查詢 alice 的資料。
    let res = app
        .client
        .get(app.url(&format!("/api/v1/data?owner_id={}", alice.id)))
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::FORBIDDEN);
}
