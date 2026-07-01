//! 契約測試網頁 Session 滑動視窗（T063，FR-014）：操作後 `expires_at` 依
//! `last_active_at + 24h` 重新計算；自最後一次操作起滿 24 小時無操作後，
//! 該 session MUST 失效並拒絕存取。

use server::auth::session::{SessionKind, SessionService};
use server::db::account_repo::AccountRepo;

#[sqlx::test(migrations = "./migrations")]
async fn active_use_extends_expiry_sliding_window(pool: sqlx::PgPool) {
    let account = AccountRepo::new(&pool)
        .create("sliding-1", "hash")
        .await
        .unwrap();
    let service = SessionService::new(&pool);
    let session = service
        .create(account.id, SessionKind::Web, None)
        .await
        .unwrap();

    let before: (chrono::DateTime<chrono::Utc>,) =
        sqlx::query_as("SELECT expires_at FROM session WHERE id = $1")
            .bind(&session.id)
            .fetch_one(&pool)
            .await
            .unwrap();

    // 模擬時間流逝後再次操作（提早設定 last_active_at/expires_at 至較早時間，驗證滑動延展）。
    sqlx::query("UPDATE session SET last_active_at = now() - interval '1 hour', expires_at = now() + interval '23 hours' WHERE id = $1")
        .bind(&session.id)
        .execute(&pool)
        .await
        .unwrap();

    let validated = service.validate_and_touch(&session.id).await.unwrap();
    assert!(validated.is_some(), "尚未逾期的 session 應通過驗證");

    let after: (chrono::DateTime<chrono::Utc>,) =
        sqlx::query_as("SELECT expires_at FROM session WHERE id = $1")
            .bind(&session.id)
            .fetch_one(&pool)
            .await
            .unwrap();

    assert!(
        after.0 > before.0,
        "每次操作後應依 last_active_at + 24h 重新計算 expires_at（滑動視窗）"
    );
}

#[sqlx::test(migrations = "./migrations")]
async fn session_expires_after_24h_without_activity(pool: sqlx::PgPool) {
    let account = AccountRepo::new(&pool)
        .create("sliding-2", "hash")
        .await
        .unwrap();
    let service = SessionService::new(&pool);
    let session = service
        .create(account.id, SessionKind::Web, None)
        .await
        .unwrap();

    // 模擬「自最後一次操作起已滿 24 小時無操作」：expires_at 設為過去。
    sqlx::query(
        "UPDATE session SET last_active_at = now() - interval '25 hours', expires_at = now() - interval '1 hour' WHERE id = $1",
    )
    .bind(&session.id)
    .execute(&pool)
    .await
    .unwrap();

    let validated = service.validate_and_touch(&session.id).await.unwrap();
    assert!(
        validated.is_none(),
        "滿 24 小時無操作後該 session 須失效並拒絕存取"
    );
}
