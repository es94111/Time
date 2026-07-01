# 任務：Windows 11 活動追蹤器（Activity Tracker）

**Input**：設計文件於 `specs/001-activity-tracker/`
**Prerequisites**：plan.md、spec.md（必要）；research.md、data-model.md、contracts/（已具備）

**Tests**：本功能納入**針對核心確定性邏輯（時間切分、彙總、主機名解析）的單元測試**——依 plan.md「Testing」與 quickstart.md，以及憲章原則 III（追蹤準確且誠實）所要求。其餘 UI／整合不強制測試。

**Organization**：任務以使用者故事分組，使每個故事可獨立實作與測試。

> 本任務文件以正體中文（zh-TW）撰寫（憲章原則 I）。

## 格式：`[ID] [P?] [Story] 描述（含檔案路徑）`

- **[P]**：可平行（不同檔案、無未完成相依）
- **[Story]**：US1／US2／US3 對應 spec.md 使用者故事
- 技術背景見 [plan.md](./plan.md)；資料結構見 [data-model.md](./data-model.md)；介面契約見 [contracts/](./contracts/)

---

## Phase 1：Setup（共用基礎建設）

**目的**：建立 Cargo workspace、Tauri 與前端骨架。

- [X] T001 建立 Cargo workspace 結構：根 `Cargo.toml` 與 `crates/tracker-core`、`crates/tracker-platform`、`crates/tracker-storage`、`src-tauri`、`ui` 目錄骨架（依 plan.md 專案結構）
- [X] T002 [P] 以 `cargo add` 為各 crate 加入最新穩定相依（`windows`、`rusqlite`+`bundled-sqlcipher-vendored-openssl`、`jiff`、`url`、`serde`、`serde_json`、`csv`、`argon2`、`aes-gcm`、`zeroize`、`tauri`、`tauri-plugin-autostart`、`tauri-plugin-single-instance`），提交 `Cargo.lock`
- [X] T003 [P] 建立 Tauri 2 應用骨架：`src-tauri/tauri.conf.json`、`src-tauri/src/main.rs`（系統匣圖示佔位）、`src-tauri/icons/`
- [X] T004 [P] 建立前端骨架：`ui/package.json`、`ui/index.html`、建置設定與輕量圖表套件（`npm install @latest`）、`ui/src/i18n/zh-TW.ts` 基底
- [X] T005 [P] 設定 `rustfmt`、`clippy` 與 `.gitignore`（排除 `target/`、`node_modules/`、`*.db`、`*.db-wal`、`*.db-shm`、任何金鑰/.env）

---

## Phase 2：Foundational（阻斷性前置；完成前不得開始任何故事）

**⚠️ 關鍵**：以下為所有故事共用之核心基礎，必須先完成。

- [X] T006 [P] 領域型別（Application、Website、ActivitySession、active/idle 列舉）於 `crates/tracker-core/src/model.rs`
- [X] T007 [P] 時間工具（`jiff`：UTC↔本機、`local_date` 計算、本機午夜邊界）於 `crates/tracker-core/src/time.rs`
- [X] T008 [P] 平台抽象 trait（Foreground/Idle/SessionPower/BrowserUrl/SecretStore）於 `crates/tracker-core/src/ports.rs`（使平台層可在非 Windows 以假實作測試）
- [X] T009 [P] DPAPI 金鑰保護/解保護（`CryptProtectData`，使用者範圍）於 `crates/tracker-platform/src/secret.rs`
- [X] T010 加密金鑰管理：DEK 產生、DPAPI 包裹、Argon2id（可選主密碼）、`zeroize` 於 `crates/tracker-storage/src/crypto.rs`（依 T009）
- [X] T011 加密 DB 啟動與遷移：以 SQLCipher 金鑰開啟、`PRAGMA key/journal_mode=WAL/foreign_keys`、建立全部資料表與索引（依 data-model.md）於 `crates/tracker-storage/src/db.rs`（依 T010）
- [X] T012 Repository 基礎：upsert application/website、insert session、共用查詢輔助於 `crates/tracker-storage/src/repository.rs`（依 T011、T006）
- [X] T013 Tauri 命令骨架與 zh-TW 結構化錯誤模型 `{code, message_zh}`、於 `main.rs` 註冊 handler，於 `src-tauri/src/commands.rs`（依 T006）
- [X] T014 背景追蹤服務骨架（背景執行緒、暫停旗標、平台來源掛接點）+ 系統匣 + 單一實例，於 `src-tauri/src/tracking_service.rs` 與 `src-tauri/src/main.rs`（依 T013）
- [X] T015 [P] 前端外殼（版面/路由、zh-TW i18n、狀態列元件）於 `ui/src/`

