---

description: "Task list template for feature implementation"
---

# Tasks: 遠端資料同步與網頁儀表板（含登入驗證）

**Input**: Design documents from `/specs/003-remote-sync-web-login/`
**Prerequisites**: plan.md、spec.md、research.md、data-model.md、contracts/rest-api.md、contracts/tauri-commands.md、quickstart.md

**Tests**: 本專案（沿用 001/002）已建立 `cargo test` 測試慣例，且 plan.md Technical Context 明確要求用戶端與伺服器皆需測試，故下列包含測試任務。

**Organization**: 任務依 User Story（P1/P2 優先序）分組，確保各故事可獨立實作與驗證。

> 本任務文件以正體中文（zh-TW）撰寫（憲章原則 I）。
> 本版已依 `/speckit.analyze`（analyze-03.md）之 C1、C2、C3、U1、U2、D1 發現修訂：新增 3 則測試任務（T036、T062、T063）、補強 T014／T021／T027／T061／T065 之任務描述。任務總數由 68 增為 71（T001–T071）。

## Format: `[ID] [P?] [Story] Description`

- **[P]**：可平行執行（不同檔案、無相依）
- **[Story]**：對應 spec.md 之 US1～US5
- 所有任務皆附精確檔案路徑

## Path Conventions

- Windows 用戶端：沿用 001/002 之 `crates/`、`src-tauri/src/`、`ui/src/` workspace
- 遠端伺服器：新增獨立 Cargo workspace `server/`（見 plan.md Structure Decision）

---

## Phase 1: Setup（共用基礎建設）

**Purpose**：初始化 `server/` 獨立 workspace 與用戶端新增之 `tracker-sync` crate 骨架

- [X] T001 建立 `server/` Cargo workspace 骨架：`server/Cargo.toml`、`server/src/main.rs`（最小 `fn main() {}` stub）、`server/.gitignore`
- [X] T002 [P] 建立 `server/Dockerfile` 多階段建置（builder：官方 `rust:<最新穩定版>-slim` → runtime：`debian:stable-slim`，依 research.md R8）
- [X] T003 [P] 建立 `server/docker-compose.yml`（`server` + `postgres` + `minio`，依 quickstart.md 服務清單）
- [X] T004 於根 `Cargo.toml` workspace `members` 新增 `crates/tracker-sync`，並建立 `crates/tracker-sync/Cargo.toml`、`crates/tracker-sync/src/lib.rs` 最小骨架
- [X] T005 [P] 於根 `Cargo.toml` `[workspace.dependencies]` 新增 `reqwest`（`rustls-tls` feature，最新版），供 `tracker-sync` 使用
- [X] T006 [P] 於 `server/Cargo.toml` 加入相依：`axum`、`tokio`、`sqlx`（`postgres`、`runtime-tokio-rustls`）、`argon2`、`tower-sessions`、`tower-sessions-sqlx-store`、`aws-sdk-s3`、`tracing`、`tracing-subscriber`、`serde`、`serde_json`、`jiff`（均取最新穩定版，依 research.md R1-R3）

---

## Phase 2: Foundational（阻塞性前置需求）

**Purpose**：所有 User Story 皆依賴的資料庫結構、伺服器骨架與用戶端純邏輯模組

**⚠️ CRITICAL**：本階段完成前不可開始任何 User Story 工作

