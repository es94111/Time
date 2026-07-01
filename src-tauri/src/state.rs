//! 應用共享狀態與控制旗標。

use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicI64};
use std::sync::{Arc, Mutex};

/// 共享的加密資料庫（未解鎖或啟用主密碼前為 `None`）。
pub type SharedDb = Arc<Mutex<Option<Database>>>;

use jiff::tz::TimeZone;
use tracker_core::metrics::MetricSnapshot;
use tracker_core::rules::{Exclusions, DEFAULT_IDLE_THRESHOLD_MS};
use tracker_core::session::CurrentActivity;
use tracker_platform::secret::DpapiSecretStore;
use tracker_storage::Database;

/// 資料檔路徑。
#[derive(Debug, Clone)]
pub struct Paths {
    pub data_dir: PathBuf,
    pub db: PathBuf,
    pub key: PathBuf,
}

impl Paths {
    /// 以 `%APPDATA%\ActivityTracker\` 為根計算路徑。
    pub fn resolve() -> Self {
        let base = std::env::var_os("APPDATA")
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from("."));
        let data_dir = base.join("ActivityTracker");
        Self {
            db: data_dir.join("activity.db"),
            key: data_dir.join("key.json"),
            data_dir,
        }
    }
}

/// 由命令執行緒與追蹤執行緒共享的控制狀態。
pub struct SharedControl {
    /// 是否暫停追蹤（FR-012）。
    pub paused: AtomicBool,
    /// 目前閒置門檻（毫秒，FR-003）。
    pub idle_threshold_ms: AtomicI64,
    /// 排除清單版本（變更時遞增，通知追蹤執行緒重載）。
    pub exclusions_version: AtomicI64,
    /// 追蹤執行緒是否已啟動（避免重複啟動）。
    pub started: AtomicBool,
    /// 排除清單（FR-013）。
    pub exclusions: Mutex<Exclusions>,
    /// 目前作用中的活動（供狀態查詢）。
    pub current: Mutex<Option<CurrentActivity>>,
}

impl SharedControl {
    /// 以初始設定建立。
    pub fn new(paused: bool, idle_threshold_ms: i64, exclusions: Exclusions) -> Self {
        Self {
            paused: AtomicBool::new(paused),
            idle_threshold_ms: AtomicI64::new(if idle_threshold_ms > 0 {
                idle_threshold_ms
            } else {
                DEFAULT_IDLE_THRESHOLD_MS
            }),
            exclusions_version: AtomicI64::new(0),
            started: AtomicBool::new(false),
            exclusions: Mutex::new(exclusions),
            current: Mutex::new(None),
        }
    }
}

/// 背景指標服務的控制狀態（002-system-metrics）。
pub struct MetricsControl {
    /// 是否啟用背景指標紀錄（FR-015）。
    pub enabled: AtomicBool,
    /// 取樣週期秒數（FR-012；1–3600）。
    pub interval_sec: AtomicI64,
    /// 保留天數（FR-011；1–3650）。
    pub retention_days: AtomicI64,
    /// 指標取樣執行緒是否已啟動（避免重複啟動）。
    pub started: AtomicBool,
    /// 最近一次整機快照（供即時檢視，FR-009）。
    pub last: Mutex<Option<MetricSnapshot>>,
}

impl MetricsControl {
    /// 以預設值建立（取樣 1 秒、保留 30 天、啟用）。
    pub fn new() -> Self {
        Self {
            enabled: AtomicBool::new(true),
            interval_sec: AtomicI64::new(1),
            retention_days: AtomicI64::new(30),
            started: AtomicBool::new(false),
            last: Mutex::new(None),
        }
    }
}

impl Default for MetricsControl {
    fn default() -> Self {
        Self::new()
    }
}

/// Tauri 管理的應用狀態。
pub struct AppState {
    pub db: SharedDb,
    pub control: Arc<SharedControl>,
    /// 指標服務控制（002-system-metrics）。
    pub metrics: Arc<MetricsControl>,
    pub tz: TimeZone,
    pub paths: Paths,
    pub secret: DpapiSecretStore,
    /// 本執行檔路徑（登入自啟登錄用）。
    pub exe_path: String,
}
