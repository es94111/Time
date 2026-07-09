//! 加密金鑰管理（T010，research.md R7）。
//!
//! 縱深防禦：
//! 1. 資料金鑰（DEK）以 OS CSPRNG 產生 256-bit。
//! 2. DEK 以 Windows DPAPI（[`SecretStore`]）保護後存於本機金鑰檔。
//! 3. 可選主密碼：以 Argon2id 衍生 KEK，再以 AES-256-GCM 包裹 DEK（雙因子）。
//! 4. 敏感緩衝以 `zeroize` 清除。
//!
//! DEK 直接作為 SQLCipher 的 256-bit 原始金鑰（見 `db.rs`）。

use std::path::Path;

use aes_gcm::aead::{Aead, KeyInit};
use aes_gcm::{Aes256Gcm, Key, Nonce};
use argon2::{Algorithm, Argon2, Params, Version};
use serde::{Deserialize, Serialize};
use tracker_core::ports::SecretStore;
use zeroize::Zeroizing;

use crate::error::{Result, StorageError};

/// DEK 位元組長度（256-bit）。
pub const DEK_LEN: usize = 32;
const SALT_LEN: usize = 16;
const NONCE_LEN: usize = 12;

/// 本機金鑰檔內容（序列化為 JSON）。位元組欄位以數字陣列存放。
#[derive(Debug, Serialize, Deserialize)]
struct KeyFile {
    version: u32,
    /// 是否以主密碼額外包裹（Argon2id + AES-GCM）。
    password_protected: bool,
    /// Argon2id salt（password_protected 時有值）。
    salt: Option<Vec<u8>>,
    /// AES-GCM nonce（password_protected 時有值）。
    nonce: Option<Vec<u8>>,
    /// 經 DPAPI 保護的酬載：DEK 或「以密碼包裹後的 DEK」。
    payload: Vec<u8>,
}

fn random_bytes(len: usize) -> Result<Zeroizing<Vec<u8>>> {
    let mut buf = Zeroizing::new(vec![0u8; len]);
    getrandom::fill(buf.as_mut_slice()).map_err(|e| StorageError::Crypto(e.to_string()))?;
    Ok(buf)
}

/// 以 Argon2id 由主密碼與 salt 衍生 256-bit KEK。
fn derive_kek(password: &str, salt: &[u8]) -> Result<Zeroizing<[u8; 32]>> {
    let argon2 = Argon2::new(Algorithm::Argon2id, Version::V0x13, Params::default());
    let mut kek = Zeroizing::new([0u8; 32]);
    argon2
        .hash_password_into(password.as_bytes(), salt, kek.as_mut_slice())
        .map_err(|e| StorageError::Crypto(format!("Argon2id 失敗：{e}")))?;
    Ok(kek)
}

fn aes_encrypt(kek: &[u8; 32], nonce: &[u8], plaintext: &[u8]) -> Result<Vec<u8>> {
    let cipher = Aes256Gcm::new(Key::<Aes256Gcm>::from_slice(kek));
    cipher
        .encrypt(Nonce::from_slice(nonce), plaintext)
        .map_err(|e| StorageError::Crypto(format!("AES-GCM 加密失敗：{e}")))
}

fn aes_decrypt(kek: &[u8; 32], nonce: &[u8], ciphertext: &[u8]) -> Result<Zeroizing<Vec<u8>>> {
    let cipher = Aes256Gcm::new(Key::<Aes256Gcm>::from_slice(kek));
    cipher
        .decrypt(Nonce::from_slice(nonce), ciphertext)
        .map(Zeroizing::new)
        .map_err(|_| StorageError::BadPassword)
}

/// 載入既有 DEK，或於不存在時產生並保存。回傳可直接作為 SQLCipher 金鑰的 DEK。
///
/// - `key_path`：金鑰檔路徑（`%APPDATA%\ActivityTracker\key.json`）。
/// - `secret`：DPAPI 等機密保護實作。
/// - `master_password`：若已啟用主密碼則必須提供；未啟用時忽略。
pub fn load_or_create_dek(
    key_path: &Path,
    secret: &dyn SecretStore,
    master_password: Option<&str>,
) -> Result<Zeroizing<Vec<u8>>> {
    if key_path.exists() {
        load_dek(key_path, secret, master_password)
    } else {
        let dek = random_bytes(DEK_LEN)?;
        let kf = wrap_dek(&dek, secret, master_password)?;
        write_key_file(key_path, &kf)?;
        Ok(dek)
    }
}

