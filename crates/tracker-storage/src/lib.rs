//! tracker-storage：加密本機儲存（SQLCipher）與匯出。
//!
//! - `crypto`：DEK 產生、DPAPI 包裹、可選 Argon2id 主密碼、zeroize。
//! - `db`：以 SQLCipher 金鑰開啟連線、PRAGMA、遷移。
//! - `repository`：工作階段寫入與摘要查詢。
//! - `export`：CSV／JSON 匯出（含 UTF-8 BOM）。

pub mod crypto;
pub mod db;
pub mod error;
pub mod export;
pub mod metrics_repo;
pub mod repository;

pub use db::Database;
pub use error::{Result, StorageError};
pub use repository::Repository;
