# Phase 1 資料模型：遠端資料同步與網頁儀表板（含登入驗證）

> 本檔以正體中文（zh-TW）撰寫（憲章原則 I）。實體對應 spec.md 的「Key Entities」。伺服器端結構化資料存於 PostgreSQL；資料快照本體存於 S3 相容物件儲存，PostgreSQL 僅存指標中繼資料。

## 伺服器端（PostgreSQL）

### account（使用者帳號）

| 欄位 | 型別 | 說明 |
|------|------|------|
| id | UUID PK | |
| email_or_username | TEXT UNIQUE | 登入識別（沿用規格「帳號」概念）|
| password_hash | TEXT | Argon2id 雜湊，MUST NOT 明文（FR-011）|
| failed_login_count | INT DEFAULT 0 | 連續失敗次數（FR-010）|
| locked_until | TIMESTAMPTZ NULL | 鎖定至此時間前拒絕登入 |
| retention_days | INT NULL | 使用者自訂保留天數；NULL 時採系統預設（FR-013）|
| deleted_at | TIMESTAMPTZ NULL | 軟刪除時間；30 天後由 retention_cleanup 永久刪除（FR-012）|
| created_at | TIMESTAMPTZ | |

**驗證規則**：`password_hash` 對應明文密碼長度 MUST ≥ 8 字元（僅於註冊/改密時驗證，不存明文）。

### device（裝置）

| 欄位 | 型別 | 說明 |
|------|------|------|
| id | UUID PK | |
| account_id | UUID FK → account | |
| hardware_fingerprint | TEXT | 來自 Windows `MachineGuid`（見 research.md R6）|
| display_name | TEXT | 裝置名稱（可由使用者編輯）|
| last_sync_at | TIMESTAMPTZ NULL | 最後成功同步時間 |
| removed_at | TIMESTAMPTZ NULL | 使用者手動移除時間；非 NULL 時拒絕該裝置後續上傳（FR-007）|
| created_at | TIMESTAMPTZ | |

**唯一鍵**：`(account_id, hardware_fingerprint)` — 用於重裝後合併回原裝置紀錄；若原裝置已被 `removed_at` 標記移除，視為全新裝置（新增一筆而非合併，對應 Edge Case）。

**狀態轉換**：`active`（`removed_at IS NULL`）→ `removed`（使用者手動移除）。移除為終態，不可復原為 active；同硬體指紋若移除後重新登入，建立新的 device 列。

### session（工作階段）

| 欄位 | 型別 | 說明 |
|------|------|------|
| id | TEXT PK | opaque session id（存於 Cookie 或用戶端本機快取）|
| account_id | UUID FK → account | |
| kind | TEXT | `web` \| `desktop`（區分兩種逾時策略）|
| device_id | UUID NULL FK → device | 僅 `desktop` 適用 |
| created_at | TIMESTAMPTZ | |
| last_active_at | TIMESTAMPTZ | 每次操作更新（web 滑動視窗依此計算）|
| expires_at | TIMESTAMPTZ | `web`：`last_active_at + 24h`（每次操作重算）；`desktop`：`created_at + 30d`（固定）|
| revoked_at | TIMESTAMPTZ NULL | 登出或變更密碼時設定，設定後立即視為失效（FR-008、FR-009）|

**驗證規則**：任何 API 存取前 MUST 檢查 `revoked_at IS NULL AND now() < expires_at`；`web` 類型每次通過驗證後 MUST 更新 `last_active_at` 與重算 `expires_at`（滑動視窗，FR-014）。變更密碼時 MUST 將該帳號下所有其他 session 之 `revoked_at` 設為 now()（FR-009）。

### sync_record（同步紀錄）

