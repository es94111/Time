# Phase 0 研究：Windows 11 活動追蹤器

**日期**：2026-06-30 ｜ **語言**：zh-TW（憲章原則 I）

本檔解析所有技術未知，並為每項決策記錄「決策／理由／已評估的替代方案」。使用者約束：**Rust**、**Windows 11 原廠語法或套件**、**套件取最新版**、**最安全的加密方式**。

---

## R1. 執行模型：使用者工作階段程序 vs Windows 服務

- **決策**：以**單一使用者工作階段程序**執行（登入時自啟、最小化至系統匣），背景以執行緒進行追蹤；**不**使用 Windows 服務。
- **理由**：前景視窗偵測（`GetForegroundWindow`/`SetWinEventHook`）、閒置（`GetLastInputInfo`）與 UI Automation 皆需在**互動式桌面工作階段**中執行。Windows 服務執行於 Session 0，與使用者桌面隔離，**無法**取得使用者的前景視窗或對其做 UIA。
- **已評估替代**：Windows 服務（被否決：Session 0 隔離）；服務 + 使用者代理雙程序（被否決：複雜度高、違反原則 IV，對單機單人無必要）。

## R2. 前景應用程式偵測（FR-001）

- **決策**：使用 `windows` crate 的 `SetWinEventHook(EVENT_SYSTEM_FOREGROUND)` 進行**事件驅動**前景切換偵測；切換時以 `GetWindowThreadProcessId` + `OpenProcess` + `QueryFullProcessImageNameW` 取得執行檔路徑，再導出穩定的應用名稱。
- **理由**：事件驅動只在焦點實際變動時被喚醒，平均 CPU 遠低於輪詢（呼應 SC‑003、原則 IV）。`QueryFullProcessImageNameW` 為現行建議 API（取代過時的 `GetModuleFileNameEx`）。
- **已評估替代**：定時輪詢 `GetForegroundWindow`（被否決：持續耗 CPU）；UWP/WinRT `AppDiagnosticInfo`（被否決：需特殊權限、涵蓋不全）。

## R3. 閒置偵測（FR-003、SC-004）

- **決策**：以輕量計時器（每數秒）呼叫 `GetLastInputInfo` 計算自上次輸入經過時間；超過閒置門檻（預設 5 分鐘，可調）即將該段標為 idle 並**回溯**自最後輸入時刻起不計入活躍。
- **理由**：`GetLastInputInfo` 為系統級、低成本的標準閒置量測。回溯扣除避免把「離開前的閒置」誤計（呼應原則 III 誠實量測）。
- **已評估替代**：低階輸入掛鉤 `SetWindowsHookEx`（被否決：成本與權限較高、易被防毒誤判）。

## R4. 睡眠／鎖定／關機處理（FR-004、邊界情況）

- **決策**：建立隱藏訊息視窗，處理 `WM_POWERBROADCAST`（`PBT_APMSUSPEND`/`PBT_APMRESUMEAUTOMATIC`）以偵測睡眠/喚醒；以 `WTSRegisterSessionNotification` 接收 `WTS_SESSION_LOCK`/`WTS_SESSION_UNLOCK`。進入睡眠或鎖定時**結算並關閉**目前工作階段，喚醒/解鎖後重新開始。
- **理由**：確保睡眠/鎖定期間不被計為活躍，且資料在中斷後保留（原則 III、FR-010）。
- **已評估替代**：僅靠閒置門檻間接涵蓋（被否決：睡眠可能短於門檻而誤計）。

## R5. 零設定網站偵測（FR-005，完整主機名）

- **決策**：當前景程序為已知瀏覽器時，使用 **UI Automation（`IUIAutomation`，`windows` crate）讀取網址列（Edit/Document 控制項）的值**取得 URL，再以 `url` crate 解析出**完整主機名**（hostname，子網域分開）。取不到 URL 時退回解析視窗標題；仍無法判定者僅計為該瀏覽器之一般應用程式時間。
- **理由**：UIA 為 Windows 原廠、零設定（免安裝瀏覽器擴充）方式，能在主流瀏覽器穩定讀到網址列；符合 FR-005「不得要求安裝附加元件」。完整主機名滿足子網域分開之釐清結論。
- **已評估替代**：瀏覽器擴充（被否決：違反零設定）；僅視窗標題（被否決：常只含頁面標題、無網域，準確度低，僅作退路）；偵錯協定/CDP（被否決：需啟特殊旗標、非零設定）。
- **隱私**：偵測到無痕視窗時不記錄逐網站明細（FR 邊界情況、原則 II）。

