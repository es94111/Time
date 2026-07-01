# 實作計畫：遠端資料同步與網頁儀表板（含登入驗證）

**Branch**: `003-remote-sync-web-login` | **Date**: 2026-07-01 | **Spec**: [spec.md](./spec.md)
**Input**: 功能規格 `specs/003-remote-sync-web-login/spec.md`

> 本計畫文件以正體中文（zh-TW）撰寫（憲章原則 I）。

## Summary

在既有 Windows 11 活動追蹤器（001）與電腦資訊監控（002）之上，新增「登入後才啟動的遠端同步」與「網頁儀表板」：使用者以同一組帳密登入 Windows 軟體與網頁；已登入裝置的本機資料以背景代理每 1 分鐘輪詢方式安全上傳至遠端保存服務；離線時本機佇列暫存、恢復連線後依時間序、固定批次補傳；網頁提供登入、裝置清單／切換、彙總檢視、唯讀分享、帳號與工作階段安全管理。

技術取向（依使用者指示）：**用戶端與遠端伺服器全部以 Rust 實作**；Windows 11 用戶端沿用 001/002 之 **Microsoft 官方 `windows` crate**（原廠 Win32 API，含以登錄檔 `MachineGuid` 做裝置指紋、DPAPI 保護本機憑證快取）；遠端 Web 伺服器**以 Docker 容器化部署**（多階段建置的官方 Rust 基底映像），伺服器端採 **Axum**（Web 框架）＋ **PostgreSQL**（帳號／裝置／工作階段／分享等結構化資料，經 `sqlx`）＋ **S3 相容物件儲存**（透過官方 `aws-sdk-s3`，可指向自架 MinIO 容器或雲端 S3，存放實際的活動／監控資料快照）；密碼以 **Argon2id** 雜湊；工作階段採**伺服器端可撤銷 session**（`tower-sessions` + PostgreSQL store），滿足「登出即失效」與「網頁滑動視窗 24 小時」之硬性需求（JWT 等純無狀態權杖無法即時撤銷，故不採用）。所有套件於實作時以 `cargo add` 取最新穩定版（見 research.md 各項版本策略）。

## Technical Context

**Language/Version**: Rust（最新穩定版；用戶端沿用 001/002 workspace edition 2021、rust-version 1.80+；新增 `server/` 為獨立 Cargo workspace，同版本策略，Linux 容器內建置）

**Primary Dependencies**:

*Windows 用戶端新增（於既有 workspace 內新增 `tracker-sync` crate，延伸 `src-tauri`／`ui`）*：
- `windows`（Microsoft 官方）— 讀取登錄檔 `HKLM\SOFTWARE\Microsoft\Cryptography\MachineGuid` 作硬體裝置指紋；DPAPI 保護本機快取的登入憑證／refresh token（沿用 001 `tracker-storage::crypto`）
- `reqwest`（rustls-tls，最新版）— 與遠端伺服器之 HTTPS 通訊（登入、批次上傳）；**非 Windows 原廠元件，於 Complexity Tracking 說明必要性**
- `rusqlite`（SQLCipher，沿用 001）— 待同步佇列表、裝置識別、登入憑證快取，續存於既有本機加密資料庫
- `serde`、`serde_json`、`jiff`（沿用）— 序列化、時間

*遠端 Web 伺服器（新增 `server/` Cargo workspace，容器化部署）*：
- `axum`＋`tokio`（最新）— HTTP／REST API 與伺服器渲染網頁儀表板
- `sqlx`（postgres、runtime-tokio-rustls，最新）— PostgreSQL 存取與 migrations（`sqlx migrate`）
- `argon2`（RustCrypto，最新）— 密碼雜湊（Argon2id）
- `tower-sessions` + `tower-sessions-sqlx-store`（最新）— 伺服器端可撤銷、可滑動延展的 Session（PostgreSQL 儲存）
- `aws-sdk-s3`（官方 AWS Rust SDK，最新）— 存取 S3 相容物件儲存（MinIO 或雲端 S3），存放資料快照 blob
- `tracing`、`tracing-subscriber` — 結構化日誌
- `serde`、`serde_json`、`jiff` — 序列化、時間（伺服器一律以「接收時間」為準排序，見 FR-...timestamp 需求）

