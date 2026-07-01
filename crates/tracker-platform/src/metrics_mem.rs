//! 記憶體採集（T013、FR-002）：以 `GlobalMemoryStatusEx` 取實體記憶體 used/total。

use windows::Win32::System::SystemInformation::{GlobalMemoryStatusEx, MEMORYSTATUSEX};

/// 讀取實體記憶體（已用位元組, 總量位元組）；失敗時各回 `None`（缺值不假冒，原則 III）。
pub fn read() -> (Option<i64>, Option<i64>) {
    let mut status = MEMORYSTATUSEX {
        dwLength: std::mem::size_of::<MEMORYSTATUSEX>() as u32,
        ..Default::default()
    };
    // SAFETY：`status` 為合法可寫緩衝且已設定 `dwLength`。
    let ok = unsafe { GlobalMemoryStatusEx(&mut status) }.is_ok();
    if !ok {
        return (None, None);
    }
    let total = status.ullTotalPhys as i64;
    let avail = status.ullAvailPhys as i64;
    let used = (total - avail).max(0);
    (Some(used), Some(total))
}
