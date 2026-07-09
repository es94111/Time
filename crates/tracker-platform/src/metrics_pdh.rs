//! PDH 效能計數器採集（T012、FR-001/003/004/005）。
//!
//! 依 research.md R6 使用**單一共用 query**、以英文計數器路徑（`PdhAddEnglishCounterW`）
//! 確保於任何顯示語言（含 zh-TW）皆可運作。速率型計數器需前後兩次 collect 方能成形，
//! 故建立時先 prime 一次。缺值一律回 `None`（原則 III）。

use windows::core::PCWSTR;
use windows::Win32::System::Performance::{
    PdhAddEnglishCounterW, PdhCloseQuery, PdhCollectQueryData, PdhGetFormattedCounterArrayW,
    PdhGetFormattedCounterValue, PdhOpenQueryW, PDH_FMT_COUNTERVALUE, PDH_FMT_COUNTERVALUE_ITEM_W,
    PDH_FMT_DOUBLE, PDH_HCOUNTER, PDH_HQUERY,
};

const PDH_MORE_DATA: u32 = 0x800007D2;
const ERROR_SUCCESS: u32 = 0;

/// 一次 PDH 採集的原始讀數（速率單位 bytes/sec，使用率 0–100）。
#[derive(Default)]
pub struct PdhReadings {
    pub cpu_pct: Option<f64>,
    /// (實例名, bytes/sec)
    pub disk_read: Vec<(String, f64)>,
    pub disk_write: Vec<(String, f64)>,
    pub net_rx: Vec<(String, f64)>,
    pub net_tx: Vec<(String, f64)>,
    /// GPU Engine（實例名含 luid_0x..._0x...）, 使用率百分比
    pub gpu_util: Vec<(String, f64)>,
}

fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}

/// 加入英文計數器；失敗時回 `None`（該項視為不可用）。
fn add_counter(query: PDH_HQUERY, path: &str) -> Option<PDH_HCOUNTER> {
    let w = wide(path);
    let mut counter = PDH_HCOUNTER::default();
    // SAFETY：query 有效、path 為以 NUL 結尾之寬字串、counter 為合法輸出位址。
    let status = unsafe { PdhAddEnglishCounterW(query, PCWSTR(w.as_ptr()), 0, &mut counter) };
    (status == ERROR_SUCCESS).then_some(counter)
}

/// PDH 取樣器：單一共用 query 與各計數器控點。
pub struct PdhSampler {
    query: PDH_HQUERY,
    cpu: Option<PDH_HCOUNTER>,
    disk_read: Option<PDH_HCOUNTER>,
    disk_write: Option<PDH_HCOUNTER>,
    net_rx: Option<PDH_HCOUNTER>,
    net_tx: Option<PDH_HCOUNTER>,
    gpu_util: Option<PDH_HCOUNTER>,
}

// PDH handles are owned by this sampler and only accessed through `&mut self`.
// The sampler may be moved into the metrics background thread, but it is not
// shared concurrently.
unsafe impl Send for PdhSampler {}

impl PdhSampler {
    /// 開啟 query、加入計數器並 prime 一次（速率計數器首次需基準）。
    pub fn new() -> Option<Self> {
        let mut query = PDH_HQUERY::default();
        // SAFETY：以 NULL 資料來源開啟即時 query。
        let status = unsafe { PdhOpenQueryW(PCWSTR::null(), 0, &mut query) };
        if status != ERROR_SUCCESS {
            return None;
        }
        let sampler = Self {
            cpu: add_counter(query, r"\Processor(_Total)\% Processor Time"),
            disk_read: add_counter(query, r"\PhysicalDisk(*)\Disk Read Bytes/sec"),
            disk_write: add_counter(query, r"\PhysicalDisk(*)\Disk Write Bytes/sec"),
            net_rx: add_counter(query, r"\Network Interface(*)\Bytes Received/sec"),
            net_tx: add_counter(query, r"\Network Interface(*)\Bytes Sent/sec"),
            gpu_util: add_counter(query, r"\GPU Engine(*)\Utilization Percentage"),
            query,
        };
        // Prime：第一次 collect 建立速率基準。
        // SAFETY：query 有效。
        unsafe { PdhCollectQueryData(sampler.query) };
        Some(sampler)
    }

