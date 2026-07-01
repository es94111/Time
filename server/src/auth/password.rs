//! Argon2id 密碼雜湊/驗證（T025，research.md R4）。

use argon2::password_hash::rand_core::OsRng;
use argon2::password_hash::{PasswordHash, PasswordHasher, PasswordVerifier, SaltString};
use argon2::Argon2;

/// 密碼明文最小長度（FR-011）。
pub const MIN_PASSWORD_LEN: usize = 8;

pub fn validate_password_len(password: &str) -> bool {
    password.chars().count() >= MIN_PASSWORD_LEN
}

/// 以 Argon2id 雜湊密碼，回傳可存入 `account.password_hash` 的編碼字串（含 salt/參數）。
pub fn hash_password(password: &str) -> Result<String, String> {
    let salt = SaltString::generate(&mut OsRng);
    Argon2::default()
        .hash_password(password.as_bytes(), &salt)
        .map(|h| h.to_string())
        .map_err(|e| e.to_string())
}

/// 驗證明文密碼是否符合已存的雜湊值。
pub fn verify_password(password: &str, hash: &str) -> bool {
    let Ok(parsed) = PasswordHash::new(hash) else {
        return false;
    };
    Argon2::default()
        .verify_password(password.as_bytes(), &parsed)
        .is_ok()
}