- [X] T007 [P] 建立 `server/migrations/0001_accounts.sql`：`account` 資料表（依 data-model.md，含 `password_hash`、`failed_login_count`、`locked_until`、`retention_days`、`deleted_at`）
- [X] T008 [P] 建立 `server/migrations/0002_devices.sql`：`device` 資料表（含 `(account_id, hardware_fingerprint)` 唯一鍵、`removed_at`）
- [X] T009 [P] 建立 `server/migrations/0003_sessions.sql`：`session` 資料表（`kind` 區分 `web`/`desktop`、`expires_at`、`revoked_at`，相容 `tower-sessions-sqlx-store` 結構）
- [X] T010 [P] 建立 `server/migrations/0004_sync_records.sql`：`sync_record` 資料表（`batch_dedup_key` 唯一鍵、`server_received_at`、`object_storage_key`）
- [X] T011 [P] 建立 `server/migrations/0005_share_grants.sql`：`share_grant` 資料表（`owner_account_id`、`grantee_account_id`、`scope`、`revoked_at`）
- [X] T012 `server/src/config.rs` 環境變數設定（`DATABASE_URL`、S3 endpoint/bucket/憑證、Session 簽章金鑰、預設保留天數）
- [X] T013 `server/src/db/mod.rs` 建立 `sqlx` PostgreSQL 連線池並於啟動時執行 migrations（depends on T007-T012）
- [X] T014 `server/src/storage/s3.rs`：`aws-sdk-s3` 封裝（put/get object），依 research.md R3 鍵值結構 `{bucket}/{account_id}/{device_id}/{yyyy}/{mm}/{dd}/{sync_record_id}.json.gz`；`put_object` 呼叫 MUST 設定伺服器端加密（SSE，依 research.md R11 決策），滿足 FR-011 靜態加密要求
- [X] T015 `server/src/main.rs` 組裝 axum Router、`tracing_subscriber` 初始化、`GET /healthz`（depends on T013）
- [X] T016 `server/src/auth/session.rs`：`tower-sessions` 設定（PostgreSQL store；`web` 滑動視窗 24 小時／`desktop` 固定 30 天雙策略中介層，依 research.md R5）（depends on T013）
- [X] T017 `server/src/api/mod.rs` 路由骨架與統一錯誤處理（`ApiError` → JSON 回應，涵蓋 401/403/423）（depends on T015, T016）
- [X] T018 [P] `crates/tracker-core/src/sync.rs`：待同步批次切分、去重鍵產生、佇列狀態純邏輯（可單元測試）
- [X] T019 [P] `crates/tracker-platform/src/device_id.rs`：讀取登錄檔 `HKLM\SOFTWARE\Microsoft\Cryptography\MachineGuid`（`windows` crate）產生裝置指紋（依 research.md R6）
- [X] T020 於 `crates/tracker-storage/src/db.rs` 的 `MIGRATION_SQL` 新增 `sync_queue`、`device_identity`、`auth_cache` 三張本機資料表（依 data-model.md 用戶端本機小節）

**Checkpoint**：基礎建設就緒，可開始任一 User Story 之實作

---

## Phase 3: User Story 1 - 建立帳號並登入 Windows 軟體 (Priority: P1) 🎯 MVP

**Goal**：使用者以帳號密碼登入 Windows 11 軟體，登入成功後顯示「已連線」，工作階段可於重啟軟體後維持

**Independent Test**：全新安裝的軟體輸入帳密登入，驗證登入成功後顯示「已連線」，重啟軟體後免重複登入

### Tests for User Story 1 ⚠️

- [X] T021 [P] [US1] 契約測試 `POST /auth/login`（單次請求：帳密正確成功、帳密錯誤 401、鎖定中 423；聚焦單次登入請求之回應行為，連續失敗達 10 次觸發鎖定的完整生命週期由 T061 驗證，避免重複）於 `server/tests/auth_login.rs`
- [X] T022 [P] [US1] 單元測試裝置指紋讀取（trait 抽象＋假硬體來源）於 `crates/tracker-platform/tests/device_id.rs`
- [X] T023 [P] [US1] 單元測試批次切分／去重鍵純邏輯於 `crates/tracker-core/tests/sync.rs`

### Implementation for User Story 1

- [X] T024 [P] [US1] `server/src/db/account_repo.rs`：Account CRUD 與依 `email_or_username` 查詢
- [X] T025 [US1] `server/src/auth/password.rs`：Argon2id 雜湊/驗證（depends on T024）
- [X] T026 [US1] `server/src/auth/lockout.rs`：連續失敗計數、寫入/檢查 `locked_until`（15 分鐘鎖定，FR-010）（depends on T024）
- [X] T027 [US1] `server/src/api/auth_routes.rs`：實作 `POST /auth/login`、`POST /auth/logout`、`POST /auth/account`（最小可用註冊，註冊密碼 MUST ≥8 字元驗證，FR-011）（depends on T025, T026, T016, T017）
- [X] T028 [P] [US1] `crates/tracker-sync/src/auth_client.rs`：呼叫伺服器登入/登出 API、憑證以 DPAPI 保護寫入本機 `auth_cache`（depends on T020）
- [X] T029 [US1] `src-tauri/src/auth_commands.rs`：`login`/`logout`/`get_connection_status` IPC 命令（depends on T028）
- [X] T030 [US1] `src-tauri/src/lib.rs` 註冊 `auth_commands` 並初始化連線狀態（depends on T029）
- [X] T031 [P] [US1] `ui/src/views/login.ts`：Windows 軟體登入畫面（依 tauri-commands.md 契約）
- [X] T032 [P] [US1] `ui/src/views/connection-status.ts`：顯示「已連線」狀態（依 tauri-commands.md `ConnectionStatus`）
- [X] T033 [US1] `src-tauri/src/tracking_service.rs` 擴充：登入成功後啟動同步代理（本階段為 stub，完整輪詢邏輯於 US2 補齊）（depends on T030）

