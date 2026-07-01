//! 契約測試 `GET /devices`、`DELETE /devices/{id}`（T052）。

mod support;

use axum::http::StatusCode;
use support::{login_desktop, login_web, spawn_app, TestAccount};

#[sqlx::test(migrations = "./migrations")]
async fn lists_devices_created_by_desktop_login(pool: sqlx::PgPool) {
    let app = spawn_app(pool).await;
    let account = TestAccount::create(&app, "henry", "correct-horse-battery").await;
    let _session_id = login_desktop(
        &app,
        &account.identifier,
        "correct-horse-battery",
        "fp-device-routes",
    )
    .await;

    login_web(&app, &account.identifier, "correct-horse-battery").await;
    let res = app
        .client
        .get(app.url("/api/v1/devices"))
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let devices: serde_json::Value = res.json().await.unwrap();
    let devices = devices.as_array().unwrap();
    assert_eq!(devices.len(), 1);
    assert_eq!(devices[0]["removed"], false);
}

#[sqlx::test(migrations = "./migrations")]
async fn removing_device_hides_it_from_list(pool: sqlx::PgPool) {
    let app = spawn_app(pool).await;
    let account = TestAccount::create(&app, "iris", "correct-horse-battery").await;
    let _session_id = login_desktop(
        &app,
        &account.identifier,
        "correct-horse-battery",
        "fp-device-remove",
    )
    .await;

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

    let after: serde_json::Value = app
        .client
        .get(app.url("/api/v1/devices"))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(after.as_array().unwrap().len(), 0);
}
