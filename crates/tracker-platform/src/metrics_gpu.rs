//! GPU 採集（T014、FR-005）：以 DXGI 列舉介面卡並 `QueryVideoMemoryInfo` 取 VRAM
//! used/total，以 LUID 為穩定識別。GPU Engine 使用率之歸戶於 [`crate`] 層以 LUID 比對。
//!
//! 依 research.md R6：快取 DXGI factory 物件以壓低取樣成本。

use windows::core::Interface;
use windows::Win32::Graphics::Dxgi::{
    CreateDXGIFactory1, IDXGIAdapter1, IDXGIAdapter3, IDXGIFactory1, DXGI_ADAPTER_FLAG,
    DXGI_ADAPTER_FLAG_SOFTWARE, DXGI_ERROR_NOT_FOUND, DXGI_MEMORY_SEGMENT_GROUP_LOCAL,
    DXGI_QUERY_VIDEO_MEMORY_INFO,
};

/// 單張 GPU 的靜態識別與即時 VRAM。
pub struct GpuInfo {
    /// 以 LUID 組成之穩定識別鍵（與 GPU Engine 計數器實例名比對用），例如 `luid_0x00000000_0x0000c4e1`。
    pub luid_key: String,
    /// 介面卡名稱。
    pub name: String,
    /// GPU 記憶體已用位元組。
    pub mem_used_bytes: Option<i64>,
    /// GPU 記憶體總量位元組。
    pub mem_total_bytes: Option<i64>,
}

/// GPU 取樣器：持有快取的 DXGI factory（R6）。
pub struct GpuSampler {
    factory: IDXGIFactory1,
}

impl GpuSampler {
    /// 建立取樣器；DXGI 不可用時回傳 `None`（呼叫端視為無 GPU 資料）。
    pub fn new() -> Option<Self> {
        // SAFETY：DXGI factory 建立不需 COM 初始化。
        let factory: IDXGIFactory1 = unsafe { CreateDXGIFactory1().ok()? };
        Some(Self { factory })
    }

    /// 列舉目前介面卡並讀取 VRAM；每次重新列舉以反映裝置增減（FR-008）。
    pub fn read(&self) -> Vec<GpuInfo> {
        let mut out = Vec::new();
        let mut i = 0u32;
        loop {
            // SAFETY：依 DXGI 慣例列舉至 DXGI_ERROR_NOT_FOUND。
            let adapter: IDXGIAdapter1 = match unsafe { self.factory.EnumAdapters1(i) } {
                Ok(a) => a,
                Err(e) if e.code() == DXGI_ERROR_NOT_FOUND => break,
                Err(_) => break,
            };
            i += 1;

            let desc = match unsafe { adapter.GetDesc1() } {
                Ok(d) => d,
                Err(_) => continue,
            };
            // 略過軟體算繪介面卡（如 Microsoft Basic Render Driver）。
            if (DXGI_ADAPTER_FLAG(desc.Flags as i32).0 & DXGI_ADAPTER_FLAG_SOFTWARE.0) != 0 {
                continue;
            }

            let name = String::from_utf16_lossy(&desc.Description)
                .trim_end_matches('\0')
                .trim()
                .to_string();
            let luid_key = format!(
                "luid_0x{:08x}_0x{:08x}",
                desc.AdapterLuid.HighPart as u32, desc.AdapterLuid.LowPart
            );

            // 總量取 DedicatedVideoMemory；已用取 QueryVideoMemoryInfo(LOCAL).CurrentUsage。
            let mem_total = Some(desc.DedicatedVideoMemory as i64);
            let mem_used = adapter.cast::<IDXGIAdapter3>().ok().and_then(|a3| {
                let mut info = DXGI_QUERY_VIDEO_MEMORY_INFO::default();
                // SAFETY：node 0、本地記憶體區段；`info` 為合法可寫緩衝。
                let ok =
                    unsafe { a3.QueryVideoMemoryInfo(0, DXGI_MEMORY_SEGMENT_GROUP_LOCAL, &mut info) }
                        .is_ok();
                ok.then_some(info.CurrentUsage as i64)
            });

            out.push(GpuInfo { luid_key, name, mem_used_bytes: mem_used, mem_total_bytes: mem_total });
        }
        out
    }
}
