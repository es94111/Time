//! 領域型別（T006）：對應 data-model.md 之 Application／Website／ActivitySession。

use serde::{Deserialize, Serialize};

/// 無法辨識前景程序時使用的保留執行檔鍵（FR-015）。
pub const UNKNOWN_EXECUTABLE: &str = "<unknown>";
/// 無法辨識前景程序時的顯示名稱（zh-TW）。
pub const UNKNOWN_DISPLAY: &str = "未知／其他";

/// 工作階段的活躍狀態。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ActivityState {
    /// 前景且距最後輸入未超過閒置門檻。
    Active,
    /// 距最後輸入已超過閒置門檻。
    Idle,
}

/// 前景應用程式的身分（由平台層解析；核心只負責規則）。
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct AppIdentity {
    /// 易讀名稱，如「Visual Studio Code」。
    pub display_name: String,
    /// 執行檔名（穩定識別鍵），如 `code.exe`。
    pub executable: String,
    /// 是否為已知瀏覽器（決定是否做網站偵測）。
    pub is_browser: bool,
}

impl AppIdentity {
    /// 建立「未知／其他」身分（FR-015）。
    pub fn unknown() -> Self {
        Self {
            display_name: UNKNOWN_DISPLAY.to_string(),
            executable: UNKNOWN_EXECUTABLE.to_string(),
            is_browser: false,
        }
    }

    /// 是否為「未知／其他」保留身分。
    pub fn is_unknown(&self) -> bool {
        self.executable == UNKNOWN_EXECUTABLE
    }
}

/// 已結算並關閉的工作階段（核心輸出；DB 寫入前尚未具備 id）。
///
/// 每筆僅屬一個本機日期（午夜切分後，FR-006）。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ClosedSession {
    /// 前景應用程式身分。
    pub app: AppIdentity,
    /// 完整主機名（僅瀏覽器且成功辨識時有值）。
    pub website: Option<String>,
    /// 起始瞬時（epoch 毫秒，UTC）。
    pub start_utc_ms: i64,
    /// 結束瞬時（epoch 毫秒，UTC）。
    pub end_utc_ms: i64,
    /// 該段活躍毫秒數（已扣除回溯閒置）。
    pub active_ms: i64,
    /// 該段所屬之本機日期（YYYY-MM-DD）。
    pub local_date: String,
}

impl ClosedSession {
    /// 不變式檢查：時間順序與 active_ms 範圍（對應 data-model.md 之 CHECK）。
    pub fn is_valid(&self) -> bool {
        self.end_utc_ms > self.start_utc_ms
            && self.active_ms >= 0
            && self.active_ms <= self.end_utc_ms - self.start_utc_ms
    }
}