**Checkpoint**：基礎就緒，可開始 US1。

---

## Phase 3：使用者故事 US1 — 查看今天每個應用程式各花了多少時間（P1）🎯 MVP

**Goal**：登入後自動於背景量測前景應用程式的活躍時間，使用者可在繁中介面看到今日各 App 時長（排序）與當日總計，並可暫停/恢復。

**Independent Test**：在多個 App 間切換一段時間並閒置一次，開啟「今日」檢視，確認各 App 以合理活躍時間排序、閒置未計入、顯示當日總計。

### US1 測試（核心確定性邏輯）

- [X] T016 [P] [US1] 單元測試：工作階段狀態機（active/idle、5 秒最短門檻、**已知操作時間下時長累計誤差 ≤ ±2%（SC-001）**）於 `crates/tracker-core/tests/session.rs`
- [X] T017 [P] [US1] 單元測試：午夜切分與今日彙總排序於 `crates/tracker-core/tests/aggregation.rs`

### US1 實作

- [X] T018 [P] [US1] 規則：5 秒最短門檻、閒置門檻（預設 300 秒，可調）於 `crates/tracker-core/src/rules.rs`
- [X] T019 [US1] 工作階段狀態機（開/合、active/idle 回溯扣除、跨午夜切分）於 `crates/tracker-core/src/session.rs`（依 T018、T007）
- [X] T020 [P] [US1] 每日彙總（各 App 時間排序、當日總活躍）於 `crates/tracker-core/src/aggregation.rs`（依 T006）
- [X] T021 [P] [US1] 前景偵測（`SetWinEventHook`/`EVENT_SYSTEM_FOREGROUND`，實作 Foreground port）於 `crates/tracker-platform/src/foreground.rs`
- [X] T022 [P] [US1] 程序名稱解析（`QueryFullProcessImageNameW` → display_name/executable）；**無法辨識前景程序時回退為「未知／其他」（`<unknown>`）（FR-015）** 於 `crates/tracker-platform/src/process.rs`
- [X] T023 [P] [US1] 閒置偵測（`GetLastInputInfo`）於 `crates/tracker-platform/src/idle.rs`
- [X] T024 [P] [US1] 工作階段/電源事件（WTS 鎖定/解鎖、`WM_POWERBROADCAST` 睡眠/喚醒：結算並關閉目前段）於 `crates/tracker-platform/src/session_power.rs`
- [X] T025 [P] [US1] 登入自啟（HKCU `Run` 註冊/移除）於 `crates/tracker-platform/src/autostart.rs`
- [X] T026 [US1] Repository：寫入工作階段（含「未知／其他」`<unknown>` fallback 列，FR-015）、`get_today` 之 App 摘要查詢於 `crates/tracker-storage/src/repository.rs`（依 T012、T020）
- [X] T027 [US1] 串接追蹤服務：foreground+idle+power → core 狀態機 → storage；暫停旗標；每 ≤60 秒 flush 進度於 `src-tauri/src/tracking_service.rs`（依 T019、T021、T022、T023、T024、T026）
- [X] T028 [US1] 命令：`get_today_summary`(apps+total)、`get_tracking_status`、`pause_tracking`、`resume_tracking`、自啟開關（對應 contracts/tauri-commands.md）於 `src-tauri/src/commands.rs`（依 T027、T026、T025）
- [X] T029 [US1] 今日檢視 UI（App 清單依時長排序、當日總計、追蹤狀態、暫停/恢復按鈕）zh-TW，於 `ui/src/views/Today.ts`（依 T028）

**Checkpoint**：US1 可獨立運作並交付為 MVP。

---

## Phase 4：使用者故事 US2 — 查看每個網站各瀏覽了多少時間（P2）

**Goal**：當瀏覽器為前景時，零設定（免擴充）以 UIA 取得網址列 URL，解析為完整主機名（子網域分開），於介面顯示各網站瀏覽時長。

**Independent Test**：於支援瀏覽器先後瀏覽 `mail.google.com` 與 `docs.google.com`，確認兩者分開計時且與瀏覽器 App 總時間分列。

### US2 測試

