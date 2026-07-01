# 契約：Tauri IPC 命令（前端 ↔ Rust 後端）

**日期**：2026-06-30 ｜ **語言**：zh-TW

本檔定義前端（WebView2/TS）呼叫 Rust 後端之 `#[tauri::command]` 介面。此為本桌面應用對「外部」（其前端）暴露的唯一介面，無任何網路端點（呼應原則 II 純本機）。時間欄位以 epoch ms（UTC）或 `YYYY-MM-DD`（本機日期）表示。所有命令為本機程序內呼叫，失敗時回傳結構化錯誤 `{ code, message_zh }`。

---

## 查詢類

### `get_tracking_status() -> TrackingStatus`
- 回傳 `{ paused: bool, current_app?: string, current_website?: string, since_utc?: int }`（FR-012）。

### `get_today_summary() -> DaySummary`
- 回傳今日（本機日）摘要（FR-007、FR-008）。
```jsonc
// DaySummary
{
  "local_date": "2026-06-30",
  "total_active_ms": 14520000,
  "apps":     [ { "display_name": "Visual Studio Code", "active_ms": 6300000 } ],   // 依 active_ms 由大到小
  "websites": [ { "hostname": "github.com", "active_ms": 1800000 } ]                 // 依 active_ms 由大到小
}
```

### `get_summary_by_date(date: string) -> DaySummary`
- 參數 `date`：`YYYY-MM-DD`（本機日）。回傳該日摘要（FR-009）。

### `get_summary_range(from: string, to: string) -> RangeSummary`
- 參數為含端點之本機日期範圍。回傳區間彙總（FR-009）。
```jsonc
{ "from": "2026-06-24", "to": "2026-06-30", "total_active_ms": 0, "apps": [], "websites": [] }
```

## 控制類

### `pause_tracking() -> TrackingStatus` ／ `resume_tracking() -> TrackingStatus`
- 暫停／恢復追蹤（FR-012）。暫停時結算目前段並停止累積。

### `get_settings() -> Settings` ／ `update_settings(patch: SettingsPatch) -> Settings`
- 讀取／更新設定：`idle_threshold_sec`、`autostart_enabled`、`ui_language`（FR-002、FR-003、FR-018）。
- 變更 `autostart_enabled` 會同步登錄 HKCU Run 項目。

## 排除類（FR-013）

### `list_exclusions() -> Exclusion[]`
### `add_exclusion(kind: "app"|"website", pattern: string) -> Exclusion`
### `remove_exclusion(id: int) -> void`

## 資料管理類

### `export_data(opts: ExportOptions) -> ExportResult`
- `ExportOptions = { format: "csv"|"json", scope: "all"|"range", from?: string, to?: string, target_path: string }`（FR-017）。
- 依 `contracts/export-formats.md` 之結構輸出至本機路徑；回傳 `{ written_path, row_count }`。

### `clear_data(scope: "all"|"range", from?: string, to?: string) -> { deleted_rows: int }`
- 手動清除資料（FR-014、憲章「資料可攜與刪除」）。`all` 須前端二次確認。

### `set_master_password(password: string) -> void` ／ `unlock(password: string) -> bool`
- 可選的 Argon2id 主密碼強化（research.md R7）。未啟用時不需 `unlock`（DPAPI 自動解金鑰）。

---

## 錯誤模型

```jsonc
{ "code": "DB_LOCKED" | "INVALID_DATE" | "EXPORT_IO" | "BAD_PASSWORD" | "INTERNAL",
  "message_zh": "可顯示給使用者的繁體中文訊息" }
```

## 契約測試要點

- `get_today_summary`：清單須依 `active_ms` 由大到小；`total_active_ms` = 各 app 之和。
- `get_summary_range`：跨午夜的工作階段須正確分屬各日（FR-006）。
- `add_exclusion(app)`：之後該 app 不再產生工作階段。
- `export_data(csv|json)`：輸出符合 export-formats.md，列數與查詢一致。
- `pause_tracking`：暫停後 `get_tracking_status().paused == true` 且不再累積。