**Storage**:
- 用戶端：既有本機 SQLCipher（AES-256）資料庫（沿用 001/002），新增 `sync_queue`、`device_identity`、`auth_cache` 資料表
- 伺服器：PostgreSQL（帳號、裝置、工作階段、同步紀錄中繼資料、分享授權）＋ S3 相容物件儲存（活動追蹤／系統監控資料快照本體，依帳號＋裝置分區存放）；兩者皆以容器化部署（Docker Compose：`server` + `postgres` + `minio`）

**Testing**: 用戶端 `cargo test`（同步佇列、批次切分、裝置指紋純邏輯，平台相依以 trait 抽象＋假實作）；伺服器 `cargo test` + 整合測試（以 Docker 啟動臨時 PostgreSQL／MinIO 或 `testcontainers-rs`）驗證登入、鎖定、Session 撤銷、批次上傳去重

**Target Platform**: 用戶端 Windows 11（含 Home 版，沿用 001）；伺服器 Linux 容器（Docker，可部署於任意支援 Docker 的主機）

**Project Type**: 混合式——既有 Windows 桌面應用程式（Rust + Tauri）＋新增遠端 Web 服務（Rust + Docker，含 REST API 與伺服器渲染網頁儀表板）

**Performance Goals**:
- 已登入裝置新資料於 5 分鐘內可於網頁查詢到（SC-002，背景同步每 1 分鐘輪詢，FR-003）
- 網頁儀表板一般查詢 API p95 < 300ms（一般裝置數與資料量級下）
- 離線補傳以固定批次（≤500 筆或 ≤5MB／批，批次間節流）避免尖峰負載（FR-004）

**Constraints**:
- 登入前不上傳任何本機資料，本機記錄與監控不受影響（FR-002，呼應原則 II 之「預設關閉、需明確同意」——登入即為明確同意動作）
- 傳輸中與靜態資料加密（TLS + 資料庫／物件儲存加密），密碼不得明文儲存（FR-011）
- 密碼最小長度 8 字元，不強制字元種類（FR-011）
- 連續 10 次登入失敗鎖定 15 分鐘（FR-010）
- Session：軟體端固定 30 天、網頁端滑動視窗 24 小時（FR-014）
- 待同步佇列磁碟滿時捨棄最舊資料並提示（FR-004）
- 帳號刪除後資料保留 30 天可復原，之後永久刪除（FR-012）；一般情況依使用者自訂保留天數自動清除（FR-013）

**Scale/Scope**: 多使用者（每帳號預期 1～5 台裝置），單一遠端服務可服務多帳號；資料量級以「帳號 × 裝置 × 保留天數」為主，須具備依帳號／裝置／時間範圍的索引與分區清理效率

## Constitution Check

*GATE: Must pass before Phase 0 research. Re-check after Phase 1 design.*

依據憲章（`.specify/memory/constitution.md` v1.1.0）的關卡：

