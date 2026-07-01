# Phase 0 研究：電腦資訊監控紀錄（System Metrics）

**日期**：2026-07-01 ｜ **語言**：zh-TW ｜ **前置**：沿用 001 之 Rust + Cargo workspace + Tauri 2 + SQLCipher 架構

本檔解析技術脈絡中的未知項，並為每項技術選擇記錄「決策／理由／被否決的替代方案」。核心約束：**一律採 Windows 11 原廠方式與套件**（Microsoft 官方 `windows` crate 之 Win32/PDH/DXGI），**不引入顯示卡廠商專有 SDK**（如 NVIDIA NVML、AMD ADL）。

---

## R1 — CPU 整體使用率

- **決策**：以 Windows PDH（Performance Data Helper）計數器 `\Processor Information(_Total)\% Processor Utility`（或相容環境退回 `\Processor(_Total)\% Processor Time`）取得整機 CPU 使用率百分比。
- **理由**：PDH 為 Windows 原廠效能計數器 API，與工作管理員同源；`% Processor Utility` 是工作管理員採用的口徑（可反映渦輪加速，較貼近使用者觀感）。透過 `windows` crate 的 `Win32::System::Performance` 模組呼叫，無第三方相依。
- **替代方案（否決）**：`GetSystemTimes`（idle/kernel/user 差分自算）——可行但需自行維護差分狀態且不含 `% Processor Utility` 口徑；PDH 更直接且與工作管理員一致。

## R2 — 實體記憶體使用率

- **決策**：以 `GlobalMemoryStatusEx`（`Win32::System::SystemInformation`）取得 `ullTotalPhys` 與 `ullAvailPhys`，使用率 = (Total − Avail) / Total × 100。同時保存已用與總量（位元組）以利日後精確呈現。
- **理由**：單次呼叫即得瞬時值、零狀態、極低成本，為 Windows 原廠標準做法。
- **替代方案（否決）**：PDH `\Memory\% Committed Bytes In Use`——衡量的是認可記憶體（commit charge）而非實體使用率，語意與規格「實體記憶體使用率」不符。

## R3 — 各硬碟讀寫速度（每一顆硬碟）

- **決策**：以 PDH 計數器 `\PhysicalDisk(instance)\Disk Read Bytes/sec` 與 `\Disk Write Bytes/sec`，逐一實體磁碟（每個 instance，如 `0 C:`）分別取樣。裝置識別以 PDH instance 名稱為鍵。
- **理由**：PhysicalDisk 計數器為 Windows 原廠、每秒位元組率口徑即為規格所需「資料量／每秒」，PDH 內部已處理時間差分；逐 instance 天然對應「每一顆硬碟」。
- **替代方案（否決）**：`DeviceIoControl` + `IOCTL_DISK_PERFORMANCE`——需自行開裝置控制代碼與差分計算、權限較敏感；PDH 更簡潔且免額外權限。`LogicalDisk` 計數器——以磁碟區（C:/D:）為單位而非實體硬碟，與規格「每一顆硬碟」不符。

## R4 — 各網路介面收發速度（每一個網路介面）

- **決策**：以 PDH 計數器 `\Network Interface(instance)\Bytes Received/sec` 與 `\Bytes Sent/sec`，逐一介面分別取樣；整機合計由各介面加總（規格 FR-004）。裝置識別以 PDH instance 名稱為鍵。
- **理由**：Network Interface 計數器為 Windows 原廠、每秒位元組率、逐介面提供，直接滿足「逐一網路介面分別紀錄且可辨識」。
- **替代方案（否決）**：`GetIfTable2`（IP Helper）——回傳的是累積位元組計數，需自行做時間差分算速率；PDH 已內建速率口徑，較省事。可於裝置命名輔助時參考 `GetIfTable2` 的友善名稱。

## R5 — GPU 使用率與 GPU 記憶體（每一張 GPU，原廠、免廠商 SDK）

- **決策**：
  - **使用率**：以 PDH 計數器 `\GPU Engine(*)\Utilization Percentage` 取得，並依 instance 名稱中的 `luid` 歸戶到各 GPU，將同一 GPU 之各 engine（3D、Copy、Compute…）取其最大值或加總為該 GPU 使用率（採「取最大」貼近工作管理員的整體 GPU 呈現）。
  - **GPU 記憶體**：以 DXGI 列舉介面卡（`CreateDXGIFactory1` → `IDXGIFactory4/6` → `IDXGIAdapter3`），呼叫 `QueryVideoMemoryInfo`（`DXGI_MEMORY_SEGMENT_GROUP_LOCAL`）取得 `CurrentUsage`（已用位元組）與 `Budget`／`AdapterDesc.DedicatedVideoMemory`（總量位元組）。同時保存已用量與總量（規格 FR-005），百分比由兩者推算。
- **理由**：PDH `GPU Engine`／`GPU Adapter Memory` 計數器與 DXGI 皆為 **Windows 11 原廠**，工作管理員即以此呈現 GPU；完全不需 NVIDIA/AMD/Intel 專有 SDK，跨廠牌一致。`windows` crate 同時提供 `Win32::Graphics::Dxgi` 與 `Win32::System::Performance`。
- **替代方案（否決）**：
  - NVML（NVIDIA）/ ADL（AMD）——非原廠、僅單一廠牌、增加相依，違反「原廠方式」與原則 IV。
  - 僅用 PDH `\GPU Process Memory` 加總——只計行程用量、無「總量」可推算百分比，且跨行程加總易重複計算；DXGI 直接給裝置層級 used/total 更準確。
