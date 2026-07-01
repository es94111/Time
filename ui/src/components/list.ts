// 具名總計清單（含 CSV 條狀圖），供各檢視重用。

import type { NamedTotal } from "../api";
import { formatDuration } from "../format";
import { t } from "../i18n/zh-TW";

/** 產生依時長排序的條狀清單元素。 */
export function totalsList(items: NamedTotal[]): HTMLElement {
  const wrap = document.createElement("div");
  wrap.className = "totals";
  if (!items || items.length === 0) {
    const empty = document.createElement("p");
    empty.className = "muted";
    empty.textContent = t.common.noData;
    wrap.appendChild(empty);
    return wrap;
  }
  const max = Math.max(...items.map((i) => i.active_ms), 1);
  for (const it of items) {
    const row = document.createElement("div");
    row.className = "row";

    const name = document.createElement("span");
    name.className = "name";
    name.textContent = it.name;
    name.title = it.name;

    const barwrap = document.createElement("div");
    barwrap.className = "barwrap";
    const bar = document.createElement("div");
    bar.className = "bar";
    bar.style.width = `${(it.active_ms / max) * 100}%`;
    barwrap.appendChild(bar);

    const dur = document.createElement("span");
    dur.className = "dur";
    dur.textContent = formatDuration(it.active_ms);

    row.append(name, barwrap, dur);
    wrap.appendChild(row);
  }
  return wrap;
}

/** 顯示短暫提示。 */
export function toast(message: string, isError = false): void {
  const el = document.createElement("div");
  el.className = "toast" + (isError ? " err" : "");
  el.textContent = message;
  document.body.appendChild(el);
  setTimeout(() => el.remove(), 3200);
}
