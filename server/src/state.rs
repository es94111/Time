//! 伺服器共享狀態（連線池、物件儲存、設定）。

use std::sync::Arc;

use sqlx::PgPool;

use crate::config::Config;
use crate::storage::s3::ObjectStore;

#[derive(Clone)]
pub struct AppState {
    pub pool: PgPool,
    pub objects: ObjectStore,
    pub config: Arc<Config>,
}
