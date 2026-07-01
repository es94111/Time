//! Repository（T012、T026、T035、T041、T046）：寫入工作階段與查詢摘要。

use rusqlite::{params, OptionalExtension};
use serde::{Deserialize, Serialize};
use tracker_core::aggregation::{DaySummary, NamedTotal, RangeSummary};
use tracker_core::model::ClosedSession;

use crate::db::Database;
use crate::error::Result;

/// 設定（對應 data-model.md Setting）。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Settings {
    pub idle_threshold_sec: i64,
    pub tracking_paused: bool,
    pub autostart_enabled: bool,
    pub master_password_set: bool,
    pub ui_language: String,
}

/// 設定部分更新（欄位皆可選）。
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SettingsPatch {
    pub idle_threshold_sec: Option<i64>,
    pub tracking_paused: Option<bool>,
    pub autostart_enabled: Option<bool>,
    pub ui_language: Option<String>,
}

/// 排除規則（FR-013）。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Exclusion {
    pub id: i64,
    pub kind: String,
    pub pattern: String,
}

/// 匯出用的單筆工作階段（join 應用與網站名稱後）。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionRow {
    pub local_date: String,
    pub start_utc_ms: i64,
    pub end_utc_ms: i64,
    pub application: String,
    pub executable: String,
    pub website: Option<String>,
    pub active_ms: i64,
}

/// 清除／匯出的範圍。
#[derive(Debug, Clone)]
pub enum Scope {
    All,
    Range { from: String, to: String },
}

/// Repository：對 [`Database`] 的具名查詢與寫入。
pub struct Repository<'a> {
    db: &'a Database,
}

impl<'a> Repository<'a> {
    /// 建立 Repository。
    pub fn new(db: &'a Database) -> Self {
        Self { db }
    }

    // ---- upsert ----

    /// upsert 應用程式並回傳 id。
    pub fn upsert_application(
        &self,
        executable: &str,
        display_name: &str,
        is_browser: bool,
    ) -> Result<i64> {
        let conn = self.db.conn();
        conn.execute(
            "INSERT INTO application(display_name, executable, is_browser) VALUES(?1, ?2, ?3)\n\
             ON CONFLICT(executable) DO UPDATE SET display_name = ?1, is_browser = ?3",
            params![display_name, executable, is_browser as i64],
        )?;
        let id = conn.query_row(
            "SELECT id FROM application WHERE executable = ?1",
            params![executable],
            |r| r.get::<_, i64>(0),
        )?;
        Ok(id)
    }

    /// upsert 網站並回傳 id。
    pub fn upsert_website(&self, hostname: &str) -> Result<i64> {
        let conn = self.db.conn();
        conn.execute(
            "INSERT INTO website(hostname) VALUES(?1) ON CONFLICT(hostname) DO NOTHING",
            params![hostname],
        )?;
        let id = conn.query_row(
            "SELECT id FROM website WHERE hostname = ?1",
            params![hostname],
            |r| r.get::<_, i64>(0),
        )?;
        Ok(id)
    }

    /// 寫入一筆已結算的工作階段（含「未知／其他」fallback，FR-015）。
    pub fn insert_session(&self, s: &ClosedSession) -> Result<()> {
        let app_id =
            self.upsert_application(&s.app.executable, &s.app.display_name, s.app.is_browser)?;
        let website_id = match &s.website {
            Some(host) => Some(self.upsert_website(host)?),
            None => None,
        };
        self.db.conn().execute(
            "INSERT INTO session(application_id, website_id, start_utc, end_utc, active_ms, local_date)\n\
             VALUES(?1, ?2, ?3, ?4, ?5, ?6)",
            params![app_id, website_id, s.start_utc_ms, s.end_utc_ms, s.active_ms, s.local_date],
        )?;
        Ok(())
    }

    // ---- 摘要查詢 ----

    /// 單日摘要（FR-007/008）。
    pub fn get_day_summary(&self, date: &str) -> Result<DaySummary> {
        let total = self.sum_active("local_date = ?1", params![date])?;
        let apps = self.app_totals("s.local_date = ?1", params![date])?;
        let websites = self.website_totals("s.local_date = ?1", params![date])?;
        Ok(DaySummary { local_date: date.to_string(), total_active_ms: total, apps, websites })
    }

    /// 區間摘要（FR-009）。
    pub fn get_range_summary(&self, from: &str, to: &str) -> Result<RangeSummary> {
        let (lo, hi) = if from <= to { (from, to) } else { (to, from) };
        let total = self.sum_active("local_date BETWEEN ?1 AND ?2", params![lo, hi])?;
        let apps = self.app_totals("s.local_date BETWEEN ?1 AND ?2", params![lo, hi])?;
        let websites = self.website_totals("s.local_date BETWEEN ?1 AND ?2", params![lo, hi])?;
        Ok(RangeSummary {
            from: lo.to_string(),
            to: hi.to_string(),
            total_active_ms: total,
            apps,
            websites,
        })
    }

