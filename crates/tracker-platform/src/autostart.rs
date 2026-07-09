//! 登入自啟（T025，FR-002，research.md R8）。
//!
//! 以 `HKCU\Software\Microsoft\Windows\CurrentVersion\Run` 登錄項註冊／移除，
//! 免系統管理員權限、僅影響目前使用者。

use std::iter::once;

use windows::core::{w, PCWSTR};
use windows::Win32::Foundation::{ERROR_FILE_NOT_FOUND, ERROR_SUCCESS};
use windows::Win32::System::Registry::{
    RegCloseKey, RegCreateKeyExW, RegDeleteValueW, RegQueryValueExW, RegSetValueExW, HKEY,
    HKEY_CURRENT_USER, KEY_READ, KEY_WRITE, REG_OPTION_NON_VOLATILE, REG_SZ, REG_VALUE_TYPE,
};

const RUN_KEY: PCWSTR = w!("Software\\Microsoft\\Windows\\CurrentVersion\\Run");
const VALUE_NAME: PCWSTR = w!("ActivityTracker");

/// 設定或移除登入自啟。回傳是否成功。
pub fn set_autostart(enabled: bool, exe_path: &str) -> bool {
    unsafe {
        let mut hkey = HKEY::default();
        let opened = RegCreateKeyExW(
            HKEY_CURRENT_USER,
            RUN_KEY,
            None,
            PCWSTR::null(),
            REG_OPTION_NON_VOLATILE,
            KEY_READ | KEY_WRITE,
            None,
            &mut hkey,
            None,
        );
        if opened != ERROR_SUCCESS {
            return false;
        }
        let result = if enabled {
            let quoted = format!("\"{exe_path}\"");
            let wide: Vec<u16> = quoted.encode_utf16().chain(once(0)).collect();
            let bytes = std::slice::from_raw_parts(wide.as_ptr() as *const u8, wide.len() * 2);
            RegSetValueExW(hkey, VALUE_NAME, None, REG_SZ, Some(bytes)) == ERROR_SUCCESS
        } else {
            let r = RegDeleteValueW(hkey, VALUE_NAME);
            r == ERROR_SUCCESS || r == ERROR_FILE_NOT_FOUND
        };
        let _ = RegCloseKey(hkey);
        result
    }
}

/// 查詢登入自啟是否已啟用。
pub fn is_autostart_enabled() -> bool {
    unsafe {
        let mut hkey = HKEY::default();
        let opened = RegCreateKeyExW(
            HKEY_CURRENT_USER,
            RUN_KEY,
            None,
            PCWSTR::null(),
            REG_OPTION_NON_VOLATILE,
            KEY_READ,
            None,
            &mut hkey,
            None,
        );
        if opened != ERROR_SUCCESS {
            return false;
        }
        let mut kind = REG_VALUE_TYPE::default();
        let mut size: u32 = 0;
        let exists =
            RegQueryValueExW(hkey, VALUE_NAME, None, Some(&mut kind), None, Some(&mut size))
                == ERROR_SUCCESS;
        let _ = RegCloseKey(hkey);
        exists
    }
}
