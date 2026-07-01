# Phase 0 研究：遠端資料同步與網頁儀表板（含登入驗證）

> 本檔以正體中文（zh-TW）撰寫（憲章原則 I）。所有 Technical Context 中的未定項目於本檔逐項解決。

## R1. 遠端 Web 伺服器語言與框架

- **Decision**：Rust + **Axum**（最新穩定版，搭配 `tokio` 執行期）。
- **Rationale**：使用者明確要求全部以 Rust 實作；Axum 是目前 Rust 生態最活躍、與 `tower` 中介層生態（session、逾時、壓縮、追蹤）整合最成熟的非同步 Web 框架，可同時提供 REST API 與伺服器渲染網頁。
- **Alternatives considered**：`actix-web`（成熟但中介層模型與 tower 生態不同，社群近期活躍度略低於 axum）；Node.js/Express、Go（皆非 Rust，違反使用者明確指示，故排除）。

## R2. 資料庫與資料存取

- **Decision**：**PostgreSQL**（最新穩定大版本），透過 `sqlx`（`postgres`、`runtime-tokio-rustls` features，最新版）存取，並以 `sqlx migrate` 管理結構。
- **Rationale**：帳號、裝置、工作階段、同步紀錄中繼資料、分享授權為**強一致性、關聯式**資料（外鍵、唯一鍵、交易），PostgreSQL 是業界標準且官方支援長期維運；`sqlx` 提供編譯期 SQL 檢查，降低執行期錯誤。
- **Alternatives considered**：SQLite（單機够用但多使用者遠端服務需要更好的併發寫入與網路存取，且無法直接以容器共享磁碟做水平擴展）；MongoDB（資料具明確關聯與交易需求，關聯式資料庫更合適）。

## R3. 大量時序資料的保存位置

- **Decision**：活動追蹤與系統監控資料快照本體存於 **S3 相容物件儲存**（透過官方 `aws-sdk-s3`），依 `帳號/裝置/日期` 分區存放；PostgreSQL 僅保存指向物件儲存的**中繼資料**（同步紀錄：時間範圍、上傳狀態、物件路徑）。
- **Rationale**：規格中列舉「如 S3 或 SERVER」；監控資料量隨時間線性成長且多為「寫入後依範圍批次讀取」的存取模式，適合物件儲存；`aws-sdk-s3` 為官方 AWS SDK，其 S3 用戶端可透過自訂 endpoint 指向自架 **MinIO**（Docker 容器）或雲端 S3，避免廠商鎖定。
- **Alternatives considered**：全部存 PostgreSQL 的 JSONB/BYTEA 欄位：長期會使資料表迅速膨脹、清理（保留天數/帳號刪除）效能差，且與規格「如 S3」之提示不符。

## R4. 密碼雜湊與帳號安全

- **Decision**：**Argon2id**（RustCrypto `argon2` crate，最新版），採用官方建議之記憶體/時間成本參數；連續 10 次失敗即於帳號紀錄寫入 `locked_until`，15 分鐘內拒絕登入嘗試（FR-010）。
- **Rationale**：Argon2id 為目前密碼雜湊之業界最佳實務（OWASP 建議），沿用 001 用戶端主密碼已採用的同一演算法家族，一致性佳。鎖定邏輯以資料庫欄位實作，不需額外的限流套件（YAGNI，呼應原則 IV）。
- **Alternatives considered**：bcrypt（可用但 Argon2id 對 GPU 破解的抗性更佳，且與現有專案已採用之演算法一致）。

## R5. 工作階段（Session）機制：伺服器端可撤銷 Session vs. JWT

- **Decision**：**伺服器端 Session**（`tower-sessions` + `tower-sessions-sqlx-store`，存於 PostgreSQL），Cookie 僅存不透明的 Session ID。軟體端（Windows）另發放**長期 refresh 憑證**（同樣是伺服器端可撤銷的 opaque token，存於用戶端 DPAPI 保護的本機快取），固定 30 天效期；網頁端 Session 採**滑動視窗**，每次操作延展至最後操作 24 小時後失效。
- **Rationale**：FR-008（登出立即失效）與 FR-009（變更密碼後其他工作階段須重新登入）要求**立即撤銷**能力；無狀態 JWT 若要支援即時撤銷，仍須維護一份黑名單／版本號，等同於重新實作伺服器端狀態，不如直接使用伺服器端 Session 單純。
- **Alternatives considered**：純 JWT + 短效期 + refresh rotation：仍需伺服器端記錄 refresh token 版本以支援撤銷，複雜度不低於方案一，且滑動視窗語意（FR-014）以 Session 儲存的 `last_active_at` 欄位更新最直觀。

## R6. Windows 裝置指紋（硬體特徵識別）

- **Decision**：讀取登錄檔 `HKLM\SOFTWARE\Microsoft\Cryptography\MachineGuid`（透過 `windows` crate 官方登錄檔 API），作為裝置指紋主要依據；重新安裝／重置系統會產生新 GUID，此時退回「裝置名稱 + 使用者帳號」比對作輔助合併規則（見 spec Edge Case：全新裝置重新綁定）。
- **Rationale**：`MachineGuid` 是 Windows 安裝時產生、跨應用程式重裝仍保留（只要作業系統未重灌）的原廠識別碼，讀取僅需標準登錄檔 API，符合原則 VI「原廠 API」要求，無需引入 WMI 等第三方 crate。
- **Alternatives considered**：`wmi` crate 查詢主機板序號：功能更廣但屬第三方非官方 crate、且需要 WMI 服務可用；用 CPU ID／磁碟序號：部分虛擬機或整新品缺乏穩定序號，穩定性不如 `MachineGuid`。

