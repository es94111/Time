//! Tauri IPC 命令（對應 contracts/tauri-commands.md）。
//!
//! 失敗時回傳結構化錯誤 `{ code, message_zh }`（zh-TW）。

use std::path::Path;
use std::sync::atomic::Ordering;

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, State};
use tracker_core::aggregation::{DaySummary, RangeSummary};
use tracker_core::metrics::MetricSnapshot;
use tracker_core::rules::Exclusions;
use tracker_core::time;
use tracker_storage::export::{self, ExportResult, Format};
use tracker_storage::metrics_repo::{
    DeviceLists, MetricsRepo, MetricsSettings, MetricsSettingsPatch, TrendSeries,
};
use tracker_storage::repository::{Exclusion, Repository, Scope, Settings, SettingsPatch};
use tracker_storage::{crypto, StorageError};

use crate::metrics_service;
use crate::state::AppState;
use crate::tracking_service;

/// 結構化錯誤模型（contracts）。
#[derive(Debug, Serialize)]
pub struct CommandError {
    pub code: String,
    pub message_zh: String,
}

impl CommandError {
    fn new(code: &str, message: impl Into<String>) -> Self {
        Self { code: code.into(), message_zh: message.into() }
    }
    fn internal(message: impl Into<String>) -> Self {
        Self::new("INTERNAL", message)
    }
    fn invalid_date() -> Self {
        Self::new("INVALID_DATE", "日期參數無效或缺少範圍端點")
    }
}

impl From<StorageError> for CommandError {
    fn from(e: StorageError) -> Self {
        let code = match &e {
            StorageError::BadPassword => "BAD_PASSWORD",
            StorageError::Sqlite(_) => "DB_LOCKED",
            StorageError::Io(_) => "EXPORT_IO",
            StorageError::KeyProtection(_) => "INTERNAL",
            _ => "INTERNAL",
        };
        Self::new(code, e.to_string())
    }
}

type CmdResult<T> = Result<T, CommandError>;

// ---- 狀態型別 ----

/// 追蹤狀態（FR-012）。
#[derive(Debug, Serialize)]
pub struct TrackingStatus {
    pub paused: bool,
    pub current_app: Option<String>,
    pub current_website: Option<String>,
    pub since_utc: Option<i64>,
}

/// 解鎖狀態。
#[derive(Debug, Serialize)]
pub struct LockState {
    pub locked: bool,
    pub password_protected: bool,
}

/// 匯出選項（contracts）。
#[derive(Debug, Deserialize)]
pub struct ExportOptions {
    pub format: String,
    pub scope: String,
    pub from: Option<String>,
    pub to: Option<String>,
    pub target_path: String,
}

/// 清除結果。
#[derive(Debug, Serialize)]
pub struct ClearResult {
    pub deleted_rows: u64,
}

// ---- 輔助 ----

fn with_db<T>(
    state: &AppState,
    f: impl FnOnce(&Repository) -> tracker_storage::Result<T>,
) -> CmdResult<T> {
    let guard = state.db.lock().map_err(|_| CommandError::internal("資料庫鎖定失敗"))?;
    let dbref = guard
        .as_ref()
        .ok_or_else(|| CommandError::new("LOCKED", "資料庫尚未解鎖，請先輸入主密碼"))?;
    let repo = Repository::new(dbref);
    f(&repo).map_err(CommandError::from)
}

fn status_of(state: &AppState) -> TrackingStatus {
    let paused = state.control.paused.load(Ordering::SeqCst);
    let cur = state.control.current.lock().unwrap();
    match &*cur {
        Some(c) => TrackingStatus {
            paused,
            current_app: Some(c.app.display_name.clone()),
            current_website: c.website.clone(),
            since_utc: Some(c.since_utc_ms),
        },
        None => TrackingStatus { paused, current_app: None, current_website: None, since_utc: None },
    }
}

fn build_scope(scope: &str, from: Option<String>, to: Option<String>) -> CmdResult<Scope> {
    match scope {
        "all" => Ok(Scope::All),
        "range" => {
            let from = from.ok_or_else(CommandError::invalid_date)?;
            let to = to.ok_or_else(CommandError::invalid_date)?;
            Ok(Scope::Range { from, to })
        }
        _ => Err(CommandError::invalid_date()),
    }
}

fn reload_exclusions(state: &AppState) -> CmdResult<()> {
    let (apps, sites) = with_db(state, |r| r.exclusion_patterns())?;
    *state.control.exclusions.lock().unwrap() = Exclusions::new(apps, sites);
    state.control.exclusions_version.fetch_add(1, Ordering::SeqCst);
    Ok(())
}

// ---- 查詢類 ----

#[tauri::command]
pub fn get_tracking_status(state: State<AppState>) -> TrackingStatus {
    status_of(&state)
}

