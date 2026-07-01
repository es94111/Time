// 資源趨勢檢視（T029，US3 / FR-010、FR-014）：選定區間回顧趨勢，長區間自動降取樣
// （後端 AVG 趨勢＋MAX 尖峰）；空缺區間以斷點呈現、不內插（原則 III）。

import { api, errText, type TrendPoint, type TrendSeries } from "../api";
import { toast } from "../components/list";
import { formatBytes, formatDateTime, formatPct, formatRate } from "../format";
import { t } from "../i18n/zh-TW";

const HOUR = 3_600_000;
const DAY = 86_400_000;

interface RangePreset {
  label: string;
  spanMs: number;
}

function presets(): RangePreset[] {
  return [
    { label: t.metrics.lastHour, spanMs: HOUR },
    { label: t.metrics.last24h, spanMs: 24 * HOUR },
    { label: t.metrics.last7d, spanMs: 7 * DAY },
    { label: t.metrics.last30d, spanMs: 30 * DAY },
  ];
}

function bucketLabel(bucketMs: number): string {
  if (bucketMs === 0) return t.metrics.bucketRaw;
  if (bucketMs === 60_000) return t.metrics.bucketMinute;
  if (bucketMs === 3_600_000) return t.metrics.bucketHour;
  return t.metrics.bucketDay;
}

/** 以 SVG 繪製迷你趨勢線；於時間空缺（> 1.5×桶寬）處斷線，缺值不連接（不內插）。 */
function sparkline(
  points: { t: number; v: number | null }[],
  gapMs: number,
  max: number,
): SVGSVGElement {
  const W = 480;
  const H = 60;
  const svg = document.createElementNS("http://www.w3.org/2000/svg", "svg");
  svg.setAttribute("viewBox", `0 0 ${W} ${H}`);
  svg.setAttribute("class", "spark");
  svg.setAttribute("preserveAspectRatio", "none");

  if (points.length === 0) return svg;
  const t0 = points[0].t;
  const t1 = points[points.length - 1].t;
  const span = Math.max(1, t1 - t0);
  const x = (tt: number) => ((tt - t0) / span) * W;
  const y = (v: number) => H - (Math.max(0, Math.min(max, v)) / max) * (H - 4) - 2;

  // 依時間空缺與缺值切分為多段折線。
  let segment: string[] = [];
  const flush = (): void => {
    if (segment.length >= 1) {
      const poly = document.createElementNS("http://www.w3.org/2000/svg", "polyline");
      poly.setAttribute("points", segment.join(" "));
      poly.setAttribute("class", "sparkline");
      svg.appendChild(poly);
    }
    segment = [];
  };
  let prevT: number | null = null;
  for (const p of points) {
    const gap = prevT !== null && gapMs > 0 && p.t - prevT > gapMs * 1.5;
    if (p.v === null || gap) flush();
    if (p.v !== null) segment.push(`${x(p.t).toFixed(1)},${y(p.v).toFixed(1)}`);
    prevT = p.t;
  }
  flush();
  return svg;
}

function seriesCard(
  title: string,
  points: TrendPoint[],
  gapMs: number,
  pick: (p: TrendPoint) => { avg: number | null; max: number | null },
  max: number,
  fmt: (v: number) => string,
): HTMLElement {
  const c = document.createElement("div");
  c.className = "card";
  const h = document.createElement("h2");
  h.textContent = title;
  c.append(h);

  const data = points.map((p) => ({ t: p.tsUtc, v: pick(p).avg }));
  const hasData = data.some((d) => d.v !== null);
  if (!hasData) {
    const p = document.createElement("p");
    p.className = "muted";
    p.textContent = t.metrics.noData;
    c.append(p);
    return c;
  }
  c.append(sparkline(data, gapMs, max));

  // 平均與尖峰摘要。
  const peaks = points.map((p) => pick(p).max).filter((v): v is number => v !== null);
  const avgs = data.map((d) => d.v).filter((v): v is number => v !== null);
  const peak = peaks.length ? Math.max(...peaks) : null;
  const avg = avgs.length ? avgs.reduce((a, b) => a + b, 0) / avgs.length : null;
  const info = document.createElement("p");
  info.className = "muted";
  info.textContent = `${t.metrics.avg}：${avg === null ? "—" : fmt(avg)}　${t.metrics.peak}：${
    peak === null ? "—" : fmt(peak)
  }`;
  c.append(info);
  return c;
}

