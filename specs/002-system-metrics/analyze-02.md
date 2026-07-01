# Specification Analysis Report — 電腦資訊監控紀錄（002-system-metrics）

**Date**: 2026-07-01（複審：修復後重跑）｜ **Mode**: 唯讀跨文件一致性分析（spec.md / plan.md / tasks.md + data-model.md / contracts / constitution）
**Result**: ✅ **無 CRITICAL／HIGH／MEDIUM 問題**。前次分析（I1、S1、S2、C1、A1）之發現皆已修復並驗證。需求 100% 有任務覆蓋，可進入 `/speckit-implement`。

## Findings

| ID | Category | Severity | Location(s) | Summary | Recommendation |
|----|----------|----------|-------------|---------|----------------|
| — | — | — | — | 本輪未發現需處理之新問題 | — |

### 前次發現之修復驗證

| 前次 ID | 嚴重度 | 狀態 | 驗證位置 |
|---------|--------|------|----------|
| I1 遷移檔位置漂移 | MEDIUM | ✅ 已修復 | plan.md 結構圖改列 `tracker-storage/src/db.rs`，與 tasks.md:49 (T005) 一致 |
| S1 `metrics_enabled` 無 FR | MEDIUM | ✅ 已修復 | spec.md 新增 **FR-015**；plan 對照表、data-model §1/§5、contracts §4、T017/T032 皆回填追溯 |
| S2 `metrics` 篩選非規格 | LOW | ✅ 已修復 | contracts §2 標註為 UI 便利參數、非 FR 要求 |
| C1 SC-001 無專屬驗收 | LOW | ✅ 已修復 | tasks.md T037 明列「量測 1 小時缺漏率 < 1%（SC-001）」 |
| A1 桶粒度未量化 | LOW | ✅ 已修復 | data-model §4 改為毫秒門檻表（7.2M/172.8M/5.184G/日）；T011 引用常數 |

## Coverage Summary

| Requirement | Has Task? | Task IDs | Notes |
|-------------|-----------|----------|-------|
| FR-001 CPU 使用率 | ✅ | T011, T012, T016 | |
| FR-002 記憶體使用率 | ✅ | T013, T016 | |
| FR-003 逐顆硬碟讀寫 | ✅ | T012, T016 | |
| FR-004 逐介面收發（可加總） | ✅ | T012, T016 | 合計於呈現層計算 |
| FR-005 逐張 GPU 使用率＋VRAM | ✅ | T014, T016 | |
| FR-006 時間戳記＋持久化 | ✅ | T016, T017 | |
| FR-007 缺值不中斷 | ✅ | T011, T016, T017 | |
| FR-008 裝置增減 | ✅ | T016, T021 | |
| FR-009 即時檢視 | ✅ | T019, T024 | |
| FR-010 歷史區間回顧 | ✅ | T026, T027 | |
| FR-011 保留 1–3650＋自動清除 | ✅ | T030, T032 | |
| FR-012 取樣 1–3600 | ✅ | T017, T032 | |
| FR-013 沿用加密儲存 | ✅ | T005 | |
| FR-014 長區間降取樣 | ✅ | T011, T026 | 桶門檻毫秒量化 |
| **FR-015 啟用／停用開關** | ✅ | T017, T032, T034 | 新增，已全鏈路追溯 |
| SC-001 缺漏率 < 1% | ✅ | T037 | 專屬驗收步驟已補 |
| SC-002 負載反映方向 | ⚠️ 間接 | US1 獨立測試 | 屬驗證性成果，非可建置工件，可接受 |
| SC-003 即時 3 秒 | ✅ | T020, T024 | |
| SC-004 逐裝置可辨識 | ✅ | T021 | |
| SC-005 保留清除量級 | ✅ | T030 | |
| SC-006 資源預算 | ✅ | T036 | |

## Constitution Alignment Issues

無。原則 I–VI 全數通過；前次原則 V 的輕微張力（`metrics_enabled`、`metrics` 篩選）已消解——前者升格為正式 FR-015，後者明確標註為非規格便利參數。plan.md 憲章檢查範圍已同步更新為 `FR-001…FR-015`。

## Unmapped Tasks

無孤立任務。基礎／跨切任務（T001–T008、T035–T037）為合理的非單一 FR 對應項。

## Metrics

- Total Functional Requirements: **15**（FR-001…FR-015）
- Total Success Criteria: 6（可建置/驗證：SC-001/003/004/005/006）
- Total Tasks: 37（T001…T037）
- Requirement Coverage: 15/15 = **100%**
- Ambiguity Count: 0
- Duplication Count: 0
- Inconsistency Count: 0
- Scope-beyond-spec Count: 0
- Critical Issues Count: **0**

## Next Actions

無 CRITICAL/HIGH/MEDIUM——**可執行 `/speckit-implement`**。

- 建議：實作 US1 前先確認 `crates/tracker-core/tests/metrics.rs` 桶粒度邊界測試（T009）涵蓋 data-model §4 的四個毫秒門檻。
- SC-002 屬驗證性成果，於 US1 獨立測試（10 分鐘高負載）時一併確認即可，無需額外任務。

---

*本報告為唯讀分析，未修改任何規格/計畫/任務檔案；本檔覆寫前一版快照以反映修復後狀態。*
