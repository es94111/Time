//! 前景程序名稱解析（T022，FR-015）。
//!
//! 由前景視窗取得 PID，再以 `QueryFullProcessImageNameW` 取執行檔路徑，
//! 並嘗試由版本資訊讀出易讀名稱（FileDescription）。無法辨識時回傳 `None`
//! （呼叫端轉為「未知／其他」）。

use std::ffi::c_void;
use std::iter::once;
use std::ptr::null_mut;

use tracker_core::browsers;
use tracker_core::model::AppIdentity;
use windows::core::{PCWSTR, PWSTR};
use windows::Win32::Foundation::{CloseHandle, HWND};
use windows::Win32::Storage::FileSystem::{
    GetFileVersionInfoSizeW, GetFileVersionInfoW, VerQueryValueW,
};
use windows::Win32::System::Threading::{
    OpenProcess, QueryFullProcessImageNameW, PROCESS_NAME_WIN32,
    PROCESS_QUERY_LIMITED_INFORMATION,
};
use windows::Win32::UI::WindowsAndMessaging::{
    GetForegroundWindow, GetWindowTextW, GetWindowThreadProcessId,
};

/// 前景應用程式（身分 + 視窗標題）。
pub struct ForegroundApp {
    pub identity: AppIdentity,
    pub title: String,
}

/// 取得目前前景應用程式；無前景或無法辨識時回傳 `None`。
pub fn foreground_app() -> Option<ForegroundApp> {
    unsafe {
        let hwnd = GetForegroundWindow();
        if hwnd.0.is_null() {
            return None;
        }
        let mut pid: u32 = 0;
        GetWindowThreadProcessId(hwnd, Some(&mut pid));
        if pid == 0 {
            return None;
        }
        let path = process_image_path(pid)?;
        let exe = exe_name(&path);
        if exe.is_empty() {
            return None;
        }
        let display = file_description(&path).unwrap_or_else(|| pretty_stem(&exe));
        let is_browser = browsers::is_browser(&exe);
        let title = window_title(hwnd);
        Some(ForegroundApp {
            identity: AppIdentity { display_name: display, executable: exe, is_browser },
            title,
        })
    }
}

unsafe fn process_image_path(pid: u32) -> Option<String> {
    let handle = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid).ok()?;
    let mut buf = vec![0u16; 1024];
    let mut size = buf.len() as u32;
    let result =
        QueryFullProcessImageNameW(handle, PROCESS_NAME_WIN32, PWSTR(buf.as_mut_ptr()), &mut size);
    let _ = CloseHandle(handle);
    result.ok()?;
    Some(String::from_utf16_lossy(&buf[..size as usize]))
}

unsafe fn window_title(hwnd: HWND) -> String {
    let mut buf = [0u16; 512];
    let len = GetWindowTextW(hwnd, &mut buf);
    if len > 0 {
        String::from_utf16_lossy(&buf[..len as usize])
    } else {
        String::new()
    }
}

/// 由執行檔的版本資訊讀取 FileDescription（如「Visual Studio Code」）。
unsafe fn file_description(path: &str) -> Option<String> {
    let wide: Vec<u16> = path.encode_utf16().chain(once(0)).collect();
    let file = PCWSTR(wide.as_ptr());

    let size = GetFileVersionInfoSizeW(file, None);
    if size == 0 {
        return None;
    }
    let mut data = vec![0u8; size as usize];
    GetFileVersionInfoW(file, 0, size, data.as_mut_ptr() as *mut c_void).ok()?;

    // 1) 取語言／字碼頁。
    let trans_query: Vec<u16> = "\\VarFileInfo\\Translation".encode_utf16().chain(once(0)).collect();
    let mut trans_ptr: *mut c_void = null_mut();
    let mut trans_len: u32 = 0;
    let ok = VerQueryValueW(
        data.as_ptr() as *const c_void,
        PCWSTR(trans_query.as_ptr()),
        &mut trans_ptr,
        &mut trans_len,
    );
    if !ok.as_bool() || trans_ptr.is_null() || trans_len < 4 {
        return None;
    }
    let lang = *(trans_ptr as *const u16);
    let codepage = *((trans_ptr as *const u16).add(1));

    // 2) 讀 FileDescription。
    let sub = format!("\\StringFileInfo\\{lang:04x}{codepage:04x}\\FileDescription");
    let sub_w: Vec<u16> = sub.encode_utf16().chain(once(0)).collect();
    let mut val_ptr: *mut c_void = null_mut();
    let mut val_len: u32 = 0;
    let ok = VerQueryValueW(
        data.as_ptr() as *const c_void,
        PCWSTR(sub_w.as_ptr()),
        &mut val_ptr,
        &mut val_len,
    );
    if ok.as_bool() && !val_ptr.is_null() && val_len > 0 {
        let s = std::slice::from_raw_parts(val_ptr as *const u16, val_len as usize);
        let desc = String::from_utf16_lossy(s);
        let desc = desc.trim_end_matches('\0').trim().to_string();
        if !desc.is_empty() {
            return Some(desc);
        }
    }
    None
}

fn exe_name(path: &str) -> String {
    path.rsplit(['\\', '/']).next().unwrap_or(path).to_ascii_lowercase()
}

fn pretty_stem(exe: &str) -> String {
    let stem = exe.strip_suffix(".exe").unwrap_or(exe);
    let mut chars = stem.chars();
    match chars.next() {
        Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
        None => stem.to_string(),
    }
}
