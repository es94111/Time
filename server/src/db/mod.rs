//! PostgreSQL 連線池建立與啟動時 migrations（T013）。

pub mod account_repo;
pub mod device_repo;
pub mod share_grant_repo;
pub mod sync_record_repo;

use sqlx::postgres::PgPoolOptions;
use sqlx::PgPool;

/// 建立連線池並執行所有待套用的 migrations。
pub async fn connect_and_migrate(database_url: &str) -> Result<PgPool, sqlx::Error> {
    let pool = PgPoolOptions::new()
        .max_connections(10)
        .connect(database_url)
        .await?;
    sqlx::migrate!("./migrations").run(&pool).await?;
    Ok(pool)
}
