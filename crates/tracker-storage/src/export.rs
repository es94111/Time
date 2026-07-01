//! CSV／JSON 匯出（T047，對應 contracts/export-formats.md）。
//!
//! 純本機檔案寫出，無外部傳輸（原則 II）。文字採 UTF-8（含 BOM 以利 Excel 顯示中文）。
//! 時間以 ISO-8601（本機時區含位移）呈現，另附 epoch ms 供程式使用。

use std::path::{Path, PathBuf};

use jiff::{tz::TimeZone, Timestamp};
use serde::{Deserialize, Serialize};
use serde_json::json;

use crate::error::{Result, StorageError};
use crate::repository::{Repository, Scope, SessionRow};

const BOM: &[u8] = &[0xEF, 0xBB, 0xBF];

/// 匯出格式。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Format {
    Csv,
    Json,
}

/// 匯出結果。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExportResult {
    pub written_path: String,
    pub row_count: usize,
}

fn iso_local(ms: i64, tz: &TimeZone) -> String {
    let ts = Timestamp::from_millisecond(ms).unwrap_or(Timestamp::UNIX_EPOCH);
    let zoned = ts.to_zoned(tz.clone());
    zoned.strftime("%Y-%m-%dT%H:%M:%S%:z").to_string()
}

/// 執行匯出，回傳寫出的主要檔案路徑與工作階段列數。
pub fn export(
    repo: &Repository,
    scope: &Scope,
    format: Format,
    target_path: &Path,
    tz: &TimeZone,
) -> Result<ExportResult> {
    let sessions = repo.export_sessions(scope)?;
    match format {
        Format::Csv => export_csv(repo, scope, &sessions, target_path, tz),
        Format::Json => export_json(repo, scope, &sessions, target_path, tz),
    }
}

fn export_csv(
    repo: &Repository,
    scope: &Scope,
    sessions: &[SessionRow],
    target_path: &Path,
    tz: &TimeZone,
) -> Result<ExportResult> {
    // 1) sessions.csv（明細）。
    let mut buf: Vec<u8> = Vec::from(BOM);
    {
        let mut w = csv::Writer::from_writer(&mut buf);
        w.write_record(["本機日期", "開始時間", "結束時間", "應用程式", "網站", "活躍秒數"])
            .map_err(csv_err)?;
        for s in sessions {
            w.write_record([
                s.local_date.clone(),
                iso_local(s.start_utc_ms, tz),
                iso_local(s.end_utc_ms, tz),
                s.application.clone(),
                s.website.clone().unwrap_or_default(),
                (s.active_ms / 1000).to_string(),
            ])
            .map_err(csv_err)?;
        }
        w.flush()?;
    }
    std::fs::write(target_path, &buf)?;

    // 2) daily_summary.csv（每日彙總，寫為兄弟檔）。
    let summary_path = sibling(target_path, "daily_summary");
    let mut sbuf: Vec<u8> = Vec::from(BOM);
    {
        let mut w = csv::Writer::from_writer(&mut sbuf);
        w.write_record(["本機日期", "類型", "名稱", "活躍秒數"]).map_err(csv_err)?;
        for date in repo.distinct_dates(scope)? {
            let day = repo.get_day_summary(&date)?;
            for a in &day.apps {
                w.write_record([
                    date.clone(),
                    "應用程式".to_string(),
                    a.name.clone(),
                    (a.active_ms / 1000).to_string(),
                ])
                .map_err(csv_err)?;
            }
            for site in &day.websites {
                w.write_record([
                    date.clone(),
                    "網站".to_string(),
                    site.name.clone(),
                    (site.active_ms / 1000).to_string(),
                ])
                .map_err(csv_err)?;
            }
        }
        w.flush()?;
    }
    std::fs::write(&summary_path, &sbuf)?;

    Ok(ExportResult { written_path: target_path.display().to_string(), row_count: sessions.len() })
}

fn export_json(
    repo: &Repository,
    scope: &Scope,
    sessions: &[SessionRow],
    target_path: &Path,
    tz: &TimeZone,
) -> Result<ExportResult> {
    let dates = repo.distinct_dates(scope)?;
    let (from, to) = match (dates.first(), dates.last()) {
        (Some(f), Some(t)) => (f.clone(), t.clone()),
        _ => (String::new(), String::new()),
    };

    let sessions_json: Vec<_> = sessions
        .iter()
        .map(|s| {
            json!({
                "local_date": s.local_date,
                "application": s.application,
                "executable": s.executable,
                "website": s.website,
                "start_utc_ms": s.start_utc_ms,
                "end_utc_ms": s.end_utc_ms,
                "active_ms": s.active_ms,
            })
        })
        .collect();

    let mut daily = Vec::new();
    for date in &dates {
        let day = repo.get_day_summary(date)?;
        daily.push(json!({
            "local_date": day.local_date,
            "total_active_ms": day.total_active_ms,
            "apps": day.apps.iter().map(|a| json!({"display_name": a.name, "active_ms": a.active_ms})).collect::<Vec<_>>(),
            "websites": day.websites.iter().map(|w| json!({"hostname": w.name, "active_ms": w.active_ms})).collect::<Vec<_>>(),
        }));
    }

    let doc = json!({
        "schema": "activity-tracker/export@1",
        "exported_at": iso_local(Timestamp::now().as_millisecond(), tz),
        "timezone": tz.iana_name().unwrap_or("local"),
        "range": { "from": from, "to": to },
        "sessions": sessions_json,
        "daily_summary": daily,
    });

    let data = serde_json::to_vec_pretty(&doc)?;
    std::fs::write(target_path, data)?;
    Ok(ExportResult { written_path: target_path.display().to_string(), row_count: sessions.len() })
}

fn sibling(path: &Path, stem_suffix: &str) -> PathBuf {
    let dir = path.parent().unwrap_or_else(|| Path::new("."));
    dir.join(format!("{stem_suffix}.csv"))
}

fn csv_err(e: csv::Error) -> StorageError {
    StorageError::Internal(format!("CSV 寫入失敗：{e}"))
}
