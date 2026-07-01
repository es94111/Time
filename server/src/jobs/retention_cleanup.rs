//! 依帳號保留天數清除逾期 `sync_record`＋S3 物件；已刪除帳號 30 天後永久清除（T068，research.md R10）。

use std::time::Duration;

use sqlx::PgPool;

use crate::storage::s3::ObjectStore;

/// 帳號軟刪除後，保留可復原的天數（FR-012）。
const DELETED_ACCOUNT_RETENTION_DAYS: i32 = 30;
/// 排程執行週期。
const RUN_INTERVAL: Duration = Duration::from_secs(3600);

/// 啟動背景排程（`tokio::time::interval`，無需外部排程套件，research.md R10）。
pub fn spawn(pool: PgPool, objects: ObjectStore, default_retention_days: i32) {
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(RUN_INTERVAL);
        loop {
            interval.tick().await;
            if let Err(e) = run_once(&pool, &objects, default_retention_days).await {
                tracing::error!(error = %e, "retention_cleanup 執行失敗");
            }
        }
    });
}

async fn run_once(
    pool: &PgPool,
    objects: &ObjectStore,
    default_retention_days: i32,
) -> Result<(), sqlx::Error> {
    purge_expired_sync_records(pool, objects, default_retention_days).await?;
    purge_deleted_accounts(pool, objects).await?;
    Ok(())
}

/// 依帳號 `retention_days`（或系統預設）刪除逾期的同步紀錄與對應物件（FR-013）。
async fn purge_expired_sync_records(
    pool: &PgPool,
    objects: &ObjectStore,
    default_retention_days: i32,
) -> Result<(), sqlx::Error> {
    let rows: Vec<(uuid::Uuid, String)> = sqlx::query_as(
        r#"SELECT sr.id, sr.object_storage_key
           FROM sync_record sr
           JOIN device d ON d.id = sr.device_id
           JOIN account a ON a.id = d.account_id
           WHERE sr.object_storage_key IS NOT NULL
             AND sr.server_received_at < now() - make_interval(days => COALESCE(a.retention_days, $1))"#,
    )
    .bind(default_retention_days)
    .fetch_all(pool)
    .await?;

    for (id, key) in rows {
        if let Err(e) = objects.delete_object(&key).await {
            tracing::warn!(error = %e, key, "刪除逾期物件失敗，略過本筆並保留紀錄待下次重試");
            continue;
        }
        sqlx::query(r#"DELETE FROM sync_record WHERE id = $1"#)
            .bind(id)
            .execute(pool)
            .await?;
    }
    Ok(())
}

/// 已標記刪除的帳號，於 30 天後永久清除其所有資料（PostgreSQL 記錄 + S3 物件，FR-012）。
async fn purge_deleted_accounts(pool: &PgPool, objects: &ObjectStore) -> Result<(), sqlx::Error> {
    let account_ids: Vec<(uuid::Uuid,)> = sqlx::query_as(
        r#"SELECT id FROM account
           WHERE deleted_at IS NOT NULL AND deleted_at < now() - make_interval(days => $1)"#,
    )
    .bind(DELETED_ACCOUNT_RETENTION_DAYS)
    .fetch_all(pool)
    .await?;

    for (account_id,) in account_ids {
        let keys: Vec<(String,)> = sqlx::query_as(
            r#"SELECT sr.object_storage_key FROM sync_record sr
               JOIN device d ON d.id = sr.device_id
               WHERE d.account_id = $1 AND sr.object_storage_key IS NOT NULL"#,
        )
        .bind(account_id)
        .fetch_all(pool)
        .await?;
        for (key,) in keys {
            let _ = objects.delete_object(&key).await;
        }

        sqlx::query(
            r#"DELETE FROM sync_record WHERE device_id IN (SELECT id FROM device WHERE account_id = $1)"#,
        )
        .bind(account_id)
        .execute(pool)
        .await?;
        sqlx::query(
            r#"DELETE FROM share_grant WHERE owner_account_id = $1 OR grantee_account_id = $1"#,
        )
        .bind(account_id)
        .execute(pool)
        .await?;
        sqlx::query(r#"DELETE FROM session WHERE account_id = $1"#)
            .bind(account_id)
            .execute(pool)
            .await?;
        sqlx::query(r#"DELETE FROM device WHERE account_id = $1"#)
            .bind(account_id)
            .execute(pool)
            .await?;
        sqlx::query(r#"DELETE FROM account WHERE id = $1"#)
            .bind(account_id)
            .execute(pool)
            .await?;
    }
    Ok(())
}
