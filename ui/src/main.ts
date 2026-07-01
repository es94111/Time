// 前端外殼（T015）：分頁路由、狀態列、解鎖流程；介面 zh-TW。

import "./styles.css";
import { api, errText } from "./api";
import { toast } from "./components/list";
import { t } from "./i18n/zh-TW";
import { fetchConnectionStatus, renderConnectionStatus } from "./views/connection-status";
import { renderHistory } from "./views/history";
import { renderLogin } from "./views/login";
import { renderMetricsHistory } from "./views/metrics-history";
import { disposeMetricsLive, renderMetricsLive } from "./views/metrics-live";
import { renderRange } from "./views/range";
import { renderSettings } from "./views/settings";
import { renderToday } from "./views/today";
import { renderWebsites } from "./views/websites";

type TabKey =
  | "today"
  | "websites"
  | "history"
  | "range"
  | "metricsLive"
  | "metricsHistory"
  | "settings";

const views: Record<TabKey, (root: HTMLElement) => Promise<void>> = {
  today: renderToday,
  websites: renderWebsites,
  history: renderHistory,
  range: renderRange,
  metricsLive: renderMetricsLive,
  metricsHistory: renderMetricsHistory,
  settings: renderSettings,
};

const appEl = document.getElementById("app") as HTMLElement;
let statusTimer: number | undefined;

async function boot(): Promise<void> {
  try {
    const lock = await api.getLockState();
    if (lock.locked) {
      renderUnlock();
      return;
    }
  } catch {
    // 後端尚未就緒：仍進入主介面，各檢視自行處理錯誤。
  }
  renderShell();
}

function renderUnlock(): void {
  appEl.innerHTML = "";
  const center = document.createElement("div");
  center.className = "center";
  const box = document.createElement("div");
  box.className = "unlock-box";
  const title = document.createElement("h1");
  title.textContent = t.unlock.title;
  const hint = document.createElement("p");
  hint.className = "muted";
  hint.textContent = t.unlock.hint;
  const input = document.createElement("input");
  input.type = "password";
  input.placeholder = t.unlock.password;
  input.style.width = "100%";
  input.style.margin = "12px 0";
  const btn = document.createElement("button");
  btn.className = "btn";
  btn.textContent = t.unlock.unlock;
  const err = document.createElement("p");
  err.className = "muted";

  const submit = async (): Promise<void> => {
    try {
      const ok = await api.unlock(input.value);
      if (ok) renderShell();
      else err.textContent = t.unlock.wrong;
    } catch (e) {
      err.textContent = errText(e);
    }
  };
  btn.onclick = () => void submit();
  input.onkeydown = (e) => {
    if (e.key === "Enter") void submit();
  };
  box.append(title, hint, input, btn, err);
  center.append(box);
  appEl.append(center);
}

function renderShell(): void {
  appEl.innerHTML = "";
  const header = document.createElement("header");
  header.className = "topbar";
  const h1 = document.createElement("h1");
  h1.textContent = t.appTitle;
  const statusline = document.createElement("div");
  statusline.className = "statusline";
  statusline.id = "statusline";
  const connStatusline = document.createElement("div");
  connStatusline.className = "statusline";
  connStatusline.id = "connStatusline";
  header.append(h1, statusline, connStatusline);

  const nav = document.createElement("nav");
  nav.className = "tabs";
  const content = document.createElement("main");
  content.className = "content";

  const tabKeys: TabKey[] = [
    "today",
    "websites",
    "history",
    "range",
    "metricsLive",
    "metricsHistory",
    "settings",
  ];
  const buttons: Partial<Record<TabKey, HTMLButtonElement>> = {};

  const select = async (key: TabKey): Promise<void> => {
    // 離開即時監控時清理事件訂閱與輪詢，避免背景持續更新。
    if (key !== "metricsLive") disposeMetricsLive();
    tabKeys.forEach((k) => buttons[k]?.classList.toggle("active", k === key));
    content.innerHTML = `<p class="muted">${t.common.loading}</p>`;
    try {
      await views[key](content);
    } catch (e) {
      toast(errText(e), true);
    }
  };

  for (const key of tabKeys) {
    const b = document.createElement("button");
    b.textContent = t.tabs[key];
    b.onclick = () => void select(key);
    buttons[key] = b;
    nav.append(b);
  }

  appEl.append(header, nav, content);
  void select("today");

  if (statusTimer) clearInterval(statusTimer);
  void updateStatusline();
  void updateConnStatusline();
  statusTimer = window.setInterval(() => {
    void updateStatusline();
    void updateConnStatusline();
  }, 3000);
}

async function updateConnStatusline(): Promise<void> {
  const el = document.getElementById("connStatusline");
  if (!el) return;
  try {
    const status = await fetchConnectionStatus();
    if (!status.logged_in) {
      el.innerHTML = "";
      const dot = document.createElement("span");
      dot.className = "dot off";
      const label = document.createElement("span");
      label.textContent = t.login.notLoggedIn;
      const loginBtn = document.createElement("button");
      loginBtn.className = "btn secondary";
      loginBtn.textContent = t.login.login;
      loginBtn.onclick = () => showLoginOverlay();
      el.append(dot, label, loginBtn);
      return;
    }
    renderConnectionStatus(el, status, () => void updateConnStatusline());
  } catch {
    // 忽略暫時性錯誤（後端尚未就緒或裝置指紋不可用）。
  }
}

function showLoginOverlay(): void {
  const overlay = document.createElement("div");
  overlay.className = "center";
  overlay.style.position = "fixed";
  overlay.style.inset = "0";
  overlay.style.background = "rgba(0,0,0,0.4)";
  overlay.style.zIndex = "1000";
  const panel = document.createElement("div");
  panel.className = "unlock-box";
  overlay.append(panel);
  document.body.append(overlay);

  renderLogin(panel, () => {
    overlay.remove();
    void updateConnStatusline();
  });

  overlay.onclick = (e) => {
    if (e.target === overlay) overlay.remove();
  };
}

async function updateStatusline(): Promise<void> {
  const el = document.getElementById("statusline");
  if (!el) return;
  try {
    const s = await api.getTrackingStatus();
    el.innerHTML = "";
    const dot = document.createElement("span");
    dot.className = "dot " + (s.paused ? "off" : "on");
    const state = document.createElement("span");
    state.textContent = s.paused ? t.status.paused : t.status.tracking;
    const cur = document.createElement("span");
    cur.className = "muted";
    cur.textContent = s.current_app
      ? `${s.current_app}${s.current_website ? " · " + s.current_website : ""}`
      : t.status.none;
    el.append(dot, state, cur);
  } catch {
    // 忽略暫時性錯誤。
  }
}

void boot();
