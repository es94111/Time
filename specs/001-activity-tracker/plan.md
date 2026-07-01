# 實作計畫：Windows 11 活動追蹤器（Activity Tracker）

**Branch**: `001-activity-tracker` | **Date**: 2026-06-30 | **Spec**: [spec.md](./spec.md)
**Input**: 功能規格 `specs/001-activity-tracker/spec.md`

> 本計畫文件以正體中文（zh-TW）撰寫（憲章原則 I）。

## Summary

打造一個常駐於 Windows 11 使用者工作階段的桌面追蹤器，誠實量測「前景應用程式使用時長」與「網站瀏覽時長」，並以繁體中文介面提供今日／歷史／區間摘要。技術取向：以 **Rust** 實作追蹤核心與資料層；系統層級量測一律透過 **Microsoft 官方 `windows` crate**（Win32／WinRT 原廠 API）；UI 採 **Tauri 2**（使用 Windows 11 原廠 WebView2 元件）。資料以 **SQLCipher（AES‑256）全資料庫加密**保存於本機，加密金鑰由 **Windows DPAPI** 綁定使用者帳戶保護，可選 **Argon2id** 主密碼強化——對應「最安全的加密方式」與隱私優先原則。所有套件於實作時以 `cargo add` 取最新穩定版。

## Technical Context

**Language/Version**: Rust（最新穩定版，edition 2021/2024；實作時以 `rustup update` 取最新）
**Primary Dependencies**:
- `windows`（Microsoft 官方 Win32/WinRT 綁定）— 前景視窗事件、閒置偵測、工作階段／電源通知、UI Automation、DPAPI、開機自啟（登錄）
- `tauri` 2.x（+ `tauri-plugin-autostart`、`tauri-plugin-single-instance`）— UI 外殼、系統匣、登入自啟
- `rusqlite`（啟用 SQLCipher，特性如 `bundled-sqlcipher-vendored-openssl`）— 加密的本機 SQLite
- `argon2`、`aes-gcm`、`zeroize`（RustCrypto）— 金鑰衍生／AEAD／記憶體清除
- `jiff` — 時區感知時間運算（本機午夜切分、DST／時鐘變更處理）
- `url`、`serde`、`serde_json`、`csv` — 主機名解析、設定序列化、JSON/CSV 匯出
- 前端：TypeScript + 輕量圖表（zh-TW 介面），執行於 WebView2

