//! 儲存層錯誤型別。

use thiserror::Error;

/// 儲存層統一錯誤。
#[derive(Debug, Error)]
pub enum StorageError {
    /// SQLite／SQLCipher 錯誤。
    #[error("資料庫錯誤：{0}")]
    Sqlite(#[from] rusqlite::Error),

    /// 檔案 I/O 錯誤。
    #[error("輸入／輸出錯誤：{0}")]
    Io(#[from] std::io::Error),

    /// 序列化／反序列化錯誤。
    #[error("序列化錯誤：{0}")]
    Serde(#[from] serde_json::Error),

    /// 金鑰保護（DPAPI 等）失敗。
    #[error("金鑰保護失敗：{0}")]
    KeyProtection(String),

    /// 主密碼錯誤。
    #[error("主密碼錯誤")]
    BadPassword,

    /// 加密／解密失敗。
    #[error("加密作業失敗：{0}")]
    Crypto(String),

    /// 其他內部錯誤。
    #[error("內部錯誤：{0}")]
    Internal(String),
}

/// 儲存層結果別名。
pub type Result<T> = std::result::Result<T, StorageError>;