    fn sum_active(&self, where_clause: &str, p: impl rusqlite::Params) -> Result<i64> {
        let sql = format!("SELECT COALESCE(SUM(active_ms), 0) FROM session WHERE {where_clause}");
        let total = self.db.conn().query_row(&sql, p, |r| r.get::<_, i64>(0))?;
        Ok(total)
    }

    fn app_totals(&self, where_clause: &str, p: impl rusqlite::Params) -> Result<Vec<NamedTotal>> {
        let sql = format!(
            "SELECT a.display_name, SUM(s.active_ms) AS m\n\
             FROM session s JOIN application a ON a.id = s.application_id\n\
             WHERE {where_clause}\n\
             GROUP BY a.display_name ORDER BY m DESC, a.display_name ASC"
        );
        let conn = self.db.conn();
        let mut stmt = conn.prepare(&sql)?;
        let rows = stmt
            .query_map(p, |r| Ok(NamedTotal { name: r.get(0)?, active_ms: r.get(1)? }))?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        Ok(rows)
    }

    fn website_totals(
        &self,
        where_clause: &str,
        p: impl rusqlite::Params,
    ) -> Result<Vec<NamedTotal>> {
        let sql = format!(
            "SELECT w.hostname, SUM(s.active_ms) AS m\n\
             FROM session s JOIN website w ON w.id = s.website_id\n\
             WHERE s.website_id IS NOT NULL AND {where_clause}\n\
             GROUP BY w.hostname ORDER BY m DESC, w.hostname ASC"
        );
        let conn = self.db.conn();
        let mut stmt = conn.prepare(&sql)?;
        let rows = stmt
            .query_map(p, |r| Ok(NamedTotal { name: r.get(0)?, active_ms: r.get(1)? }))?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        Ok(rows)
    }

    // ---- 排除規則 ----

    /// 列出排除規則。
    pub fn list_exclusions(&self) -> Result<Vec<Exclusion>> {
        let conn = self.db.conn();
        let mut stmt = conn.prepare("SELECT id, kind, pattern FROM exclusion ORDER BY kind, pattern")?;
        let rows = stmt
            .query_map([], |r| {
                Ok(Exclusion { id: r.get(0)?, kind: r.get(1)?, pattern: r.get(2)? })
            })?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        Ok(rows)
    }

    /// 新增排除規則並同步 application/website 的 excluded 旗標。
    pub fn add_exclusion(&self, kind: &str, pattern: &str) -> Result<Exclusion> {
        let conn = self.db.conn();
        let pattern = pattern.to_ascii_lowercase();
        conn.execute(
            "INSERT OR IGNORE INTO exclusion(kind, pattern) VALUES(?1, ?2)",
            params![kind, pattern],
        )?;
        match kind {
            "app" => {
                conn.execute(
                    "UPDATE application SET excluded = 1 WHERE lower(executable) = ?1",
                    params![pattern],
                )?;
            }
            "website" => {
                conn.execute(
                    "UPDATE website SET excluded = 1 WHERE lower(hostname) = ?1",
                    params![pattern],
                )?;
            }
            _ => {}
        }
        let id = conn.query_row(
            "SELECT id FROM exclusion WHERE kind = ?1 AND pattern = ?2",
            params![kind, pattern],
            |r| r.get::<_, i64>(0),
        )?;
        Ok(Exclusion { id, kind: kind.to_string(), pattern })
    }

