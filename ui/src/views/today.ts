// 今日檢視（T029，FR-007/008）：各 App 時長排序、當日總計、暫停/恢復。

import { api, errText } from "../api";
import { toast, totalsList } from "../components/list";
import { formatDuration } from "../format";
import { t } from "../i18n/zh-TW";

export async function renderToday(root: HTMLElement): Promise<void> {
  root.innerHTML = "";
  const controls = document.createElement("div");
  controls.className = "card";
  const summary = document.createElement("div");
  summary.className = "card";
  root.append(controls, summary);

  async function refresh(): Promise<void> {
    try {
      const [status, day] = await Promise.all([api.getTrackingStatus(), api.getTodaySummary()]);

      controls.innerHTML = "";
      const h = document.createElement("h2");
      h.textContent = t.tabs.today;
      const bar = document.createElement("div");
      bar.className = "rowflex";
      const pauseBtn = document.createElement("button");
      pauseBtn.className = "btn" + (status.paused ? "" : " secondary");
      pauseBtn.textContent = status.paused ? t.status.resume : t.status.pause;
      pauseBtn.onclick = async () => {
        try {
          status.paused ? await api.resume() : await api.pause();
          await refresh();
        } catch (e) {
          toast(errText(e), true);
        }
      };
      const refreshBtn = document.createElement("button");
      refreshBtn.className = "btn secondary";
      refreshBtn.textContent = t.common.refresh;
      refreshBtn.onclick = () => void refresh();
      bar.append(pauseBtn, refreshBtn);
      controls.append(h, bar);

      summary.innerHTML = "";
      const totalH = document.createElement("h2");
      totalH.textContent = `${t.status.total}　`;
      const total = document.createElement("span");
      total.className = "big";
      total.textContent = formatDuration(day.total_active_ms);
      totalH.appendChild(total);
      const appsH = document.createElement("h2");
      appsH.textContent = t.common.app;
      summary.append(totalH, appsH, totalsList(day.apps));
    } catch (e) {
      toast(errText(e), true);
    }
  }

  await refresh();
}
