//! `share_grant` CRUD 與存取範圍查詢（T047）。

use sqlx::PgPool;
use uuid::Uuid;

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct ShareGrant {
    pub id: Uuid,
    pub owner_account_id: Uuid,
    pub grantee_account_id: Uuid,
    pub scope: String,
    pub revoked_at: Option<chrono::DateTime<chrono::Utc>>,
}

pub struct ShareGrantRepo<'a> {
    pool: &'a PgPool,
}

impl<'a> ShareGrantRepo<'a> {
    pub fn new(pool: &'a PgPool) -> Self {
        Self { pool }
    }

    pub async fn create(
        &self,
        owner_account_id: Uuid,
        grantee_account_id: Uuid,
        scope: &str,
    ) -> Result<ShareGrant, sqlx::Error> {
        sqlx::query_as::<_, ShareGrant>(
            r#"INSERT INTO share_grant (owner_account_id, grantee_account_id, scope)
               VALUES ($1, $2, $3)
               RETURNING id, owner_account_id, grantee_account_id, scope, revoked_at"#,
        )
        .bind(owner_account_id)
        .bind(grantee_account_id)
        .bind(scope)
        .fetch_one(self.pool)
        .await
    }

    pub async fn revoke(&self, id: Uuid) -> Result<(), sqlx::Error> {
        sqlx::query(r#"UPDATE share_grant SET revoked_at = now() WHERE id = $1"#)
            .bind(id)
            .execute(self.pool)
            .await?;
        Ok(())
    }

    /// 被授權帳號目前可存取的（未撤銷）分享清單，僅唯讀存取（FR-006）。
    pub async fn active_grants_for_grantee(
        &self,
        grantee_account_id: Uuid,
    ) -> Result<Vec<ShareGrant>, sqlx::Error> {
        sqlx::query_as::<_, ShareGrant>(
            r#"SELECT id, owner_account_id, grantee_account_id, scope, revoked_at
               FROM share_grant WHERE grantee_account_id = $1 AND revoked_at IS NULL"#,
        )
        .bind(grantee_account_id)
        .fetch_all(self.pool)
        .await
    }
}
