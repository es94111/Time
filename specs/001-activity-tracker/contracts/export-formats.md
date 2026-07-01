# 契約：匯出格式（CSV / JSON）

**日期**：2026-06-30 ｜ **語言**：zh-TW ｜ 對應 FR-017、`export_data` 命令

支援兩種匯出格式。匯出為純本機檔案寫出，無外部傳輸（原則 II）。所有時間欄位以 ISO‑8601（本機時區含位移）呈現以利人讀；另附 epoch ms 供程式使用。文字編碼 **UTF‑8（含 BOM 以利 Excel 正確顯示中文）**。

---

## 1. CSV 格式

### 1a. 工作階段明細 `sessions.csv`
標頭（繁體中文）：

```csv
本機日期,開始時間,結束時間,應用程式,網站,活躍秒數
2026-06-30,2026-06-30T09:15:03+08:00,2026-06-30T09:48:21+08:00,Visual Studio Code,,1998
2026-06-30,2026-06-30T09:48:21+08:00,2026-06-30T10:05:00+08:00,Google Chrome,github.com,1000
```
- 「網站」欄位於非瀏覽器或未辨識時留空。
- 一列一工作階段；跨午夜者已切分為多列。

### 1b. 每日彙總 `daily_summary.csv`
```csv
本機日期,類型,名稱,活躍秒數
2026-06-30,應用程式,Visual Studio Code,6300
2026-06-30,網站,github.com,1800
```

---

## 2. JSON 格式 `export.json`

```jsonc
{
  "schema": "activity-tracker/export@1",
  "exported_at": "2026-06-30T23:10:00+08:00",
  "timezone": "Asia/Taipei",
  "range": { "from": "2026-06-24", "to": "2026-06-30" },
  "sessions": [
    {
      "local_date": "2026-06-30",
      "application": "Visual Studio Code",
      "executable": "code.exe",
      "website": null,
      "start_utc_ms": 1782800103000,
      "end_utc_ms": 1782802101000,
      "active_ms": 1998000
    }
  ],
  "daily_summary": [
    {
      "local_date": "2026-06-30",
      "total_active_ms": 14520000,
      "apps": [ { "display_name": "Visual Studio Code", "active_ms": 6300000 } ],
      "websites": [ { "hostname": "github.com", "active_ms": 1800000 } ]
    }
  ]
}
```

- `schema` 欄位帶版本，未來格式演進時遞增。
- `website` 在非瀏覽器或未辨識時為 `null`。

---

## 契約測試要點
- CSV 以 UTF‑8 BOM 輸出，中文標頭與名稱在 Excel 正確顯示。
- JSON 通過 `serde` 反序列化；`active_ms` 之和與 `daily_summary.total_active_ms` 一致。
- 同一份資料以 CSV 與 JSON 匯出，工作階段列數一致。
- 範圍匯出僅含 `from`～`to`（含端點）之本機日期。
