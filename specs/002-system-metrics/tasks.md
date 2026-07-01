---
description: "任務清單：電腦資訊監控紀錄（System Metrics）"
---

# Tasks: 電腦資訊監控紀錄（System Metrics）

**Input**: 設計文件位於 `specs/002-system-metrics/`
**Prerequisites**: plan.md（必要）、spec.md（使用者故事）、research.md、data-model.md、contracts/tauri-commands.md

**Tests**: 依 plan.md「Testing」策略，本功能對**純邏輯（tracker-core）**與**儲存聚合／清理（tracker-storage）**納入單元測試（呼應憲章原則 III 追蹤準確且誠實）；平台採集以 trait 抽象後於非 Windows 以假實作測試。UI 不強制測試。

**Organization**: 任務依使用者故事分組，各故事可獨立實作與驗證。

> 本任務文件以正體中文（zh-TW）撰寫（憲章原則 I）。

## Format: `[ID] [P?] [Story] Description`

- **[P]**: 可平行執行（不同檔案、無未完成相依）
- **[Story]**: 所屬使用者故事（US1／US2／US3）
- 每個任務均含明確檔案路徑

## Path Conventions（沿用 001 Cargo workspace 多 crate + Tauri）

- 純邏輯：`crates/tracker-core/`
- 平台量測：`crates/tracker-platform/`
- 儲存：`crates/tracker-storage/`
- 後端服務／IPC：`src-tauri/src/`
- 前端：`ui/src/`

---

## Phase 1: Setup（共用基礎設施）

**Purpose**: 於既有 workspace 內建立本功能所需的模組骨架與相依 features（不新增第三方套件）。

- [X] T001 於 `crates/tracker-platform/Cargo.toml` 為官方 `windows` crate 啟用額外 features：`Win32_System_Performance`（PDH）、`Win32_System_SystemInformation`（GlobalMemoryStatusEx）、`Win32_Graphics_Dxgi` 與 `Win32_Graphics_Dxgi_Common`（DXGI／QueryVideoMemoryInfo）、`Win32_NetworkManagement_IpHelper`（介面友善名稱，選用）
- [X] T002 [P] 建立 `crates/tracker-core/src/metrics.rs` 空模組並於 `crates/tracker-core/src/lib.rs` 新增 `pub mod metrics;`
- [X] T003 [P] 建立 `crates/tracker-platform/src/metrics_pdh.rs`、`metrics_mem.rs`、`metrics_gpu.rs` 空模組並於 `crates/tracker-platform/src/lib.rs` 宣告對應 `mod`
- [X] T004 [P] 建立 `crates/tracker-storage/src/metrics_repo.rs` 空模組並於 `crates/tracker-storage/src/lib.rs` 宣告 `pub mod metrics_repo;`

---

## Phase 2: Foundational（阻斷式前置作業）

**Purpose**: 所有使用者故事共同依賴的核心結構——資料表、領域型別、平台採集抽象、設定鍵。

**⚠️ CRITICAL**: 本階段完成前，任何使用者故事皆不可開始。

- [X] T005 於 `crates/tracker-storage/src/db.rs`（沿用 001 遷移機制）新增指標資料表遷移：`metric_sample`、`disk_device`、`disk_sample`、`network_interface`、`net_sample`、`gpu_device`、`gpu_sample` 及 `idx_metric_sample_ts` 索引，含 `ON DELETE CASCADE`（依 data-model.md §3）
- [X] T006 於 `crates/tracker-core/src/metrics.rs` 定義領域型別與單位／缺值約定：`MetricSample`、`DiskReading`、`NetReading`、`GpuReading`、`MetricSnapshot`（時間 epoch ms、速率 bytes/sec、記憶體/VRAM bytes、缺值以 `Option::None`）依 data-model.md §1
- [X] T007 於 `crates/tracker-platform/src/lib.rs` 定義平台採集抽象 trait `MetricsSampler`（回傳單次整機 `MetricSnapshot`），供 Windows 實作與非 Windows 假實作共用（呼應 plan.md 測試策略）
- [X] T008 於 `crates/tracker-storage/src/metrics_repo.rs` 實作指標設定鍵讀寫與預設值：`metrics_sample_interval_sec=1`、`metrics_retention_days=30`、`metrics_enabled=true`（沿用 001 `setting` 表，依 data-model.md §1 Setting）

