//! 平台抽象 trait（T008）：使核心與服務層可在非 Windows 以假實作測試。
//!
//! 真正的 Windows 實作位於 `tracker-platform`；金鑰保護位於 `tracker-storage`/平台層。

use crate::model::AppIdentity;

/// 平台層錯誤（保持簡單，細節由實作層記錄）。
#[derive(Debug, Clone)]
pub struct PortError(pub String);

impl std::fmt::Display for PortError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "平台錯誤：{}", self.0)
    }
}

impl std::error::Error for PortError {}

/// 前景應用程式偵測來源（對應 FR-001）。
pub trait ForegroundPort: Send + Sync {
    /// 目前前景應用程式；無法辨識時回傳 `None`（呼叫端會轉為「未知／其他」）。
    fn current(&self) -> Option<AppIdentity>;
}

/// 閒置偵測來源（對應 FR-003）。
pub trait IdlePort: Send + Sync {
    /// 距最後一次使用者輸入的毫秒數。
    fn idle_ms(&self) -> i64;
}

/// 瀏覽器網址偵測來源（對應 FR-005）。
pub trait BrowserUrlPort: Send + Sync {
    /// 目前前景瀏覽器網址列的 URL；無法取得或無痕時回傳 `None`。
    fn current_url(&self) -> Option<String>;
}

/// 工作階段／電源狀態（鎖定、睡眠）。`true` 表示桌面互動可累計活躍。
pub trait SessionPowerPort: Send + Sync {
    /// 系統目前是否處於可累計活躍的狀態（未鎖定、未睡眠）。
    fn is_session_active(&self) -> bool;
}

/// 機密保護（DPAPI 等作業系統金鑰保護）。
pub trait SecretStore: Send + Sync {
    /// 以使用者範圍保護資料（加密）。
    fn protect(&self, plaintext: &[u8]) -> Result<Vec<u8>, PortError>;
    /// 解保護（解密）先前以 [`SecretStore::protect`] 保護的資料。
    fn unprotect(&self, ciphertext: &[u8]) -> Result<Vec<u8>, PortError>;
}