**Checkpoint**：User Story 1 可獨立完整運作與測試（登入/登出/工作階段維持）

---

## Phase 4: User Story 2 - 資料自動上傳並離線補傳 (Priority: P1)

**Goal**：已登入裝置定期上傳本機資料；離線時佇列暫存，恢復連線後依時間序、固定批次補傳，零遺漏零重複

**Independent Test**：中斷網路後持續使用電腦，確認本機仍正常記錄；恢復網路後確認資料完整出現且無遺漏無重複

### Tests for User Story 2 ⚠️

- [X] T034 [P] [US2] 契約測試 `POST /sync/batches`（冪等、裝置移除 403、帳號停用 403）於 `server/tests/sync_batches.rs`
- [X] T035 [P] [US2] 整合測試磁碟滿捨棄最舊、`dedup_key` 唯一鍵於 `crates/tracker-storage/tests/sync_queue.rs`
- [X] T036 [P] [US2] 端對端整合測試（SC-003）：模擬長時間離線累積之大量佇列資料，恢復連線後依固定批次（≤500 筆／≤5MB）依序補傳，核對補傳後遠端總筆數與本機記錄一致（零遺漏零重複）於 `server/tests/offline_resync_e2e.rs`（依 quickstart.md 第 5 節情境）

### Implementation for User Story 2

- [X] T037 [P] [US2] `crates/tracker-storage/src/sync_queue_repo.rs`：佇列 CRUD、磁碟空間檢查與捨棄最舊（`discarded_disk_full`）邏輯（depends on T020）
- [X] T038 [US2] `crates/tracker-sync/src/queue_repo.rs`：封裝佇列讀寫（呼叫 `tracker-storage::sync_queue_repo`）（depends on T037）
- [X] T039 [US2] `crates/tracker-sync/src/uploader.rs`：`reqwest` 呼叫 `POST /sync/batches`，固定批次（≤500 筆／≤5MB）＋節流補傳（depends on T038, T028）
- [X] T040 [US2] `crates/tracker-sync/src/agent.rs`：每 1 分鐘輪詢背景同步迴圈（登入後啟動、登出即停止）（depends on T039）
- [X] T041 [US2] `src-tauri/src/tracking_service.rs` 串接完整 `agent` 啟動/停止邏輯（取代 T033 stub）（depends on T040）
- [X] T042 [P] [US2] `server/src/db/sync_record_repo.rs`：`sync_record` CRUD／依 `batch_dedup_key` 冪等查詢
- [X] T043 [US2] `server/src/api/sync_routes.rs`：`POST /sync/batches`（冪等寫入 S3＋`sync_record`、裝置移除/帳號停用檢查）（depends on T042, T014, T017）
- [X] T044 [US2] `ui/src/views/connection-status.ts` 擴充顯示 `pending_queue_count`／`last_sync_error`（延伸 T032）（depends on T040）

**Checkpoint**：User Story 1、2 皆可獨立運作（登入＋資料同步／離線補傳）

---

## Phase 5: User Story 3 - 透過網頁登入查看個人數據 (Priority: P1)

**Goal**：使用者於瀏覽器登入網頁後可查看自己上傳的活動追蹤與系統監控資料；未登入無法存取任何資料

**Independent Test**：以已註冊帳號登入網頁，驗證可看到先前由軟體上傳的資料；未登入或錯誤帳密應被拒絕

### Tests for User Story 3 ⚠️

- [X] T045 [P] [US3] 契約測試 `GET /data`（未登入 401、僅回傳本人資料）於 `server/tests/data_routes.rs`

### Implementation for User Story 3

- [X] T046 [US3] `server/src/api/data_routes.rs`：`GET /data`（依 `server_received_at` 排序、session 授權中介層檢查）（depends on T042, T017）
- [X] T047 [US3] `server/src/db/share_grant_repo.rs`：`share_grant` CRUD 與存取範圍查詢（depends on T024）
- [X] T048 [US3] `server/src/api/share_routes.rs`：`POST /shares`、`DELETE /shares/{id}`（唯讀分享授權建立/撤銷，FR-006）（depends on T047）
- [X] T049 [US3] 於 `server/src/api/data_routes.rs` 加入分享授權過濾邏輯（被授權帳號僅能唯讀查詢 owner 資料）（depends on T046, T047）
- [X] T050 [US3] `server/src/web/login.rs`：伺服器渲染登入頁（依 quickstart.md `/login`）（depends on T027）
- [X] T051 [US3] `server/src/web/dashboard.rs`：伺服器渲染資料圖表/清單頁（depends on T046, T050）

