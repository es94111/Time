# Phase 1 資料模型：Windows 11 活動追蹤器

**日期**：2026-06-30 ｜ **語言**：zh-TW ｜ **儲存**：SQLite（SQLCipher，AES‑256）於 `%APPDATA%\ActivityTracker\activity.db`

本檔由規格「關鍵實體」與功能需求導出領域型別、驗證規則、狀態轉換與 SQL 結構。所有時間以 **UTC 瞬時**儲存，呈現時換算本機時區（見 research.md R6）。

---

## 1. 領域實體

### Application（應用程式）
| 欄位 | 型別 | 說明／驗證 |
|------|------|------------|
| id | i64（PK） | 內部識別 |
| display_name | text | 易讀名稱（如「Visual Studio Code」）；不可空 |
| executable | text | 執行檔名（如 `code.exe`），作為穩定識別鍵；不可空、唯一 |
| is_browser | bool | 是否為已知瀏覽器（決定是否做網站偵測）|
| excluded | bool | 是否被使用者排除追蹤（FR-013）|

- **未知應用**：無法判定執行檔者，歸入保留紀錄 `executable = "<unknown>"`、`display_name = "未知／其他"`（FR-015）。

### Website（網站）
| 欄位 | 型別 | 說明／驗證 |
|------|------|------------|
| id | i64（PK） | 內部識別 |
| hostname | text | **完整主機名**（如 `mail.google.com`）；子網域分開；不可空、唯一 |
| excluded | bool | 是否被排除追蹤（FR-013）|

- 主機名一律小寫正規化；去除 port 與使用者資訊。

### ActivitySession（活動工作階段）
| 欄位 | 型別 | 說明／驗證 |
|------|------|------------|
| id | i64（PK） | 內部識別 |
| application_id | i64（FK→Application） | 不可空 |
| website_id | i64（FK→Website，nullable） | 僅瀏覽器且成功辨識主機名時有值 |
| start_utc | int（epoch ms） | 起始瞬時；不可空 |
| end_utc | int（epoch ms） | 結束瞬時；`end_utc > start_utc` |
| active_ms | int | 該段**活躍**毫秒數（已扣除回溯閒置）；`0 ≤ active_ms ≤ end-start` |
| local_date | text（YYYY-MM-DD） | 該段所屬之**本機日期**（午夜切分後每段僅屬一日，FR-006）|

- **最短門檻（FR-019）**：`active_ms < 5000` 的前景停留不建立工作階段。
- **午夜切分（FR-006）**：跨本機午夜的停留切成多筆，各筆 `local_date` 不同。

### ExclusionRule（排除規則）
| 欄位 | 型別 | 說明 |
|------|------|------|
| id | i64（PK） | |
| kind | text | `app` 或 `website` |
| pattern | text | 執行檔名或主機名 |

### Setting（設定）
| 鍵 | 預設 | 說明 |
|----|------|------|
| idle_threshold_sec | 300 | 閒置門檻秒數（FR-003，可調）|
| tracking_paused | false | 是否暫停追蹤（FR-012）|
| autostart_enabled | true | 登入自啟（FR-002）|
| master_password_set | false | 是否啟用 Argon2id 主密碼（research.md R7）|
| ui_language | zh-TW | 介面語言（FR-018）|

### DailySummary（每日摘要，衍生非儲存）
由 `ActivitySession` 依 `local_date` 彙總而得：總活躍時間、各應用時間、各網站時間（皆排序）。用於 FR-007/008/009 之檢視。

---

## 2. 工作階段狀態轉換（呼應原則 III 之明確定義）

```text
[無作用中工作階段]
   │  前景切換到 App X（且非排除、非無痕逐站情境）
   ▼
[作用中：App X (、Website H)]  ── 焦點維持、有輸入 ──▶ 累積 active_ms
   │  ① 前景切換到其他 App/網站
   │  ② 閒置超過門檻（回溯扣除門檻後的閒置）
   │  ③ 系統睡眠 / 鎖定
   │  ④ 跨越本機午夜
   ▼
[結算並關閉目前段] → 寫入 DB（≥5 秒才保留）
   │  ④ 午夜：於午夜後立即開新段（同 App/Website）
   ▼
[視情況開新段或回到無作用中]
```

- **作用中（active）**：目前前景 App 取得焦點，且距最後輸入未超過閒置門檻。
- **閒置（idle）**：距最後輸入超過門檻；該門檻起算之後不計 active。
- **未知**：無法辨識前景 App → 計入「未知／其他」；無法辨識網站 → 僅計 App 時間、`website_id = NULL`。

---

## 3. SQL 結構（SQLCipher）

```sql
-- 連線後立即套用金鑰與安全 PRAGMA（見 tracker-storage/db.rs）
-- PRAGMA key = "x'<256-bit hex>'";  -- 來自 DPAPI 解保護之 DEK
PRAGMA journal_mode = WAL;          -- 降低當機資料損失（FR-010, SC-005）
PRAGMA foreign_keys = ON;

CREATE TABLE application (
  id           INTEGER PRIMARY KEY,
  display_name TEXT NOT NULL,
  executable   TEXT NOT NULL UNIQUE,
  is_browser   INTEGER NOT NULL DEFAULT 0,
  excluded     INTEGER NOT NULL DEFAULT 0
);

CREATE TABLE website (
  id        INTEGER PRIMARY KEY,
  hostname  TEXT NOT NULL UNIQUE,
  excluded  INTEGER NOT NULL DEFAULT 0
);

CREATE TABLE session (
  id             INTEGER PRIMARY KEY,
  application_id INTEGER NOT NULL REFERENCES application(id),
  website_id     INTEGER REFERENCES website(id),
  start_utc      INTEGER NOT NULL,
  end_utc        INTEGER NOT NULL,
  active_ms      INTEGER NOT NULL,
  local_date     TEXT NOT NULL,            -- 'YYYY-MM-DD'
  CHECK (end_utc > start_utc),
  CHECK (active_ms >= 0 AND active_ms <= end_utc - start_utc)
);
CREATE INDEX idx_session_date        ON session(local_date);
CREATE INDEX idx_session_app_date    ON session(application_id, local_date);
CREATE INDEX idx_session_site_date   ON session(website_id, local_date);

CREATE TABLE exclusion (
  id      INTEGER PRIMARY KEY,
  kind    TEXT NOT NULL CHECK (kind IN ('app','website')),
  pattern TEXT NOT NULL,
  UNIQUE(kind, pattern)
);

CREATE TABLE setting (
  key   TEXT PRIMARY KEY,
  value TEXT NOT NULL
);
```

- **索引**讓「今日／某日／區間」彙總查詢於資料量大時仍 < 3 秒（SC-002）。
- **WAL** 搭配定期 checkpoint，使當機損失 ≤ 最近 1 分鐘（SC-005）；常駐迴圈每 ≤ 60 秒 flush 一次目前段的進度。

---

## 4. 對應規格之驗證

| 規格 | 資料模型對應 |
|------|--------------|
| FR-005 完整主機名 | `website.hostname` 唯一、子網域分開 |
| FR-006 午夜切分／每日彙總 | `session.local_date` + 切分規則 + 日期索引 |
| FR-013 排除 | `application.excluded`/`website.excluded` + `exclusion` 表 |
| FR-014 無限期保留 + 手動清除 | 無自動刪除；提供 `clear_data` 命令 |
| FR-015 未知／其他 | 保留 `<unknown>` application 列 |
| FR-016 加密 | SQLCipher 金鑰 + DPAPI（見 contracts 與 research R7）|
| FR-019 5 秒門檻 | 寫入前 `active_ms ≥ 5000` 檢查 |