- [X] T030 [P] [US2] 單元測試：主機名解析（子網域分開、正規化小寫、去 port）於 `crates/tracker-core/tests/hostname.rs`

### US2 實作

- [X] T031 [P] [US2] 由 URL 取完整主機名（`url` crate）於 `crates/tracker-core/src/hostname.rs`
- [X] T032 [P] [US2] 已知瀏覽器清單與 `is_browser`、無痕略過規則於 `crates/tracker-core/src/browsers.rs`
- [X] T033 [P] [US2] 以 UI Automation 讀取瀏覽器網址列 URL（實作 BrowserUrl port）於 `crates/tracker-platform/src/browser_url.rs`
- [X] T034 [US2] 擴充工作階段狀態機：前景為瀏覽器且成功辨識主機名時附加 `website_id`（無痕不記逐站）於 `crates/tracker-core/src/session.rs`（依 T019、T031、T032）
- [X] T035 [US2] Repository：website upsert 與今日網站摘要查詢於 `crates/tracker-storage/src/repository.rs`（依 T026）
- [X] T036 [US2] 將 BrowserUrl 串入追蹤服務於 `src-tauri/src/tracking_service.rs`（依 T027、T033、T034）
- [X] T037 [US2] 命令：擴充 `get_today_summary` 之 websites 與網站摘要於 `src-tauri/src/commands.rs`（依 T028、T035）
- [X] T038 [US2] 網站檢視 UI（主機名清單依時長排序，與 App 區分）zh-TW，於 `ui/src/views/Websites.ts`（依 T037）

**Checkpoint**：US1 與 US2 皆可獨立運作。

---

## Phase 5：使用者故事 US3 — 回顧歷史與長期趨勢（P3）

**Goal**：使用者可選取過去某日或日期範圍，檢視該期間 App 與網站的彙總（跨日午夜切分正確）。

**Independent Test**：累積數日資料後，選取過去某日與過去一週範圍，確認摘要正確且無缺漏日。

### US3 測試

- [X] T039 [P] [US3] 單元測試：跨多日之區間彙總（含跨午夜段歸屬）於 `crates/tracker-core/tests/range.rs`

### US3 實作

- [X] T040 [US3] 區間/單日彙總輔助於 `crates/tracker-core/src/aggregation.rs`（依 T020）
- [X] T041 [US3] Repository 查詢：`get_summary_by_date`、`get_summary_range` 於 `crates/tracker-storage/src/repository.rs`（依 T035）
- [X] T042 [US3] 命令：`get_summary_by_date`、`get_summary_range` 於 `src-tauri/src/commands.rs`（依 T037、T041）
- [X] T043 [P] [US3] 歷史檢視 UI（日期選擇、單日摘要）zh-TW，於 `ui/src/views/History.ts`（依 T042）
- [X] T044 [P] [US3] 區間檢視 UI（範圍選擇、彙總）zh-TW，於 `ui/src/views/Range.ts`（依 T042）

**Checkpoint**：三個故事皆可獨立運作。

---

## Phase 6：Polish 與跨領域關注

**目的**：完成跨故事的功能需求（排除、匯出、清除、設定、安全與效能）。