fn load_dek(
    key_path: &Path,
    secret: &dyn SecretStore,
    master_password: Option<&str>,
) -> Result<Zeroizing<Vec<u8>>> {
    let raw = std::fs::read(key_path)?;
    let kf: KeyFile = serde_json::from_slice(&raw)?;
    let unprotected = secret
        .unprotect(&kf.payload)
        .map_err(|e| StorageError::KeyProtection(e.to_string()))?;
    let unprotected = Zeroizing::new(unprotected);

    if !kf.password_protected {
        return Ok(unprotected);
    }
    let password = master_password.ok_or(StorageError::BadPassword)?;
    let salt = kf.salt.ok_or_else(|| StorageError::Internal("缺少 salt".into()))?;
    let nonce = kf.nonce.ok_or_else(|| StorageError::Internal("缺少 nonce".into()))?;
    let kek = derive_kek(password, &salt)?;
    let dek = aes_decrypt(&kek, &nonce, &unprotected)?;
    Ok(dek)
}

fn wrap_dek(
    dek: &[u8],
    secret: &dyn SecretStore,
    master_password: Option<&str>,
) -> Result<KeyFile> {
    match master_password {
        None => {
            let payload = secret
                .protect(dek)
                .map_err(|e| StorageError::KeyProtection(e.to_string()))?;
            Ok(KeyFile {
                version: 1,
                password_protected: false,
                salt: None,
                nonce: None,
                payload,
            })
        }
        Some(pw) => {
            let salt = random_bytes(SALT_LEN)?;
            let nonce = random_bytes(NONCE_LEN)?;
            let kek = derive_kek(pw, &salt)?;
            let wrapped = aes_encrypt(&kek, &nonce, dek)?;
            let payload = secret
                .protect(&wrapped)
                .map_err(|e| StorageError::KeyProtection(e.to_string()))?;
            Ok(KeyFile {
                version: 1,
                password_protected: true,
                salt: Some(salt.to_vec()),
                nonce: Some(nonce.to_vec()),
                payload,
            })
        }
    }
}

fn write_key_file(key_path: &Path, kf: &KeyFile) -> Result<()> {
    if let Some(parent) = key_path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let data = serde_json::to_vec_pretty(kf)?;
    std::fs::write(key_path, data)?;
    Ok(())
}

/// 設定／變更主密碼：以既有 DEK 重新包裹並覆寫金鑰檔（T051）。
///
/// 先以現行設定載入 DEK，再以新密碼（`Some` 設定、`None` 移除）重新包裹。
pub fn set_master_password(
    key_path: &Path,
    secret: &dyn SecretStore,
    current_password: Option<&str>,
    new_password: Option<&str>,
) -> Result<()> {
    let dek = load_dek(key_path, secret, current_password)?;
    let kf = wrap_dek(&dek, secret, new_password)?;
    write_key_file(key_path, &kf)?;
    Ok(())
}

/// 是否已啟用主密碼（讀取金鑰檔旗標）。
pub fn is_password_protected(key_path: &Path) -> Result<bool> {
    if !key_path.exists() {
        return Ok(false);
    }
    let raw = std::fs::read(key_path)?;
    let kf: KeyFile = serde_json::from_slice(&raw)?;
    Ok(kf.password_protected)
}

/// 將 DEK 轉為 SQLCipher 所需的 `x'HEX'` 金鑰字串。
pub fn dek_to_sqlcipher_key(dek: &[u8]) -> Zeroizing<String> {
    let mut s = String::with_capacity(dek.len() * 2 + 4);
    s.push_str("x'");
    for b in dek {
        s.push_str(&format!("{b:02x}"));
    }
    s.push('\'');
    Zeroizing::new(s)
}
