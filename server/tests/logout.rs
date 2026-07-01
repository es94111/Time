//! 契約測試 `POST /auth/logout`（T062）：登出後該 session 之 `revoked_at` 立即生效，
//! 後續以該 session 存取任何端點 MUST 回應 401（FR-008）。

mod support;

use axum::http::StatusCode;
use serde_json::json;
use support::{spawn_app, TestAccount};

#[sqlx::test(migrations = "./migrations")]
async fn logout_immediately_invalidates_session(pool: sqlx::PgPool) {
    let app = spawn_app(pool).await;
    let account = TestAccount::create(&app, "mona", "correct-horse-battery").await;

    let login_res = app
        .client
        .post(app.url("/api/v1/auth/login"))
        .json(&json!({ "identifier": account.identifier, "password": "correct-horse-battery", "client": "web" }))
        .send()
        .await
        .unwrap();
    assert!(login_res.status().is_success());

    // 登出前可正常存取。
    let before = app
        .client
        .get(app.url("/api/v1/devices"))
        .send()
        .await
        .unwrap();
    assert_eq!(before.status(), StatusCode::OK);

    let logout_res = app
        .client
        .post(app.url("/api/v1/auth/logout"))
        .send()
        .await
        .unwrap();
    assert_eq!(logout_res.status(), StatusCode::NO_CONTENT);

    // 登出後立即失效，同一 Cookie 存取任何端點皆須 401。
    let after = app
        .client
        .get(app.url("/api/v1/devices"))
        .send()
        .await
        .unwrap();
    assert_eq!(after.status(), StatusCode::UNAUTHORIZED);

    let after_data = app
        .client
        .get(app.url("/api/v1/data"))
        .send()
        .await
        .unwrap();
    assert_eq!(after_data.status(), StatusCode::UNAUTHORIZED);
}