**Checkpoint**: 基礎就緒——使用者故事可開始（可平行）。

---

## Phase 3: User Story 1 - 持續紀錄整機資源使用狀況 (Priority: P1) 🎯 MVP

**Goal**: 背景以設定的取樣週期持續量測並持久化 CPU／記憶體／逐顆硬碟讀寫／逐介面收發／逐張 GPU 使用率與 VRAM，缺值不中斷整體流程。

**Independent Test**: 讓工具執行約 10 分鐘並刻意製造高負載（複製大檔、下載、GPU 運算），事後確認資料庫存在對應時間區間、數值隨負載明顯變化、且多硬碟/多 GPU 可逐一辨識的紀錄。

### Tests for User Story 1 ⚠️

> 先寫測試並確認 FAIL，再實作。

- [X] T009 [P] [US1] 於 `crates/tracker-core/tests/metrics.rs` 撰寫純邏輯單元測試：速率換算（前後計數器差／時間差）、缺值規則（單項失敗記 NULL、其餘照常）、桶粒度選擇邊界
- [X] T010 [P] [US1] 於 `crates/tracker-storage/tests/metrics_repo.rs` 以記憶體 SQLite 測試：寫入一筆整機樣本與各裝置子列、裝置 upsert 歸戶（同 identifier 不重複）、缺值欄位為 NULL

### Implementation for User Story 1

- [X] T011 [US1] 於 `crates/tracker-core/src/metrics.rs` 實作純邏輯：速率計算、缺值標記規則、桶粒度選擇函式（以 data-model.md §4 之毫秒門檻常數：≤7_200_000ms 原始／≤172_800_000ms 分鐘 60_000ms／≤5_184_000_000ms 小時 3_600_000ms／更長日 86_400_000ms）
- [X] T012 [P] [US1] 於 `crates/tracker-platform/src/metrics_pdh.rs` 實作 PDH 單一共用 query，採集 CPU（Processor）、PhysicalDisk 讀寫、Network Interface 收發、GPU Engine 使用率（依 research.md／plan.md，共用 query 壓低成本 R6）
- [X] T013 [P] [US1] 於 `crates/tracker-platform/src/metrics_mem.rs` 以 `GlobalMemoryStatusEx` 取實體記憶體 used/total（bytes）
- [X] T014 [P] [US1] 於 `crates/tracker-platform/src/metrics_gpu.rs` 以 DXGI 列舉介面卡並 `QueryVideoMemoryInfo` 取 VRAM used/total、以 LUID 為穩定識別，並將 GPU Engine 使用率歸戶至各 GPU（快取 DXGI 物件 R6）
- [X] T015 [US1] 於 `crates/tracker-platform/src/lib.rs` 提供 Windows `MetricsSampler` 實作，整合 T012–T014 為單次 `MetricSnapshot`；並提供非 Windows 假實作（供 T009/T010 及 CI）
- [X] T016 [US1] 於 `crates/tracker-storage/src/metrics_repo.rs` 實作寫入：`insert_snapshot`（寫 `metric_sample` + 各裝置子列）、裝置 upsert（`disk_device`／`network_interface`／`gpu_device` 依 identifier）、缺值寫 NULL（FR-006/007/008）
- [X] T017 [US1] 建立 `src-tauri/src/metrics_service.rs`：背景取樣迴圈（讀取 `metrics_sample_interval_sec` → 呼叫 `MetricsSampler` → `tracker-core` 換算 → `metrics_repo` 寫入），依 `metrics_enabled` 旗標檢查啟用狀態（停用時暫停取樣，FR-015），錯誤不中斷迴圈
- [X] T018 [US1] 於 `src-tauri/src/lib.rs` 與 `src-tauri/src/state.rs` 啟動 `metrics_service` 背景執行緒並與既有 001 追蹤服務並存於同一應用程式狀態

**Checkpoint**: US1 可獨立運作——工具背景持續產生可回顧的指標紀錄（MVP 完成）。

---

## Phase 4: User Story 2 - 即時檢視目前資源狀態 (Priority: P2)

**Goal**: 於介面顯示「現在這一刻」各指標最新值；無資料／不適用項以明確狀態呈現而非誤導性 0。

