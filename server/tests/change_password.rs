//! 契約測試 `POST /auth/change-password`（T060）：成功後撤銷其他既有 session。

mod support;

use axum::http::StatusCode;
use serde_json::json;
use support::{spawn_app, TestAccount};

#[sqlx::test(migrations = "./migrations")]
async fn changing_password_revokes_other_sessions(pool: sqlx::PgPool) {
    let app = spawn_app(pool).await;
    let account = TestAccount::create(&app, "julia", "old-password-123").await;

    // 兩個獨立的網頁 session（各自獨立的 cookie jar，模擬兩台瀏覽器）。
    let client_a = reqwest::Client::builder()
        .cookie_store(true)
        .build()
        .unwrap();
    let client_b = reqwest::Client::builder()
        .cookie_store(true)
        .build()
        .unwrap();

    for client in [&client_a, &client_b] {
        let res = client
            .post(app.url("/api/v1/auth/login"))
            .json(&json!({ "identifier": account.identifier, "password": "old-password-123", "client": "web" }))
            .send()
            .await
            .unwrap();
        assert!(res.status().is_success());
    }

    // client_a 變更密碼。
    let change = client_a
        .post(app.url("/api/v1/auth/change-password"))
        .json(
            &json!({ "current_password": "old-password-123", "new_password": "new-password-456" }),
        )
        .send()
        .await
        .unwrap();
    assert_eq!(change.status(), StatusCode::OK);

    // client_a 自己的 session 仍應有效（未撤銷自身）。
    let self_check = client_a
        .get(app.url("/api/v1/devices"))
        .send()
        .await
        .unwrap();
    assert_eq!(self_check.status(), StatusCode::OK);

    // client_b 的既有 session 應被撤銷，須重新登入。
    let other_check = client_b
        .get(app.url("/api/v1/devices"))
        .send()
        .await
        .unwrap();
    assert_eq!(other_check.status(), StatusCode::UNAUTHORIZED);
}

#[sqlx::test(migrations = "./migrations")]
async fn rejects_short_new_password(pool: sqlx::PgPool) {
    let app = spawn_app(pool).await;
    let account = TestAccount::create(&app, "kevin", "old-password-123").await;
    let client = reqwest::Client::builder()
        .cookie_store(true)
        .build()
        .unwrap();
    client
        .post(app.url("/api/v1/auth/login"))
        .json(&json!({ "identifier": account.identifier, "password": "old-password-123", "client": "web" }))
        .send()
        .await
        .unwrap();

    let res = client
        .post(app.url("/api/v1/auth/change-password"))
        .json(&json!({ "current_password": "old-password-123", "new_password": "short" }))
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::BAD_REQUEST);
}
