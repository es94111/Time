//! 閒置偵測（T023，FR-003）：以 `GetLastInputInfo` 計算自上次輸入經過毫秒。

use tracker_core::ports::IdlePort;
use windows::Win32::System::SystemInformation::GetTickCount;
use windows::Win32::UI::Input::KeyboardAndMouse::{GetLastInputInfo, LASTINPUTINFO};

/// 以 Windows `GetLastInputInfo` 實作的 [`IdlePort`]。
pub struct WinIdle;

impl WinIdle {
    /// 建立實例。
    pub fn new() -> Self {
        Self
    }
}

impl Default for WinIdle {
    fn default() -> Self {
        Self::new()
    }
}

impl IdlePort for WinIdle {
    fn idle_ms(&self) -> i64 {
        unsafe {
            let mut lii = LASTINPUTINFO {
                cbSize: std::mem::size_of::<LASTINPUTINFO>() as u32,
                dwTime: 0,
            };
            if GetLastInputInfo(&mut lii).as_bool() {
                let now = GetTickCount();
                // GetTickCount 為 32-bit 毫秒計數；wrapping_sub 正確處理小幅回繞。
                now.wrapping_sub(lii.dwTime) as i64
            } else {
                0
            }
        }
    }
}
