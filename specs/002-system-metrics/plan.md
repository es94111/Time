# 實作計畫：電腦資訊監控紀錄（System Metrics）

**Branch**: `002-system-metrics` | **Date**: 2026-07-01 | **Spec**: [spec.md](./spec.md)
**Input**: 功能規格 `specs/002-system-metrics/spec.md`

> 本計畫文件以正體中文（zh-TW）撰寫（憲章原則 I）。

## Summary

在既有的 Windows 11 活動追蹤器（001）之上，新增「電腦資訊監控紀錄」：以背景取樣持續量測並紀錄 **CPU 使用率、實體記憶體使用率、每顆硬碟讀寫速度、每個網路介面收發速度、每張 GPU 使用率與 GPU 記憶體（已用/總量）**，並提供**即時檢視**與**歷史趨勢回顧**。技術取向：延續 001 架構——以 **Rust** 實作、系統量測一律透過 **Microsoft 官方 `windows` crate** 的 **PDH（效能計數器）** 與 **DXGI**（**Windows 11 原廠方式，不使用任何顯示卡廠商專有 SDK**）；資料續存於既有 **SQLCipher（AES‑256）加密資料庫**（FR-013）；UI 沿用 **Tauri 2 + WebView2**，zh-TW 介面。取樣週期預設 1 秒（可調 1–3600 秒），保留預設 30 天（可調 1–3650 天），長區間趨勢於查詢時自動降取樣、原始樣本完整保留。

## Technical Context

**Language/Version**: Rust（最新穩定版；沿用 001 workspace，edition 2021、rust-version 1.80+）
**Primary Dependencies**（皆沿用 001，僅於官方 `windows` crate 啟用額外 features，不新增第三方套件）：
- `windows`（Microsoft 官方）— **PDH**（`Win32_System_Performance`）取 CPU／PhysicalDisk／Network Interface／GPU Engine 計數器；`GlobalMemoryStatusEx`（`Win32_System_SystemInformation`）取記憶體；**DXGI**（`Win32_Graphics_Dxgi`）列舉 GPU 並 `QueryVideoMemoryInfo` 取 VRAM 已用/總量；（選用）`IpHelper` 取介面友善名稱
- `rusqlite`（SQLCipher）— 續存指標於既有加密資料庫
- `tauri` 2.x + WebView2 — 即時檢視與歷史趨勢 UI
- `jiff`、`serde`、`serde_json` — 時間、序列化
**Storage**: 既有本機加密 SQLite（SQLCipher，AES‑256）`%APPDATA%\ActivityTracker\activity.db`，新增指標資料表（FR-013）；金鑰經 DPAPI 保護（沿用 001）
**Testing**: `cargo test`——`tracker-core` 純邏輯（桶粒度、速率、缺值、保留）單元測試；平台採集以 trait 抽象後於非 Windows 以假實作測試；儲存以記憶體 SQLite 測試聚合與清理
**Target Platform**: Windows 11（含 Home 版）桌面，使用者互動工作階段（沿用 001，非 Session 0 服務）
**Project Type**: Windows 桌面應用程式（既有 Cargo workspace 多 crate + Tauri，本功能為其上新增模組）
**Performance Goals**: 閒置且預設 1 秒取樣時，本工具自身平均 CPU < 2%、記憶體 < 150 MB（SC-006）；即時檢視變化後 3 秒內反映（SC-003）；1 小時取樣缺漏率 < 1%（SC-001）
**Constraints**: 純本機、預設不外傳（原則 II）；低 CPU／低記憶體（原則 IV）；缺值據實標示、睡眠空缺不補值（原則 III）；取樣 1–3600 秒、保留 1–3650 天（FR-011/012）；UI 與文件 zh-TW（原則 I）
**Scale/Scope**: 單一使用者、單機；資料量級為每秒一筆 ×（磁碟＋介面＋GPU）子列，保留期內自動清理以維持在「保留天數 × 每日取樣數」量級（SC-005）