- **原則 I — 正體中文優先**：✅ plan/research/data-model/quickstart/contracts 全部 zh-TW；網頁與用戶端介面文字皆 zh-TW。
- **原則 II — 隱私優先與本機資料**：✅ 本功能將「預設關閉的外傳」以**登入**作為使用者明確同意的動作（FR-002）；未登入前資料仍僅存本機、行為不變。上傳內容（活動追蹤／系統監控資料）於登入流程與網頁均需清楚揭露；帳號可刪除且資料有明確保留與清除時程（FR-012、FR-013），符合「可關閉、預設關閉、需揭露」之精神——此處「關閉」以「未登入＝不同步」實現。
- **原則 III — 追蹤準確且誠實**：✅ 同步不改變本機量測邏輯（沿用 001/002 之實測值）；多裝置時間戳記以「伺服器接收時間」為準排序，裝置本機時間僅作輔助欄位，並於 data-model 明確標示，不以推估掩蓋時鐘誤差。
- **原則 IV — 簡潔與低資源佔用**：⚠️ 需說明——本原則之「低 CPU／低記憶體／低耗電」文字明確鎖定於「長時間於背景執行的應用程式」，即 Windows 用戶端背景程序。用戶端新增相依僅 `reqwest`（HTTP 客戶端，事件驅動、非輪詢常駐）與既有 SQLCipher 佇列表，成本輕微，已於 Complexity Tracking 說明。**遠端 Web 伺服器是新的、獨立部署的服務（非常駐於使用者電腦），其相依套件（axum/sqlx/tower-sessions/aws-sdk-s3/argon2）不在「背景常駐應用程式」的低資源佔用約束範圍內**，但仍遵循 YAGNI──每個套件皆對應規格中一項硬性需求（見下表），未引入非必要抽象層。
- **原則 V — 規格驅動開發**：✅ 本計畫逐項對應已核可規格（FR-001…FR-014、SC-001…SC-006），未超出範圍。
- **原則 VI — Rust 實作語言**：✅ Windows 11 用戶端與追蹤核心維持 Rust（原則硬性要求）；**遠端 Web 伺服器雖非原則 VI 之強制範圍（該原則文字僅涵蓋 Windows 11 桌面應用程式），但依使用者明確指示亦全部以 Rust 實作**，未使用其他語言之執行期元件。唯一非 Rust 執行期元件為用戶端既有之 SQLCipher C 函式庫（001 已說明理由）與 Docker 容器基底映像（作業系統層，非應用邏輯語言）。

任何違反項目**必須**記錄於下方「Complexity Tracking」並附正當理由，否則不得進入實作。

**結論**：通過（原則 IV 之伺服器相依經上述範圍界定與逐項對應後，非屬未經說明之違反）。

## Project Structure

### Documentation (this feature)

```text
specs/003-remote-sync-web-login/
├── plan.md              # 本檔（/speckit-plan 輸出）
├── research.md          # Phase 0 輸出（R1–R10 技術決策）
├── data-model.md        # Phase 1 輸出（帳號/裝置/工作階段/同步紀錄/快照/分享 實體）
├── quickstart.md        # Phase 1 輸出（本機開發、Docker 啟動、驗證步驟）
├── contracts/           # Phase 1 輸出
│   ├── rest-api.md         # 網頁/用戶端 ↔ 伺服器 REST API 契約
│   └── tauri-commands.md   # 用戶端登入/同步狀態 IPC 命令契約（新增）
└── tasks.md             # Phase 2 輸出（由 /speckit-tasks 產生，非本指令）
```

### Source Code (repository root)

