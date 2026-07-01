# Tauri IPC 命令契約（新增）：登入與同步狀態

> 本檔以正體中文（zh-TW）撰寫（憲章原則 I）。延伸既有 001/002 之 `contracts/tauri-commands.md` 命令集，新增下列命令於 `src-tauri/src/auth_commands.rs`。

## `login(identifier: string, password: string) -> Result<ConnectionStatus, LoginError>`

- 呼叫伺服器 `POST /auth/login`（`client="desktop"`，附裝置指紋），成功後將 `refresh_token` 以 DPAPI 保護寫入本機 `auth_cache`，並啟動 `tracker-sync::agent` 背景同步迴圈（FR-002）。
- `LoginError`：`InvalidCredentials` | `AccountLocked { retry_after_seconds }` | `NetworkError`。

## `logout() -> Result<(), Error>`

- 呼叫伺服器 `POST /auth/logout`，清除本機 `auth_cache`，停止背景同步迴圈（FR-008）。本機既有活動追蹤／監控功能不受影響（僅停止上傳）。

## `get_connection_status() -> ConnectionStatus`

```ts
type ConnectionStatus = {
  logged_in: boolean;
  account_hint?: string;       // 顯示用帳號名稱
  last_successful_sync_at?: string;
  pending_queue_count: number; // sync_queue 中 upload_state='queued' 筆數
  last_sync_error?: "network" | "device_removed" | "account_disabled" | null;
};
```

供 UI（`ui/src/views/connection-status.ts`）輪詢顯示「已連線／待同步 N 筆／錯誤原因」。

## `get_local_queue_disk_usage() -> { used_bytes: number, discarded_recent: boolean }`

供 UI 於磁碟接近上限並觸發捨棄最舊資料時顯示提示（FR-004 Edge Case）。
