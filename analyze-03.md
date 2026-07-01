# Specification Analysis Report

**功能**：003-remote-sync-web-login（遠端資料同步與網頁儀表板，含登入驗證）
**分析對象**：`spec.md`、`plan.md`、`tasks.md`（含 `research.md`、`data-model.md`、`contracts/rest-api.md`、`contracts/tauri-commands.md`、`quickstart.md` 交叉參照）
**憲章版本**：v1.1.0
**任務總數（現況）**：71（T001–T071）

> 本報告為唯讀分析，未修改任何檔案。此為第二輪分析：上一輪（舊版 `analyze-03.md`）之 7 項發現（C1、C2、C3、U1、U2、I1、D1）已確認修復（詳見下方「前輪發現複驗」），本輪聚焦剩餘與新發現之項目。

## 前輪發現複驗（已修復，複驗通過）

| ID | 摘要 | 複驗結果 |
|----|------|----------|
| C1 | FR-008 登出即失效缺契約測試 | ✅ T062（`server/tests/logout.rs`）已存在 |
| C2 | FR-014 滑動視窗缺測試 | ✅ T063（`server/tests/session_sliding_window.rs`）已存在 |
| C3 | SC-003 缺端對端整合測試 | ✅ T036（`server/tests/offline_resync_e2e.rs`）已存在 |
| U1 | 靜態加密（SSE）決策未記錄 | ✅ research.md R11 已新增，T014 已補上 SSE 要求 |
| U2 | 密碼最小長度驗證未明示於任務 | ✅ T027、T065 已補上「MUST ≥8 字元」字樣 |
| I1 | MVP 階段裝置合併尚未生效易誤解 | ✅ quickstart.md「已知限制（MVP 階段）」段落已新增 |
| D1 | T021 與 T060（舊編號）鎖定情境職責重疊 | ✅ T021／T061 已於任務描述互相註明分工 |

## 本輪新發現

| ID | 類別 | 嚴重度 | 位置 | 摘要 | 建議 |
|----|------|--------|------|------|------|
| C4 | Coverage Gap | MEDIUM | spec.md FR-006；tasks.md Phase 5 Tests（僅 T045）、T048 | 分享授權建立／撤銷（`POST /shares`、`DELETE /shares/{id}`）與「被授權帳號僅唯讀、不得再分享」之限制，除 T045（`GET /data`）外無任何專屬契約測試任務 | 於 Phase 5 新增契約測試任務（如 `server/tests/share_routes.rs`）涵蓋：建立/撤銷分享成功案例、被授權帳號嘗試呼叫管理端點應被拒絕 |
| C5 | Coverage Gap | MEDIUM | spec.md FR-012、FR-013；tasks.md T067、T068（Phase 8 無測試小節） | `PATCH /account/settings`、`DELETE /account` 與 `retention_cleanup` 排程（保留天數清除、刪除後 30 天永久清除）僅靠 T070 手動走查驗證，無自動化測試任務，與其他 FR 皆有契約/單元測試的模式不一致 | 新增測試任務：驗證 `retention_days` 更新生效、逾期資料清除邏輯、帳號刪除 30 天後永久清除排程（可用假時鐘/時間注入方式單元測試） |
| U3 | Ambiguity | MEDIUM | research.md R7；contracts/rest-api.md `POST /sync/batches` | 批次上傳酬載格式明確標註「multipart 或 JSON+gzip，依 payload 大小，實作時擇一並於此更新」，屬 Phase 1 設計階段遺留的未定案項目，影響 T034（契約測試）、T039（uploader）、T043（sync_routes）三者對「同一格式」的假設是否一致 | 建議在進入 Phase 2/Foundational 實作前，於 `research.md`／`contracts/rest-api.md` 先定案單一酬載格式（或明確訂出依大小切換的門檻值），避免測試與實作各自猜測不同格式 |
| I2 | Inconsistency | MEDIUM | research.md R6 vs data-model.md `device` 表／tasks.md T055 | R6 提到「重新安裝／重置系統產生新 GUID 時，退回『裝置名稱＋使用者帳號』比對作輔助合併規則」，但 `data-model.md` 之裝置唯一鍵僅為 `(account_id, hardware_fingerprint)`，T055（`device_merge.rs`）任務描述亦僅提及硬體指紋比對，未反映此輔助合併規則 | 澄清此輔助規則是否納入本次範圍：若是，於 `data-model.md` 補充比對欄位與 T055 補充邏輯說明；若否，於 `research.md` 移除該敘述或註明「非本規格範圍（OS 重灌情境）」 |
| C6 | Coverage Gap | LOW | spec.md FR-002；tasks.md（US1/US2 測試小節） | FR-002「登入前不得上傳任何本機資料」目前僅以「同步代理僅於登入後啟動」之架構設計實現（T033/T041），未見測試任務驗證「未登入狀態下佇列資料確實不會被上傳」 | 於 US2 測試新增一則整合測試，確認未登入時 `agent` 不啟動、`sync_queue` 僅累積而無上傳行為 |
| I3 | Inconsistency | LOW | contracts/rest-api.md `GET /devices`（註記「或分享範圍內」）vs spec.md FR-006／data-model.md `share_grant` | `GET /devices` 契約提及被分享者可查詢裝置清單，但 FR-006 與 `share_grant` 資料模型僅描述「資料（活動追蹤／系統監控）」之唯讀分享，未明確定義裝置清單（裝置名稱等中繼資料）是否亦屬分享範圍 | 於 `data-model.md`／contracts 中澄清被分享者是否可見裝置清單；若不屬分享範圍，修改 `GET /devices` 契約移除該推論文字 |

