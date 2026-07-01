//! Account CRUD 與依 `email_or_username` 查詢（T024）。

use jiff::Timestamp;
use sqlx::PgPool;
use uuid::Uuid;

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct Account {
    pub id: Uuid,
    pub email_or_username: String,
    pub password_hash: String,
    pub failed_login_count: i32,
    pub locked_until: Option<chrono::DateTime<chrono::Utc>>,
    pub retention_days: Option<i32>,
    pub deleted_at: Option<chrono::DateTime<chrono::Utc>>,
}

pub struct AccountRepo<'a> {
    pool: &'a PgPool,
}

impl<'a> AccountRepo<'a> {
    pub fn new(pool: &'a PgPool) -> Self {
        Self { pool }
    }

    pub async fn create(
        &self,
        identifier: &str,
        password_hash: &str,
    ) -> Result<Account, sqlx::Error> {
        sqlx::query_as::<_, Account>(
            r#"INSERT INTO account (email_or_username, password_hash)
               VALUES ($1, $2)
               RETURNING id, email_or_username, password_hash, failed_login_count, locked_until, retention_days, deleted_at"#,
        )
        .bind(identifier)
        .bind(password_hash)
        .fetch_one(self.pool)
        .await
    }

    pub async fn find_by_identifier(
        &self,
        identifier: &str,
    ) -> Result<Option<Account>, sqlx::Error> {
        sqlx::query_as::<_, Account>(
            r#"SELECT id, email_or_username, password_hash, failed_login_count, locked_until, retention_days, deleted_at
               FROM account WHERE email_or_username = $1 AND deleted_at IS NULL"#,
        )
        .bind(identifier)
        .fetch_optional(self.pool)
        .await
    }

    pub async fn find_by_id(&self, id: Uuid) -> Result<Option<Account>, sqlx::Error> {
        sqlx::query_as::<_, Account>(
            r#"SELECT id, email_or_username, password_hash, failed_login_count, locked_until, retention_days, deleted_at
               FROM account WHERE id = $1"#,
        )
        .bind(id)
        .fetch_optional(self.pool)
        .await
    }

    /// 累加連續失敗次數，回傳累加後的次數（供呼叫端判斷是否觸發鎖定，FR-010）。
    pub async fn increment_failed_login(&self, id: Uuid) -> Result<i32, sqlx::Error> {
        let row: (i32,) = sqlx::query_as(
            r#"UPDATE account SET failed_login_count = failed_login_count + 1
               WHERE id = $1 RETURNING failed_login_count"#,
        )
        .bind(id)
        .fetch_one(self.pool)
        .await?;
        Ok(row.0)
    }

    pub async fn set_locked_until(&self, id: Uuid, until: Timestamp) -> Result<(), sqlx::Error> {
        sqlx::query(r#"UPDATE account SET locked_until = $2 WHERE id = $1"#)
            .bind(id)
            .bind(to_chrono(until))
            .execute(self.pool)
            .await?;
        Ok(())
    }

    pub async fn reset_failed_login(&self, id: Uuid) -> Result<(), sqlx::Error> {
        sqlx::query(
            r#"UPDATE account SET failed_login_count = 0, locked_until = NULL WHERE id = $1"#,
        )
        .bind(id)
        .execute(self.pool)
        .await?;
        Ok(())
    }

    pub async fn update_password_hash(
        &self,
        id: Uuid,
        password_hash: &str,
    ) -> Result<(), sqlx::Error> {
        sqlx::query(r#"UPDATE account SET password_hash = $2 WHERE id = $1"#)
            .bind(id)
            .bind(password_hash)
            .execute(self.pool)
            .await?;
        Ok(())
    }

    pub async fn update_retention_days(
        &self,
        id: Uuid,
        retention_days: Option<i32>,
    ) -> Result<(), sqlx::Error> {
        sqlx::query(r#"UPDATE account SET retention_days = $2 WHERE id = $1"#)
            .bind(id)
            .bind(retention_days)
            .execute(self.pool)
            .await?;
        Ok(())
    }

    /// 軟刪除帳號（FR-012），30 天後由 `jobs::retention_cleanup` 永久清除。
    pub async fn soft_delete(&self, id: Uuid) -> Result<(), sqlx::Error> {
        sqlx::query(r#"UPDATE account SET deleted_at = now() WHERE id = $1"#)
            .bind(id)
            .execute(self.pool)
            .await?;
        Ok(())
    }
}

fn to_chrono(ts: Timestamp) -> chrono::DateTime<chrono::Utc> {
    chrono::DateTime::from_timestamp(ts.as_second(), ts.subsec_nanosecond() as u32)
        .unwrap_or_else(chrono::Utc::now)
}
