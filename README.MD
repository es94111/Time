# Time — Windows 11 活動追蹤器

Time 是一款以 Rust 與 Tauri 2 開發的 Windows 11 桌面應用程式，用來在背景記錄前景應用程式使用時長、網站瀏覽時長與系統資源監控資料。專案也包含遠端同步服務與網頁儀表板，讓使用者登入後可將本機資料安全同步到自架或雲端環境，並在瀏覽器查看多裝置紀錄。

## 主要功能

- 背景追蹤前景應用程式使用時長，支援今日摘要、歷史查詢與日期區間彙總。
- 辨識瀏覽器目前網站，依完整主機名統計瀏覽時長，不需安裝瀏覽器外掛。
- 記錄 CPU、記憶體、磁碟、網路與 GPU 使用狀態，提供即時監控與歷史趨勢資料。
- 本機資料以 SQLCipher 加密保存，金鑰透過 Windows DPAPI 綁定使用者帳戶。
- 登入後啟用遠端同步；離線時先寫入本機佇列，恢復連線後自動補傳。
- 提供 Rust Web 伺服器、PostgreSQL 與 S3 相容物件儲存，用於帳號、裝置、同步資料與網頁儀表板。

## 技術架構

| 區域 | 技術 |
|---|---|
| 桌面應用 | Rust、Tauri 2、WebView2 |
| Windows 整合 | Microsoft 官方 `windows` crate、DPAPI、UI Automation、PDH、DXGI |
| 前端 | TypeScript、Vite、繁體中文介面 |
| 本機儲存 | SQLCipher SQLite、Argon2id、AES-GCM |
| 遠端伺服器 | Rust、Axum、Tokio、sqlx |
| 遠端儲存 | PostgreSQL、S3 相容物件儲存（可使用 MinIO） |
| 部署 | Docker、Docker Compose |

## 專案結構

```text
crates/
├── tracker-core/        # 純 Rust 領域邏輯：工作階段、彙總、規則、同步批次
├── tracker-platform/    # Windows 平台整合：前景偵測、閒置、電源、UIA、DPAPI、系統指標
├── tracker-storage/     # 加密 SQLite、匯出、同步佇列與本機憑證快取
└── tracker-sync/        # 遠端登入、佇列讀取、批次上傳與同步代理
src-tauri/               # Tauri 應用、系統匣、IPC 命令、背景服務
ui/                      # 前端 TypeScript/Vite 專案
server/                  # 遠端 Axum 伺服器、Dockerfile、docker-compose.yml
specs/                   # Spec Kit 規格、計畫、契約與任務文件
```

## 開發需求

- Windows 11
- Rust 最新穩定版（專案目前 `rust-version = "1.80"`）
- Node.js 與 npm
- WebView2 Runtime
- Docker Desktop（執行遠端伺服器、PostgreSQL、MinIO 時需要）
- Perl（Windows 上建置 vendored OpenSSL/SQLCipher 時需要）

## 快速開始

### 安裝前端相依套件

```powershell
cd ui
npm install
```

### 啟動前端開發伺服器

```powershell
cd ui
npm run dev
```

### 建置前端

```powershell
cd ui
npm run build
```

### 執行桌面應用測試

```powershell
cargo test -p tracker-core
cargo check -p tracker-platform
```

完整根 workspace 測試會建置 SQLCipher vendored OpenSSL；Windows 環境需先安裝 Perl。

## 遠端伺服器本機開發

```powershell
cd server
docker compose up -d
```

伺服器預設包含：

- `postgres`：帳號、裝置、工作階段與同步中繼資料。
- `minio`：本機開發用 S3 相容物件儲存。
- `server`：Rust Axum API 與網頁儀表板，預設監聽 `http://localhost:8080`。

健康檢查：

```powershell
curl http://localhost:8080/healthz
```

整合測試需要可用的 PostgreSQL 測試資料庫，並設定 `DATABASE_URL`。

## 常用驗證指令

```powershell
# 前端建置與安全檢查
cd ui
npm run build
npm audit

# Rust 核心與平台檢查
cd ..
cargo test -p tracker-core
cargo check -p tracker-platform

# 伺服器測試（需 DATABASE_URL）
cd server
cargo test
```

## 文件索引

- `SRS.md`：軟體需求規格書與版本歷程。
- `changelog.json`：使用者導向的版本變更紀錄。
- `specs/001-activity-tracker/`：活動追蹤器核心規格。
- `specs/002-system-metrics/`：系統資源監控規格。
- `specs/003-remote-sync-web-login/`：遠端同步、網頁儀表板與登入驗證規格。
- `server/docker-compose.yml`：本機遠端服務開發環境。

## 隱私與安全原則

Time 預設以本機保存為核心，未登入前不會上傳本機活動或監控資料。登入代表使用者明確啟用遠端同步；同步資料傳輸與靜態保存皆需加密，密碼僅以 Argon2id 雜湊保存，不儲存明文密碼。

## 授權

本專案採 MIT License。