## Coverage Summary Table（現況，任務編號 T001–T071）

| Requirement Key | Has Task? | Task IDs | Notes |
|---|---|---|---|
| FR-001 共用帳密登入 | ✅ | T027, T028-T033 | |
| FR-002 登入前不上傳 | ✅（架構實現，測試待補） | T033, T041 | C6：無獨立反向測試任務 |
| FR-003 每 1 分鐘輪詢 | ✅ | T040 | |
| FR-004 離線佇列/固定批次/磁碟滿捨棄 | ✅ | T034-T039 | U3：批次酬載格式尚未定案 |
| FR-005 網頁登入頁/未登入拒絕 | ✅ | T045, T046, T050 | |
| FR-006 僅本人資料+唯讀分享 | ✅（分享測試待補） | T046-T049 | C4：分享建立/撤銷/唯讀限制無專屬測試 |
| FR-007 多裝置清單/移除/合併 | ✅ | T052-T059 | I2：R6 輔助合併規則未反映於 data-model/任務 |
| FR-008 登出即失效 | ✅ | T027, T062 | |
| FR-009 改密後其他 session 需重新登入 | ✅ | T060, T064, T065 | |
| FR-010 連續 10 次失敗鎖定 15 分鐘 | ✅ | T021, T026, T061 | |
| FR-011 加密/密碼雜湊/最小長度 | ✅ | T014, T025, T027, T065 | |
| FR-012 帳號刪除保留 30 天 | ✅（測試待補） | T067, T068 | C5：無自動化測試任務 |
| FR-013 自訂保留天數 | ✅（測試待補） | T067, T068 | C5：無自動化測試任務 |
| FR-014 網頁滑動視窗 24h | ✅ | T016, T063 | |
| SC-002 5 分鐘內可查詢 | ✅ | T040, T043, T046 | |
| SC-003 24h 離線零遺漏零重複 | ✅ | T034, T035, T036 | |
| SC-006 未授權 100% 拒絕 | ✅ | T021, T045 | |
| SC-001/SC-004/SC-005（體感/滿意度指標） | 手動驗證 | T070 | 非可建置基礎設施項目，依 quickstart.md 手動驗證 |

## Constitution Alignment Issues

無違反。`plan.md` 之 Constitution Check 與 Post-Design Re-Check 完整涵蓋原則 I–VI；伺服器端相依套件皆於 Complexity Tracking 具正當理由；本輪新發現（C4、C5、U3、I2、I3、C6）皆屬測試覆蓋深度與設計細節澄清層級，非憲章原則違反。

## Unmapped Tasks

無。Polish 階段任務（T067-T071）分別對應 FR-012/013（T067/T068）與品質關卡（T069-T071），非孤立任務。

## Metrics

- Total Requirements：20（14 個 FR + 6 個 SC）
- Total Tasks：71
- Coverage %（至少 1 個任務，含實作或測試）：FR 14/14 = 100%；SC（可建置基礎設施類）3/3 = 100%
- 測試深度缺口（新發現）：3 項（C4、C5、C6）— 屬「有實作任務但測試覆蓋不足」，非零覆蓋
- Ambiguity Count：1（U3）
- Duplication Count：0
- Inconsistency Count：2（I2、I3）
- Critical Issues Count：0

## Next Actions

- 無 CRITICAL 問題，現況**可**維持 `/speckit-implement` 排程；惟建議優先處理 U3（批次酬載格式定案）——因其直接影響 T034 契約測試與 T039/T043 實作能否對齊同一介面，屬進入 Phase 2/4 前的高槓桿修正點。
- 建議在對應 Phase 動工前，依優先序處理：
  1. U3（Foundational/US2 前）：於 `research.md`／`contracts/rest-api.md` 定案批次上傳酬載格式
  2. C4（US3/Phase 5）：新增 `share_routes` 契約測試任務
  3. I2（US4/Phase 6）：釐清裝置輔助合併規則是否入本次範圍，同步更新 data-model.md 與 T055
  4. C5（Polish/Phase 8）：新增保留天數清理與帳號刪除排程之自動化測試
  5. C6（US2）、I3（US4/US3 分享澄清）：視資源許可補齊，屬 LOW 優先級
- 若採納上述建議，可視需要重新執行 `/speckit-tasks` 或手動編輯 `tasks.md` 新增對應任務後，再次執行 `/speckit-analyze` 複驗。

## Offer Remediation

是否要我針對本輪 6 項新發現（C4、C5、U3、I2、C6、I3）提出具體的修訂建議（例如新增測試任務的確切描述與檔案路徑、或 data-model.md／research.md 的補充文字草稿）？（僅提供建議，不會自動修改任何檔案。）
