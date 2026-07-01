//! 登入憑證快取（T028；data-model.md `auth_cache`）：
//! `refresh_token` 於呼叫端以 DPAPI 保護後再傳入本模組寫入，本模組不做加解密。

use rusqlite::OptionalExtension;

use crate::db::Database;
use crate::error::Result;

#[derive(Debug, Clone)]
pub struct AuthCache {
    pub refresh_token: Vec<u8>,
    pub account_hint: String,
    pub desktop_session_expires_at: String,
}

pub struct AuthCacheRepo<'a> {
    db: &'a Database,
}

impl<'a> AuthCacheRepo<'a> {
    pub fn new(db: &'a Database) -> Self {
        Self { db }
    }

    pub fn save(&self, cache: &AuthCache) -> Result<()> {
        self.db.conn().execute(
            "INSERT INTO auth_cache (id, refresh_token, account_hint, desktop_session_expires_at)
             VALUES (1, ?1, ?2, ?3)
             ON CONFLICT(id) DO UPDATE SET
               refresh_token = excluded.refresh_token,
               account_hint = excluded.account_hint,
               desktop_session_expires_at = excluded.desktop_session_expires_at",
            rusqlite::params![cache.refresh_token, cache.account_hint, cache.desktop_session_expires_at],
        )?;
        Ok(())
    }

    pub fn load(&self) -> Result<Option<AuthCache>> {
        self.db
            .conn()
            .query_row(
                "SELECT refresh_token, account_hint, desktop_session_expires_at FROM auth_cache WHERE id = 1",
                [],
                |row| {
                    Ok(AuthCache {
                        refresh_token: row.get(0)?,
                        account_hint: row.get(1)?,
                        desktop_session_expires_at: row.get(2)?,
                    })
                },
            )
            .optional()
            .map_err(Into::into)
    }

    /// 登出時清除本機憑證快取（FR-008）。
    pub fn clear(&self) -> Result<()> {
        self.db.conn().execute("DELETE FROM auth_cache", [])?;
        Ok(())
    }
}