**Storage**: 本機加密 SQLite（SQLCipher，AES‑256），位於 `%APPDATA%\ActivityTracker\`；金鑰經 DPAPI 保護
**Testing**: `cargo test`（核心邏輯單元測試）+ 整合測試；平台 API 以 trait 抽象後可在非 Windows 上以假實作測試
**Target Platform**: Windows 11（含 Home 版）桌面，使用者互動工作階段（非 Session 0 服務）
**Project Type**: Windows 桌面應用程式（Cargo workspace 多 crate + Tauri 應用 + 前端）
**Performance Goals**: 前景偵測採事件驅動（`SetWinEventHook`）以維持平均 CPU < 2%（SC‑003）；開啟摘要 < 3 秒（SC‑002）；時長量測誤差 ≤ ±2%（SC‑001）
**Constraints**: 純本機、預設不外傳（原則 II）；低 CPU／低記憶體／低耗電（原則 IV）；當機後資料損失 ≤ 最近 1 分鐘（SC‑005）；UI 與所有文件 zh-TW（原則 I）
**Scale/Scope**: 單一使用者、單機；資料量級為每日數百至數千筆工作階段、無限期保留（需具備索引與彙總查詢效率）

## Constitution Check

*GATE：須於 Phase 0 前通過，並於 Phase 1 設計後再次檢查。*

依據憲章（`.specify/memory/constitution.md` v1.1.0）：

- **原則 I — 正體中文優先**：✅ plan/research/data-model/quickstart/contracts 全部 zh-TW；UI 主要語言 zh-TW（FR-018）。
- **原則 II — 隱私優先與本機資料**：✅ 僅本機儲存、無任何外部傳輸、無遙測；全資料庫加密（FR-011、FR-016）；提供匯出與徹底刪除（FR-017、FR-014）。
- **原則 III — 追蹤準確且誠實**：✅ 明確定義「前景／作用中／閒置」量測（見 research.md）；事件驅動量測真實焦點時間；閒置與睡眠不計（FR-003、FR-004）；無法辨識者標為「未知／其他」而非臆測（FR-015）。
- **原則 IV — 簡潔與低資源佔用**：✅ 事件驅動取代輪詢；UIA 僅於前景切換至瀏覽器時查詢一次（非持續輪詢）；相依套件逐項說明必要性（見下與 research.md）；非 Rust 執行期元件列入 Complexity Tracking。
- **原則 V — 規格驅動開發**：✅ 本計畫逐項對應已核可規格（FR-001…FR-019、SC-001…SC-007），未超出範圍。
- **原則 VI — Rust 實作語言**：✅ 追蹤核心、平台整合、資料層、應用後端皆 Rust；UI 採憲章明文允許的 Tauri；唯一非 Rust 執行期元件（WebView2 內的 TS 前端、SQLCipher 之 C 函式庫）已於 Complexity Tracking 說明必要性與被否決之 Rust 替代方案。

**結論**：通過。非 Rust 執行期元件已具正當理由（見 Complexity Tracking），不構成未經說明之違反。

## Project Structure

### Documentation (this feature)

```text
specs/001-activity-tracker/
├── plan.md              # 本檔（/speckit-plan 輸出）
├── research.md          # Phase 0 輸出
├── data-model.md        # Phase 1 輸出
├── quickstart.md        # Phase 1 輸出
├── contracts/           # Phase 1 輸出
│   ├── tauri-commands.md   # 前端↔Rust 後端 IPC 命令契約
│   └── export-formats.md   # CSV / JSON 匯出格式契約
└── tasks.md             # Phase 2 輸出（由 /speckit-tasks 產生，非本指令）
```

### Source Code (repository root)

```text
Cargo.toml                      # Cargo workspace 根
crates/
├── tracker-core/               # 純 Rust 領域邏輯（無 OS 相依，完全可單元測試）
│   ├── src/
│   │   ├── session.rs          # 工作階段狀態機（active/idle、開合、午夜切分）
│   │   ├── aggregation.rs      # 每日／區間彙總、排序
│   │   ├── hostname.rs         # 由 URL 取完整主機名
│   │   ├── rules.rs            # 5 秒最短門檻、閒置門檻、排除規則
│   │   └── model.rs            # 領域型別（Application、Website、Session…）
│   └── tests/
├── tracker-platform/           # Windows 原廠整合（windows crate）
│   ├── src/
│   │   ├── foreground.rs       # SetWinEventHook + EVENT_SYSTEM_FOREGROUND
│   │   ├── idle.rs             # GetLastInputInfo
│   │   ├── session_power.rs    # WTS 鎖定/解鎖、WM_POWERBROADCAST 睡眠/喚醒
│   │   ├── browser_url.rs      # UI Automation 讀取網址列 → 主機名
│   │   ├── process.rs          # QueryFullProcessImageNameW → 應用名稱
│   │   ├── autostart.rs        # HKCU Run 登入自啟
│   │   └── secret.rs           # DPAPI 保護/解保護金鑰
│   └── tests/
├── tracker-storage/            # 加密 SQLite（SQLCipher）+ 匯出
│   ├── src/
│   │   ├── db.rs               # 連線、金鑰、PRAGMA、遷移
│   │   ├── repository.rs       # 寫入工作階段、查詢摘要
│   │   ├── crypto.rs           # DEK 產生、Argon2id、DPAPI 包裹、zeroize
│   │   └── export.rs           # CSV / JSON 匯出
│   └── tests/
src-tauri/                      # Tauri 2 應用（後端膠合、系統匣、命令）
├── src/
│   ├── main.rs                 # 啟動、tray、單例、自啟、背景追蹤執行緒
│   ├── commands.rs             # #[tauri::command]（對應 contracts/tauri-commands.md）
│   └── tracking_service.rs     # 串接 platform → core → storage 的常駐迴圈
├── Cargo.toml
├── tauri.conf.json
└── icons/
ui/                             # 前端（TypeScript，zh-TW 介面）
├── index.html
├── src/
│   ├── views/                  # 今日、歷史、區間、設定
│   ├── components/             # 清單、長條/圓餅圖、狀態列
│   └── i18n/zh-TW.ts
└── package.json
```

**Structure Decision**：採 **Cargo workspace 多 crate**，將「純領域邏輯（tracker-core）」與「Windows 平台整合（tracker-platform）」「加密儲存（tracker-storage）」分離，使核心規則可在無 Windows 環境下單元測試（呼應原則 III 之可驗證量測定義與測試需求）。`src-tauri` 僅做膠合與常駐迴圈；`ui` 提供 zh-TW 介面。**全部以單一使用者工作階段程序執行**（登入時自啟、最小化至系統匣），**不採 Windows 服務**——理由見 research.md（Session 0 隔離無法存取前景視窗與 UIA）。

## Complexity Tracking

> 僅列出需說明正當理由之非 Rust 執行期元件（憲章允許 Tauri，故非「違反」，惟為透明起見記錄）。

| 元件 | 為何需要 | 被否決的較簡單／純 Rust 替代方案及原因 |
|------|----------|----------------------------------------|
| WebView2 內之 TypeScript 前端（Tauri） | 提供豐富的儀表板（清單、圖表、日期選擇、zh-TW i18n），開發成本最低；WebView2 為 Windows 11 原廠元件、共用且輕量 | 否決 `egui`/`slint` 純 Rust GUI：圖表、表格、在地化與排版的成熟度與開發效率較差；憲章已明文允許 Tauri |
| SQLCipher（C 函式庫，經 rusqlite 連結） | 業界標準、經審計之 SQLite 全資料庫 AES‑256 透明加密，最契合「最安全的加密方式」 | 否決「純 Rust KV（redb/sled）+ 自寫頁面加密」：透明全庫加密成熟度不足、易自製出錯（違反「最安全」）；否決 Windows EFS：**Windows 11 Home 不支援 EFS** |

## 階段產出對照

| 規格條目 | 對應設計位置 |
|----------|--------------|
| FR-001 前景偵測 | tracker-platform/foreground.rs（SetWinEventHook）|
| FR-002 登入自啟、背景常駐 | tracker-platform/autostart.rs、src-tauri（tray + 背景執行緒）|
| FR-003 閒置排除 | tracker-platform/idle.rs + tracker-core/rules.rs |
| FR-004 睡眠/鎖定不計 | tracker-platform/session_power.rs |
| FR-005 零設定網站（完整主機名） | tracker-platform/browser_url.rs（UIA）+ tracker-core/hostname.rs |
| FR-006 每日彙總、午夜切分 | tracker-core/session.rs、aggregation.rs |
| FR-007/008/009 摘要與歷史 | tracker-storage/repository.rs、contracts/tauri-commands.md |
| FR-010 持久化、≤1 分鐘損失 | tracker-storage/db.rs（WAL、定期 flush）|
| FR-011/016 本機加密 | tracker-storage/crypto.rs（SQLCipher + DPAPI + Argon2id）|
| FR-012 暫停/恢復、狀態 | src-tauri/tracking_service.rs、commands.rs |
| FR-013 排除規則 | tracker-core/rules.rs、tracker-storage（exclusion 表）|
| FR-014 無限期保留 + 手動清除 | tracker-storage/repository.rs |
| FR-015 易讀時間、未知/其他 | tracker-core/model.rs、ui 格式化 |
| FR-017 CSV/JSON 匯出 | tracker-storage/export.rs、contracts/export-formats.md |
| FR-018 zh-TW 介面 | ui/i18n/zh-TW.ts |
| FR-019 5 秒最短門檻 | tracker-core/rules.rs |

## Post-Design Constitution Re-Check

Phase 1 設計完成後重新檢查：

- **I**：所有產出（research/data-model/quickstart/contracts）皆 zh-TW。✅
- **II**：資料模型無任何外傳欄位；contracts 無網路端點；匯出與刪除命令齊備。✅
- **III**：data-model 明確定義 session 的 active/idle 狀態與午夜切分規則；無臆測填補。✅
- **IV**：相依套件數量受控、皆有用途；UIA 僅於前景切換至瀏覽器時觸發。✅
- **V**：contracts 與 data-model 僅覆蓋規格範圍內項目。✅
- **VI**：所有後端邏輯 Rust；非 Rust 元件已於 Complexity Tracking 說明。✅

**Gate 狀態**：通過，可進入 `/speckit-tasks`。

## 下一步

執行 **`/speckit-tasks`** 依本計畫與設計產出可執行、相依排序的 `tasks.md`。
