# 更新文件與版號

完成功能開發或修正後，依序執行以下步驟確保所有文件保持同步。

> **撰寫 changelog 條目前必讀**：`.specify/memory/changelog-style.md`（Changelog 公開撰寫規格）。本指令的步驟 2 必須完全遵守該規格。

## 步驟 1：判斷版號

根據異動規模決定版號：
- **大版本**（如 4.0）：新增重大模組
- **小版本**（如 3.8）：新增功能或重要改進
- **修正版**（如 3.7.1）：Bug 修正

讀取 `changelog.json` 的 `currentVersion` 確認目前版號，再決定新版號。

## 步驟 2：更新 `changelog.json`

> **重要**：`changelog.json` 是給**一般使用者**看的，不是給開發者看的。完整規格見 `.specify/memory/changelog-style.md`。

### 2.1 結構

1. 將 `currentVersion` 改為新版號
2. 在 `releases` 陣列**最前面**插入新版本紀錄：

```json
{
  "version": "X.Y.Z",
  "date": "YYYY-MM-DD",
  "title": "12–25 字一句話描述本次更新",
  "type": "new | feature | improved | fixed | removed | warning",
  "changes": [
    { "tag": "warning", "text": "升級需注意事項（如有）" },
    { "tag": "new", "text": "新增的功能說明" },
    { "tag": "improved", "text": "改進的功能說明" },
    { "tag": "fixed", "text": "修正的問題說明" }
  ]
}
```

`tag` 可用值：`new`（新增）、`improved`（改進）、`fixed`（修正）、`removed`（移除）、`warning`（升級需注意）

### 2.2 撰寫前必做：技術 → 使用者翻譯

先寫一份「給工程師看」的內部筆記（含完整技術細節，用於 commit message／PR），再對照下表逐條翻譯成「給使用者看」的版本：

| 不可出現 | 範例 | 改寫方向 |
|---|---|---|
| 內部規格代號 | `FR-033`、`T099`、`Round 1 Q5`、`SC-004`、`CT-1` | 直接刪掉 |
| 分支／spec 編號 | `008-frontend-routing` | 改成功能名稱 |
| 內部審查標記 | `Copilot Review v4.18.2`、`CodeQL 警告` | 改成「資安修正」「程式碼審查修正」 |
| API 路徑 | `POST /api/transactions/import` | 改成「匯入交易功能」 |
| 檔名／函式／變數 | `server.js`、`refreshRecFxUi()`、`SERVER_TIME_OFFSET` | 改成它做的事情 |
| 資料表／欄位 | `system_settings`、`token_version`、`recurring.amount` | 改成「設定值」「登入憑證」「金額」 |
| 環境變數 | `TWSE_MAX_CONCURRENCY`、`MTLS_CF_ONLY` | 改成「並發上限」「Cloudflare 模式」 |
| 程式術語 | `BEGIN/COMMIT/ROLLBACK`、`setImmediate`、`partial unique index`、`bcrypt.compare` | 改成「全部成功或全部回復」「背景執行」「重複保護」 |
| 套件版號／CVE 細節 | `resend 6.1.3 → 6.12.2`、`GHSA-…` | 改成「升級寄信套件」「修補已知漏洞」 |
| Schema migration 細節 | `ALTER TABLE … REAL → INTEGER`、`冪等 ALTER` | 用 `warning` tag 提醒「升級時自動轉換，建議先備份」 |
| 內部演算法名稱 | `三層 fallback`、`atomic delete + insert`、`token bucket` | 改成「自動備援」「原子化更新」 |

允許保留：服務／品牌名（Google、Resend、Cloudflare、Passkey、IPinfo…）、通用前端詞（深色模式、響應式、彈窗、圓餅圖）、投資領域用詞（FIFO、ETF、除權息、定期定額、實現損益）、合規標示、有意義的數字（90 天保留、500 筆上限等）。

### 2.3 條目撰寫規則

