# Quickstart：電腦資訊監控紀錄（System Metrics）

**日期**：2026-07-01 ｜ **語言**：zh-TW ｜ **前置**：本功能建構於 001 既有 Cargo workspace 之上。

本檔說明如何在既有專案結構中開發、執行與驗證本功能。**採 Windows 11 原廠方式**：所有系統量測透過 Microsoft 官方 `windows` crate 的 PDH（效能計數器）與 DXGI，**不使用任何顯示卡廠商專有 SDK**。

---

## 1. 前置需求

- Windows 11（含 Home 版）；GPU 指標於有 WDDM 驅動之顯示卡上可用，無獨顯時 GPU 指標顯示「不適用」。
- Rust 最新穩定版（`rustup update`）。
- 沿用 001 之相依（`windows`、`rusqlite` SQLCipher、`tauri` 2、`jiff`、`serde` 等），無需新增第三方套件即可完成本功能。

## 2. 新增／擴充的相依（皆為 Microsoft 官方，於既有 `windows` crate 啟用 features）

於根 `Cargo.toml` 的 `windows` 相依補上本功能所需 features（實作時以 `cargo add` 微調版本）：

```toml
windows = { version = "0.58", features = [
  # 既有（001）...
  "Win32_System_Performance",       # PDH：CPU / PhysicalDisk / Network Interface / GPU Engine 計數器
  "Win32_System_SystemInformation", # GlobalMemoryStatusEx
  "Win32_Graphics_Dxgi",            # DXGI：列舉介面卡、QueryVideoMemoryInfo（VRAM 已用/總量）
  "Win32_Graphics_Dxgi_Common",
  "Win32_NetworkManagement_IpHelper" # （選用）介面友善名稱
] }
```

> 不新增 NVML/ADL 等廠商 crate（原則 IV、原廠要求）。

## 3. 專案結構（於既有 crate 內新增模組，不新增 crate）

```text
crates/
├── tracker-core/
│   └── src/
│       └── metrics.rs          # 【新】取樣排程參數、桶粒度選擇、缺值/速率純邏輯（可單元測試）
├── tracker-platform/
│   └── src/
│       ├── metrics_pdh.rs      # 【新】PDH query：CPU、PhysicalDisk、Network Interface、GPU Engine
│       ├── metrics_mem.rs      # 【新】GlobalMemoryStatusEx
│       └── metrics_gpu.rs      # 【新】DXGI 列舉 + QueryVideoMemoryInfo（VRAM）+ GPU Engine 歸戶
└── tracker-storage/
    └── src/
        ├── metrics_repo.rs     # 【新】寫入樣本、區間查詢、降取樣聚合、保留清理
        └── migrations/         # 【新】新增指標資料表之遷移
src-tauri/src/
├── metrics_service.rs          # 【新】背景取樣迴圈（platform → core → storage）
└── commands.rs                 # 【擴充】新增 contracts/tauri-commands.md 之命令
ui/src/
├── views/metrics-live.ts       # 【新】即時檢視（US2）
├── views/metrics-history.ts    # 【新】歷史趨勢（US3）
└── i18n/zh-TW.ts               # 【擴充】新增指標相關字串
```

**設計原則**：純邏輯（桶粒度、速率換算、缺值規則）放 `tracker-core` 以便在非 Windows 上單元測試；平台量測放 `tracker-platform`；儲存放 `tracker-storage`。（呼應原則 III 之可驗證性與 001 之分層。）

## 4. 開發流程

1. **資料層先行**：於 `tracker-storage` 加遷移，建立 data-model.md 之資料表，寫 `metrics_repo` 的寫入／查詢／降取樣／清理，並以單元測試驗證聚合與保留邏輯（可用記憶體 SQLite）。
2. **平台量測**：實作 `metrics_pdh`／`metrics_mem`／`metrics_gpu`，以 trait 抽象採集介面，非 Windows 環境以假實作測試 `tracker-core` 邏輯。
3. **背景服務**：`metrics_service` 依 `metrics_sample_interval_sec` 週期取樣，共用單一 PDH query（R6），每週期一次 `PdhCollectQueryData` 後批次讀出並寫入一筆 `metric_sample` 及各裝置子列。
4. **命令與事件**：於 `commands.rs` 實作契約命令；每次取樣後 emit `metrics://sample` 供即時檢視。
5. **前端**：即時檢視與歷史趨勢畫面，zh-TW 介面；GPU 缺席時顯示「不適用」。

## 5. 執行

```powershell
# 開發模式（Tauri）
cargo tauri dev

# 發行建置
cargo tauri build
```

## 6. 驗證（對應 Success Criteria）

- **SC-001（缺漏率 <1%）**：以預設 1 秒取樣執行 1 小時，查 `SELECT COUNT(*) FROM metric_sample`，應約 3600 筆（睡眠空缺除外）。
- **SC-002（負載可見）**：複製大檔／下載／跑 GPU 運算，於歷史趨勢確認對應指標明顯上升。
- **SC-003（即時 3 秒）**：製造負載後，即時檢視畫面於 3 秒內反映。
- **SC-004（逐裝置可辨識）**：多硬碟／多 GPU 機器上，`list_metric_devices` 與圖例逐一區分。
- **SC-005（保留清理）**：將 `metrics_retention_days` 設短並回填舊資料，觸發清理後確認最舊列被刪、筆數維持在量級。
- **SC-006（資源預算）**：預設 1 秒取樣、系統閒置時，以工作管理員觀察本工具自身平均 CPU < 2%、記憶體 < 150 MB。

## 7. 常見注意事項

- **PDH 首次讀值**：速率型計數器（bytes/sec、utilization）需兩次 collect 才有有效差分；服務啟動後第一筆可能為 0 或缺值，屬正常。
- **GPU Engine 歸戶**：`GPU Engine(*)` instance 名稱含 `luid_0x…_phys_…_eng_…_engtype_…`，以 `luid` 歸到 `gpu_device`，同 GPU 各 engine 取最大值為使用率（R5）。
- **無獨顯**：DXGI 僅列出內顯或列不到獨立 GPU 時，GPU 指標以「不適用」呈現，非錯誤（US2）。
- **睡眠喚醒**：期間不取樣、不補值；時間軸空缺由前端以「無資料」呈現（原則 III）。
