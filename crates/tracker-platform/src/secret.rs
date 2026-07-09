//! DPAPI 金鑰保護（T009，research.md R7）。
//!
//! 以 `CryptProtectData`／`CryptUnprotectData`（使用者範圍）保護資料金鑰，
//! 綁定當前 Windows 使用者帳戶（FR-016）。

use tracker_core::ports::{PortError, SecretStore};
use windows::core::PCWSTR;
use windows::Win32::Foundation::{LocalFree, HLOCAL};
use windows::Win32::Security::Cryptography::{
    CryptProtectData, CryptUnprotectData, CRYPTPROTECT_UI_FORBIDDEN, CRYPT_INTEGER_BLOB,
};

/// 以 DPAPI 實作的 [`SecretStore`]。
pub struct DpapiSecretStore;

impl DpapiSecretStore {
    /// 建立實例。
    pub fn new() -> Self {
        Self
    }
}

impl Default for DpapiSecretStore {
    fn default() -> Self {
        Self::new()
    }
}

fn in_blob(data: &[u8]) -> CRYPT_INTEGER_BLOB {
    CRYPT_INTEGER_BLOB { cbData: data.len() as u32, pbData: data.as_ptr() as *mut u8 }
}

unsafe fn take_blob(blob: &CRYPT_INTEGER_BLOB) -> Vec<u8> {
    let slice = std::slice::from_raw_parts(blob.pbData, blob.cbData as usize);
    let out = slice.to_vec();
    // DPAPI 以 LocalAlloc 配置輸出，須以 LocalFree 釋放。
    let _ = LocalFree(Some(HLOCAL(blob.pbData as *mut core::ffi::c_void)));
    out
}

impl SecretStore for DpapiSecretStore {
    fn protect(&self, plaintext: &[u8]) -> Result<Vec<u8>, PortError> {
        unsafe {
            let input = in_blob(plaintext);
            let mut output = CRYPT_INTEGER_BLOB::default();
            CryptProtectData(
                &input,
                PCWSTR::null(),
                None,
                None,
                None,
                CRYPTPROTECT_UI_FORBIDDEN,
                &mut output,
            )
            .map_err(|e| PortError(format!("CryptProtectData 失敗：{e}")))?;
            Ok(take_blob(&output))
        }
    }

    fn unprotect(&self, ciphertext: &[u8]) -> Result<Vec<u8>, PortError> {
        unsafe {
            let input = in_blob(ciphertext);
            let mut output = CRYPT_INTEGER_BLOB::default();
            CryptUnprotectData(
                &input,
                None,
                None,
                None,
                None,
                CRYPTPROTECT_UI_FORBIDDEN,
                &mut output,
            )
            .map_err(|e| PortError(format!("CryptUnprotectData 失敗：{e}")))?;
            Ok(take_blob(&output))
        }
    }
}
