# 契約：Tauri IPC 命令（電腦資訊監控紀錄）

**日期**：2026-07-01 ｜ **語言**：zh-TW ｜ **範圍**：前端（WebView2/TS）↔ Rust 後端（`src-tauri`）之 `#[tauri::command]` 契約。

沿用 001 的命令風格：全部本機、無網路端點（原則 II）。時間以 epoch ms（UTC）傳遞；速率為 bytes/sec；記憶體／VRAM 為 bytes；缺值以 `null` 表示。以下型別以 TypeScript 介面描述對應之 `serde` 序列化結果。

---

## 共用型別

```typescript
// 單一裝置的即時／某時點數值
interface DiskReading   { id: number; name: string | null; readBps: number | null;  writeBps: number | null; }
interface NetReading    { id: number; name: string | null; rxBps:  number | null;  txBps:   number | null; }
interface GpuReading    { id: number; name: string | null; utilPct: number | null;
                          memUsedBytes: number | null; memTotalBytes: number | null; }

// 某一取樣時點的整機快照
interface MetricSnapshot {
  tsUtc: number;                 // epoch ms
  cpuPct: number | null;
  memUsedBytes: number | null;
  memTotalBytes: number | null;
  disks: DiskReading[];
  nets:  NetReading[];
  gpus:  GpuReading[];           // 空陣列＝無 GPU（US2：呈現「不適用」）
}
```

---

## 1. `get_current_metrics` — 即時檢視（US2 / FR-009）

取得最近一次取樣的整機快照，供即時檢視畫面顯示。

- **輸入**：無
- **輸出**：`MetricSnapshot | null`（尚無任何樣本時為 `null`）
- **錯誤**：後端無法取得時回傳 `Err(string)`（zh-TW 訊息）
- **對應**：FR-009、SC-003（變化後 3 秒內反映）

```typescript
invoke<MetricSnapshot | null>('get_current_metrics')
```

---

## 2. `get_metrics_range` — 歷史區間查詢／趨勢（US3 / FR-010、FR-014）

依時間區間回傳趨勢資料，長區間自動降取樣（R10）。

- **輸入**：
  ```typescript
  interface RangeQuery {
    fromUtc: number;             // epoch ms（含）
    toUtc: number;               // epoch ms（含），須 > fromUtc
    metrics?: string[];          // 可選：篩選 ['cpu','mem','disk','net','gpu']；預設全部
  }
  ```
  > 註：`metrics` 為 UI 便利性參數，非規格（FR）所要求；後端得忽略或全支援，前端不傳即代表全部。
- **輸出**：
  ```typescript
  interface TrendSeries {
    bucketMs: number;            // 實際採用的桶粒度（0＝原始逐筆）
    points: TrendPoint[];        // 依時間遞增；缺值桶不產生點（呈現為空缺）
  }
  interface TrendPoint {
    tsUtc: number;               // 桶起點（或原始樣本時間）
    cpuAvg: number | null; cpuMax: number | null;
    memUsedAvg: number | null; memTotalBytes: number | null;
    disks: { id: number; name: string | null; readAvg: number | null; readMax: number | null;
             writeAvg: number | null; writeMax: number | null; }[];
    nets:  { id: number; name: string | null; rxAvg: number | null; rxMax: number | null;
             txAvg: number | null; txMax: number | null; }[];
    gpus:  { id: number; name: string | null; utilAvg: number | null; utilMax: number | null;
             memUsedAvg: number | null; memTotalBytes: number | null; }[];
  }
  ```
- **行為**：桶粒度依 `toUtc − fromUtc` 自動決定（≤2h 原始；≤2d 分鐘；≤2mo 小時；更長日）。空缺區間（睡眠／未執行）不產生點（US3 邊界，不內插）。
- **錯誤**：`toUtc ≤ fromUtc` 或區間非法 → `Err(string)`。
- **對應**：FR-010、FR-014、US3。

```typescript
invoke<TrendSeries>('get_metrics_range', { query })
```

---

## 3. `list_metric_devices` — 裝置清單（含已移除）

列出歷史中出現過的所有硬碟／網路介面／GPU（供篩選與圖例）。

- **輸入**：無
- **輸出**：
  ```typescript
  interface DeviceLists {
    disks: { id: number; name: string | null }[];
    nets:  { id: number; name: string | null }[];
    gpus:  { id: number; name: string | null }[];
  }
  ```
- **對應**：FR-008（含已移除裝置之歷史）、SC-004（逐裝置可辨識）。

---

## 4. `get_metrics_settings` / `set_metrics_settings` — 取樣與保留設定

- **`get_metrics_settings`**
  - 輸出：
    ```typescript
    interface MetricsSettings {
      sampleIntervalSec: number;   // 目前取樣週期
      retentionDays: number;       // 目前保留天數
      enabled: boolean;
    }
    ```
- **`set_metrics_settings`**
  - 輸入：`Partial<MetricsSettings>`（僅傳要修改的欄位）
  - **驗證**（FR-011 / FR-012）：
    - `sampleIntervalSec`：整數，`1 ≤ 值 ≤ 3600`；超出 → `Err('取樣秒數須介於 1 至 3600 秒')`
    - `retentionDays`：整數，`1 ≤ 值 ≤ 3650`；超出 → `Err('保留天數須介於 1 至 3650 天')`
  - **行為**：變更取樣週期後於下一取樣週期生效；變更保留天數後於下次清理生效（並可立即觸發一次清理）；`enabled` 切為 `false` 時暫停背景取樣與寫入、切回 `true` 於下一取樣週期恢復（FR-015），既有歷史不受影響。
  - 輸出：更新後的 `MetricsSettings`

---

## 5. `clear_metrics_data` — 清除指標紀錄（資料可刪除，憲章技術約束）

刪除全部（或指定區間）指標樣本，供使用者徹底刪除自身資料。

- **輸入**：`{ beforeUtc?: number }`（省略＝全部刪除；提供＝刪除該時點前）
- **輸出**：`{ deletedSamples: number }`
- **對應**：憲章「資料可攜與刪除」；與 001 `clear_data` 一致風格。

---

## 事件（後端 → 前端推播，選用）

即時檢視畫面可訂閱後端於每次取樣後發出的事件，避免前端輪詢：

- 事件名：`metrics://sample`
- 酬載：`MetricSnapshot`
- 用途：SC-003 之「3 秒內反映」；前端亦可退回定期 `get_current_metrics` 輪詢。
