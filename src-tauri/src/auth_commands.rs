//! `login`/`logout`/`get_connection_status` IPC 命令（T029，contracts/tauri-commands.md）。

use serde::Serialize;
use tauri::State;
use tracker_platform::device_id::{self, DeviceIdSource};
use tracker_platform::secret::DpapiSecretStore;
use tracker_sync::agent;
use tracker_sync::auth_client::{self, LoginError};

use crate::state::AppState;

#[derive(Debug, Serialize)]
#[serde(tag = "code")]
pub enum LoginErrorView {
    InvalidCredentials,
    AccountLocked { retry_after_seconds: i64 },
    NetworkError { message: String },
}

impl From<LoginError> for LoginErrorView {
    fn from(e: LoginError) -> Self {
        match e {
            LoginError::InvalidCredentials => LoginErrorView::InvalidCredentials,
            LoginError::AccountLocked { retry_after_seconds } => {
                LoginErrorView::AccountLocked { retry_after_seconds }
            }
            LoginError::NetworkError(message) => LoginErrorView::NetworkError { message },
        }
    }
}

#[derive(Debug, Serialize)]
pub struct ConnectionStatus {
    pub logged_in: bool,
    pub account_hint: Option<String>,
    pub last_successful_sync_at: Option<String>,
    pub pending_queue_count: i64,
    pub last_sync_error: Option<String>,
}

fn device_fingerprint() -> Option<String> {
    device_id::default_source().machine_guid()
}

/// 登入（FR-002）：成功後啟動背景同步代理。
#[tauri::command]
pub fn login(state: State<AppState>, identifier: String, password: String) -> Result<ConnectionStatus, LoginErrorView> {
    let fingerprint = device_fingerprint()
        .ok_or_else(|| LoginErrorView::NetworkError { message: "無法讀取裝置識別碼".into() })?;
    let device_name = whoami_device_name();

    let db_guard = state.db.lock().unwrap();
    let db = db_guard
        .as_ref()
        .ok_or_else(|| LoginErrorView::NetworkError { message: "本機資料庫尚未就緒".into() })?;

    // `login` 為同步 Tauri 命令，內部以獨立 runtime 阻塞執行非同步登入請求。
    let rt = tokio::runtime::Builder::new_current_thread().enable_all().build().unwrap();
    let result = rt.block_on(auth_client::login(
        &state.sync.server_base_url,
        db,
        &state.secret,
        &identifier,
        &password,
        &fingerprint,
        &device_name,
    ));

    match result {
        Ok(_) => {
            drop(db_guard);
            start_agent(&state, fingerprint);
            Ok(connection_status(&state))
        }
        Err(e) => Err(e.into()),
    }
}

fn start_agent(state: &State<AppState>, device_fingerprint: String) {
    let mut agent_guard = state.sync.agent.lock().unwrap();
    if agent_guard.is_some() {
        return; // 已啟動，避免重複。
    }
    let handle = agent::spawn(
        state.sync.server_base_url.clone(),
        state.db.clone(),
        std::sync::Arc::new(DpapiSecretStore::new()),
        device_fingerprint,
        state.sync.status.clone(),
    );
    *agent_guard = Some(handle);
}

/// 登出（FR-008）：呼叫伺服器登出、清除本機憑證快取、停止背景同步。
#[tauri::command]
pub fn logout(state: State<AppState>) -> Result<(), String> {
    if let Some(handle) = state.sync.agent.lock().unwrap().take() {
        handle.stop();
    }
    let db_guard = state.db.lock().unwrap();
    if let Some(db) = db_guard.as_ref() {
        let rt = tokio::runtime::Builder::new_current_thread().enable_all().build().unwrap();
        rt.block_on(auth_client::logout(&state.sync.server_base_url, db, &state.secret))
            .map_err(|e| format!("{e:?}"))?;
    }
    Ok(())
}

#[tauri::command]
pub fn get_connection_status(state: State<AppState>) -> ConnectionStatus {
    connection_status(&state)
}

fn connection_status(state: &State<AppState>) -> ConnectionStatus {
    let db_guard = state.db.lock().unwrap();
    let (logged_in, account_hint) = db_guard
        .as_ref()
        .and_then(|db| tracker_storage::auth_cache_repo::AuthCacheRepo::new(db).load().ok().flatten())
        .map(|c| (true, Some(c.account_hint)))
        .unwrap_or((false, None));

    let pending_queue_count =
        db_guard.as_ref().and_then(|db| tracker_sync::queue_repo::queued_count(db).ok()).unwrap_or(0);

    let sync_status = state.sync.status.lock().unwrap();
    let last_successful_sync_at = sync_status
        .last_successful_sync_at_ms
        .and_then(|ms| jiff::Timestamp::from_millisecond(ms).ok())
        .map(|t| t.to_string());

    ConnectionStatus {
        logged_in,
        account_hint,
        last_successful_sync_at,
        pending_queue_count,
        last_sync_error: sync_status.last_error.clone(),
    }
}

fn whoami_device_name() -> String {
    std::env::var("COMPUTERNAME").unwrap_or_else(|_| "我的裝置".to_string())
}
