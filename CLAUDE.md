<!-- SPECKIT START -->
作用中功能：003-remote-sync-web-login（遠端資料同步與網頁儀表板，含登入驗證）
技術背景與專案結構、shell 指令與重要資訊，請閱讀目前計畫：
`specs/003-remote-sync-web-login/plan.md`
（基礎架構沿用 001：`specs/001-activity-tracker/plan.md`、002：`specs/002-system-metrics/plan.md`）

關鍵技術決策（摘要）：
- 語言：全部 Rust（最新穩定版）；Windows 11 用戶端一律用 Microsoft 官方 `windows` crate（原廠）
- 用戶端新增：裝置指紋（登錄檔 MachineGuid）、待同步佇列（沿用 SQLCipher）、`reqwest` 呼叫伺服器 API
- 遠端伺服器（`server/`，獨立 Cargo workspace，Docker 容器化）：Axum + PostgreSQL（sqlx）+ S3 相容物件儲存（aws-sdk-s3，可指向自架 MinIO）
- 認證：Argon2id 密碼雜湊；伺服器端可撤銷 Session（tower-sessions + PostgreSQL），網頁滑動視窗 24 小時、軟體固定 30 天
- 帳號安全：連續 10 次登入失敗鎖定 15 分鐘；密碼最小長度 8 字元
- 同步：登入後每 1 分鐘輪詢上傳；離線佇列固定批次（≤500 筆/≤5MB）補傳；磁碟滿捨棄最舊
- 部署：`server/Dockerfile`（多階段建置）+ `docker-compose.yml`（server+postgres+minio）
- 所有規格/計畫/任務/文件與程式註解優先正體中文（憲章原則 I）
<!-- SPECKIT END -->
