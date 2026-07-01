//! 裝置身分快取（data-model.md `device_identity`）：登入成功後寫入伺服器回傳的 `device.id`。

use rusqlite::OptionalExtension;

use crate::db::Database;
use crate::error::Result;

pub struct DeviceIdentityRepo<'a> {
    db: &'a Database,
}

impl<'a> DeviceIdentityRepo<'a> {
    pub fn new(db: &'a Database) -> Self {
        Self { db }
    }

    pub fn save(&self, hardware_fingerprint: &str, server_device_id: &str) -> Result<()> {
        self.db.conn().execute(
            "INSERT INTO device_identity (hardware_fingerprint, server_device_id)
             VALUES (?1, ?2)
             ON CONFLICT(hardware_fingerprint) DO UPDATE SET server_device_id = excluded.server_device_id",
            rusqlite::params![hardware_fingerprint, server_device_id],
        )?;
        Ok(())
    }

    pub fn server_device_id(&self, hardware_fingerprint: &str) -> Result<Option<String>> {
        self.db
            .conn()
            .query_row(
                "SELECT server_device_id FROM device_identity WHERE hardware_fingerprint = ?1",
                [hardware_fingerprint],
                |row| row.get(0),
            )
            .optional()
            .map_err(Into::into)
    }
}
