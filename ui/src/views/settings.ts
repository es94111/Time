// 設定檢視（T050/T046/T048/T049/T051）：閒置門檻、自啟、語言、排除清單、匯出、清除、主密碼。

import { api, errText, type Exclusion } from "../api";
import { toast } from "../components/list";
import { t } from "../i18n/zh-TW";

function card(title: string): HTMLElement {
  const c = document.createElement("div");
  c.className = "card";
  const h = document.createElement("h2");
  h.textContent = title;
  c.append(h);
  return c;
}

export async function renderSettings(root: HTMLElement): Promise<void> {
  root.innerHTML = "";

  // 1) 一般設定
  const general = card(t.tabs.settings);
  const idleLabel = document.createElement("label");
  idleLabel.textContent = t.settings.idleThreshold;
  const idle = document.createElement("input");
  idle.type = "number";
  idle.min = "10";
  const autoLabel = document.createElement("label");
  const auto = document.createElement("input");
  auto.type = "checkbox";
  autoLabel.append(auto, document.createTextNode(" " + t.settings.autostart));
  const saveBtn = document.createElement("button");
  saveBtn.className = "btn";
  saveBtn.textContent = t.common.confirm;
  saveBtn.onclick = async () => {
    try {
      await api.updateSettings({
        idle_threshold_sec: parseInt(idle.value, 10) || 300,
        autostart_enabled: auto.checked,
      });
      toast(t.settings.saved);
    } catch (e) {
      toast(errText(e), true);
    }
  };
  general.append(idleLabel, idle, autoLabel, document.createElement("br"), saveBtn);

  // 2) 排除清單
  const exclCard = card(t.settings.exclusions);
  const exclList = document.createElement("div");
  const appRow = document.createElement("div");
  appRow.className = "rowflex";
  const appInput = document.createElement("input");
  appInput.placeholder = t.settings.addExclusionApp;
  appInput.style.flex = "1";
  const appBtn = document.createElement("button");
  appBtn.className = "btn secondary";
  appBtn.textContent = t.settings.add;
  appBtn.onclick = () => void addExcl("app", appInput);
  appRow.append(appInput, appBtn);
  const siteRow = document.createElement("div");
  siteRow.className = "rowflex";
  const siteInput = document.createElement("input");
  siteInput.placeholder = t.settings.addExclusionWebsite;
  siteInput.style.flex = "1";
  const siteBtn = document.createElement("button");
  siteBtn.className = "btn secondary";
  siteBtn.textContent = t.settings.add;
  siteBtn.onclick = () => void addExcl("website", siteInput);
  siteRow.append(siteInput, siteBtn);
  exclCard.append(exclList, appRow, siteRow);

  async function refreshExcl(): Promise<void> {
    try {
      const items = await api.listExclusions();
      exclList.innerHTML = "";
      if (items.length === 0) {
        const p = document.createElement("p");
        p.className = "muted";
        p.textContent = t.common.noData;
        exclList.append(p);
      }
      for (const it of items) {
        exclList.append(exclItem(it, refreshExcl));
      }
    } catch (e) {
      toast(errText(e), true);
    }
  }
  async function addExcl(kind: "app" | "website", input: HTMLInputElement): Promise<void> {
    const pattern = input.value.trim();
    if (!pattern) return;
    try {
      await api.addExclusion(kind, pattern);
      input.value = "";
      await refreshExcl();
    } catch (e) {
      toast(errText(e), true);
    }
  }

  // 3) 匯出
  const exportCard = card(t.settings.exportTitle);
  const pathInput = document.createElement("input");
  pathInput.placeholder = "C:\\Users\\<你>\\Desktop\\activity_export.csv";
  pathInput.style.width = "100%";
  const exRow = document.createElement("div");
  exRow.className = "rowflex";
  const csvBtn = document.createElement("button");
  csvBtn.className = "btn";
  csvBtn.textContent = t.settings.exportCsv;
  csvBtn.onclick = () => void doExport("csv");
  const jsonBtn = document.createElement("button");
  jsonBtn.className = "btn";
  jsonBtn.textContent = t.settings.exportJson;
  jsonBtn.onclick = () => void doExport("json");
  exRow.append(csvBtn, jsonBtn);
  exportCard.append(pathInput, exRow);
  async function doExport(format: "csv" | "json"): Promise<void> {
    const target = pathInput.value.trim();
    if (!target) {
      toast(t.errorPrefix + "請先填寫匯出路徑", true);
      return;
    }
    try {
      const r = await api.exportData({ format, scope: "all", target_path: target });
      toast(t.settings.exportDone(r.written_path, r.row_count));
    } catch (e) {
      toast(errText(e), true);
    }
  }

  // 4) 清除
  const clearCard = card(t.settings.clearTitle);
  const clearBtn = document.createElement("button");
  clearBtn.className = "btn danger";
  clearBtn.textContent = t.settings.clearAll;
  clearBtn.onclick = async () => {
    if (!window.confirm(t.settings.clearConfirm)) return;
    try {
      const r = await api.clearData("all");
      toast(t.settings.cleared(r.deleted_rows));
    } catch (e) {
      toast(errText(e), true);
    }
  };
  clearCard.append(clearBtn);

  // 4.5) 指標設定（002-system-metrics）
  const metricsCard = card(t.tabs.metricsLive);
  const intervalLabel = document.createElement("label");
  intervalLabel.textContent = `${t.metrics.samplingInterval}（${t.metrics.intervalHint}）`;
  const interval = document.createElement("input");
  interval.type = "number";
  interval.min = "1";
  interval.max = "3600";
  const retentionLabel = document.createElement("label");
  retentionLabel.textContent = `${t.metrics.retentionDays}（${t.metrics.retentionHint}）`;
  const retention = document.createElement("input");
  retention.type = "number";
  retention.min = "1";
  retention.max = "3650";
  const enabledLabel = document.createElement("label");
  const enabled = document.createElement("input");
  enabled.type = "checkbox";
  enabledLabel.append(enabled, document.createTextNode(" " + t.metrics.enabled));
  const metricsSaveBtn = document.createElement("button");
  metricsSaveBtn.className = "btn";
  metricsSaveBtn.textContent = t.common.confirm;
  metricsSaveBtn.onclick = async () => {
    try {
      await api.setMetricsSettings({
        sampleIntervalSec: parseInt(interval.value, 10) || 1,
        retentionDays: parseInt(retention.value, 10) || 30,
        enabled: enabled.checked,
      });
      toast(t.settings.saved);
    } catch (e) {
      toast(errText(e), true);
    }
  };
  const clearMetricsBtn = document.createElement("button");
  clearMetricsBtn.className = "btn danger";
  clearMetricsBtn.textContent = t.metrics.clearMetrics;
  clearMetricsBtn.onclick = async () => {
    if (!window.confirm(t.metrics.clearMetricsConfirm)) return;
    try {
      const r = await api.clearMetricsData();
      toast(t.metrics.clearedMetrics(r.deletedSamples));
    } catch (e) {
      toast(errText(e), true);
    }
  };
  metricsCard.append(
    intervalLabel,
    interval,
    retentionLabel,
    retention,
    enabledLabel,
    document.createElement("br"),
    metricsSaveBtn,
    document.createElement("br"),
    clearMetricsBtn,
  );

  // 5) 主密碼
  const pwCard = card(t.settings.masterPassword);
  const newPw = document.createElement("input");
  newPw.type = "password";
  newPw.placeholder = t.settings.newPassword;
  newPw.style.width = "100%";
  const curPw = document.createElement("input");
  curPw.type = "password";
  curPw.placeholder = t.settings.currentPassword;
  curPw.style.width = "100%";
  const pwBtn = document.createElement("button");
  pwBtn.className = "btn";
  pwBtn.textContent = t.settings.setPassword;
  pwBtn.onclick = async () => {
    try {
      await api.setMasterPassword(newPw.value, curPw.value || undefined);
      newPw.value = "";
      curPw.value = "";
      toast(t.settings.saved);
    } catch (e) {
      toast(errText(e), true);
    }
  };
  pwCard.append(newPw, curPw, document.createElement("br"), pwBtn);

  root.append(general, exclCard, exportCard, clearCard, metricsCard, pwCard);

  // 初始載入設定值。
  try {
    const s = await api.getSettings();
    idle.value = String(s.idle_threshold_sec);
    auto.checked = s.autostart_enabled;
  } catch (e) {
    toast(errText(e), true);
  }
  try {
    const ms = await api.getMetricsSettings();
    interval.value = String(ms.sampleIntervalSec);
    retention.value = String(ms.retentionDays);
    enabled.checked = ms.enabled;
  } catch (e) {
    toast(errText(e), true);
  }
  await refreshExcl();
}

function exclItem(it: Exclusion, onChange: () => Promise<void>): HTMLElement {
  const row = document.createElement("div");
  row.className = "excl-item";
  const tag = document.createElement("span");
  tag.className = "tag";
  tag.textContent = it.kind === "app" ? t.common.app : t.common.website;
  const name = document.createElement("span");
  name.textContent = it.pattern;
  name.style.flex = "1";
  const rm = document.createElement("button");
  rm.className = "btn danger";
  rm.textContent = t.settings.remove;
  rm.onclick = async () => {
    try {
      await api.removeExclusion(it.id);
      await onChange();
    } catch (e) {
      toast(errText(e), true);
    }
  };
  row.append(tag, name, rm);
  return row;
}