    /// 採集一次；回傳各計數器讀數。
    pub fn collect(&mut self) -> PdhReadings {
        // SAFETY：query 有效。
        let status = unsafe { PdhCollectQueryData(self.query) };
        if status != ERROR_SUCCESS {
            return PdhReadings::default();
        }
        PdhReadings {
            cpu_pct: self.cpu.and_then(read_single),
            disk_read: self.disk_read.map(read_array).unwrap_or_default(),
            disk_write: self.disk_write.map(read_array).unwrap_or_default(),
            net_rx: self.net_rx.map(read_array).unwrap_or_default(),
            net_tx: self.net_tx.map(read_array).unwrap_or_default(),
            gpu_util: self.gpu_util.map(read_array).unwrap_or_default(),
        }
    }
}

impl Drop for PdhSampler {
    fn drop(&mut self) {
        // SAFETY：query 有效且僅關閉一次。
        unsafe {
            let _ = PdhCloseQuery(self.query);
        }
    }
}

/// 讀取單一實例計數器（如 CPU _Total）。
fn read_single(counter: PDH_HCOUNTER) -> Option<f64> {
    let mut value = PDH_FMT_COUNTERVALUE::default();
    // SAFETY：counter 有效、value 為合法輸出位址。
    let status =
        unsafe { PdhGetFormattedCounterValue(counter, PDH_FMT_DOUBLE, None, &mut value) };
    if status != ERROR_SUCCESS || value.CStatus != ERROR_SUCCESS {
        return None;
    }
    // SAFETY：以 PDH_FMT_DOUBLE 取值，union 之 doubleValue 有效。
    Some(unsafe { value.Anonymous.doubleValue })
}

/// 讀取萬用字元計數器陣列，回傳 (實例名, 值)；`_Total` 實例略過。
fn read_array(counter: PDH_HCOUNTER) -> Vec<(String, f64)> {
    let mut buffer_size = 0u32;
    let mut item_count = 0u32;
    // 第一次以 None 取得所需緩衝大小。
    // SAFETY：counter 有效；緩衝為 None 時 PDH 回報所需大小。
    let status = unsafe {
        PdhGetFormattedCounterArrayW(
            counter,
            PDH_FMT_DOUBLE,
            &mut buffer_size,
            &mut item_count,
            None,
        )
    };
    if status != PDH_MORE_DATA || buffer_size == 0 {
        return Vec::new();
    }

    let mut buffer = vec![0u8; buffer_size as usize];
    // SAFETY：buffer 對齊足夠（u8 向量），大小為 PDH 回報值；items 指向其起始。
    let items_ptr = buffer.as_mut_ptr() as *mut PDH_FMT_COUNTERVALUE_ITEM_W;
    let status = unsafe {
        PdhGetFormattedCounterArrayW(
            counter,
            PDH_FMT_DOUBLE,
            &mut buffer_size,
            &mut item_count,
            Some(items_ptr),
        )
    };
    if status != ERROR_SUCCESS {
        return Vec::new();
    }

    let mut out = Vec::with_capacity(item_count as usize);
    // SAFETY：PDH 已填入 item_count 個結構於 buffer 中。
    let items = unsafe { std::slice::from_raw_parts(items_ptr, item_count as usize) };
    for item in items {
        if item.FmtValue.CStatus != ERROR_SUCCESS {
            continue;
        }
        // SAFETY：szName 為 PDH 提供之以 NUL 結尾寬字串。
        let name = unsafe { item.szName.to_string() }.unwrap_or_default();
        if name.is_empty() || name.eq_ignore_ascii_case("_Total") {
            continue;
        }
        // SAFETY：以 PDH_FMT_DOUBLE 取值。
        let value = unsafe { item.FmtValue.Anonymous.doubleValue };
        out.push((name, value));
    }
    out
}
