# Quickstart：遠端資料同步與網頁儀表板（含登入驗證）

> 本檔以正體中文（zh-TW）撰寫（憲章原則 I）。

## 前置需求

- Rust 最新穩定版（`rustup update`）
- Docker Desktop（Windows，用於本機啟動伺服器相依服務）
- 既有 001/002 Windows 用戶端開發環境（Tauri 2、WebView2）

## 1. 啟動遠端伺服器（本機開發，Docker）

```powershell
cd server
docker compose up -d        # 啟動 postgres + minio + server
sqlx migrate run --database-url $env:DATABASE_URL   # 首次或有新 migration 時執行
```

`docker-compose.yml` 預設服務：
- `postgres`：帳號/裝置/工作階段等結構化資料
- `minio`：S3 相容物件儲存（本機開發用；正式環境可改指向雲端 S3）
- `server`：Rust（Axum）API + 網頁儀表板，監聽 `:8080`（純 HTTP，正式環境前應加反向代理處理 TLS，見 research.md R9）

## 2. 驗證伺服器

```powershell
curl http://localhost:8080/healthz
```

## 3. 建立測試帳號並登入網頁

於 `http://localhost:8080/login` 以測試帳號登入（帳號建立方式見伺服器 README，非本規格重點）。

## 4. Windows 用戶端登入並驗證同步

1. 建置並啟動既有 Tauri 應用（沿用 001/002 之 `cargo tauri dev`）。
2. 於軟體內登入畫面（`ui/src/views/login.ts`）輸入同一組帳密。
3. 登入成功後確認：
   - UI 顯示「已連線」（`get_connection_status`）。
   - 約 1 分鐘後，網頁儀表板 `GET /data` 可查詢到剛產生的活動追蹤／系統監控資料（SC-002）。

## 5. 驗證離線補傳

1. 登入狀態下中斷網路（或關閉 `server` 容器）。
2. 持續使用電腦數分鐘，確認軟體本機功能正常、`pending_queue_count` 增加。
3. 恢復網路（重啟 `server` 容器），確認佇列依序補傳、`pending_queue_count` 歸零，網頁資料筆數與本機一致，無遺漏無重複（SC-003）。

## 6. 驗證帳號安全機制

- 連續輸入 10 次錯誤密碼，確認第 10 次後回應 `423 Locked` 且 15 分鐘內無法登入（FR-010）。
- 變更密碼後，確認其他既有已登入的瀏覽器分頁於下次操作被要求重新登入（FR-009）。

## 已知限制（MVP 階段）

- MVP 交付範圍（User Story 1＋2＋3）僅實作簡易裝置建立邏輯：每次桌面登入若無相符之 `(account_id, hardware_fingerprint)` 紀錄即新增一筆 `device`。硬體指紋比對合併回原裝置（FR-007，重灌／重置軟體後沿用原裝置身分）之完整規則要待 User Story 4（多裝置檢視與切換）完成後才會生效；在此之前，同一台電腦重灌後重新登入可能於裝置清單中出現重複項目，非缺陷、屬分期交付之預期行為。

## 停止與清理

```powershell
docker compose down -v   # -v 一併移除本機開發用的 postgres/minio volume
```
