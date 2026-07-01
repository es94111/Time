//! 單元測試：裝置合併規則（同硬體指紋合併回原裝置；已移除裝置視為全新綁定，T053）。

use server::db::account_repo::AccountRepo;
use server::db::device_repo::DeviceRepo;
use server::jobs::device_merge::resolve_or_create_device;

#[sqlx::test(migrations = "./migrations")]
async fn same_fingerprint_merges_to_existing_active_device(pool: sqlx::PgPool) {
    let account = AccountRepo::new(&pool)
        .create("merge-test-1", "hash")
        .await
        .unwrap();

    let first = resolve_or_create_device(&pool, account.id, "guid-1", "裝置A")
        .await
        .unwrap();
    let second = resolve_or_create_device(&pool, account.id, "guid-1", "裝置A（重新登入）")
        .await
        .unwrap();

    assert_eq!(
        first.id, second.id,
        "同一帳號、同硬體指紋且未被移除，應合併回同一裝置"
    );
}

#[sqlx::test(migrations = "./migrations")]
async fn removed_device_causes_new_binding_on_next_login(pool: sqlx::PgPool) {
    let account = AccountRepo::new(&pool)
        .create("merge-test-2", "hash")
        .await
        .unwrap();

    let original = resolve_or_create_device(&pool, account.id, "guid-2", "裝置B")
        .await
        .unwrap();
    DeviceRepo::new(&pool)
        .mark_removed(original.id)
        .await
        .unwrap();

    let rebound = resolve_or_create_device(&pool, account.id, "guid-2", "裝置B（重灌後）")
        .await
        .unwrap();

    assert_ne!(
        original.id, rebound.id,
        "原裝置已被移除，重新登入應視為全新裝置"
    );
    assert_eq!(rebound.hardware_fingerprint, "guid-2");
}

#[sqlx::test(migrations = "./migrations")]
async fn different_fingerprints_never_merge(pool: sqlx::PgPool) {
    let account = AccountRepo::new(&pool)
        .create("merge-test-3", "hash")
        .await
        .unwrap();

    let a = resolve_or_create_device(&pool, account.id, "guid-a", "裝置A")
        .await
        .unwrap();
    let b = resolve_or_create_device(&pool, account.id, "guid-b", "裝置B")
        .await
        .unwrap();

    assert_ne!(a.id, b.id);
}
