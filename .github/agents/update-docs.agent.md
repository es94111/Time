---
description: Update versioned release documents and changelog after implementation work
---

# 更新文件與版號

完成功能開發或修正後，依序執行以下步驟確保所有文件保持同步。

> **撰寫 changelog 條目前必讀**：`.specify/memory/changelog-style.md`（Changelog 公開撰寫規格）。若該檔案不存在，仍必須遵守本指令內的使用者導向撰寫規則。

## 步驟 1：判斷版號

根據異動規模決定版號：
- **大版本**（如 4.0）：新增重大模組
- **小版本**（如 3.8）：新增功能或重要改進
- **修正版**（如 3.7.1）：Bug 修正或維護性更新

讀取 `changelog.json` 的 `currentVersion` 確認目前版號，再決定新版號。

## 步驟 2：更新 `changelog.json`

`changelog.json` 是給一般使用者看的，不是給開發者看的。將 `currentVersion` 改為新版號，並在 `releases` 陣列最前面插入新版本紀錄。

條目規則：
- **標題**：12–25 字，不含規格代號、分支名、Review 標記。
- **每條 change**：40–120 字一句完整中文，從使用者角度描述結果。
- **條目順序**：`warning` → `new` → `improved` → `fixed` → `removed`。
- 不要出現 API 路徑、檔名、函式名、資料表欄位、環境變數、內部規格代號、分支名、套件版號或 CVE 細節；改寫成使用者可理解的結果。

提交前驗證：

```bash
node -e "require('./changelog.json')"
grep -n "FR-[0-9]" changelog.json
grep -n "/api/" changelog.json
grep -nE "[a-z]+\.js" changelog.json
grep -n "Round [0-9]" changelog.json
grep -n "Copilot" changelog.json
grep -n "CodeQL" changelog.json
```

## 步驟 3：更新 `SRS.md`

找到版本歷程表（8.2 節），在表格最前面插入一行：

```text
| X.Y.Z | YYYY-MM-DD | 簡短說明 |
```

SRS.md 是給開發者看的內部規格，可以保留技術術語。

## 步驟 4：更新 `README.md`（若存在）

若 README.md 中有版本徽章或變更日誌區塊，同步更新版本號。

## 步驟 5：提交與發布

文件與版號更新完成後，依專案流程提交、推送、合併到 `main`，並在版本 Tag 已建立後，以 `changelog.json` 對應版本條目建立 GitHub Release。
