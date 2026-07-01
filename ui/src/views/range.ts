// 區間檢視（T044，FR-009）：選取日期範圍，檢視期間 App 與網站彙總。

import { api, errText } from "../api";
import { toast, totalsList } from "../components/list";
import { formatDuration, todayLocalDate } from "../format";
import { t } from "../i18n/zh-TW";

function daysAgo(n: number): string {
  const d = new Date();
  d.setDate(d.getDate() - n);
  const y = d.getFullYear();
  const m = String(d.getMonth() + 1).padStart(2, "0");
  const day = String(d.getDate()).padStart(2, "0");
  return `${y}-${m}-${day}`;
}

export async function renderRange(root: HTMLElement): Promise<void> {
  root.innerHTML = "";
  const ctrl = document.createElement("div");
  ctrl.className = "card";
  const result = document.createElement("div");
  result.className = "card";
  root.append(ctrl, result);

  const fromInput = document.createElement("input");
  fromInput.type = "date";
  fromInput.value = daysAgo(6);
  const toInput = document.createElement("input");
  toInput.type = "date";
  toInput.value = todayLocalDate();
  const btn = document.createElement("button");
  btn.className = "btn";
  btn.textContent = t.common.query;

  const row = document.createElement("div");
  row.className = "rowflex";
  const fl = document.createElement("label");
  fl.textContent = t.common.from;
  const tl = document.createElement("label");
  tl.textContent = t.common.to;
  row.append(fl, fromInput, tl, toInput, btn);
  ctrl.append(row);

  async function query(): Promise<void> {
    try {
      const r = await api.getSummaryRange(fromInput.value, toInput.value);
      result.innerHTML = "";
      const totalH = document.createElement("h2");
      totalH.textContent = `${t.common.total}：${formatDuration(r.total_active_ms)}`;
      const ah = document.createElement("h2");
      ah.textContent = t.common.app;
      const wh = document.createElement("h2");
      wh.textContent = t.common.website;
      result.append(totalH, ah, totalsList(r.apps), wh, totalsList(r.websites));
    } catch (e) {
      toast(errText(e), true);
    }
  }
  btn.onclick = () => void query();
  await query();
}
