// 即時監控檢視（T024，US2 / FR-009、SC-003）：訂閱 metrics://sample 事件即時更新，
// 退回定期 get_current_metrics 輪詢；缺值以「無資料」明確呈現，不以 0 假冒（原則 III）。

import { api, errText, type MetricSnapshot, type UnlistenFn } from "../api";
import { toast } from "../components/list";
import { formatBytes, formatPct, formatRate } from "../format";
import { t } from "../i18n/zh-TW";

let unlisten: UnlistenFn | undefined;
let pollTimer: number | undefined;

/** 呈現一個「名稱 + 值」的量測列（缺值以無資料標示）。 */
function metricRow(label: string, valueText: string | null): HTMLElement {
  const row = document.createElement("div");
  row.className = "row";
  const name = document.createElement("span");
  name.className = "name";
  name.textContent = label;
  const val = document.createElement("span");
  val.className = "dur";
  if (valueText === null) {
    val.textContent = t.metrics.noData;
    val.classList.add("muted");
  } else {
    val.textContent = valueText;
  }
  row.append(name, val);
  return row;
}

/** 條狀進度（0–100），供 CPU／記憶體／GPU 使用率視覺化。 */
function gauge(label: string, pct: number | null, sub: string | null): HTMLElement {
  const wrap = document.createElement("div");
  wrap.className = "row";
  const name = document.createElement("span");
  name.className = "name";
  name.textContent = label;
  const barwrap = document.createElement("div");
  barwrap.className = "barwrap";
  const bar = document.createElement("div");
  bar.className = "bar";
  bar.style.width = pct === null ? "0%" : `${Math.max(0, Math.min(100, pct))}%`;
  barwrap.appendChild(bar);
  const val = document.createElement("span");
  val.className = "dur";
  val.textContent = pct === null ? t.metrics.noData : sub ? `${formatPct(pct)}（${sub}）` : formatPct(pct);
  if (pct === null) val.classList.add("muted");
  wrap.append(name, barwrap, val);
  return wrap;
}

function card(title: string): HTMLElement {
  const c = document.createElement("div");
  c.className = "card";
  const h = document.createElement("h2");
  h.textContent = title;
  c.append(h);
  return c;
}

function render(root: HTMLElement, snap: MetricSnapshot | null): void {
  root.innerHTML = "";
  if (!snap) {
    const empty = card(t.metrics.live);
    const p = document.createElement("p");
    p.className = "muted";
    p.textContent = t.metrics.noData;
    empty.append(p);
    root.append(empty);
    return;
  }

  // CPU 與記憶體。
  const overview = card(t.metrics.live);
  overview.append(gauge(t.metrics.cpu, snap.cpuPct, null));
  const memPct =
    snap.memUsedBytes !== null && snap.memTotalBytes && snap.memTotalBytes > 0
      ? (snap.memUsedBytes / snap.memTotalBytes) * 100
      : null;
  const memSub =
    snap.memUsedBytes !== null && snap.memTotalBytes !== null
      ? `${formatBytes(snap.memUsedBytes)} / ${formatBytes(snap.memTotalBytes)}`
      : null;
  overview.append(gauge(t.metrics.memory, memPct, memSub));
  root.append(overview);

  // 硬碟（逐顆）。
  const disks = card(t.metrics.disks);
  if (snap.disks.length === 0) {
    disks.append(metricRow(t.metrics.naDevice, null));
  } else {
    for (const d of snap.disks) {
      const name = d.name ?? `#${d.id}`;
      disks.append(
        metricRow(
          `${name}　${t.metrics.read}/${t.metrics.write}`,
          d.readBps === null && d.writeBps === null
            ? null
            : `${d.readBps === null ? "—" : formatRate(d.readBps)}　${
                d.writeBps === null ? "—" : formatRate(d.writeBps)
              }`,
        ),
      );
    }
  }
  root.append(disks);

  // 網路（逐介面）。
  const nets = card(t.metrics.nets);
  if (snap.nets.length === 0) {
    nets.append(metricRow(t.metrics.naDevice, null));
  } else {
    for (const n of snap.nets) {
      const name = n.name ?? `#${n.id}`;
      nets.append(
        metricRow(
          `${name}　${t.metrics.rx}/${t.metrics.tx}`,
          n.rxBps === null && n.txBps === null
            ? null
            : `${n.rxBps === null ? "—" : formatRate(n.rxBps)}　${
                n.txBps === null ? "—" : formatRate(n.txBps)
              }`,
        ),
      );
    }
  }
  root.append(nets);

  // GPU（逐張）。
  const gpus = card(t.metrics.gpus);
  if (snap.gpus.length === 0) {
    const p = document.createElement("p");
    p.className = "muted";
    p.textContent = t.metrics.noGpu;
    gpus.append(p);
  } else {
    for (const g of snap.gpus) {
      const name = g.name ?? `#${g.id}`;
      gpus.append(gauge(`${name}　${t.metrics.util}`, g.utilPct, null));
      const vram =
        g.memUsedBytes !== null && g.memTotalBytes !== null
          ? `${formatBytes(g.memUsedBytes)} / ${formatBytes(g.memTotalBytes)}`
          : null;
      gpus.append(metricRow(`${name}　${t.metrics.vram}`, vram));
    }
  }
  root.append(gpus);
}

/** 清理事件訂閱與輪詢（切換分頁時呼叫）。 */
export function disposeMetricsLive(): void {
  if (unlisten) {
    unlisten();
    unlisten = undefined;
  }
  if (pollTimer) {
    clearInterval(pollTimer);
    pollTimer = undefined;
  }
}

export async function renderMetricsLive(root: HTMLElement): Promise<void> {
  disposeMetricsLive();
  root.innerHTML = "";

  const load = async (): Promise<void> => {
    try {
      const snap = await api.getCurrentMetrics();
      render(root, snap);
    } catch (e) {
      toast(errText(e), true);
    }
  };

  await load();

  // 優先以事件即時更新（SC-003）；訂閱失敗則以輪詢退回。
  try {
    unlisten = await api.onMetricsSample((snap) => render(root, snap));
  } catch {
    pollTimer = window.setInterval(() => void load(), 2000);
  }
}