## R7. 用戶端與伺服器通訊

- **Decision**：`reqwest`（`rustls-tls` feature，最新版）以 HTTPS 呼叫伺服器 REST API；批次上傳採 `multipart` 或 JSON + gzip 壓縮（依資料型態選擇，於 contracts/rest-api.md 定義）。
- **Rationale**：`reqwest` 是 Rust 生態最成熟的 HTTP 客戶端，`rustls-tls` 避免依賴系統 OpenSSL，簡化 Windows 部署；已於 Constitution Check / Complexity Tracking 說明其為用戶端唯一新增之非 Windows 原廠元件的必要性。
- **Alternatives considered**：直接綁定 WinHTTP（`windows` crate）：需自行處理 TLS/憑證/連線池，開發與維運成本高，非必要之重造輪子。

## R8. Docker 容器化與部署

- **Decision**：`server/Dockerfile` 採**多階段建置**：builder 階段使用官方 `rust:<最新穩定版>-slim` 映像編譯 release 二進位，runtime 階段使用精簡的 `debian:stable-slim`（或 `gcr.io/distroless/cc` 視相依動態函式庫需求）僅複製編譯後二進位與必要憑證，縮小映像體積與攻擊面。`docker-compose.yml` 編排 `server` + `postgres`（官方 `postgres:<最新穩定版>` 映像）+ `minio`（官方 `minio/minio` 映像），供自架部署參考；正式環境亦可將 PostgreSQL／物件儲存改指向受管服務。
- **Rationale**：符合使用者指示「web 伺服器要使用 Docker 套件」；多階段建置是 Rust 官方文件與社群公認的最佳實務，可大幅縮小最終映像。
- **Alternatives considered**：單階段建置（映像包含完整 Rust 工具鏈，體積大、攻擊面大，不採用）。

## R9. TLS 終止與反向代理

- **Decision**：伺服器應用本身以純 HTTP 提供服務，TLS 終止交由部署環境的反向代理（如 Caddy、Nginx 或雲端負載平衡器）處理；`docker-compose.yml` 範例中以註解說明可選掛載反向代理，實際憑證管理（如 Let's Encrypt）留待部署階段依環境決定，非本規格重點（呼應 spec「Assumptions」）。
- **Rationale**：將 TLS 憑證輪替與應用邏輯解耦是常見雲原生實務，可避免在應用程式內管理憑證生命週期；規格本身未指定特定雲端供應商。
- **Alternatives considered**：應用內建 `rustls` 直接處理 TLS：需自行管理憑證更新與 ACME 流程，複雜度較高，非必要（YAGNI）。

## R10. 保留天數與帳號刪除排程清理

- **Decision**：伺服器背景排程工作（`server/src/jobs/retention_cleanup.rs`，以 `tokio::time::interval` 定期執行，無需額外排程套件）：
  1. 依帳號設定之保留天數，刪除逾期的同步紀錄中繼資料與對應物件儲存 blob；
  2. 對已標記刪除的帳號，於 30 天後永久清除其所有資料（PostgreSQL 記錄 + S3 物件）。
- **Rationale**：清理頻率與資料量在個人使用規模下無需引入 `cron`/`sidekiq` 類的外部排程系統，`tokio` 內建定時器即足夠（YAGNI，呼應原則 IV）。
- **Alternatives considered**：外部排程服務（如 Kubernetes CronJob）：規格未要求特定部署拓樸，先以應用內建排程滿足需求，未來如需水平擴展多副本執行清理工作，再引入分散式鎖（如 PostgreSQL advisory lock）。

## R11. 靜態加密（Data at Rest）

- **Decision**：物件儲存（S3/MinIO）由應用程式於每次 `PutObject` 時明確設定伺服器端加密（SSE，如 SSE-S3；自架 MinIO 亦支援 SSE-S3/SSE-KMS），對應 `server/src/storage/s3.rs` 之封裝；PostgreSQL 之靜態加密則由部署環境的磁碟／儲存層負責（如受管資料庫服務內建加密，或自架時使用加密磁碟區），非應用程式邏輯範圍，與 R9 之 TLS 終止同屬部署層關注點。
- **Rationale**：FR-011 要求「傳輸中與遠端保存的使用者資料進行加密保護」。活動追蹤／系統監控資料快照為應用程式直接寫入物件儲存的酬載，理應由應用程式在寫入時明確要求 SSE，不依賴儲存端的預設設定；關聯式資料庫（帳號、裝置、Session 等中繼資料）之靜態加密屬基礎設施層責任，比照憑證管理（R9）以部署環境（受管服務或加密磁碟區）滿足即可，避免在應用程式內重造資料庫層加密機制（YAGNI，呼應原則 IV）。
- **Alternatives considered**：應用層自行對寫入 PostgreSQL 的欄位逐一加密（如欄位級 AES-GCM）：規格未要求特定欄位級加密，且徒增金鑰管理複雜度並限制可查詢性，不採用。
