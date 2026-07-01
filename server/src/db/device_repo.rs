//! device CRUD、依 `(account_id, hardware_fingerprint)` 查詢（T054）。

use sqlx::PgPool;
use uuid::Uuid;

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct Device {
    pub id: Uuid,
    pub account_id: Uuid,
    pub hardware_fingerprint: String,
    pub display_name: String,
    pub last_sync_at: Option<chrono::DateTime<chrono::Utc>>,
    pub removed_at: Option<chrono::DateTime<chrono::Utc>>,
}

pub struct DeviceRepo<'a> {
    pool: &'a PgPool,
}

impl<'a> DeviceRepo<'a> {
    pub fn new(pool: &'a PgPool) -> Self {
        Self { pool }
    }

    /// 查詢帳號下「使用中」（未移除）之裝置，依硬體指紋比對（合併規則，data-model.md）。
    pub async fn find_active_by_fingerprint(
        &self,
        account_id: Uuid,
        fingerprint: &str,
    ) -> Result<Option<Device>, sqlx::Error> {
        sqlx::query_as::<_, Device>(
            r#"SELECT id, account_id, hardware_fingerprint, display_name, last_sync_at, removed_at
               FROM device WHERE account_id = $1 AND hardware_fingerprint = $2 AND removed_at IS NULL"#,
        )
        .bind(account_id)
        .bind(fingerprint)
        .fetch_optional(self.pool)
        .await
    }

    pub async fn create(
        &self,
        account_id: Uuid,
        fingerprint: &str,
        display_name: &str,
    ) -> Result<Device, sqlx::Error> {
        sqlx::query_as::<_, Device>(
            r#"INSERT INTO device (account_id, hardware_fingerprint, display_name)
               VALUES ($1, $2, $3)
               RETURNING id, account_id, hardware_fingerprint, display_name, last_sync_at, removed_at"#,
        )
        .bind(account_id)
        .bind(fingerprint)
        .bind(display_name)
        .fetch_one(self.pool)
        .await
    }

    pub async fn find_by_id(&self, id: Uuid) -> Result<Option<Device>, sqlx::Error> {
        sqlx::query_as::<_, Device>(
            r#"SELECT id, account_id, hardware_fingerprint, display_name, last_sync_at, removed_at
               FROM device WHERE id = $1"#,
        )
        .bind(id)
        .fetch_optional(self.pool)
        .await
    }

    pub async fn list_by_account(&self, account_id: Uuid) -> Result<Vec<Device>, sqlx::Error> {
        sqlx::query_as::<_, Device>(
            r#"SELECT id, account_id, hardware_fingerprint, display_name, last_sync_at, removed_at
               FROM device WHERE account_id = $1 ORDER BY created_at"#,
        )
        .bind(account_id)
        .fetch_all(self.pool)
        .await
    }

    pub async fn mark_removed(&self, id: Uuid) -> Result<(), sqlx::Error> {
        sqlx::query(r#"UPDATE device SET removed_at = now() WHERE id = $1"#)
            .bind(id)
            .execute(self.pool)
            .await?;
        Ok(())
    }

    pub async fn touch_last_sync(&self, id: Uuid) -> Result<(), sqlx::Error> {
        sqlx::query(r#"UPDATE device SET last_sync_at = now() WHERE id = $1"#)
            .bind(id)
            .execute(self.pool)
            .await?;
        Ok(())
    }
}
