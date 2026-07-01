//! 工作階段／電源事件（T024，FR-004，research.md R4）。
//!
//! 建立訊息專用（message-only）視窗，接收：
//! - `WM_WTSSESSION_CHANGE`（鎖定／解鎖，透過 `WTSRegisterSessionNotification`）。
//! - `WM_POWERBROADCAST`（睡眠／喚醒，透過 `RegisterSuspendResumeNotification`）。
//!
//! 以 [`SessionSignal`] 對外提供「目前是否可累計活躍」的狀態；睡眠或鎖定時為 `false`
//! （呼叫端據此結算並停止累計）。

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, OnceLock};
use std::thread;

use tracker_core::ports::SessionPowerPort;
use windows::core::w;
use windows::Win32::Foundation::{HANDLE, HINSTANCE, HWND, LPARAM, LRESULT, WPARAM};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::System::Power::RegisterSuspendResumeNotification;
use windows::Win32::System::RemoteDesktop::{WTSRegisterSessionNotification, NOTIFY_FOR_THIS_SESSION};
use windows::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, DefWindowProcW, DispatchMessageW, GetMessageW, PostQuitMessage,
    RegisterClassW, TranslateMessage, DEVICE_NOTIFY_WINDOW_HANDLE, HMENU, HWND_MESSAGE, MSG,
    PBT_APMRESUMEAUTOMATIC, PBT_APMRESUMESUSPEND, PBT_APMSUSPEND, WINDOW_EX_STYLE, WINDOW_STYLE,
    WM_DESTROY, WM_POWERBROADCAST, WM_WTSSESSION_CHANGE, WNDCLASSW, WTS_SESSION_LOCK,
    WTS_SESSION_UNLOCK,
};

/// 目前可累計活躍與否的共享狀態。
pub struct SessionSignal {
    active: AtomicBool,
    locked: AtomicBool,
    suspended: AtomicBool,
}

impl SessionSignal {
    fn new() -> Self {
        Self {
            active: AtomicBool::new(true),
            locked: AtomicBool::new(false),
            suspended: AtomicBool::new(false),
        }
    }

    fn recompute(&self) {
        let active = !(self.locked.load(Ordering::SeqCst) || self.suspended.load(Ordering::SeqCst));
        self.active.store(active, Ordering::SeqCst);
    }

    /// 目前是否可累計活躍（未鎖定、未睡眠）。
    pub fn is_active(&self) -> bool {
        self.active.load(Ordering::SeqCst)
    }
}

static SIGNAL: OnceLock<Arc<SessionSignal>> = OnceLock::new();

/// 以工作階段／電源事件更新的 [`SessionPowerPort`]。
pub struct WinSessionPower {
    signal: Arc<SessionSignal>,
}

impl SessionPowerPort for WinSessionPower {
    fn is_session_active(&self) -> bool {
        self.signal.is_active()
    }
}

/// 啟動事件執行緒並回傳共享狀態。重複呼叫回傳既有狀態。
pub fn start() -> Arc<SessionSignal> {
    if let Some(existing) = SIGNAL.get() {
        return existing.clone();
    }
    let signal = Arc::new(SessionSignal::new());
    let _ = SIGNAL.set(signal.clone());
    thread::Builder::new()
        .name("tracker-events".into())
        .spawn(|| unsafe { run_message_loop() })
        .ok();
    signal
}

/// 取得可注入服務的 [`SessionPowerPort`]。
pub fn port(signal: Arc<SessionSignal>) -> WinSessionPower {
    WinSessionPower { signal }
}

unsafe fn run_message_loop() {
    let hmodule = match GetModuleHandleW(None) {
        Ok(h) => h,
        Err(_) => return,
    };
    let hinstance: HINSTANCE = hmodule.into();
    let class_name = w!("ActivityTrackerEvents");
    let wc = WNDCLASSW {
        lpfnWndProc: Some(wndproc),
        hInstance: hinstance,
        lpszClassName: class_name,
        ..Default::default()
    };
    RegisterClassW(&wc);

    let hwnd = match CreateWindowExW(
        WINDOW_EX_STYLE(0),
        class_name,
        w!("ActivityTrackerEvents"),
        WINDOW_STYLE(0),
        0,
        0,
        0,
        0,
        HWND_MESSAGE,
        HMENU::default(),
        hinstance,
        None,
    ) {
        Ok(h) => h,
        Err(_) => return,
    };

    let _ = WTSRegisterSessionNotification(hwnd, NOTIFY_FOR_THIS_SESSION);
    let _ = RegisterSuspendResumeNotification(HANDLE(hwnd.0), DEVICE_NOTIFY_WINDOW_HANDLE);

    let mut msg = MSG::default();
    while GetMessageW(&mut msg, HWND::default(), 0, 0).as_bool() {
        let _ = TranslateMessage(&msg);
        DispatchMessageW(&msg);
    }
}

extern "system" fn wndproc(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    unsafe {
        match msg {
            WM_WTSSESSION_CHANGE => {
                if let Some(sig) = SIGNAL.get() {
                    match wparam.0 as u32 {
                        WTS_SESSION_LOCK => {
                            sig.locked.store(true, Ordering::SeqCst);
                            sig.recompute();
                        }
                        WTS_SESSION_UNLOCK => {
                            sig.locked.store(false, Ordering::SeqCst);
                            sig.recompute();
                        }
                        _ => {}
                    }
                }
                LRESULT(0)
            }
            WM_POWERBROADCAST => {
                if let Some(sig) = SIGNAL.get() {
                    match wparam.0 as u32 {
                        PBT_APMSUSPEND => {
                            sig.suspended.store(true, Ordering::SeqCst);
                            sig.recompute();
                        }
                        PBT_APMRESUMEAUTOMATIC | PBT_APMRESUMESUSPEND => {
                            sig.suspended.store(false, Ordering::SeqCst);
                            sig.recompute();
                        }
                        _ => {}
                    }
                }
                LRESULT(1)
            }
            WM_DESTROY => {
                PostQuitMessage(0);
                LRESULT(0)
            }
            _ => DefWindowProcW(hwnd, msg, wparam, lparam),
        }
    }
}
