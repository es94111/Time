# 快速開始：Windows 11 活動追蹤器（開發者）

**日期**：2026-06-30 ｜ **語言**：zh-TW ｜ **平台**：Windows 11（含 Home）

本檔說明如何在本機建置、執行與測試本專案。所有套件以「最新穩定版」安裝（使用者要求）。

---

## 1. 先決條件

- **Windows 11**（開發/執行皆於此；追蹤需互動式工作階段）。
- **Rust 工具鏈**：`rustup`（`rustup default stable && rustup update`）。
- **WebView2 Runtime**：Windows 11 內建（Tauri UI 使用之原廠元件）。
- **Tauri CLI**：`cargo install tauri-cli`（或使用 `cargo tauri`）。
- **Node.js（LTS）**：前端建置（`npm`）。
- **C 建置工具**：MSVC build tools（SQLCipher 經 `bundled-sqlcipher-vendored-openssl` 編譯，內含 OpenSSL，無需另裝系統 OpenSSL）。

## 2. 取得相依（取最新版）

```powershell
# 後端 crates（於各 crate 內以 cargo add 取最新）
cargo add windows --features "Win32_UI_WindowsAndMessaging Win32_System_Threading Win32_UI_Accessibility Win32_Security_Cryptography Win32_System_RemoteDesktop"
cargo add rusqlite --features "bundled-sqlcipher-vendored-openssl"
cargo add jiff url serde serde_json csv argon2 aes-gcm zeroize
# Tauri
cargo add tauri --features "tray-icon"
cargo add tauri-plugin-autostart tauri-plugin-single-instance

# 前端
cd ui && npm install   # 圖表與建置工具取 @latest
```

> 提交 `Cargo.lock` 與 `package-lock.json` 以確保可重現建置。

## 3. 建置與執行

```powershell
# 開發模式（熱重載 UI + Rust 後端）
cargo tauri dev

# 正式建置（產生安裝檔）
cargo tauri build
```

首次執行會：
1. 於 `%APPDATA%\ActivityTracker\` 建立加密資料庫，產生 256‑bit DEK 並以 **DPAPI** 保護。
2. 註冊 **HKCU Run** 登入自啟（可於設定關閉）。
3. 最小化至**系統匣**並開始背景追蹤。

## 4. 測試

```powershell
# 核心領域邏輯（無 OS 相依，跨平台可跑）
cargo test -p tracker-core

# 全部（含平台與儲存整合測試，建議於 Windows 執行）
cargo test --workspace
```

重點測試對象：
- `tracker-core`：午夜切分、5 秒最短門檻、閒置回溯扣除、主機名解析、彙總排序。
- `tracker-storage`：SQLCipher 開啟/金鑰、WAL 當機恢復、CSV/JSON 匯出（對照 `contracts/export-formats.md`）。
- `tracker-platform`：以 trait 抽象 Windows API，於非 Windows 以假實作驗證邏輯；於 Windows 做煙霧測試。

## 5. 驗收對照（手動）

| 成功標準 | 驗證方式 |
|----------|----------|
| SC-002（<3 秒開啟摘要） | 累積資料後開啟今日檢視計時 |
| SC-003（CPU <2%） | 工作管理員觀察常駐期間平均 CPU |
| SC-004（閒置不計） | 離開 >5 分鐘後返回，確認該段未計 |
| SC-005（當機 ≤1 分鐘損失） | 強制結束程序後重啟，比對最後資料 |
| FR-005（完整主機名） | 於瀏覽器切換 mail.google.com 與 docs.google.com，確認分開計 |
| FR-016（加密） | 以外部工具開啟 `.db` 應為亂碼；換帳戶無法解 DPAPI 金鑰 |

## 6. 安全與隱私須知

- **不得**提交任何金鑰、`.db`、或 `.env` 至版本庫（憲章「機密管理」）。`.gitignore` 應含 `*.db`、`*.db-wal`、`*.db-shm`。
- 資料僅存本機；本專案無任何網路傳輸程式碼。