#[tauri::command]
pub fn get_lock_state(state: State<AppState>) -> LockState {
    let locked = state.db.lock().map(|g| g.is_none()).unwrap_or(true);
    let password_protected =
        crypto::is_password_protected(&state.paths.key).unwrap_or(false);
    LockState { locked, password_protected }
}

#[tauri::command]
pub fn get_today_summary(state: State<AppState>) -> CmdResult<DaySummary> {
    let date = time::local_date(time::now_ms(), &state.tz);
    with_db(&state, |r| r.get_day_summary(&date))
}

#[tauri::command]
pub fn get_summary_by_date(state: State<AppState>, date: String) -> CmdResult<DaySummary> {
    with_db(&state, |r| r.get_day_summary(&date))
}

#[tauri::command]
pub fn get_summary_range(
    state: State<AppState>,
    from: String,
    to: String,
) -> CmdResult<RangeSummary> {
    with_db(&state, |r| r.get_range_summary(&from, &to))
}

// ---- 控制類 ----

#[tauri::command]
pub fn pause_tracking(state: State<AppState>) -> CmdResult<TrackingStatus> {
    state.control.paused.store(true, Ordering::SeqCst);
    with_db(&state, |r| r.set_setting("tracking_paused", "true"))?;
    Ok(status_of(&state))
}

#[tauri::command]
pub fn resume_tracking(state: State<AppState>) -> CmdResult<TrackingStatus> {
    state.control.paused.store(false, Ordering::SeqCst);
    with_db(&state, |r| r.set_setting("tracking_paused", "false"))?;
    Ok(status_of(&state))
}

#[tauri::command]
pub fn get_settings(state: State<AppState>) -> CmdResult<Settings> {
    with_db(&state, |r| r.get_settings())
}

#[tauri::command]
pub fn update_settings(state: State<AppState>, patch: SettingsPatch) -> CmdResult<Settings> {
    let autostart_change = patch.autostart_enabled;
    let settings = with_db(&state, |r| r.update_settings(&patch))?;
    // 套用到執行時控制。
    state
        .control
        .idle_threshold_ms
        .store(settings.idle_threshold_sec.max(1) * 1000, Ordering::SeqCst);
    // 同步登入自啟登錄項。
    if let Some(enabled) = autostart_change {
        tracker_platform::autostart::set_autostart(enabled, &state.exe_path);
    }
    Ok(settings)
}

// ---- 排除類（FR-013）----

#[tauri::command]
pub fn list_exclusions(state: State<AppState>) -> CmdResult<Vec<Exclusion>> {
    with_db(&state, |r| r.list_exclusions())
}

#[tauri::command]
pub fn add_exclusion(
    state: State<AppState>,
    kind: String,
    pattern: String,
) -> CmdResult<Exclusion> {
    let exclusion = with_db(&state, |r| r.add_exclusion(&kind, &pattern))?;
    reload_exclusions(&state)?;
    Ok(exclusion)
}

#[tauri::command]
pub fn remove_exclusion(state: State<AppState>, id: i64) -> CmdResult<()> {
    with_db(&state, |r| r.remove_exclusion(id))?;
    reload_exclusions(&state)?;
    Ok(())
}

// ---- 資料管理類 ----

#[tauri::command]
pub fn export_data(state: State<AppState>, opts: ExportOptions) -> CmdResult<ExportResult> {
    let format = match opts.format.as_str() {
        "json" => Format::Json,
        _ => Format::Csv,
    };
    let scope = build_scope(&opts.scope, opts.from, opts.to)?;
    let guard = state.db.lock().map_err(|_| CommandError::internal("資料庫鎖定失敗"))?;
    let dbref = guard
        .as_ref()
        .ok_or_else(|| CommandError::new("LOCKED", "資料庫尚未解鎖"))?;
    let repo = Repository::new(dbref);
    export::export(&repo, &scope, format, Path::new(&opts.target_path), &state.tz)
        .map_err(CommandError::from)
}

#[tauri::command]
pub fn clear_data(
    state: State<AppState>,
    scope: String,
    from: Option<String>,
    to: Option<String>,
) -> CmdResult<ClearResult> {
    let sc = build_scope(&scope, from, to)?;
    let deleted = with_db(&state, |r| r.clear_data(&sc))?;
    Ok(ClearResult { deleted_rows: deleted })
}

// ---- 主密碼（可選，T051）----

#[tauri::command]
pub fn set_master_password(
    state: State<AppState>,
    new_password: String,
    current_password: Option<String>,
) -> CmdResult<()> {
    let new = if new_password.is_empty() { None } else { Some(new_password.as_str()) };
    crypto::set_master_password(
        &state.paths.key,
        &state.secret,
        current_password.as_deref(),
        new,
    )
    .map_err(CommandError::from)?;
    with_db(&state, |r| {
        r.set_setting("master_password_set", if new.is_some() { "true" } else { "false" })
    })?;
    Ok(())
}