**Checkpoint**：三項 P1 User Story（US1/US2/US3）皆完成 — 可視為 MVP 交付點

---

## Phase 6: User Story 4 - 多裝置檢視與切換 (Priority: P2)

**Goal**：使用者可於網頁查看裝置清單、切換單一裝置／彙總檢視，並可手動移除裝置；重裝軟體後系統依硬體指紋合併回原裝置

**Independent Test**：同一帳號於兩台裝置登入並產生資料，網頁登入後應能看到兩台裝置並可切換或彙總檢視

> **注意（對應 quickstart.md 已知限制）**：MVP（US1+US2+US3）階段登入流程僅以簡易邏輯建立裝置（T027），尚未套用本階段之硬體指紋合併規則；FR-007「重灌後合併回原裝置」須待本階段（US4）完成後才完全生效。

### Tests for User Story 4 ⚠️

- [X] T052 [P] [US4] 契約測試 `GET /devices`、`DELETE /devices/{id}` 於 `server/tests/device_routes.rs`
- [X] T053 [P] [US4] 單元測試裝置合併規則（同硬體指紋合併回原裝置；已移除裝置視為全新綁定）於 `server/tests/device_merge.rs`

### Implementation for User Story 4

- [X] T054 [P] [US4] `server/src/db/device_repo.rs`：`device` CRUD、依 `(account_id, hardware_fingerprint)` 查詢
- [X] T055 [US4] `server/src/jobs/device_merge.rs`：硬體指紋比對合併規則（依 data-model.md 狀態轉換）（depends on T054）
- [X] T056 [US4] 修改 `server/src/api/auth_routes.rs`：desktop 登入流程改呼叫 `device_merge`（取代 US1 之簡易建立邏輯）（depends on T055, T027）
- [X] T057 [US4] `server/src/api/device_routes.rs`：`GET /devices`、`DELETE /devices/{id}`（depends on T054）
- [X] T058 [US4] `server/src/web/devices.rs`：網頁裝置清單頁與單一裝置/彙總切換檢視（depends on T057, T051）
- [X] T059 [US4] 於 `server/src/api/data_routes.rs` 支援 `device_id=all` 彙總與單一裝置過濾（depends on T046）

**Checkpoint**：User Story 4 可獨立驗證（裝置清單／切換／移除／合併）

---

## Phase 7: User Story 5 - 帳號與工作階段安全管理 (Priority: P2)

**Goal**：使用者可於軟體與網頁分別登出；變更密碼後所有既有工作階段須重新驗證；連續登入失敗鎖定帳號

**Independent Test**：登入後變更密碼，驗證其他既有工作階段被要求重新登入；登出後驗證存取權立即失效

### Tests for User Story 5 ⚠️

- [X] T060 [P] [US5] 契約測試 `POST /auth/change-password`（成功後撤銷其他既有 session）於 `server/tests/change_password.rs`
- [X] T061 [P] [US5] 契約測試連續 10 次登入失敗觸發鎖定、並於 15 分鐘後解除（聚焦失敗計數累積、鎖定觸發與到期解除之完整生命週期；與 T021 之單次登入請求案例區分職責）於 `server/tests/lockout.rs`
- [X] T062 [P] [US5] 契約測試 `POST /auth/logout`（登出後該 session 之 `revoked_at` 立即生效，後續以該 session 存取任何端點 MUST 回應 401，FR-008）於 `server/tests/logout.rs`
- [X] T063 [P] [US5] 契約測試網頁 Session 滑動視窗（FR-014）：操作後 `expires_at` 依 `last_active_at + 24h` 重新計算；自最後一次操作起滿 24 小時無操作後，該 session MUST 失效並拒絕存取，於 `server/tests/session_sliding_window.rs`（depends on T016）

### Implementation for User Story 5

- [X] T064 [US5] `server/src/auth/session.rs` 新增「撤銷帳號所有其他 session」函式（depends on T016）
- [X] T065 [US5] `server/src/api/auth_routes.rs` 新增 `POST /auth/change-password`（`new_password` 長度 MUST ≥8 字元驗證；成功後呼叫 T064 撤銷其他 session，FR-009）（depends on T064, T027）
- [X] T066 [US5] `crates/tracker-sync/src/auth_client.rs` 處理 session 失效（伺服器回應 401 時清除本機 `auth_cache` 並提示重新登入）（depends on T028）

**Checkpoint**：五項 User Story 皆可獨立運作與驗證

---

## Phase 8: Polish & Cross-Cutting Concerns

**Purpose**：涵蓋多個 User Story 的收尾項目（保留天數清理、帳號刪除、程式碼品質、文件驗證）

