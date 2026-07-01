//! tracker-sync：用戶端同步代理（登入、佇列補傳、背景輪詢）。
//!
//! - `auth_client`：呼叫伺服器登入/登出 API，憑證以 DPAPI 保護寫入本機 `auth_cache`。
//! - `queue_repo`：封裝待同步佇列讀寫（沿用 `tracker-storage::sync_queue_repo`）。
//! - `uploader`：`reqwest` 呼叫 `POST /sync/batches`，固定批次＋節流補傳。
//! - `agent`：每 1 分鐘輪詢的背景同步迴圈（登入後啟動、登出即停止）。

pub mod agent;
pub mod auth_client;
pub mod queue_repo;
pub mod uploader;