## R6. 時間與午夜切分（FR-006、DST 邊界）

- **決策**：採用 `jiff` 進行時區感知運算；工作階段以 UTC 瞬時記錄起訖，彙總時以**本機時區**換算，跨越本機午夜者於午夜邊界切分為前後兩段。
- **理由**：`jiff` 對時區/DST/民用時間處理嚴謹，能正確處理 DST 轉換與時鐘變更（對應「系統時鐘變更」「跨越午夜」邊界情況）。
- **已評估替代**：`chrono`（可行但時區/DST 處理較易出錯）；自寫時間數學（被否決：DST 易錯，違反原則 III）。

## R7. 加密與金鑰管理（FR-016「最安全的加密方式」）

- **決策（縱深防禦）**：
  1. **全資料庫加密**：SQLite + **SQLCipher（AES‑256‑CBC + HMAC‑SHA‑512，PBKDF2 金鑰衍生；SQLCipher 4 預設組態）**。
  2. **資料金鑰（DEK）**：首次啟動以 OS CSPRNG 產生 256‑bit 隨機金鑰。
  3. **金鑰保護（at rest）**：以 **Windows DPAPI（`CryptProtectData`，使用者範圍）** 保護 DEK，綁定當前 Windows 使用者帳戶 → 其他系統管理員在未取得該使用者登入憑證下無法解密（精確滿足 FR-016）。
  4. **可選強化**：使用者可設定主密碼，以 **Argon2id**（PHC 優勝、抗 GPU）衍生 KEK 再包裹 DEK，形成「DPAPI + 密碼」雙因子。
  5. **記憶體衛生**：金鑰與敏感緩衝以 `zeroize` 清除。
  6. 任何 app 層級加密（如加密匯出檔）採 **AES‑256‑GCM**（AEAD）。
- **理由**：SQLCipher 為經審計的標準全庫加密；DPAPI 為 Windows 原廠、帳戶綁定的金鑰保護；Argon2id 為現行最強 KDF；AES‑256‑GCM 為現代 AEAD。組合即「最安全且符合 Windows 原廠」。
- **已評估替代**：Windows **EFS**（被否決：**Windows 11 Home 不支援**）；僅 BitLocker 全碟（被否決：非每應用、且使用者可能未啟用）；自製 KV + 自寫加密（被否決：易錯，違反「最安全」）。

## R8. 登入自啟（FR-002）

- **決策**：使用 **`HKCU\Software\Microsoft\Windows\CurrentVersion\Run`** 登錄項註冊登入自啟（經 `tauri-plugin-autostart` 或直接 `windows`/登錄 API）；以 `tauri-plugin-single-instance` 確保單一實例。
- **理由**：HKCU Run 為原廠、免系統管理員權限、針對目前使用者；單例避免重複追蹤。
- **已評估替代**：工作排程器（被否決：較重、對單人單機無必要）；啟動資料夾捷徑（可行，登錄更易程式化管理）。

## R9. UI 框架（FR-018、原則 I/IV/VI）

- **決策**：**Tauri 2** + WebView2（原廠）+ TypeScript 前端（zh-TW），輕量圖表呈現清單與長條/圓餅圖。
- **理由**：憲章明文允許 Tauri；WebView2 為 Windows 11 原廠共用元件，安裝體積與記憶體佔用低；最易做出在地化的豐富儀表板。
- **已評估替代**：`egui`/`slint` 純 Rust GUI（被否決：圖表/表格/在地化成熟度與效率較差）。詳見 plan.md 之 Complexity Tracking。

## R10. 套件版本策略（使用者要求「最新版本」）

- **決策**：實作時對每個相依執行 `cargo add <crate>`（預設取最新相容版）並提交 `Cargo.lock`；UI 端以 `npm install <pkg>@latest`。本計畫不寫死次版號，僅記錄主版線（如 Tauri 2.x），由實作當下解析最新穩定版。
- **理由**：誠實反映「最新版本」要求，同時以 `Cargo.lock` 確保可重現建置。
- **已評估替代**：在計畫寫死精確版本（被否決：易過時、與「最新」要求衝突）。

---

## 未解決事項（NEEDS CLARIFICATION）

無。Technical Context 中所有項目均已由使用者約束與上述研究解析完成。
