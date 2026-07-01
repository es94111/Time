# 軟體需求規格書（SRS）
# Time — Windows 11 活動追蹤器

**文件版本**：依下方 8.2 版本歷程表  
**最後更新**：2026-07-01  
**語言**：正體中文（zh-TW）

---

## 1. 專案概述

Time 是一款常駐於 Windows 11 使用者工作階段的桌面追蹤器，量測並記錄「前景應用程式使用時長」與「網站瀏覽時長」，以繁體中文介面提供今日摘要、歷史查詢與日期區間彙總。

---

## 2. 技術架構

- **語言**：Rust（最新穩定版，edition 2024）
- **UI 框架**：Tauri 2 + WebView2（Windows 11 原廠元件）
- **前端**：TypeScript（zh-TW 介面）
- **儲存**：本機 SQLite（SQLCipher AES-256 加密），位於 `%APPDATA%\ActivityTracker\`
- **金鑰保護**：Windows DPAPI（綁定使用者帳戶），可選 Argon2id 主密碼強化
- **執行模型**：使用者工作階段程序 + 系統匣（非 Windows 服務）
- **前景偵測**：SetWinEventHook（EVENT_SYSTEM_FOREGROUND，事件驅動）
- **網站辨識**：UI Automation 讀取瀏覽器網址列 → 完整主機名

---

## 3. Cargo Workspace 結構

```
crates/
├── tracker-core/        # 純 Rust 領域邏輯（工作階段狀態機、彙總、規則）
├── tracker-platform/    # Windows 原廠整合（前景偵測、閒置、電源、UIA、DPAPI）
└── tracker-storage/     # 加密 SQLite + 匯出（SQLCipher、Argon2id）
src-tauri/               # Tauri 2 應用（系統匣、IPC 命令、背景追蹤執行緒）
ui/                      # 前端（TypeScript，zh-TW 介面）
```

---

## 4. 功能規格索引

| 功能 | 規格文件 | 狀態 |
|------|----------|------|
| 001 — 活動追蹤器 | specs/001-activity-tracker/spec.md | 已實作 |
| 002 — 電腦資訊監控紀錄 | specs/002-system-metrics/spec.md | 規格草稿 |

---

## 5. 功能需求（FR）摘要 — v1.0

| FR | 說明 | 實作位置 |
|----|------|----------|
| FR-001 | 前景視窗偵測 | tracker-platform/foreground.rs |
| FR-002 | 登入自啟、背景常駐 | tracker-platform/autostart.rs、src-tauri |
| FR-003 | 閒置時間排除 | tracker-platform/idle.rs、tracker-core/rules.rs |
| FR-004 | 睡眠/鎖定不計時 | tracker-platform/session_power.rs |
| FR-005 | 網站辨識（完整主機名，零設定） | tracker-platform/browser_url.rs、tracker-core/hostname.rs |
| FR-006 | 每日彙總、午夜切分 | tracker-core/session.rs、aggregation.rs |
| FR-007~009 | 今日/歷史/區間摘要 | tracker-storage/repository.rs |
| FR-010 | 持久化（WAL，≤1 分鐘損失） | tracker-storage/db.rs |
| FR-011/016 | 本機加密（SQLCipher + DPAPI + Argon2id） | tracker-storage/crypto.rs |
| FR-012 | 暫停/恢復追蹤 | src-tauri/tracking_service.rs |
| FR-013 | 排除規則 | tracker-core/rules.rs |
| FR-014 | 資料無限期保留 + 手動清除 | tracker-storage/repository.rs |
| FR-015 | 易讀時間顯示、未知應用標記 | tracker-core/model.rs、ui |
| FR-017 | CSV/JSON 匯出 | tracker-storage/export.rs |
| FR-018 | 繁體中文介面 | ui/i18n/zh-TW.ts |
| FR-019 | 5 秒最短記錄門檻 | tracker-core/rules.rs |

---

## 6. 成功標準（SC）摘要 — v1.0

| SC | 標準 |
|----|------|
| SC-001 | 時長量測誤差 ≤ ±2% |
| SC-002 | 開啟摘要回應時間 < 3 秒 |
| SC-003 | 閒置時平均 CPU < 2% |
| SC-004 | 不同日期的歷史資料能正確分日查詢 |
| SC-005 | 當機後資料損失 ≤ 最近 1 分鐘 |
| SC-006 | 私密瀏覽模式預設不記錄逐網站明細 |
| SC-007 | 5 秒以下閃切不產生記錄 |

---

## 7. 非功能需求

- **安全性**：靜態資料 AES-256 加密；金鑰綁定 Windows 帳戶；其他本機帳戶無法存取。
- **效能**：事件驅動量測，閒置時 CPU < 2%、記憶體 < 150 MB（後續功能另訂）。
- **隱私**：純本機，無外部傳輸，無遙測。
- **平台**：Windows 11（含 Home 版），使用者互動工作階段。

---

## 8. 附錄

### 8.1 依賴套件清單（主要）

| 套件 | 用途 |
|------|------|
| windows | Microsoft 官方 Win32/WinRT 綁定 |
| tauri 2.x | UI 外殼、系統匣、IPC |
| rusqlite + SQLCipher | 加密本機 SQLite |
| argon2、aes-gcm、zeroize | 金鑰衍生、AEAD、記憶體清除 |
| jiff | 時區感知時間運算（午夜切分、DST）|
| serde、serde_json、csv | 設定序列化、JSON/CSV 匯出 |

### 8.2 版本歷程

| 版本 | 日期 | 說明 |
|------|------|------|
| 2.0 | 2026-07-01 | 新增遠端資料同步與網頁儀表板模組（003-remote-sync-web-login）：Windows 軟體與網頁共用帳號登入（Argon2id 雜湊、伺服器端可撤銷 Session）；裝置指紋識別（登錄檔 MachineGuid）；待同步佇列（沿用 SQLCipher，固定批次補傳、磁碟滿捨棄最舊）；`server/` 新增獨立 Cargo workspace（Axum + PostgreSQL + S3 相容物件儲存），Docker 容器化部署；多裝置檢視、唯讀分享、帳號鎖定（連續 10 次失敗鎖定 15 分鐘）等安全機制 |
| 1.1 | 2026-07-01 | 新增系統資源監控模組（002-system-metrics）：PDH 效能計數器（CPU / PhysicalDisk / Network Interface / GPU Engine）＋ DXGI VRAM；即時監控頁面（metrics://sample 事件推送）；歷史趨勢查詢含四段降取樣（raw / 60s / 3600s / 86400s）；7 張新增資料表（metric_sample, disk_device, disk_sample, network_interface, net_sample, gpu_device, gpu_sample）；6 個 Tauri IPC 指令；metrics_service 背景執行緒 |
| 1.0 | 2026-07-01 | 初始發佈：活動追蹤器核心功能（應用程式使用時長 + 網站瀏覽時長 + 歷史查詢 + 本機加密 + CSV/JSON 匯出） |
