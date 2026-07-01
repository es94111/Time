//! tracker-platform：Windows 原廠 API 整合（windows crate）。
//!
//! 各模組實作 `tracker-core::ports` 之 trait，使服務層可注入真實的 Windows 來源；
//! 於非 Windows 平台僅提供占位，邏輯測試以核心的假實作進行。

#[cfg(windows)]
pub mod autostart;
#[cfg(windows)]
pub mod browser_url;
#[cfg(windows)]
pub mod foreground;
#[cfg(windows)]
pub mod idle;
#[cfg(windows)]
pub mod process;
#[cfg(windows)]
pub mod secret;
#[cfg(windows)]
pub mod session_power;
