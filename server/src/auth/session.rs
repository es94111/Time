//! Session 服務（T016）：建立、驗證（含撤銷/逾時檢查與滑動視窗延展）、撤銷。
//!
//! `web`：滑動視窗 24 小時（每次通過驗證的請求重算 `expires_at = last_active_at + 24h`，FR-014）。
//! `desktop`：固定 30 天（`expires_at = created_at + 30d`，不隨請求延展）。

use jiff::{SignedDuration, Timestamp};
use sqlx::PgPool;
use uuid::Uuid;

pub const WEB_SLIDING_WINDOW: SignedDuration = SignedDuration::from_hours(24);
pub const DESKTOP_FIXED_TTL: SignedDuration = SignedDuration::from_hours(24 * 30);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SessionKind {
    Web,
    Desktop,
}

impl SessionKind {
    fn as_str(self) -> &'static str {
        match self {
            SessionKind::Web => "web",
            SessionKind::Desktop => "desktop",
        }
    }
}

#[derive(Debug, Clone)]
pub struct AuthSession {
    pub id: String,
    pub account_id: Uuid,
    pub kind: SessionKind,
    pub device_id: Option<Uuid>,
}

pub struct SessionService<'a> {
    pool: &'a PgPool,
}

fn to_chrono(ts: Timestamp) -> chrono::DateTime<chrono::Utc> {
    chrono::DateTime::from_timestamp(ts.as_second(), ts.subsec_nanosecond() as u32)
        .unwrap_or_else(chrono::Utc::now)
}

fn from_chrono(dt: chrono::DateTime<chrono::Utc>) -> Timestamp {
    Timestamp::from_second(dt.timestamp()).unwrap_or(Timestamp::UNIX_EPOCH)
}

impl<'a> SessionService<'a> {
    pub fn new(pool: &'a PgPool) -> Self {
        Self { pool }
    }

    /// 建立新 Session；`web` 初始 `expires_at = now + 24h`，`desktop` 為 `now + 30d`（固定）。
    pub async fn create(
        &self,
        account_id: Uuid,
        kind: SessionKind,
        device_id: Option<Uuid>,
    ) -> Result<AuthSession, sqlx::Error> {
        let id = Uuid::new_v4().to_string();
        let now = Timestamp::now();
        let ttl = match kind {
            SessionKind::Web => WEB_SLIDING_WINDOW,
            SessionKind::Desktop => DESKTOP_FIXED_TTL,
        };
        let expires_at = now + ttl;

        sqlx::query(
            r#"INSERT INTO session (id, account_id, kind, device_id, created_at, last_active_at, expires_at)
               VALUES ($1, $2, $3, $4, $5, $5, $6)"#,
        )
        .bind(&id)
        .bind(account_id)
        .bind(kind.as_str())
        .bind(device_id)
        .bind(to_chrono(now))
        .bind(to_chrono(expires_at))
        .execute(self.pool)
        .await?;

        Ok(AuthSession {
            id,
            account_id,
            kind,
            device_id,
        })
    }

    /// 驗證 Session：檢查 `revoked_at IS NULL AND now() < expires_at`；
    /// `web` 類型驗證通過後 MUST 更新 `last_active_at` 並重算 `expires_at`（滑動視窗，FR-014）。
    pub async fn validate_and_touch(
        &self,
        session_id: &str,
    ) -> Result<Option<AuthSession>, sqlx::Error> {
        let row = sqlx::query_as::<_, SessionRow>(
            r#"SELECT id, account_id, kind, device_id, expires_at, revoked_at
               FROM session WHERE id = $1"#,
        )
        .bind(session_id)
        .fetch_optional(self.pool)
        .await?;

        let Some(row) = row else { return Ok(None) };
        if row.revoked_at.is_some() {
            return Ok(None);
        }
        let now = Timestamp::now();
        if from_chrono(row.expires_at) <= now {
            return Ok(None);
        }

        let kind = match row.kind.as_str() {
            "web" => SessionKind::Web,
            _ => SessionKind::Desktop,
        };

        if kind == SessionKind::Web {
            let new_expiry = now + WEB_SLIDING_WINDOW;
            sqlx::query(r#"UPDATE session SET last_active_at = $2, expires_at = $3 WHERE id = $1"#)
                .bind(session_id)
                .bind(to_chrono(now))
                .bind(to_chrono(new_expiry))
                .execute(self.pool)
                .await?;
        }

        Ok(Some(AuthSession {
            id: row.id,
            account_id: row.account_id,
            kind,
            device_id: row.device_id,
        }))
    }

    /// 撤銷單一 Session（登出，FR-008）。
    pub async fn revoke(&self, session_id: &str) -> Result<(), sqlx::Error> {
        sqlx::query(r#"UPDATE session SET revoked_at = now() WHERE id = $1"#)
            .bind(session_id)
            .execute(self.pool)
            .await?;
        Ok(())
    }

    /// 撤銷帳號下所有其他 Session（變更密碼後，FR-009）。
    pub async fn revoke_all_others(
        &self,
        account_id: Uuid,
        keep_session_id: &str,
    ) -> Result<(), sqlx::Error> {
        sqlx::query(
            r#"UPDATE session SET revoked_at = now()
               WHERE account_id = $1 AND id <> $2 AND revoked_at IS NULL"#,
        )
        .bind(account_id)
        .bind(keep_session_id)
        .execute(self.pool)
        .await?;
        Ok(())
    }
}

#[derive(sqlx::FromRow)]
struct SessionRow {
    id: String,
    account_id: Uuid,
    kind: String,
    device_id: Option<Uuid>,
    expires_at: chrono::DateTime<chrono::Utc>,
    revoked_at: Option<chrono::DateTime<chrono::Utc>>,
}