**Independent Test**: 開啟即時檢視畫面，於另一程式製造某類負載（如大量寫入硬碟），確認畫面對應指標於數秒內反映變化（SC-003）。

### Implementation for User Story 2

- [X] T019 [US2] 於 `src-tauri/src/commands.rs` 新增 `get_current_metrics` 命令，回傳最近一次 `MetricSnapshot | null`（contracts §1）
- [X] T020 [US2] 於 `src-tauri/src/metrics_service.rs` 於每次取樣後 emit `metrics://sample` 事件並附 `MetricSnapshot` 酬載（contracts 事件節；SC-003）
- [X] T021 [US2] 於 `src-tauri/src/commands.rs` 新增 `list_metric_devices` 命令，列出歷史出現過的硬碟／網路介面／GPU（含已移除，contracts §3）
- [X] T022 [P] [US2] 於 `ui/src/api.ts` 新增 `getCurrentMetrics`／`listMetricDevices` invoke 包裝與 `metrics://sample` 事件訂閱
- [X] T023 [P] [US2] 於 `ui/src/i18n/zh-TW.ts` 新增指標相關 zh-TW 字串（各指標名稱、「不適用」「無資料」等）
- [X] T024 [US2] 建立 `ui/src/views/metrics-live.ts` 即時檢視畫面：訂閱事件更新、缺值以明確狀態呈現、退回定期 `get_current_metrics` 輪詢；並於 `ui/src/main.ts` 加入導覽入口

**Checkpoint**: US1 與 US2 皆可獨立運作。

---

## Phase 5: User Story 3 - 回顧歷史趨勢 (Priority: P3)

**Goal**: 以時間軸回顧區間趨勢，長區間自動降取樣（AVG 趨勢＋MAX 尖峰），空缺區間以「無資料」呈現、不內插。

**Independent Test**: 選定一段已有紀錄的區間檢視趨勢，確認尖峰時間點與實際高負載事件吻合；選跨年區間確認以降取樣彙總點呈現且回應流暢（US3 驗收情境）。

### Tests for User Story 3 ⚠️

- [X] T025 [P] [US3] 於 `crates/tracker-storage/tests/metrics_repo.rs` 新增降取樣測試：桶聚合 AVG/MAX 正確、空缺桶不產生資料點、桶粒度依區間長度切換（data-model.md §4）

### Implementation for User Story 3

- [X] T026 [US3] 於 `crates/tracker-storage/src/metrics_repo.rs` 實作 `get_metrics_range`：依區間長度選桶粒度、對整機與各裝置以 AVG/MAX 桶聚合、空缺不補點、原始樣本不受影響（FR-010/014）
- [X] T027 [US3] 於 `src-tauri/src/commands.rs` 新增 `get_metrics_range` 命令，驗證 `toUtc > fromUtc` 否則回傳 zh-TW 錯誤（contracts §2）
- [X] T028 [P] [US3] 於 `ui/src/api.ts` 新增 `getMetricsRange` invoke 包裝（`TrendSeries` 型別）
- [X] T029 [US3] 建立 `ui/src/views/metrics-history.ts` 趨勢畫面（區間選擇、顯示彙總點、空缺標示「無資料」）；並於 `ui/src/main.ts` 加入導覽入口

**Checkpoint**: 三個使用者故事皆可獨立運作。

---

## Phase 6: Polish & Cross-Cutting Concerns（跨故事）

**Purpose**: 設定管理、保留清理、資料刪除、能力宣告與資源預算驗證。

