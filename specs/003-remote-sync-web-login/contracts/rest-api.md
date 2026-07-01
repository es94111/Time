# REST API 契約：遠端伺服器（Windows 用戶端與網頁共用）

> 本檔以正體中文（zh-TW）撰寫（憲章原則 I）。所有端點皆為 `https://<server>/api/v1/...`，皆需 TLS（原則 II、FR-011）。除 `POST /auth/login` 外，其餘端點皆需有效 Session（Cookie 或 `Authorization: Bearer <session-id>`，供 Windows 用戶端使用）。

## 認證

### `POST /auth/login`

請求：`{ "identifier": string, "password": string, "client": "web" | "desktop", "device_fingerprint"?: string, "device_name"?: string }`

- `client = "desktop"` 時 MUST 提供 `device_fingerprint`；伺服器依 R6/FR-007 規則比對或建立 `device`。
- 成功：`200 { "session_id": string, "expires_at": string }`；`web` 端另以 `Set-Cookie` 設定 HttpOnly Session Cookie。
- 帳號鎖定中：`423 Locked { "retry_after_seconds": number }`（FR-010）。
- 帳密錯誤：`401 Unauthorized`，並使該帳號 `failed_login_count += 1`，達 10 次觸發鎖定。

### `POST /auth/logout`

撤銷目前 Session（`revoked_at = now()`）。`204 No Content`（FR-008）。

### `POST /auth/change-password`

請求：`{ "current_password": string, "new_password": string }`（`new_password` 長度 MUST ≥ 8）。成功後撤銷該帳號**所有其他** Session（FR-009），回傳 `200`。

### `POST /auth/account` （註冊，細節見 spec Assumptions，非本規格重點）

## 同步（Windows 用戶端使用）

### `POST /sync/batches`

Header：需 `kind=desktop` 之有效 Session。

請求（multipart 或 JSON+gzip，依 payload 大小，實作時擇一並於此更新）：
```json
{
  "dedup_key": "string（裝置指紋+本機序號，保證同批次重試冪等）",
  "device_local_time_range": ["2026-07-01T00:00:00Z", "2026-07-01T00:01:00Z"],
  "record_count": 123,
  "payload": "gzip(JSON) base64 或 multipart 檔案欄位"
}
```

- 單批 MUST ≤ 500 筆或 ≤ 5MB（FR-004）；超過由用戶端切分。
- 冪等：`dedup_key` 已存在時直接回傳先前結果（`200`），不得重複寫入物件儲存。
- 回應：`201 { "sync_record_id": string, "server_received_at": string }`。
- 裝置已被移除（`device.removed_at IS NOT NULL`）：`403 Forbidden { "reason": "device_removed" }`，用戶端 MUST 停止重試並提示重新登入（Edge Case）。
- 帳號已停用/刪除：`403 Forbidden { "reason": "account_disabled" }`。

## 裝置管理（網頁使用）

### `GET /devices` — 回傳該帳號（或分享範圍內）裝置清單：`[{ id, display_name, last_sync_at, removed: false }]`

### `DELETE /devices/{id}` — 標記 `removed_at = now()`；`204`（FR-007）

## 資料查詢（網頁使用）

### `GET /data?device_id={id|"all"}&from=&to=`

- 未帶 `device_id` 或帶 `"all"`：回傳彙總所有裝置（含被分享者依 `scope=all_devices` 存取時）。
- 回應資料一律以 `server_received_at` 排序（原則 III、FR 時間戳記需求）。
- 被分享者（`share_grant.grantee_account_id = 當前使用者`）僅能查詢 `owner` 之資料，且僅唯讀，不含此清單以外的管理端點存取權。

## 分享（網頁使用）

### `POST /shares` — `{ "grantee_identifier": string, "scope": "single_device:<id>" | "all_devices" }` → `201`

### `DELETE /shares/{id}` — 撤銷（`revoked_at = now()`）→ `204`

## 帳號與保留設定（網頁使用）

### `PATCH /account/settings` — `{ "retention_days"?: number }`

### `DELETE /account` — 軟刪除帳號（`deleted_at = now()`），30 天後由背景排程永久刪除（FR-012）
