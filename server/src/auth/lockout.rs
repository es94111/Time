//! 連續失敗計數、寫入/檢查 `locked_until`（15 分鐘鎖定，FR-010，T026）。

use jiff::{SignedDuration, Timestamp};
use sqlx::PgPool;
use uuid::Uuid;

use crate::db::account_repo::{Account, AccountRepo};

/// 連續失敗達此次數即觸發鎖定。
pub const MAX_FAILED_ATTEMPTS: i32 = 10;
/// 鎖定時長。
pub const LOCKOUT_DURATION: SignedDuration = SignedDuration::from_mins(15);

/// 若帳號目前鎖定中，回傳剩餘秒數；否則回傳 `None`。
pub fn remaining_lock_seconds(account: &Account) -> Option<i64> {
    let locked_until = account.locked_until?;
    let locked_until = Timestamp::from_second(locked_until.timestamp()).ok()?;
    let now = Timestamp::now();
    if locked_until > now {
        Some(locked_until.duration_since(now).as_secs())
    } else {
        None
    }
}

/// 登入失敗：累加計數，達門檻即寫入 `locked_until`。
pub async fn record_failure(pool: &PgPool, account_id: Uuid) -> Result<(), sqlx::Error> {
    let repo = AccountRepo::new(pool);
    let count = repo.increment_failed_login(account_id).await?;
    if count >= MAX_FAILED_ATTEMPTS {
        let until = Timestamp::now() + LOCKOUT_DURATION;
        repo.set_locked_until(account_id, until).await?;
    }
    Ok(())
}

/// 登入成功：重置失敗計數與鎖定狀態。
pub async fn record_success(pool: &PgPool, account_id: Uuid) -> Result<(), sqlx::Error> {
    AccountRepo::new(pool).reset_failed_login(account_id).await
}
