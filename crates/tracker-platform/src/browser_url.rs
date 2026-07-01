//! 瀏覽器網址讀取（T033，FR-005，research.md R5）。
//!
//! 以 UI Automation（`IUIAutomation`）讀取前景瀏覽器網址列（Edit 控制項）的值，
//! 零設定、免安裝擴充。取不到時回傳 `None`（呼叫端退回僅計瀏覽器 App 時間）。

use std::cell::RefCell;

use windows::core::VARIANT;
use windows::Win32::System::Com::{
    CoCreateInstance, CoInitializeEx, CLSCTX_INPROC_SERVER, COINIT_APARTMENTTHREADED,
};
use windows::Win32::UI::Accessibility::{
    CUIAutomation, IUIAutomation, IUIAutomationValuePattern, TreeScope_Descendants,
    UIA_ControlTypePropertyId, UIA_EditControlTypeId, UIA_ValuePatternId,
};
use windows::Win32::UI::WindowsAndMessaging::GetForegroundWindow;

/// UIA 網址讀取器（僅於單一執行緒使用；非 Send/Sync）。
pub struct WinBrowserUrl {
    automation: RefCell<Option<IUIAutomation>>,
}

impl WinBrowserUrl {
    /// 建立實例（延遲初始化 COM 與自動化物件）。
    pub fn new() -> Self {
        Self { automation: RefCell::new(None) }
    }

    fn automation(&self) -> Option<IUIAutomation> {
        if self.automation.borrow().is_none() {
            unsafe {
                // 於本執行緒初始化 STA；已初始化時回傳 S_FALSE，忽略即可。
                let _ = CoInitializeEx(None, COINIT_APARTMENTTHREADED);
                let created: IUIAutomation =
                    CoCreateInstance(&CUIAutomation, None, CLSCTX_INPROC_SERVER).ok()?;
                *self.automation.borrow_mut() = Some(created);
            }
        }
        self.automation.borrow().clone()
    }

    /// 讀取目前前景視窗（若為瀏覽器）網址列的 URL。
    pub fn url_for_foreground(&self) -> Option<String> {
        unsafe {
            let automation = self.automation()?;
            let hwnd = GetForegroundWindow();
            if hwnd.0.is_null() {
                return None;
            }
            let element = automation.ElementFromHandle(hwnd).ok()?;
            let condition = automation
                .CreatePropertyCondition(
                    UIA_ControlTypePropertyId,
                    &VARIANT::from(UIA_EditControlTypeId.0),
                )
                .ok()?;
            let edit = element.FindFirst(TreeScope_Descendants, &condition).ok()?;
            let value_pattern: IUIAutomationValuePattern =
                edit.GetCurrentPatternAs(UIA_ValuePatternId).ok()?;
            let bstr = value_pattern.CurrentValue().ok()?;
            let url = bstr.to_string();
            if url.trim().is_empty() {
                None
            } else {
                Some(url)
            }
        }
    }
}

impl Default for WinBrowserUrl {
    fn default() -> Self {
        Self::new()
    }
}
