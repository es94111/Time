<!-- SPECKIT START -->
作用中功能：002-system-metrics（電腦資訊監控紀錄）
技術背景與專案結構、shell 指令與重要資訊，請閱讀目前計畫：
`specs/002-system-metrics/plan.md`
（基礎架構沿用 001：`specs/001-activity-tracker/plan.md`）

關鍵技術決策（摘要）：
- 語言：Rust（最新穩定版）；系統 API 一律用 Microsoft 官方 `windows` crate（原廠）
- 量測：PDH 效能計數器（CPU／PhysicalDisk／Network Interface／GPU Engine）＋ DXGI（VRAM）；不使用顯示卡廠商 SDK
- UI：Tauri 2 + WebView2（Windows 11 原廠），前端 TypeScript，介面 zh-TW
- 儲存：本機 SQLite（SQLCipher AES‑256），指標存於既有加密資料庫；金鑰經 Windows DPAPI 保護
- 取樣預設 1 秒（1–3600），保留預設 30 天（1–3650）；長區間趨勢查詢時降取樣、原始樣本保留
- 執行模型：使用者工作階段程序 + 系統匣（非 Windows 服務）
- 所有規格/計畫/任務/文件與程式註解優先正體中文（憲章原則 I）
<!-- SPECKIT END -->
