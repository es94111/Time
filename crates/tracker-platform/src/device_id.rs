//! 裝置指紋（T019，research.md R6）：讀取登錄檔 `HKLM\SOFTWARE\Microsoft\Cryptography\MachineGuid`。
//!
//! 以 trait 抽象平台來源，使核心/服務層可在非 Windows 以假實作測試（T022）。

/// 硬體裝置指紋來源。
pub trait DeviceIdSource: Send + Sync {
    /// 讀取本機硬體指紋；讀取失敗時回傳 `None`（呼叫端應提示使用者或重試）。
    fn machine_guid(&self) -> Option<String>;
}

/// 測試用假來源。
pub struct FakeDeviceIdSource {
    pub guid: Option<String>,
}

impl FakeDeviceIdSource {
    pub fn new(guid: impl Into<String>) -> Self {
        Self { guid: Some(guid.into()) }
    }

    pub fn missing() -> Self {
        Self { guid: None }
    }
}

impl DeviceIdSource for FakeDeviceIdSource {
    fn machine_guid(&self) -> Option<String> {
        self.guid.clone()
    }
}

/// 建立本平台預設裝置指紋來源；Windows 上讀取真實登錄檔，其餘平台回傳假來源。
pub fn default_source() -> Box<dyn DeviceIdSource> {
    #[cfg(windows)]
    {
        Box::new(WinDeviceIdSource)
    }
    #[cfg(not(windows))]
    {
        Box::new(FakeDeviceIdSource::missing())
    }
}

#[cfg(windows)]
pub use win::WinDeviceIdSource;

#[cfg(windows)]
mod win {
    use super::DeviceIdSource;
    use windows::core::PCWSTR;
    use windows::Win32::System::Registry::{
        RegCloseKey, RegOpenKeyExW, RegQueryValueExW, HKEY, HKEY_LOCAL_MACHINE, KEY_READ, REG_SZ, REG_VALUE_TYPE,
    };

    /// 以 Windows 官方登錄檔 API 讀取 `MachineGuid` 之原廠實作。
    pub struct WinDeviceIdSource;

    fn to_wide(s: &str) -> Vec<u16> {
        s.encode_utf16().chain(std::iter::once(0)).collect()
    }

    impl DeviceIdSource for WinDeviceIdSource {
        fn machine_guid(&self) -> Option<String> {
            unsafe {
                let subkey = to_wide(r"SOFTWARE\Microsoft\Cryptography");
                let mut hkey = HKEY::default();
                // `WIN32_ERROR::ok()` 轉為 `windows::core::Result<()>`，再以標準 `Result::ok()` 轉 `Option`。
                RegOpenKeyExW(HKEY_LOCAL_MACHINE, PCWSTR(subkey.as_ptr()), 0u32, KEY_READ, &mut hkey).ok().ok()?;

                let value_name = to_wide("MachineGuid");
                let mut buf = [0u16; 64];
                let mut buf_len: u32 = (buf.len() * std::mem::size_of::<u16>()) as u32;
                let mut value_type = REG_SZ;

                let result = RegQueryValueExW(
                    hkey,
                    PCWSTR(value_name.as_ptr()),
                    None,
                    Some(&mut value_type as *mut REG_VALUE_TYPE),
                    Some(buf.as_mut_ptr() as *mut u8),
                    Some(&mut buf_len),
                );
                let _ = RegCloseKey(hkey);
                result.ok().ok()?;

                let len_u16 = (buf_len as usize / 2).saturating_sub(1).min(buf.len());
                Some(String::from_utf16_lossy(&buf[..len_u16]))
            }
        }
    }
}