```text
# 既有 Windows 用戶端 workspace（沿用 001/002，新增以下項目）
crates/
├── tracker-core/
│   └── src/sync.rs               # 【新】待同步批次切分、去重鍵、佇列狀態純邏輯（可單元測試）
├── tracker-platform/
│   └── src/device_id.rs          # 【新】讀取 MachineGuid 產生裝置指紋（windows crate 登錄檔 API）
└── tracker-sync/                 # 【新】用戶端同步代理 crate
    └── src/
        ├── auth_client.rs        # 登入/登出、憑證快取（DPAPI 保護，存 tracker-storage）
        ├── queue_repo.rs         # 佇列讀寫（沿用 SQLCipher DB，新增 sync_queue 表）
        ├── uploader.rs           # reqwest 呼叫伺服器 REST API、固定批次＋節流補傳
        └── agent.rs              # 每 1 分鐘輪詢的背景同步迴圈（登入後啟動、登出即停止）
src-tauri/src/
├── auth_commands.rs               # 【新】login/logout/get_connection_status IPC 命令
└── tracking_service.rs            # 【擴充】登入後啟動 tracker-sync::agent

ui/src/
├── views/login.ts                 # 【新】Windows 軟體登入畫面
└── views/connection-status.ts     # 【新】已連線／待同步狀態顯示

# 新增：遠端 Web 伺服器（獨立 Cargo workspace，容器化部署）
server/
├── Cargo.toml
├── Dockerfile                     # 多階段建置（builder: rust:slim 官方映像 → runtime: debian-slim）
├── docker-compose.yml             # server + postgres + minio（本機/自架部署範例）
├── migrations/                    # sqlx migrations（accounts/devices/sessions/... ）
├── src/
│   ├── main.rs
│   ├── config.rs                  # 環境變數設定（DB/S3/JWT-less session 金鑰/保留天數預設值）
│   ├── auth/
│   │   ├── password.rs            # Argon2id 雜湊/驗證
│   │   ├── lockout.rs             # 連續失敗計數、15 分鐘鎖定
│   │   └── session.rs             # tower-sessions 設定（滑動視窗/固定 30 天雙策略）
│   ├── api/
│   │   ├── auth_routes.rs         # 登入/登出/變更密碼
│   │   ├── device_routes.rs       # 裝置清單/移除/合併判定
│   │   ├── sync_routes.rs         # 批次上傳、去重（同步紀錄）
│   │   ├── data_routes.rs         # 資料查詢（單裝置/彙總、依保留設定過濾）
│   │   └── share_routes.rs        # 唯讀分享授權建立/撤銷
│   ├── web/                       # 伺服器渲染網頁儀表板（登入頁、裝置清單、圖表頁）
│   ├── db/                        # sqlx 模型與查詢
│   ├── storage/s3.rs              # aws-sdk-s3 封裝，讀寫快照 blob
│   └── jobs/
│       ├── retention_cleanup.rs   # 依帳號保留天數/刪除後 30 天 排程清除
│       └── device_merge.rs        # 硬體指紋比對，合併回既有裝置紀錄
└── tests/
```

**Structure Decision**：Windows 用戶端**沿用 001/002 之 Cargo workspace 分層**，僅新增 `tracker-sync` crate 與既有 crate 內的少量模組（裝置指紋、同步佇列純邏輯），維持「純邏輯可測試、平台相依隔離」慣例。遠端 Web 伺服器因**目標平台（Linux 容器）與建置工具鏈與 Windows 用戶端不同**，獨立為 `server/` 下的第二個 Cargo workspace，避免交叉編譯與相依衝突；以 Docker 容器化部署（`Dockerfile` + `docker-compose.yml`）符合使用者指示之「web 伺服器要使用 Docker 套件」。

## Complexity Tracking

> **Fill ONLY if Constitution Check has violations that must be justified**

| Violation | Why Needed | Simpler Alternative Rejected Because |
|-----------|------------|-------------------------------------|
| 用戶端新增 `reqwest`（非 Windows 原廠 HTTP 元件） | 需與遠端伺服器進行 HTTPS 登入與批次上傳；`windows` crate 未提供符合人體工學的高階 HTTP 客戶端 API | 否決直接以 `windows` crate 之 WinHTTP/WinINet 原生綁定手刻：開發與維護成本高、錯誤處理與 TLS/憑證管理需自行實作，風險高於採用成熟且純 Rust（rustls，非 OpenSSL）之 `reqwest` |
| 伺服器端 `tower-sessions`（伺服器端 Session 儲存，非無狀態 JWT） | FR-008／FR-009 要求登出與變更密碼後**立即**使工作階段失效，且 FR-014 要求網頁端**滑動視窗**；純無狀態 JWT 無法在不維護黑名單的情況下即時撤銷 | 否決純 JWT：需額外實作撤銷清單（等同於重新發明伺服器端 session），複雜度更高且更易出錯 |
| 伺服器端導入 PostgreSQL + S3 相容物件儲存兩套儲存 | 帳號/裝置/工作階段/分享等**結構化、強一致性**中繼資料適合關聯式資料庫；活動與監控資料為**大量、依時間範圍讀取的 blob**，適合物件儲存並可對接雲端 S3；規格本身允許「如 S3 或 SERVER」 | 否決單一 PostgreSQL 存放所有資料：長期累積的高頻率監控樣本存於關聯式資料庫會使資料表暴增、查詢與清理效能劣化，且無法直接對應規格提及的 S3 相容保存方式 |

