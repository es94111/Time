<!-- SPECKIT START -->
作用中功能：001-activity-tracker（Windows 11 活動追蹤器）
技術背景與專案結構、shell 指令與重要資訊，請閱讀目前計畫：
`specs/001-activity-tracker/plan.md`

關鍵技術決策（摘要）：
- 語言：Rust（最新穩定版）；系統 API 一律用 Microsoft 官方 `windows` crate（原廠）
- UI：Tauri 2 + WebView2（Windows 11 原廠），前端 TypeScript，介面 zh-TW
- 儲存：本機 SQLite（SQLCipher AES‑256）；金鑰經 Windows DPAPI 保護，可選 Argon2id 主密碼
- 執行模型：使用者工作階段程序 + 系統匣（非 Windows 服務）；前景偵測用 SetWinEventHook
- 所有規格/計畫/任務/文件與程式註解優先正體中文（憲章原則 I）
<!-- SPECKIT END -->
