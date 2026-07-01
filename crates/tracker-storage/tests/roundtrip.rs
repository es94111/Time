//! 儲存整合測試：加密 DB 往返、摘要查詢、匯出、持久化與錯誤金鑰（對應 quickstart.md）。

use std::path::PathBuf;

use jiff::tz::TimeZone;
use tracker_core::model::{AppIdentity, ClosedSession};
use tracker_core::ports::{PortError, SecretStore};
use tracker_storage::export::{self, Format};
use tracker_storage::repository::Scope;
use tracker_storage::{crypto, Database, Repository};

/// 測試用機密保護（恆等）：略過 DPAPI 以便跨平台測試 crypto/db/repository/export。
struct FakeSecret;
impl SecretStore for FakeSecret {
    fn protect(&self, p: &[u8]) -> Result<Vec<u8>, PortError> {
        Ok(p.to_vec())
    }
    fn unprotect(&self, c: &[u8]) -> Result<Vec<u8>, PortError> {
        Ok(c.to_vec())
    }
}

fn unique_temp_dir() -> PathBuf {
    let mut d = std::env::temp_dir();
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    d.push(format!("tracker_test_{nanos}"));
    std::fs::create_dir_all(&d).unwrap();
    d
}

fn sess(
    display: &str,
    exe: &str,
    is_browser: bool,
    website: Option<&str>,
    active_ms: i64,
    date: &str,
) -> ClosedSession {
    ClosedSession {
        app: AppIdentity { display_name: display.into(), executable: exe.into(), is_browser },
        website: website.map(|s| s.to_string()),
        start_utc_ms: 1_000_000,
        end_utc_ms: 1_000_000 + active_ms,
        active_ms,
        local_date: date.into(),
    }
}

#[test]
fn encrypted_roundtrip_summary_and_export() {
    let dir = unique_temp_dir();
    let key = dir.join("key.json");
    let db_path = dir.join("activity.db");
    let secret = FakeSecret;

    let dek = crypto::load_or_create_dek(&key, &secret, None).unwrap();
    assert_eq!(dek.len(), crypto::DEK_LEN);

    {
        let db = Database::open(&db_path, &dek).unwrap();
        let repo = Repository::new(&db);
        repo.insert_session(&sess("Visual Studio Code", "code.exe", false, None, 600_000, "2026-06-30"))
            .unwrap();
        repo.insert_session(&sess("Google Chrome", "chrome.exe", true, Some("github.com"), 300_000, "2026-06-30"))
            .unwrap();
        repo.insert_session(&sess("Google Chrome", "chrome.exe", true, Some("github.com"), 120_000, "2026-06-30"))
            .unwrap();

        let day = repo.get_day_summary("2026-06-30").unwrap();
        assert_eq!(day.total_active_ms, 600_000 + 300_000 + 120_000);
        assert_eq!(day.apps[0].name, "Visual Studio Code");
        assert_eq!(day.websites[0].name, "github.com");
        assert_eq!(day.websites[0].active_ms, 420_000);

        // CSV 與 JSON 匯出列數一致（對應 export-formats.md 契約測試要點）。
        let csv = dir.join("out.csv");
        let rc = export::export(&repo, &Scope::All, Format::Csv, &csv, &TimeZone::UTC).unwrap();
        assert_eq!(rc.row_count, 3);
        assert!(csv.exists());

        let json = dir.join("out.json");
        let rj = export::export(&repo, &Scope::All, Format::Json, &json, &TimeZone::UTC).unwrap();
        assert_eq!(rj.row_count, 3);
        assert!(json.exists());

        // CSV 應以 UTF-8 BOM 起始。
        let bytes = std::fs::read(&csv).unwrap();
        assert_eq!(&bytes[..3], &[0xEF, 0xBB, 0xBF]);
    }

    // 重新開啟驗證持久化（WAL）與金鑰正確。
    {
        let db = Database::open(&db_path, &dek).unwrap();
        let repo = Repository::new(&db);
        let day = repo.get_day_summary("2026-06-30").unwrap();
        assert_eq!(day.total_active_ms, 1_020_000);
    }

    // 錯誤金鑰無法開啟（FR-016）。
    let wrong = vec![0u8; crypto::DEK_LEN];
    assert!(Database::open(&db_path, &wrong).is_err());

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn exclusions_and_settings_persist() {
    let dir = unique_temp_dir();
    let key = dir.join("key.json");
    let db_path = dir.join("activity.db");
    let secret = FakeSecret;
    let dek = crypto::load_or_create_dek(&key, &secret, None).unwrap();

    let db = Database::open(&db_path, &dek).unwrap();
    let repo = Repository::new(&db);

    repo.add_exclusion("app", "Discord.exe").unwrap();
    repo.add_exclusion("website", "Example.COM").unwrap();
    let (apps, sites) = repo.exclusion_patterns().unwrap();
    assert!(apps.contains(&"discord.exe".to_string()));
    assert!(sites.contains(&"example.com".to_string()));

    let s = repo.get_settings().unwrap();
    assert_eq!(s.idle_threshold_sec, 300);
    assert_eq!(s.ui_language, "zh-TW");

    let _ = std::fs::remove_dir_all(&dir);
}
