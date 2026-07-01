//! tracker-core：活動追蹤器的純領域邏輯。
//!
//! 本 crate 不依賴任何作業系統 API，所有規則（工作階段切分、閒置回溯扣除、
//! 午夜切分、彙總排序、主機名解析）皆可在任意平台上單元測試
//! （呼應憲章原則 III：追蹤準確且誠實）。

pub mod aggregation;
pub mod browsers;
pub mod hostname;
pub mod metrics;
pub mod model;
pub mod ports;
pub mod rules;
pub mod session;
pub mod time;

pub use model::{
    ActivityState, AppIdentity, ClosedSession, UNKNOWN_DISPLAY, UNKNOWN_EXECUTABLE,
};
pub use rules::{Exclusions, DEFAULT_IDLE_THRESHOLD_MS, MIN_SESSION_MS};
