//! 硬體指紋比對合併規則（T055，data-model.md 狀態轉換）。
//!
//! 同一帳號、同硬體指紋且**尚未被移除**的裝置視為同一裝置（合併，重灌後仍延續同一 `device.id`）；
//! 若原裝置已被使用者移除（`removed_at` 非 NULL），則視為全新裝置，另建一筆紀錄
//! （因 `idx_device_active_fingerprint` 唯一索引僅涵蓋 `removed_at IS NULL` 的列，故可並存）。

use sqlx::PgPool;
use uuid::Uuid;

use crate::db::device_repo::{Device, DeviceRepo};

pub async fn resolve_or_create_device(
    pool: &PgPool,
    account_id: Uuid,
    fingerprint: &str,
    display_name: &str,
) -> Result<Device, sqlx::Error> {
    let repo = DeviceRepo::new(pool);
    if let Some(existing) = repo
        .find_active_by_fingerprint(account_id, fingerprint)
        .await?
    {
        return Ok(existing);
    }
    repo.create(account_id, fingerprint, display_name).await
}