- [X] T030 於 `crates/tracker-storage/src/metrics_repo.rs` 實作保留清理 `purge_expired`（依 `metrics_retention_days` 刪除過期 `metric_sample`，CASCADE 連帶清子列，FR-011／SC-005）
- [X] T031 於 `src-tauri/src/metrics_service.rs` 定期觸發保留清理（每日一次或啟動時），變更保留天數後可立即觸發一次
- [X] T032 於 `src-tauri/src/commands.rs` 新增 `get_metrics_settings`／`set_metrics_settings`，驗證 `sampleIntervalSec ∈ [1,3600]`、`retentionDays ∈ [1,3650]`，超出回傳 zh-TW 錯誤訊息；並支援切換 `metrics_enabled` 啟用／停用（contracts §4，FR-011/012/015）
- [X] T033 於 `src-tauri/src/commands.rs` 新增 `clear_metrics_data`（`beforeUtc?` 省略即全刪），回傳 `deletedSamples`（contracts §5，憲章資料可刪除）
- [X] T034 [P] 於 `ui/src/views/settings.ts` 擴充指標設定（取樣週期、保留天數、啟用開關、清除資料），並於 `ui/src/api.ts`／`ui/src/i18n/zh-TW.ts` 補上對應包裝與字串
- [X] T035 於 `src-tauri/capabilities/default.json` 允許本功能新增之全部 IPC 命令與 `metrics://sample` 事件
- [ ] T036 資源預算驗證：確認單一共用 PDH query 與快取 DXGI 生效，於閒置＋1 秒取樣下量測本工具平均 CPU < 2%、記憶體 < 150 MB（SC-006）
- [ ] T037 依 `specs/002-system-metrics/quickstart.md` 執行端到端驗證（開發／執行／資料檢查）；其中 MUST 量測連續執行 1 小時的實際紀錄筆數，確認缺漏率 < 1%（睡眠／休眠空缺除外，SC-001）

---

## Dependencies & Execution Order

### Phase Dependencies

- **Setup (Phase 1)**: 無相依，可立即開始
- **Foundational (Phase 2)**: 依賴 Setup 完成——阻斷所有使用者故事
- **User Stories (Phase 3–5)**: 皆依賴 Foundational 完成
  - US1（P1）為 MVP，建議先行；US2、US3 可於 Foundational 後平行或依序（P1→P2→P3）
- **Polish (Phase 6)**: 依賴所需使用者故事完成

### User Story Dependencies

- **US1 (P1)**: Foundational 後即可開始，不依賴其他故事
- **US2 (P2)**: Foundational 後即可開始；讀取 US1 產生之樣本，但即時檢視可對「最近一次樣本」獨立驗證
- **US3 (P3)**: Foundational 後即可開始；查詢 US1 之歷史資料，聚合查詢邏輯可獨立測試

### Within Each User Story

- 測試（若含）先寫並 FAIL → 再實作
- 型別／模型 → 平台採集 → 儲存 → 服務 → IPC → UI

### Parallel Opportunities

- Setup 中 T002／T003／T004 可平行
- US1 平台採集 T012／T013／T014 可平行（不同檔案）
- US1 測試 T009／T010 可平行
- US2 中 T022／T023 可平行；US3 中 T028 可平行
- Foundational 完成後，US1／US2／US3 可由不同開發者平行進行

---

## Parallel Example: User Story 1

```bash
# 先啟動 US1 測試（平行）：
Task: "tracker-core 純邏輯測試 in crates/tracker-core/tests/metrics.rs"
Task: "tracker-storage 寫入/歸戶測試 in crates/tracker-storage/tests/metrics_repo.rs"

# 平台採集三模組平行實作：
Task: "PDH 採集 in crates/tracker-platform/src/metrics_pdh.rs"
Task: "記憶體採集 in crates/tracker-platform/src/metrics_mem.rs"
Task: "GPU/DXGI 採集 in crates/tracker-platform/src/metrics_gpu.rs"
```

---

## Implementation Strategy

### MVP First（僅 User Story 1）

1. 完成 Phase 1 Setup
2. 完成 Phase 2 Foundational（阻斷所有故事）
3. 完成 Phase 3 User Story 1
4. **STOP and VALIDATE**：以 10 分鐘高負載測試獨立驗證 US1
5. 就緒即可 demo（已能累積可回顧的效能歷史）

### Incremental Delivery

1. Setup + Foundational → 基礎就緒
2. 加入 US1 → 獨立驗證 → demo（MVP）
3. 加入 US2 → 獨立驗證 → demo
4. 加入 US3 → 獨立驗證 → demo
5. Polish（設定／清理／刪除／資源預算）

---

## Notes

- [P] = 不同檔案、無相依，可平行
- [Story] 標籤對應使用者故事以利追溯
- 各使用者故事應可獨立完成與驗證
- 實作前先確認測試 FAIL
- 每完成一個任務或邏輯群組即提交
- 遵守憲章：不新增第三方相依、量測一律用官方 `windows` crate（PDH/DXGI）、缺值據實標示不假冒、UI 與文件 zh-TW