## 階段產出對照

| 規格條目 | 對應設計位置 |
|----------|--------------|
| FR-001 共用帳密登入（軟體＋網頁） | server/api/auth_routes.rs、src-tauri/auth_commands.rs |
| FR-002 登入前不上傳、本機功能不受影響 | tracker-sync/agent.rs（登入後才啟動迴圈）|
| FR-003 每 1 分鐘輪詢上傳 | tracker-sync/agent.rs、uploader.rs |
| FR-004 離線佇列、固定批次補傳、磁碟滿捨棄最舊 | tracker-sync/queue_repo.rs、tracker-core/sync.rs |
| FR-005 網頁登入頁、未登入無法存取資料 | server/web、server/api（session 中介層）|
| FR-006 僅顯示本人資料＋唯讀分享 | server/api/data_routes.rs、share_routes.rs |
| FR-007 多裝置清單/切換/移除/硬體指紋合併 | server/api/device_routes.rs、jobs/device_merge.rs、tracker-platform/device_id.rs |
| FR-008 登出即失效 | server/auth/session.rs |
| FR-009 變更密碼後其他工作階段須重新登入 | server/auth/session.rs（撤銷既有 session）|
| FR-010 連續 10 次失敗鎖定 15 分鐘 | server/auth/lockout.rs |
| FR-011 傳輸/靜態加密、密碼雜湊、最小長度 | 全站 TLS、server/auth/password.rs（Argon2id）、storage/s3（SSE）|
| FR-012 帳號刪除保留 30 天後永久刪除 | server/jobs/retention_cleanup.rs |
| FR-013 使用者自訂保留天數 | server/jobs/retention_cleanup.rs、data-model 帳號設定欄位 |
| FR-014 網頁滑動視窗 24 小時／軟體固定 30 天 | server/auth/session.rs |
| SC-002 5 分鐘內可查詢 | tracker-sync/agent.rs（1 分鐘輪詢）＋ server/api/sync_routes.rs |
| SC-006 未授權存取 100% 拒絕 | server 全域 session 中介層 + api 授權檢查 |

## Post-Design Constitution Re-Check

Phase 1 設計完成後重新檢查：

- **I**：research/data-model/contracts/quickstart 全部 zh-TW。✅
- **II**：登入即同意動作，未登入前用戶端行為與資料流不變（data-model `sync_queue` 僅本機、上傳僅發生於已登入且 session 有效時）；帳號刪除 30 天可復原、逾期永久刪除（data-model `account.deleted_at`）；分享僅唯讀（`share_grant`）。✅
- **III**：`sync_record.server_received_at` 明確定義為排序與顯示依據，裝置本機時間僅存 `device_local_time_range` 作輔助欄位，未以推估掩蓋時鐘誤差。✅
- **IV**：用戶端僅新增 `reqwest` 一項新相依（已列 Complexity Tracking）；伺服器相依皆對應規格硬性需求（見「階段產出對照」），未見無用途之抽象層或套件。✅
- **V**：data-model／contracts 僅涵蓋 FR-001…FR-014、SC-001…SC-006 範圍，無擴張。✅
- **VI**：用戶端維持 Rust；伺服器雖非原則 VI 強制範圍，仍依使用者指示全 Rust 實作；Docker 基底映像為部署層非應用邏輯語言。✅

**Gate 狀態**：通過，可進入 `/speckit-tasks`。

## 下一步

執行 **`/speckit-tasks`** 依本計畫與設計產出可執行、相依排序的 `tasks.md`。