- **標題**：12–25 字，不含規格代號、分支名、Review 標記。多重點用「+」串接。
- **每條 change**：40–120 字一句完整中文，從使用者角度寫，主詞通常是「使用者可以…」「系統會自動…」「介面新增…」。描述「結果」勝於「實作」。
- **條目順序**：`warning` →`new` → `improved` → `fixed` → `removed`。
- **warning 條目**：必須明確說會發生什麼 + 給出具體建議。範例：「升級時系統會自動執行 Email 正規化；若大小寫混用造成重複帳號，將保留最早建立者並合併其餘資料。建議升級前先備份資料庫。」

### 2.4 提交前驗證

逐項執行：

```bash
# 1. JSON 格式有效
node -e "require('./changelog.json')"

# 2. 沒有禁用字眼殘留（任一指令有輸出代表還沒清乾淨）
grep -n "FR-[0-9]" changelog.json
grep -n "/api/" changelog.json
grep -nE "[a-z]+\.js" changelog.json
grep -n "Round [0-9]" changelog.json
grep -n "Copilot" changelog.json
grep -n "CodeQL" changelog.json
```

### 2.5 自我檢查清單

- [ ] JSON 格式有效（步驟 2.4 第 1 項通過）
- [ ] `currentVersion` 與最新版本一致
- [ ] 標題沒有規格代號、分支名、Review 標記
- [ ] 沒有 API 路徑、檔名、函式名、資料表名稱
- [ ] 沒有 `FR-XXX`、`T0XX`、`Round X Q Y` 之類的內部代號
- [ ] 每條 change 都從使用者角度描述
- [ ] `warning` 條目有清楚的建議動作
- [ ] 全文為繁體中文（Constitution Principle I）

## 步驟 3：更新 `SRS.md`

找到版本歷程表（8.2 節），在表格**最前面**插入一行：

```
| X.Y.Z | YYYY-MM-DD | 簡短說明 |
```

SRS.md 是給開發者看的內部規格，**可以**保留技術術語（API 路徑、資料表名稱等），與 changelog.json 規範不同。

## 步驟 4：更新 `README.md`（若存在）

若 README.md 中有版本徽章或變更日誌區塊，同步更新版本號。

## 步驟 5：提交、合併到 `main`、推送並打版本 Tag

文件與版號更新完成後，將本次變更提交、合併進 `main`、推送到 GitHub，最後在 `main` 上打版本 Tag。

### 5.1 提交變更

commit message 用 Conventional Commits 格式；**可保留技術細節**（與 changelog.json 的使用者導向規範不同，用途同 PR 說明）。若目前在 `main`，先開功能分支再提交：

```bash
git checkout -b <feature-branch>   # 已在功能分支可略過
git add -A
git commit -m "feat: <一句話描述本次更新> 並更新版號至 X.Y.Z"
```

### 5.2 推送並合併到 `main`（squash merge）

本專案以 **squash merge** 合併 PR，合併後 `main` 會產生一顆**全新的** commit（訊息結尾含 `(#PR編號)`）：

```bash
git push -u origin <feature-branch>
gh pr create --base main --fill
gh pr merge --squash --delete-branch   # 或於 GitHub 網頁 squash 合併
```

> 若本次採直接提交到 `main` 的流程（無 PR），則 `git push origin main` 即可，再進入 5.3。

### 5.3 在 `main` 的合併 commit 上打 Tag

> **鐵則：Tag 一定要在 PR 以 squash 合併進 `main` 之後才打，而且要打在 `main` 上那顆合併後的 commit。**

squash merge 會在 `main` 上產生一顆全新 commit，與合併前功能分支上的原始 commit **SHA 不同**。若把 Tag 打在合併前的功能分支 commit、又在合併後刪除該分支，那顆 commit 就不再被任何分支包含，GitHub 會顯示「This commit does not belong to any branch on this repository」。

正確流程（在 `main` 上操作）：