function render(root: HTMLElement, controls: HTMLElement, series: TrendSeries): void {
  // 清掉舊圖表（保留控制列）。
  root.innerHTML = "";
  root.append(controls);

  const meta = document.createElement("p");
  meta.className = "muted";
  const first = series.points[0];
  const last = series.points[series.points.length - 1];
  meta.textContent = series.points.length
    ? `${bucketLabel(series.bucketMs)}　${formatDateTime(first.tsUtc)} – ${formatDateTime(last.tsUtc)}`
    : t.metrics.noData;
  root.append(meta);

  if (series.points.length === 0) return;
  const gap = series.bucketMs;

  // CPU。
  root.append(
    seriesCard(t.metrics.cpu, series.points, gap, (p) => ({ avg: p.cpuAvg, max: p.cpuMax }), 100, formatPct),
  );
  // 記憶體使用率（由 used/total 推算）。
  const memMax = Math.max(
    1,
    ...series.points.map((p) => p.memTotalBytes ?? 0),
  );
  root.append(
    seriesCard(
      t.metrics.memory,
      series.points,
      gap,
      (p) => ({ avg: p.memUsedAvg, max: p.memUsedAvg }),
      memMax,
      formatBytes,
    ),
  );

  // 逐硬碟（讀取速度）。
  const diskIds = new Map<number, string>();
  for (const p of series.points) for (const d of p.disks) diskIds.set(d.id, d.name ?? `#${d.id}`);
  const diskMax = Math.max(
    1,
    ...series.points.flatMap((p) => p.disks.map((d) => d.readMax ?? 0)),
  );
  for (const [id, name] of diskIds) {
    root.append(
      seriesCard(
        `${t.metrics.disks}：${name}（${t.metrics.read}）`,
        series.points,
        gap,
        (p) => {
          const d = p.disks.find((x) => x.id === id);
          return { avg: d?.readAvg ?? null, max: d?.readMax ?? null };
        },
        diskMax,
        formatRate,
      ),
    );
  }

  // 逐網路介面（下載速度）。
  const netIds = new Map<number, string>();
  for (const p of series.points) for (const n of p.nets) netIds.set(n.id, n.name ?? `#${n.id}`);
  const netMax = Math.max(1, ...series.points.flatMap((p) => p.nets.map((n) => n.rxMax ?? 0)));
  for (const [id, name] of netIds) {
    root.append(
      seriesCard(
        `${t.metrics.nets}：${name}（${t.metrics.rx}）`,
        series.points,
        gap,
        (p) => {
          const n = p.nets.find((x) => x.id === id);
          return { avg: n?.rxAvg ?? null, max: n?.rxMax ?? null };
        },
        netMax,
        formatRate,
      ),
    );
  }

  // 逐 GPU（使用率）。
  const gpuIds = new Map<number, string>();
  for (const p of series.points) for (const g of p.gpus) gpuIds.set(g.id, g.name ?? `#${g.id}`);
  for (const [id, name] of gpuIds) {
    root.append(
      seriesCard(
        `${t.metrics.gpus}：${name}（${t.metrics.util}）`,
        series.points,
        gap,
        (p) => {
          const g = p.gpus.find((x) => x.id === id);
          return { avg: g?.utilAvg ?? null, max: g?.utilMax ?? null };
        },
        100,
        formatPct,
      ),
    );
  }
}

export async function renderMetricsHistory(root: HTMLElement): Promise<void> {
  root.innerHTML = "";
  const controls = document.createElement("div");
  controls.className = "card rowflex";
  root.append(controls);

  let current = presets()[1]; // 預設最近 24 小時

  const load = async (): Promise<void> => {
    try {
      const now = Date.now();
      const series = await api.getMetricsRange(now - current.spanMs, now);
      render(root, controls, series);
    } catch (e) {
      toast(errText(e), true);
    }
  };

  const label = document.createElement("span");
  label.className = "name";
  label.textContent = t.metrics.range;
  controls.append(label);
  for (const p of presets()) {
    const b = document.createElement("button");
    b.className = "btn secondary";
    b.textContent = p.label;
    b.onclick = () => {
      current = p;
      void load();
    };
    controls.append(b);
  }
  const refresh = document.createElement("button");
  refresh.className = "btn secondary";
  refresh.textContent = t.common.refresh;
  refresh.onclick = () => void load();
  controls.append(refresh);

  await load();
}