#[tauri::command]
pub fn unlock(app: AppHandle, state: State<AppState>, password: String) -> CmdResult<bool> {
    match tracking_service::open_and_start(&state, Some(&password)) {
        Ok(()) => {
            // 資料庫解鎖後啟動背景指標取樣（002-system-metrics）。
            metrics_service::start(&app);
            Ok(true)
        }
        Err(StorageError::BadPassword) => Ok(false),
        Err(e) => Err(e.into()),
    }
}

// ---- 系統指標（002-system-metrics，對應 contracts/tauri-commands.md）----

/// 指標查詢的輔助：以 [`MetricsRepo`] 存取已解鎖的資料庫。
fn with_metrics<T>(
    state: &AppState,
    f: impl FnOnce(&MetricsRepo) -> tracker_storage::Result<T>,
) -> CmdResult<T> {
    let guard = state.db.lock().map_err(|_| CommandError::internal("資料庫鎖定失敗"))?;
    let dbref = guard
        .as_ref()
        .ok_or_else(|| CommandError::new("LOCKED", "資料庫尚未解鎖，請先輸入主密碼"))?;
    let repo = MetricsRepo::new(dbref);
    f(&repo).map_err(CommandError::from)
}

/// 歷史區間查詢輸入（contracts §2）。
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RangeQuery {
    pub from_utc: i64,
    pub to_utc: i64,
    /// UI 便利性參數（篩選），後端目前全支援。
    #[serde(default)]
    pub metrics: Option<Vec<String>>,
}

/// 清除結果（contracts §5）。
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ClearMetricsResult {
    pub deleted_samples: u64,
}

/// 即時檢視：最近一次整機快照（US2 / FR-009）。
#[tauri::command]
pub fn get_current_metrics(state: State<AppState>) -> CmdResult<Option<MetricSnapshot>> {
    // 優先回傳服務快取之最近快照；無快取時退回讀取資料庫最新樣本。
    if let Ok(last) = state.metrics.last.lock() {
        if last.is_some() {
            return Ok(last.clone());
        }
    }
    with_metrics(&state, |r| r.get_latest_snapshot())
}

/// 歷史區間查詢／趨勢（US3 / FR-010、FR-014）。
#[tauri::command]
pub fn get_metrics_range(state: State<AppState>, query: RangeQuery) -> CmdResult<TrendSeries> {
    if query.to_utc <= query.from_utc {
        return Err(CommandError::new("INVALID_RANGE", "結束時間須大於開始時間"));
    }
    with_metrics(&state, |r| r.get_metrics_range(query.from_utc, query.to_utc))
}

/// 裝置清單（含已移除，FR-008 / SC-004）。
#[tauri::command]
pub fn list_metric_devices(state: State<AppState>) -> CmdResult<DeviceLists> {
    with_metrics(&state, |r| r.list_devices())
}

/// 取得指標設定（contracts §4）。
#[tauri::command]
pub fn get_metrics_settings(state: State<AppState>) -> CmdResult<MetricsSettings> {
    with_metrics(&state, |r| r.get_metrics_settings())
}

/// 設定指標設定：驗證界限、持久化、套用到執行時控制（FR-011/012/015）。
#[tauri::command]
pub fn set_metrics_settings(
    state: State<AppState>,
    patch: MetricsSettingsPatch,
) -> CmdResult<MetricsSettings> {
    // 界限驗證（contracts §4）。
    if let Some(v) = patch.sample_interval_sec {
        if !(1..=3600).contains(&v) {
            return Err(CommandError::new("INVALID_SETTING", "取樣秒數須介於 1 至 3600 秒"));
        }
    }
    if let Some(v) = patch.retention_days {
        if !(1..=3650).contains(&v) {
            return Err(CommandError::new("INVALID_SETTING", "保留天數須介於 1 至 3650 天"));
        }
    }

    let retention_changed = patch.retention_days.is_some();
    let settings = with_metrics(&state, |r| r.set_metrics_settings(&patch))?;

    // 套用到執行時控制旗標。
    state.metrics.interval_sec.store(settings.sample_interval_sec, Ordering::SeqCst);
    state.metrics.retention_days.store(settings.retention_days, Ordering::SeqCst);
    state.metrics.enabled.store(settings.enabled, Ordering::SeqCst);

    // 變更保留天數後可立即觸發一次清理。
    if retention_changed {
        metrics_service::purge_now(&state);
    }
    Ok(settings)
}

/// 清除指標紀錄（省略 `beforeUtc` 即全刪，contracts §5）。
#[tauri::command]
pub fn clear_metrics_data(
    state: State<AppState>,
    before_utc: Option<i64>,
) -> CmdResult<ClearMetricsResult> {
    let deleted = with_metrics(&state, |r| r.clear_metrics(before_utc))?;
    // 一併清空即時快取，避免呈現已刪除資料。
    if let Ok(mut last) = state.metrics.last.lock() {
        *last = None;
    }
    Ok(ClearMetricsResult { deleted_samples: deleted })
}
