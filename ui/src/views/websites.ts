// 網站檢視（T038，FR-005）：今日各網站瀏覽時長，與 App 分列。

import { api, errText } from "../api";
import { toast, totalsList } from "../components/list";
import { t } from "../i18n/zh-TW";

export async function renderWebsites(root: HTMLElement): Promise<void> {
  root.innerHTML = "";
  const card = document.createElement("div");
  card.className = "card";
  const h = document.createElement("h2");
  h.textContent = `${t.tabs.websites}（${t.tabs.today}）`;
  card.append(h);
  root.append(card);
  try {
    const day = await api.getTodaySummary();
    card.append(totalsList(day.websites));
  } catch (e) {
    toast(errText(e), true);
  }
}