| 欄位 | 型別 | 說明 |
|------|------|------|
| id | UUID PK | |
| device_id | UUID FK → device | |
| batch_dedup_key | TEXT UNIQUE | 用戶端產生之批次去重鍵（裝置ID+本機序號），防止補傳重複寫入 |
| device_local_time_range | TSTZRANGE | 裝置本機時間範圍（輔助參考欄位，見 FR 時間戳記需求）|
| server_received_at | TIMESTAMPTZ | 伺服器接收時間，MUST 作為排序與網頁時間軸顯示依據 |
| status | TEXT | `pending`（罕見，僅上傳中）\| `completed` \| `failed` |
| object_storage_key | TEXT NULL | 指向 S3 相容物件儲存中的資料快照物件路徑 |
| record_count | INT | 本批筆數（供補傳完整性核對，SC-003 零遺漏零重複）|

**驗證規則**：`batch_dedup_key` 唯一鍵確保同一批次重試上傳不會重複寫入（呼應 FR-004「不遺漏亦不重複」）。

### remote_data_snapshot（遠端資料快照，邏輯實體）

實際內容存於物件儲存（見「物件儲存」小節），此處僅為概念實體：由 `sync_record.object_storage_key` 指向的一組序列化活動追蹤／系統監控資料，依帳號與裝置分類、依 `account.retention_days`（或系統預設）自動清除逾期物件。

### share_grant（分享授權）

| 欄位 | 型別 | 說明 |
|------|------|------|
| id | UUID PK | |
| owner_account_id | UUID FK → account | 授權來源（資料擁有者）|
| grantee_account_id | UUID FK → account | 被授權帳號 |
| scope | TEXT | `single_device:<device_id>` \| `all_devices` |
| revoked_at | TIMESTAMPTZ NULL | 非 NULL 時授權失效 |
| created_at | TIMESTAMPTZ | |

**驗證規則**：被授權帳號存取資料 API 時僅取得**唯讀**權限，不得呼叫任何修改/分享/裝置管理端點（FR-006）；`owner_account_id` MUST NOT 等於 `grantee_account_id`。

## 物件儲存（S3 相容，鍵值結構）

```
{bucket}/{account_id}/{device_id}/{yyyy}/{mm}/{dd}/{sync_record_id}.json.gz
```

- 內容：該批次的活動追蹤／系統監控原始樣本（沿用 001/002 之領域模型序列化為 JSON，並以 gzip 壓縮）。
- 存取控制：僅伺服器端服務帳號持有物件儲存憑證，用戶端與瀏覽器**不得**直接存取物件儲存，一律經由伺服器 API 中介（授權檢查、分享範圍過濾皆在 API 層完成）。

## 用戶端本機（沿用既有 SQLCipher 資料庫，新增資料表）

### sync_queue（待同步佇列）

| 欄位 | 型別 | 說明 |
|------|------|------|
| id | INTEGER PK | |
| payload | BLOB | 待上傳之序列化資料片段 |
| created_at_local | TEXT | 裝置本機時間（輔助欄位）|
| dedup_key | TEXT UNIQUE | 對應伺服器 `sync_record.batch_dedup_key` 之來源鍵 |
| upload_state | TEXT | `queued` \| `uploading` \| `uploaded` \| `discarded_disk_full` |

**驗證規則**：磁碟空間接近上限時，MUST 依 `created_at_local` 由舊到新捨棄 `queued` 列並標記 `discarded_disk_full`、提示使用者（FR-004）。

### device_identity（裝置身分快取）

| 欄位 | 型別 | 說明 |
|------|------|------|
| hardware_fingerprint | TEXT | 由 `MachineGuid` 取得，開機/登入時讀取一次快取 |
| server_device_id | TEXT NULL | 伺服器回傳的 device.id，登入成功後寫入 |

### auth_cache（登入憑證快取）

| 欄位 | 型別 | 說明 |
|------|------|------|
| refresh_token | BLOB | 以 DPAPI 保護後存放（沿用 001 `tracker-storage::crypto`）|
| account_hint | TEXT | 顯示用帳號名稱（非密碼）|
| desktop_session_expires_at | TEXT | 對應伺服器 `session.expires_at`（`kind='desktop'`）|
