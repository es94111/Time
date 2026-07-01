//! 每 1 分鐘輪詢的背景同步迴圈（登入後啟動、登出即停止，T040，FR-003）。

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use tracker_core::ports::SecretStore;
use tracker_storage::Database;

use crate::uploader::{self, UploadError};

/// 輪詢間隔（FR-003：每 1 分鐘）。
pub const POLL_INTERVAL: Duration = Duration::from_secs(60);

/// 最近一次同步結果，供 UI 輪詢顯示（`get_connection_status`，contracts）。
#[derive(Debug, Clone, Default)]
pub struct SyncStatus {
    pub last_successful_sync_at_ms: Option<i64>,
    pub last_error: Option<String>,
}

pub struct AgentHandle {
    stop_flag: Arc<AtomicBool>,
}

impl AgentHandle {
    /// 停止背景同步迴圈（登出時呼叫，FR-008）。
    pub fn stop(&self) {
        self.stop_flag.store(true, Ordering::SeqCst);
    }
}

/// 啟動背景同步執行緒（獨立於 host 應用的 tokio runtime，內建最小 current-thread runtime）。
pub fn spawn(
    server_base_url: String,
    db: Arc<Mutex<Option<Database>>>,
    secret: Arc<dyn SecretStore>,
    device_fingerprint: String,
    status: Arc<Mutex<SyncStatus>>,
) -> AgentHandle {
    let stop_flag = Arc::new(AtomicBool::new(false));
    let stop_flag_thread = stop_flag.clone();

    std::thread::Builder::new()
        .name("tracker-sync-agent".into())
        .spawn(move || {
            let rt = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .expect("建立同步代理 runtime 失敗");
            rt.block_on(run_loop(server_base_url, db, secret, device_fingerprint, status, stop_flag_thread));
        })
        .ok();

    AgentHandle { stop_flag }
}

async fn run_loop(
    server_base_url: String,
    db: Arc<Mutex<Option<Database>>>,
    secret: Arc<dyn SecretStore>,
    device_fingerprint: String,
    status: Arc<Mutex<SyncStatus>>,
    stop_flag: Arc<AtomicBool>,
) {
    while !stop_flag.load(Ordering::SeqCst) {
        {
            let guard = db.lock().unwrap();
            if let Some(dbref) = guard.as_ref() {
                let result =
                    uploader::upload_pending(&server_base_url, dbref, secret.as_ref(), &device_fingerprint).await;
                let mut st = status.lock().unwrap();
                match result {
                    Ok(_) => {
                        st.last_successful_sync_at_ms = Some(tracker_core::time::now_ms());
                        st.last_error = None;
                    }
                    Err(UploadError::NotLoggedIn) => {
                        // 尚未登入或憑證已清除：停止本輪，等待下次登入重新啟動代理。
                        break;
                    }
                    Err(UploadError::SessionExpired) => {
                        // Session 已被撤銷（登出／變更密碼）：本機憑證已清除，停止代理並提示重新登入。
                        st.last_error = Some("session_expired".into());
                        break;
                    }
                    Err(UploadError::DeviceRemoved) => {
                        st.last_error = Some("device_removed".into());
                    }
                    Err(UploadError::AccountDisabled) => {
                        st.last_error = Some("account_disabled".into());
                    }
                    Err(UploadError::Network(msg)) => {
                        st.last_error = Some(format!("network: {msg}"));
                    }
                }
            }
        }
        tokio::time::sleep(POLL_INTERVAL).await;
    }
}