- [X] T067 [P] `server/src/api/account_routes.rs`：`PATCH /account/settings`（`retention_days`）、`DELETE /account`（軟刪除，FR-012/FR-013）
- [X] T068 `server/src/jobs/retention_cleanup.rs`：依帳號保留天數清除逾期 `sync_record`＋S3 物件；已刪除帳號 30 天後永久清除（`tokio::time::interval`，依 research.md R10）（depends on T042, T014, T067）
- [X] T069 [P] 執行 `cargo clippy --workspace` 與 `cargo fmt --check`（根 workspace 與 `server/` 各一次）並修正告警
- [X] T070 依 `quickstart.md` 完整走過本機開發驗證流程（`docker compose up`、登入、同步、離線補傳、鎖定、改密）
- [X] T071 [P] 校對 `server/src/web/`、`ui/src/views/` 新增檔案之介面文字為正體中文（憲章原則 I）

---

## Dependencies & Execution Order

### Phase Dependencies

- **Setup（Phase 1）**：無相依，可立即開始
- **Foundational（Phase 2）**：依賴 Setup 完成 — 阻塞所有 User Story
- **User Stories（Phase 3-7）**：皆依賴 Foundational 完成
  - US1（P1）→ US2（P1，同步邏輯建立於 US1 登入之上）→ US3（P1，網頁需 US1/US2 已產生資料以供查詢）
  - US4（P2）依賴 US1 之登入流程（修改 `auth_routes.rs`）與 US3 之 `data_routes.rs`/`dashboard.rs`
  - US5（P2）依賴 US1 之 `auth_routes.rs`/`session.rs`
- **Polish（Phase 8）**：依賴所有欲交付之 User Story 完成

### User Story Dependencies

- **US1（P1）**：Foundational 完成後即可開始，其他故事之基礎
- **US2（P1）**：依賴 US1 之登入與 `auth_client.rs`（T028）
- **US3（P1）**：依賴 US1（登入頁）與 US2（需有資料可查詢，`sync_record_repo`）
- **US4（P2）**：依賴 US1（`auth_routes.rs`）與 US3（`data_routes.rs`、`dashboard.rs`）
- **US5（P2）**：依賴 US1（`auth_routes.rs`、`session.rs`）

### Within Each User Story

- 測試先寫、先失敗，再實作
- Repo/Model → Service/中介層 → Route/Command → UI
- 故事內部完成後才進入下一優先序故事

### Parallel Opportunities

- Setup 內標 [P] 之任務可並行
- Foundational 內標 [P] 之任務可並行（尤其 T007-T011 五個 migration 檔案）
- 同一 User Story 內標 [P] 之任務（不同檔案、無相依）可並行
- 不同開發者可並行負責不同 User Story（待 Foundational 完成後）

---

## Parallel Example: User Story 1

```bash
# 平行執行 US1 測試：
Task: "契約測試 POST /auth/login 於 server/tests/auth_login.rs"
Task: "單元測試裝置指紋讀取於 crates/tracker-platform/tests/device_id.rs"
Task: "單元測試批次切分/去重鍵於 crates/tracker-core/tests/sync.rs"

# 平行執行 US1 部分實作（不同檔案）：
Task: "server/src/db/account_repo.rs Account CRUD"
Task: "ui/src/views/login.ts 登入畫面"
```

---

## Implementation Strategy

### MVP 優先（US1 + US2 + US3）

1. 完成 Phase 1：Setup
2. 完成 Phase 2：Foundational（阻塞性，務必完整）
3. 完成 Phase 3-5：US1 → US2 → US3
4. **停下並驗證**：依 quickstart.md 獨立測試三項 P1 故事
5. 視需要部署/展示（MVP）

### 漸進交付

1. Setup + Foundational → 基礎就緒
2. US1 → 獨立驗證登入 → 展示
3. US2 → 獨立驗證同步/離線補傳 → 展示
4. US3 → 獨立驗證網頁查看 → 展示（MVP 完成）
5. US4 → 獨立驗證多裝置 → 展示
6. US5 → 獨立驗證帳號安全 → 展示
7. Polish → 保留天數清理、程式碼品質、文件驗證

---

## Notes

- [P] 任務代表不同檔案且無相依，可平行進行
- [Story] 標籤將任務對應至特定 User Story 以利追蹤
- 每個 User Story 應可獨立完成與測試
- 實作前先確認測試會失敗
- 每完成一個任務或邏輯群組即可提交
- 於任一 Checkpoint 可停下獨立驗證該故事
- 避免：模糊任務、同檔案衝突、破壞故事獨立性的跨故事相依