## Constitution Check

*GATE：須於 Phase 0 前通過，並於 Phase 1 設計後再次檢查。*

依據憲章（`.specify/memory/constitution.md` v1.1.0）：

- **原則 I — 正體中文優先**：✅ plan/research/data-model/quickstart/contracts 全部 zh-TW；UI 與字串 zh-TW。
- **原則 II — 隱私優先與本機資料**：✅ 指標僅存本機既有加密資料庫、無任何外部傳輸與遙測（FR-013）；提供清除命令（`clear_metrics_data`，呼應資料可刪除）。
- **原則 III — 追蹤準確且誠實**：✅ 指標數值一律取自 Windows 原廠計數器／API 之實測；缺值以 NULL 據實標示、不以 0 假冒（FR-007）；睡眠空缺不內插（US3 邊界）；降取樣僅呈現層聚合、不竄改原始樣本（FR-014）。
- **原則 IV — 簡潔與低資源佔用**：✅ **不新增任何第三方套件**，僅於既有官方 `windows` crate 啟用 features；共用單一 PDH query、快取 DXGI 物件以壓低取樣成本（R6）；查詢時降取樣取代預先 rollup（YAGNI）。相依成本於 research.md 逐項說明。
- **原則 V — 規格驅動開發**：✅ 本計畫逐項對應已核可規格（FR-001…FR-015、SC-001…SC-006），未超出範圍。
- **原則 VI — Rust 實作語言**：✅ 量測、儲存、服務、後端皆 Rust；UI 採憲章明文允許之 Tauri；**未引入任何非 Rust 執行期新元件**（GPU 亦以原廠 DXGI/PDH，而非 C/C++ 廠商 SDK）。

**結論**：通過。本功能未新增非 Rust 執行期元件、未新增第三方相依，無需列入 Complexity Tracking。

## Project Structure

### Documentation (this feature)

```text
specs/002-system-metrics/
├── plan.md              # 本檔（/speckit-plan 輸出）
├── research.md          # Phase 0 輸出（R1–R12 技術決策）
├── data-model.md        # Phase 1 輸出（指標資料表與驗證）
├── quickstart.md        # Phase 1 輸出（開發／執行／驗證）
├── contracts/           # Phase 1 輸出
│   └── tauri-commands.md   # 前端↔Rust 後端 IPC 命令契約
└── tasks.md             # Phase 2 輸出（由 /speckit-tasks 產生，非本指令）
```

### Source Code (repository root) — 於既有 workspace 內新增模組

```text
crates/
├── tracker-core/
│   └── src/metrics.rs            # 【新】取樣參數、桶粒度選擇、速率/缺值純邏輯（可單元測試）
├── tracker-platform/
│   └── src/
│       ├── metrics_pdh.rs        # 【新】PDH：CPU / PhysicalDisk / Network Interface / GPU Engine
│       ├── metrics_mem.rs        # 【新】GlobalMemoryStatusEx（記憶體 used/total）
│       └── metrics_gpu.rs        # 【新】DXGI 列舉 + QueryVideoMemoryInfo（VRAM）+ GPU Engine 歸戶
└── tracker-storage/
    └── src/
        ├── metrics_repo.rs       # 【新】寫入樣本、區間查詢、降取樣聚合、保留清理
        └── db.rs                 # 【擴充】沿用 001 遷移機制，新增指標資料表遷移
src-tauri/src/
├── metrics_service.rs            # 【新】背景取樣迴圈（platform → core → storage）+ emit 事件
└── commands.rs                   # 【擴充】新增 IPC 命令（見 contracts）
ui/src/
├── views/metrics-live.ts         # 【新】即時檢視（US2）
├── views/metrics-history.ts      # 【新】歷史趨勢（US3）
└── i18n/zh-TW.ts                 # 【擴充】指標字串
```

