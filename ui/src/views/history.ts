// 歷史檢視（T043，FR-009）：選取某日，檢視該日 App 與網站彙總。

import { api, errText } from "../api";
import { toast, totalsList } from "../components/list";
import { formatDuration, todayLocalDate } from "../format";
import { t } from "../i18n/zh-TW";

export async function renderHistory(root: HTMLElement): Promise<void> {
  root.innerHTML = "";
  const ctrl = document.createElement("div");
  ctrl.className = "card";
  const result = document.createElement("div");
  result.className = "card";
  root.append(ctrl, result);

  const label = document.createElement("label");
  label.textContent = t.common.date;
  const input = document.createElement("input");
  input.type = "date";
  input.value = todayLocalDate();
  const btn = document.createElement("button");
  btn.className = "btn";
  btn.textContent = t.common.query;
  const row = document.createElement("div");
  row.className = "rowflex";
  row.append(input, btn);
  ctrl.append(label, row);

  async function query(): Promise<void> {
    try {
      const day = await api.getSummaryByDate(input.value);
      result.innerHTML = "";
      const totalH = document.createElement("h2");
      totalH.textContent = `${t.common.total}：${formatDuration(day.total_active_ms)}`;
      const ah = document.createElement("h2");
      ah.textContent = t.common.app;
      const wh = document.createElement("h2");
      wh.textContent = t.common.website;
      result.append(totalH, ah, totalsList(day.apps), wh, totalsList(day.websites));
    } catch (e) {
      toast(errText(e), true);
    }
  }
  btn.onclick = () => void query();
  await query();
}
