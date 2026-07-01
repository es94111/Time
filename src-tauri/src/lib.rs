//! 活動追蹤器 Tauri 應用進入點（T013、T014）。
//!
//! 職責：單一實例、系統匣、關閉即最小化至系統匣、以（可選）主密碼延遲開啟加密 DB、
//! 啟動背景追蹤服務、註冊 IPC 命令。

mod commands;
mod state;
mod tracking_service;

use std::sync::{Arc, Mutex};

use tauri::menu::{Menu, MenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{Manager, WindowEvent};
use tracker_core::rules::{Exclusions, DEFAULT_IDLE_THRESHOLD_MS};
use tracker_platform::secret::DpapiSecretStore;
use tracker_storage::{crypto, Repository};

use state::{AppState, Paths, SharedControl};

/// 啟動應用程式。
pub fn run() {
    let paths = Paths::resolve();
    let tz = tracker_core::time::system_tz();
    let exe_path = std::env::current_exe()
        .ok()
        .and_then(|p| p.to_str().map(String::from))
        .unwrap_or_default();

    let control = Arc::new(SharedControl::new(false, DEFAULT_IDLE_THRESHOLD_MS, Exclusions::default()));
    let db = Arc::new(Mutex::new(None));

    let app_state = AppState {
        db,
        control,
        tz,
        paths,
        secret: DpapiSecretStore::new(),
        exe_path,
    };

    tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
            if let Some(window) = app.get_webview_window("main") {
                let _ = window.show();
                let _ = window.set_focus();
            }
        }))
        .manage(app_state)
        .invoke_handler(tauri::generate_handler![
            commands::get_tracking_status,
            commands::get_lock_state,
            commands::get_today_summary,
            commands::get_summary_by_date,
            commands::get_summary_range,
            commands::pause_tracking,
            commands::resume_tracking,
            commands::get_settings,
            commands::update_settings,
            commands::list_exclusions,
            commands::add_exclusion,
            commands::remove_exclusion,
            commands::export_data,
            commands::clear_data,
            commands::set_master_password,
            commands::unlock,
        ])
        .on_window_event(|window, event| {
            // 關閉主視窗即最小化至系統匣（背景常駐，FR-002）。
            if let WindowEvent::CloseRequested { api, .. } = event {
                let _ = window.hide();
                api.prevent_close();
            }
        })
        .setup(|app| {
            setup_tray(app)?;

            let st = app.state::<AppState>();
            // 未啟用主密碼時，啟動即開啟資料庫並開始追蹤。
            let protected = crypto::is_password_protected(&st.paths.key).unwrap_or(false);
            if !protected {
                match tracking_service::open_and_start(&st, None) {
                    Ok(()) => sync_autostart(&st),
                    Err(e) => eprintln!("開啟資料庫失敗：{e}"),
                }
            }
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("啟動 Tauri 應用失敗");
}

/// 依設定同步登入自啟登錄項（FR-002）。
fn sync_autostart(state: &AppState) {
    let enabled = {
        let guard = match state.db.lock() {
            Ok(g) => g,
            Err(_) => return,
        };
        guard
            .as_ref()
            .map(|db| Repository::new(db).get_setting("autostart_enabled").unwrap_or_default() == "true")
            .unwrap_or(false)
    };
    tracker_platform::autostart::set_autostart(enabled, &state.exe_path);
}

fn setup_tray(app: &mut tauri::App) -> tauri::Result<()> {
    let show = MenuItem::with_id(app, "show", "顯示主視窗", true, None::<&str>)?;
    let quit = MenuItem::with_id(app, "quit", "結束", true, None::<&str>)?;
    let menu = Menu::with_items(app, &[&show, &quit])?;

    let mut builder = TrayIconBuilder::with_id("main")
        .menu(&menu)
        .tooltip("活動追蹤器")
        .on_menu_event(|app, event| match event.id.as_ref() {
            "show" => {
                if let Some(window) = app.get_webview_window("main") {
                    let _ = window.show();
                    let _ = window.set_focus();
                }
            }
            "quit" => app.exit(0),
            _ => {}
        })
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                ..
            } = event
            {
                let app = tray.app_handle();
                if let Some(window) = app.get_webview_window("main") {
                    let _ = window.show();
                    let _ = window.set_focus();
                }
            }
        });

    if let Some(icon) = app.default_window_icon().cloned() {
        builder = builder.icon(icon);
    }
    builder.build(app)?;
    Ok(())
}