- [X] T045 排除規則強制：追蹤時略過被排除的 App/網站於 `crates/tracker-core/src/rules.rs` 與 `crates/tracker-core/src/session.rs`（FR-013）
- [X] T046 排除規則儲存與命令：`list_exclusions`/`add_exclusion`/`remove_exclusion` 於 `crates/tracker-storage/src/repository.rs` 與 `src-tauri/src/commands.rs`（依 T045）
- [X] T047 [P] CSV/JSON 匯出實作（依 contracts/export-formats.md，UTF‑8 BOM）於 `crates/tracker-storage/src/export.rs`（FR-017）
- [X] T048 匯出命令與 UI：`export_data` 於 `src-tauri/src/commands.rs` 與 `ui/src/views/Settings.ts`（依 T047）
- [X] T049 清除資料命令與 UI（二次確認）：`clear_data` 於 `src-tauri/src/commands.rs` 與 `ui`（FR-014）
- [X] T050 設定命令與 UI：閒置門檻、自啟開關、語言，以及**排除清單管理（檢視/新增/移除應用與網站，呼叫 T046 之 `list/add/remove_exclusion`，FR-013）**：`get_settings`/`update_settings` 於 `src-tauri/src/commands.rs` 與 `ui/src/views/Settings.ts`（FR-002/003/013/018；依 T046）
- [X] T051 [P] 可選 Argon2id 主密碼：`set_master_password`/`unlock` 命令與 UI 解鎖流程（依 T010）
- [ ] T052 [P] 效能驗證：確認事件驅動、WAL checkpoint ≤60 秒 flush、平均 CPU <2%（SC-003、SC-005）於 `src-tauri/src/tracking_service.rs` 與 `crates/tracker-storage/src/db.rs`（**設計已就緒：2 秒 tick + 事件執行緒處理鎖定/睡眠、每 ≤60 秒 checkpoint；CPU 平均值待實機執行量測**）
- [X] T053 [P] 安全強化：`zeroize` 稽核、`.gitignore` 機密、確認全專案無任何網路傳輸程式碼（原則 II）
- [X] T054 [P] zh-TW 一致性審查：UI 字串與程式註解（原則 I 語言關卡）於 `ui/` 與 `crates/`
- [ ] T055 執行 quickstart.md 驗收（建置、執行、SC-001…SC-007 對照；**SC-001 以「已知操作時間 vs. 記錄時長」量測誤差 ≤ ±2%**）（**建置✅、`cargo test --workspace` 全綠✅、SC-001 由 `session.rs` 單元測試涵蓋✅；執行 GUI 之端到端手動驗收待使用者於本機啟動——啟動會開始實際前景追蹤並註冊登入自啟，故不自動執行**）

---

## 相依與執行順序

### 階段相依

- **Setup（Phase 1）**：無相依，可立即開始。
- **Foundational（Phase 2）**：依賴 Setup；**阻斷所有故事**。
- **User Stories（Phase 3–5）**：皆依賴 Foundational 完成；其後可平行（若人力允許）或依 P1→P2→P3 順序。
- **Polish（Phase 6）**：依賴目標故事完成。

### 故事相依

- **US1（P1）**：Foundational 後即可開始，無對其他故事相依（MVP）。
- **US2（P2）**：Foundational 後可開始；擴充 US1 的工作階段狀態機（T034 依 T019），但網站檢視可獨立測試。
- **US3（P3）**：Foundational 後可開始；查詢層依 US2 的 website 寫入（T041 依 T035），歷史/區間檢視可獨立測試。

### 故事內部順序

- 測試（若有）先於實作；model → rules → 狀態機 → 平台來源 → repository → 服務串接 → 命令 → UI。

### 平行機會

- Setup 中 T002–T005 可平行。
- Foundational 中 T006、T007、T008、T009、T015 可平行；T010→T011→T012 為序列。
- US1 中 T021–T025（不同平台檔案）可平行；T016/T017 測試可平行。
- 不同故事在 Foundational 後可由不同開發者平行進行。

---

## 平行範例：US1 平台層

```text
# 同時啟動 US1 的平台來源（不同檔案、互不相依）：
Task: 前景偵測 crates/tracker-platform/src/foreground.rs
Task: 程序名稱 crates/tracker-platform/src/process.rs
Task: 閒置偵測 crates/tracker-platform/src/idle.rs
Task: 工作階段/電源 crates/tracker-platform/src/session_power.rs
Task: 登入自啟 crates/tracker-platform/src/autostart.rs
```

---

## 實作策略

### MVP 優先（僅 US1）

1. 完成 Phase 1 Setup。
2. 完成 Phase 2 Foundational（關鍵，阻斷所有故事）。
3. 完成 Phase 3 US1。
4. **停下並驗證**：獨立測試 US1（今日 App 時長）。
5. 可作為 MVP 交付/示範。

### 漸進交付

1. Setup + Foundational → 基礎就緒。
2. US1 → 獨立測試 → MVP。
3. US2（網站時長）→ 獨立測試 → 交付。
4. US3（歷史/區間）→ 獨立測試 → 交付。
5. Phase 6 Polish（排除、匯出、清除、設定、安全、效能）。

---

## 備註

- [P] = 不同檔案、無未完成相依；同一檔案的任務不標 [P]。
- 每個故事皆應可獨立完成與測試；於各 Checkpoint 停下驗證。
- 核心時間/彙總邏輯需先讓測試失敗再實作（呼應原則 III 誠實量測）。
- 每完成一個任務或邏輯群組即提交；提交訊息得用正體中文，且不得加入未授權的 Co-Authored-By（憲章）。