    /// 移除排除規則。
    pub fn remove_exclusion(&self, id: i64) -> Result<()> {
        let conn = self.db.conn();
        if let Some((kind, pattern)) = conn
            .query_row(
                "SELECT kind, pattern FROM exclusion WHERE id = ?1",
                params![id],
                |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)),
            )
            .optional()?
        {
            conn.execute("DELETE FROM exclusion WHERE id = ?1", params![id])?;
            match kind.as_str() {
                "app" => {
                    conn.execute(
                        "UPDATE application SET excluded = 0 WHERE lower(executable) = ?1",
                        params![pattern],
                    )?;
                }
                "website" => {
                    conn.execute(
                        "UPDATE website SET excluded = 0 WHERE lower(hostname) = ?1",
                        params![pattern],
                    )?;
                }
                _ => {}
            }
        }
        Ok(())
    }

    /// 取得核心排除清單（執行檔名、主機名）。
    pub fn exclusion_patterns(&self) -> Result<(Vec<String>, Vec<String>)> {
        let conn = self.db.conn();
        let mut stmt = conn.prepare("SELECT kind, pattern FROM exclusion")?;
        let mut apps = Vec::new();
        let mut sites = Vec::new();
        let rows = stmt.query_map([], |r| {
            Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?))
        })?;
        for row in rows {
            let (kind, pattern) = row?;
            match kind.as_str() {
                "app" => apps.push(pattern),
                "website" => sites.push(pattern),
                _ => {}
            }
        }
        Ok((apps, sites))
    }

    // ---- 設定 ----

    /// 讀取全部設定。
    pub fn get_settings(&self) -> Result<Settings> {
        Ok(Settings {
            idle_threshold_sec: self.get_setting("idle_threshold_sec")?.parse().unwrap_or(300),
            tracking_paused: self.get_setting("tracking_paused")? == "true",
            autostart_enabled: self.get_setting("autostart_enabled")? == "true",
            master_password_set: self.get_setting("master_password_set")? == "true",
            ui_language: self.get_setting("ui_language")?,
        })
    }

    /// 套用設定部分更新並回傳更新後設定。
    pub fn update_settings(&self, patch: &SettingsPatch) -> Result<Settings> {
        if let Some(v) = patch.idle_threshold_sec {
            self.set_setting("idle_threshold_sec", &v.to_string())?;
        }
        if let Some(v) = patch.tracking_paused {
            self.set_setting("tracking_paused", &v.to_string())?;
        }
        if let Some(v) = patch.autostart_enabled {
            self.set_setting("autostart_enabled", &v.to_string())?;
        }
        if let Some(v) = &patch.ui_language {
            self.set_setting("ui_language", v)?;
        }
        self.get_settings()
    }

    /// 讀取單一設定（不存在時回傳空字串）。
    pub fn get_setting(&self, key: &str) -> Result<String> {
        let v = self
            .db
            .conn()
            .query_row("SELECT value FROM setting WHERE key = ?1", params![key], |r| {
                r.get::<_, String>(0)
            })
            .optional()?
            .unwrap_or_default();
        Ok(v)
    }

    /// 寫入單一設定。
    pub fn set_setting(&self, key: &str, value: &str) -> Result<()> {
        self.db.conn().execute(
            "INSERT INTO setting(key, value) VALUES(?1, ?2)\n\
             ON CONFLICT(key) DO UPDATE SET value = ?2",
            params![key, value],
        )?;
        Ok(())
    }

    // ---- 清除與匯出 ----

    /// 清除資料（FR-014）。回傳刪除的工作階段列數。
    pub fn clear_data(&self, scope: &Scope) -> Result<u64> {
        let conn = self.db.conn();
        let deleted = match scope {
            Scope::All => {
                let n = conn.execute("DELETE FROM session", [])?;
                conn.execute("DELETE FROM website", [])?;
                // 保留 application 與設定，但清除使用統計；亦可保留 application 列。
                n
            }
            Scope::Range { from, to } => {
                let (lo, hi) = if from <= to { (from, to) } else { (to, from) };
                conn.execute(
                    "DELETE FROM session WHERE local_date BETWEEN ?1 AND ?2",
                    params![lo, hi],
                )?
            }
        };
        self.db.checkpoint()?;
        Ok(deleted as u64)
    }

    /// 取得匯出用工作階段列（依起始時間排序）。
    pub fn export_sessions(&self, scope: &Scope) -> Result<Vec<SessionRow>> {
        let conn = self.db.conn();
        let base = "SELECT s.local_date, s.start_utc, s.end_utc, a.display_name, a.executable, w.hostname, s.active_ms\n\
                    FROM session s\n\
                    JOIN application a ON a.id = s.application_id\n\
                    LEFT JOIN website w ON w.id = s.website_id";
        let map = |r: &rusqlite::Row| {
            Ok(SessionRow {
                local_date: r.get(0)?,
                start_utc_ms: r.get(1)?,
                end_utc_ms: r.get(2)?,
                application: r.get(3)?,
                executable: r.get(4)?,
                website: r.get(5)?,
                active_ms: r.get(6)?,
            })
        };
        let rows = match scope {
            Scope::All => {
                let sql = format!("{base} ORDER BY s.start_utc ASC");
                let mut stmt = conn.prepare(&sql)?;
                let out = stmt.query_map([], map)?.collect::<std::result::Result<Vec<_>, _>>()?;
                out
            }
            Scope::Range { from, to } => {
                let (lo, hi) = if from <= to { (from, to) } else { (to, from) };
                let sql = format!("{base} WHERE s.local_date BETWEEN ?1 AND ?2 ORDER BY s.start_utc ASC");
                let mut stmt = conn.prepare(&sql)?;
                let out = stmt
                    .query_map(params![lo, hi], map)?
                    .collect::<std::result::Result<Vec<_>, _>>()?;
                out
            }
        };
        Ok(rows)
    }

    /// 取得匯出範圍內所有出現過的本機日期（已排序、去重）。
    pub fn distinct_dates(&self, scope: &Scope) -> Result<Vec<String>> {
        let conn = self.db.conn();
        let rows = match scope {
            Scope::All => {
                let mut stmt =
                    conn.prepare("SELECT DISTINCT local_date FROM session ORDER BY local_date")?;
                let out = stmt
                    .query_map([], |r| r.get::<_, String>(0))?
                    .collect::<std::result::Result<Vec<_>, _>>()?;
                out
            }
            Scope::Range { from, to } => {
                let (lo, hi) = if from <= to { (from, to) } else { (to, from) };
                let mut stmt = conn.prepare(
                    "SELECT DISTINCT local_date FROM session WHERE local_date BETWEEN ?1 AND ?2 ORDER BY local_date",
                )?;
                let out = stmt
                    .query_map(params![lo, hi], |r| r.get::<_, String>(0))?
                    .collect::<std::result::Result<Vec<_>, _>>()?;
                out
            }
        };
        Ok(rows)
    }
}