```bash
# 1. 先確認 PR 已 squash 合併進 main，切到 main 並更新到最新
git checkout main && git pull origin main

# 2. 確認 HEAD 就是該 PR 的合併 commit（訊息應含 (#PR編號)）
git log -1 --oneline

# 3. 在 main 的合併 commit 上打 annotated tag
git tag -a vX.Y.Z -m "vX.Y.Z — 一句話描述本次更新"

# 4. 推送 tag
git push origin vX.Y.Z
```

若不慎把 tag 打錯位置（指到合併前的分支 commit），改指到 `main` 上正確的 commit 後強制覆寫遠端 tag：

```bash
git tag -f -a vX.Y.Z <main 上正確的合併 commit> -m "vX.Y.Z — 一句話描述本次更新"
git push origin vX.Y.Z --force
```

> **注意（本專案 tag 多由 CI 自動建立）**：`main` 一旦有新 commit，`docker-publish.yml` 會自 `changelog.json` 的 `currentVersion` **自動建立並推送 `vX.Y.Z` tag**。因此通常**不需要**手動執行 5.3；手動打同名 tag 會與 CI 相撞。實務上：合併進 `main` 後，等 `docker-publish` 跑完、確認遠端已有 `vX.Y.Z` tag 即可，再進入步驟 6。

## 步驟 6：建立 GitHub Release

在 `vX.Y.Z` tag 已存在於遠端後（多由步驟 5 的 `docker-publish` 自動建立，或手動打的 tag），以該 tag 建立 GitHub Release。Release Notes **直接取自 `changelog.json` 該版本條目**——它已是使用者導向用語、且已通過步驟 2 的禁用字檢查，無需再翻譯。

### 6.1 由 `changelog.json` 產生標題與 Release Notes

```bash
VERSION="X.Y.Z"

# 標題取該版本 title；Notes 取 changes 並依 warning→new→improved→fixed→removed 排序
TITLE=$(node -e 'const c=require("./changelog.json");const r=(c.releases||[]).find(r=>String(r.version)===process.argv[1]);process.stdout.write(r?r.title:"")' "$VERSION")
NOTES=$(node -e '
  const c=require("./changelog.json");
  const r=(c.releases||[]).find(r=>String(r.version)===process.argv[1]);
  if(!r){console.error("changelog.json 找不到版本 "+process.argv[1]);process.exit(1);}
  const label={warning:"注意",new:"新增",improved:"改進",fixed:"修正",removed:"移除"};
  const order=["warning","new","improved","fixed","removed"];
  const sorted=[...(r.changes||[])].sort((a,b)=>order.indexOf(a.tag)-order.indexOf(b.tag));
  console.log(sorted.map(ch=>`- **${label[ch.tag]||ch.tag}**：${ch.text}`).join("\n"));
' "$VERSION")
```

### 6.2 建立 Release

```bash
# 確保本地看得到 CI 推上來的 tag
git fetch --tags
# --verify-tag：tag 不存在就中止，避免搶在 docker-publish 建 tag 之前
gh release create "v$VERSION" \
  --title "v$VERSION — $TITLE" \
  --notes "$NOTES" \
  --verify-tag \
  --latest
```

- `--verify-tag`：若 `vX.Y.Z` tag 還沒被 `docker-publish` 推上遠端，指令會中止——稍等其跑完、`git fetch --tags` 後再重試。
- 該版本 Release 已存在時 `create` 會失敗；改用更新：`gh release edit "v$VERSION" --title "v$VERSION — $TITLE" --notes "$NOTES"`。
- **本機環境**：`gh` 不在 PATH，需以 PowerShell + 完整路徑執行 `"C:\Program Files\GitHub CLI\gh.exe"`（見記憶 `gh-cli-path`）；Bash 工具跑不動。

---

完成後回報：「已更新版號至 X.Y.Z，changelog.json（已通過格式與用語檢查）、SRS.md 已同步；變更已合併進 `main` 並推送，版本 Tag vX.Y.Z（由 docker-publish 自動建立／或手動打在 `main` 合併 commit）已就緒，GitHub Release vX.Y.Z 已以 changelog 條目為 Release Notes 建立完成。」