**Structure Decision**：**沿用 001 之 Cargo workspace 分層，於既有 crate 內新增模組而非新增 crate**（原則 IV 簡潔）。純邏輯（桶粒度、速率換算、缺值規則）置於 `tracker-core` 以便於非 Windows 環境單元測試（呼應原則 III）；Windows 原廠量測置於 `tracker-platform`；指標續存於 `tracker-storage`（同一 SQLCipher 資料庫）。背景取樣以**單一常駐執行緒 + 共用 PDH query** 實作，與 001 的追蹤服務並存於同一 Tauri 應用程式（單一使用者工作階段程序）。

## Complexity Tracking

> 無違反項目：本功能未新增第三方相依、未新增非 Rust 執行期元件，故不需填列。

## 階段產出對照

| 規格條目 | 對應設計位置 |
|----------|--------------|
| FR-001 CPU 使用率 | tracker-platform/metrics_pdh.rs（PDH Processor）|
| FR-002 記憶體使用率 | tracker-platform/metrics_mem.rs（GlobalMemoryStatusEx）|
| FR-003 逐顆硬碟讀寫 | metrics_pdh.rs（PhysicalDisk）+ data-model disk_sample |
| FR-004 逐介面收發、可加總 | metrics_pdh.rs（Network Interface）+ net_sample |
| FR-005 逐張 GPU 使用率＋VRAM used/total | metrics_gpu.rs（DXGI + GPU Engine）+ gpu_sample |
| FR-006 時間戳記＋持久化 | metrics_service.rs + tracker-storage（metric_sample）|
| FR-007 缺值不中斷 | tracker-core/metrics.rs（缺值規則）+ nullable 欄位 |
| FR-008 裝置增減 | metrics_pdh/gpu 重列舉（R7）+ Device 表保留舊列 |
| FR-009 即時檢視 | commands.rs `get_current_metrics` + 事件 metrics://sample |
| FR-010 歷史區間回顧 | metrics_repo.rs 區間查詢 + commands `get_metrics_range` |
| FR-011 保留 1–3650、自動清除 | metrics_repo.rs 清理（CASCADE）+ setting |
| FR-012 取樣 1–3600 | metrics_service.rs + set_metrics_settings 驗證 |
| FR-013 沿用加密儲存 | tracker-storage（同一 SQLCipher 資料庫/DPAPI 金鑰）|
| FR-014 長區間降取樣 | metrics_repo.rs 桶聚合查詢（原始樣本保留）|
| FR-015 啟用／停用開關 | setting `metrics_enabled` + metrics_service 迴圈旗標檢查 + set_metrics_settings |
| SC-003 即時 3 秒 | metrics_service emit + ui/views/metrics-live.ts |
| SC-004 逐裝置可辨識 | Device 表 + list_metric_devices |
| SC-006 資源預算 | 單一 PDH query、快取 DXGI（R6）|

## Post-Design Constitution Re-Check

Phase 1 設計完成後重新檢查：

- **I**：所有產出（research/data-model/quickstart/contracts）皆 zh-TW。✅
- **II**：資料模型無任何外傳欄位；contracts 無網路端點；具清除命令。✅
- **III**：data-model 明確以 NULL 表缺值、睡眠空缺不內插；降取樣不改原始樣本。✅
- **IV**：零新增第三方相依；共用 PDH query／快取 DXGI；查詢時聚合取代預先 rollup。✅
- **V**：contracts 與 data-model 僅覆蓋規格範圍（FR-001…FR-015、SC-001…SC-006）。✅
- **VI**：所有後端邏輯 Rust；GPU 亦採原廠 DXGI/PDH，無廠商 SDK；無新增非 Rust 執行期元件。✅

**Gate 狀態**：通過，可進入 `/speckit-tasks`。

## 下一步

執行 **`/speckit-tasks`** 依本計畫與設計產出可執行、相依排序的 `tasks.md`。
