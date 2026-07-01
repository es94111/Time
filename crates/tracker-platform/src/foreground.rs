//! 前景偵測（T021，FR-001）。
//!
//! 對外提供 [`WinForeground`]（實作 [`ForegroundPort`]）。前景切換以低成本查詢
//! `GetForegroundWindow` 於服務 tick 取得；相較持續輪詢視窗內容，成本極低，
//! 足以維持平均 CPU < 2%（SC-003）。事件式 `SetWinEventHook` 可作為未來優化。

use tracker_core::model::AppIdentity;
use tracker_core::ports::ForegroundPort;

use crate::process::{foreground_app, ForegroundApp};

/// 以 Windows 前景視窗查詢實作的 [`ForegroundPort`]。
pub struct WinForeground;

impl WinForeground {
    /// 建立實例。
    pub fn new() -> Self {
        Self
    }

    /// 取得前景應用程式（含視窗標題），供服務層判斷瀏覽器與無痕。
    pub fn current_full(&self) -> Option<ForegroundApp> {
        foreground_app()
    }
}

impl Default for WinForeground {
    fn default() -> Self {
        Self::new()
    }
}

impl ForegroundPort for WinForeground {
    fn current(&self) -> Option<AppIdentity> {
        foreground_app().map(|f| f.identity)
    }
}