- **無 GPU / 僅內顯情境**：DXGI 列舉不到獨立介面卡、或 PDH 無 GPU 計數器時，該指標以「不適用／無資料」呈現（規格 US2/FR-007），不視為錯誤。

## R6 — 取樣排程與資源預算（SC-006）

- **決策**：以單一背景取樣執行緒，採固定週期喚醒（預設 1 秒，範圍 1–3600 秒，FR-012）。所有 PDH 計數器共用同一個 PDH query，每週期呼叫一次 `PdhCollectQueryData` 後批次讀出各計數器，降低系統呼叫成本。DXGI 介面卡物件於裝置清單建立時快取、重複使用。
- **理由**：共用單一 PDH query 與快取 DXGI 物件可將每次取樣成本壓到極低，維持「閒置時平均 CPU < 2%、記憶體 < 150 MB」（SC-006）。事件驅動不適用（指標為連續量，需定期取樣）。
- **替代方案（否決）**：每指標各自建立 query／每次重新列舉裝置——系統呼叫與配置成本高，難達資源預算。

## R7 — 裝置熱插拔（硬碟／GPU／網路介面新增或移除）

- **決策**：每個取樣週期前，若距上次列舉超過設定間隔（如每 30 秒）或 PDH 回報 instance 變動，重新列舉可用 instance／DXGI 介面卡，更新「目前裝置組合」。已移除裝置的歷史紀錄保留不動（規格 US1 邊界、FR-008）。裝置以穩定識別鍵（PDH instance 名稱、GPU LUID）對應資料庫既有列，新裝置則新增列。
- **理由**：兼顧「後續取樣反映最新裝置」與「歷史紀錄保留已移除裝置資料」；避免每秒重新列舉的成本。
- **替代方案（否決）**：每秒重列舉——成本偏高；監聽 `WM_DEVICECHANGE`——可作為未來優化，但對本功能非必要（YAGNI，原則 IV）。

## R8 — 睡眠／休眠造成的時間軸空缺

- **決策**：沿用 001 之 `session_power`（`WM_POWERBROADCAST`）感知睡眠／喚醒；睡眠期間不取樣、不補值。回顧時，相鄰樣本時間差顯著大於取樣週期者，前端以「無資料」空缺呈現，不內插假造數值（規格 US3 邊界、原則 III）。
- **理由**：誠實反映實際量測（原則 III）；重用既有電源事件整合，無新增成本。
- **替代方案（否決）**：以 0 或內插填補——違反「不以臆測值填補」（原則 III）。

## R9 — 缺值處理

- **決策**：任一指標於當次取樣讀取失敗（驅動忙碌、計數器暫缺、感測器逾時）時，該指標欄位寫入 NULL，其餘指標照常寫入（FR-007）。缺值於呈現時明確標示，不以 0 混淆。
- **理由**：符合「紀錄缺值而非中斷整體取樣」（規格邊界、FR-007）。
- **替代方案（否決）**：整筆略過——會造成不必要的時間軸空缺，違反規格期望。

## R10 — 歷史趨勢的長區間降取樣（FR-014）

- **決策**：查詢時依所選區間長度自動選擇時間桶（bucket）粒度（如 ≤2 小時→原始逐筆；≤2 天→分鐘；≤2 個月→小時；更長→日），以 SQL 對每桶做 `AVG`（趨勢）與 `MAX`（保留尖峰）聚合後回傳。**原始逐筆樣本永久保留於保留期內，不被覆寫或刪除**（僅呈現層降取樣）。
- **理由**：長區間（可達 3650 天保留 × 每秒取樣）逐筆回傳將達數億列，查詢與繪圖過重；桶聚合維持回應流暢又不失尖峰資訊，且不竄改原始資料（原則 III、FR-014）。
- **替代方案（否決）**：背景預先產生多層彙總表（分鐘／小時／日 rollup）——效能更佳但增加寫入複雜度與儲存；先採查詢時聚合（較簡單，符合 YAGNI），若日後量測顯示不足再引入 rollup。

## R11 — 儲存與加密

- **決策**：沿用 001 之單一 SQLCipher 加密資料庫（`%APPDATA%\ActivityTracker\activity.db`），本功能新增指標相關資料表（見 data-model.md），共用既有 DPAPI 金鑰與連線（FR-013）。取樣以 WAL 模式寫入，定期 checkpoint。
- **理由**：符合「沿用既有本機加密儲存機制」（FR-013、原則 II）；不另建儲存系統（規格 Assumptions、原則 IV）。
- **替代方案（否決）**：獨立資料庫或明文檔——違反 FR-013 與隱私原則。

## R12 — 保留清理（FR-011）

- **決策**：每日（或每次啟動時）執行一次清理，刪除 `ts_utc < now − retention_days` 的樣本列（含各裝置子表，藉外鍵 `ON DELETE CASCADE`）。保留天數預設 30、範圍 1–3650（FR-011）。
- **理由**：使資料庫維持在「保留天數 × 每日取樣數」量級（SC-005），避免無限成長。
- **替代方案（否決）**：每次取樣即清理——頻率過高、無必要成本。

---

## 未解決項目

無。所有技術脈絡未知項均已解析，可進入 Phase 1 設計。
