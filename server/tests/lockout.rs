//! 契約測試連續 10 次登入失敗觸發鎖定、並於 15 分鐘後解除（T061）。
//! 與 `tests/auth_login.rs` 之單次登入請求案例區分：本測試聚焦失敗計數累積、
//! 鎖定觸發與到期解除之完整生命週期。

mod support;

use axum::http::StatusCode;
use serde_json::json;
use server::auth::lockout::{LOCKOUT_DURATION, MAX_FAILED_ATTEMPTS};
use server::db::account_repo::AccountRepo;
use support::{spawn_app, TestAccount};

#[sqlx::test(migrations = "./migrations")]
async fn locks_after_max_failed_attempts_and_unlocks_after_duration(pool: sqlx::PgPool) {
    let app = spawn_app(pool.clone()).await;
    let account = TestAccount::create(&app, "laura", "correct-horse-battery").await;

    // 依契約（rest-api.md）：帳密錯誤一律回應 401 並使 failed_login_count += 1；
    // 達 10 次後於**下一次**請求前即被鎖定（423）。故連續 10 次錯誤密碼皆為 401。
    for i in 0..MAX_FAILED_ATTEMPTS {
        let res = app
            .client
            .post(app.url("/api/v1/auth/login"))
            .json(
                &json!({ "identifier": account.identifier, "password": "wrong", "client": "web" }),
            )
            .send()
            .await
            .unwrap();
        assert_eq!(res.status(), StatusCode::UNAUTHORIZED, "第 {} 次失敗應回應 401", i + 1);
    }

    // 鎖定中，即使密碼正確也應拒絕。
    let still_locked = app
        .client
        .post(app.url("/api/v1/auth/login"))
        .json(&json!({ "identifier": account.identifier, "password": "correct-horse-battery", "client": "web" }))
        .send()
        .await
        .unwrap();
    assert_eq!(still_locked.status(), StatusCode::LOCKED);

    // 模擬鎖定時長已過（直接調整 locked_until 至過去），驗證鎖定到期後可正常登入。
    let acc = AccountRepo::new(&pool)
        .find_by_identifier(&account.identifier)
        .await
        .unwrap()
        .unwrap();
    let past = jiff::Timestamp::now() - LOCKOUT_DURATION - jiff::SignedDuration::from_secs(1);
    AccountRepo::new(&pool)
        .set_locked_until(acc.id, past)
        .await
        .unwrap();

    let after_unlock = app
        .client
        .post(app.url("/api/v1/auth/login"))
        .json(&json!({ "identifier": account.identifier, "password": "correct-horse-battery", "client": "web" }))
        .send()
        .await
        .unwrap();
    assert_eq!(
        after_unlock.status(),
        StatusCode::OK,
        "鎖定到期後應可正常登入"
    );
}
