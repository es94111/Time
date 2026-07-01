// 顯示「已連線」／待同步狀態（T032，擴充顯示 pending_queue_count／last_sync_error，T044）。

import { api, errText, type ConnectionStatus } from "../api";
import { t } from "../i18n/zh-TW";

function syncErrorText(code: string | null): string | null {
  switch (code) {
    case "device_removed":
      return t.login.syncErrorDeviceRemoved;
    case "account_disabled":
      return t.login.syncErrorAccountDisabled;
    case "session_expired":
      return t.login.syncErrorSessionExpired;
    case null:
    case undefined:
      return null;
    default:
      return t.login.syncErrorNetwork;
  }
}

/** 渲染連線狀態列（供嵌入頂部狀態列或設定頁）。呼叫端負責定時呼叫以更新。 */
export function renderConnectionStatus(el: HTMLElement, status: ConnectionStatus, onLogout: () => void): void {
  el.innerHTML = "";

  const dot = document.createElement("span");
  dot.className = "dot " + (status.logged_in ? "on" : "off");

  const state = document.createElement("span");
  state.textContent = status.logged_in
    ? t.login.loggedInAs(status.account_hint ?? "")
    : t.login.notLoggedIn;

  el.append(dot, state);

  if (status.logged_in) {
    const queue = document.createElement("span");
    queue.className = "muted";
    queue.textContent = " · " + t.login.pendingQueue(status.pending_queue_count);
    el.append(queue);

    if (status.last_successful_sync_at) {
      const last = document.createElement("span");
      last.className = "muted";
      last.textContent = " · " + t.login.lastSync(status.last_successful_sync_at);
      el.append(last);
    }

    const errMsg = syncErrorText(status.last_sync_error);
    if (errMsg) {
      const errEl = document.createElement("span");
      errEl.className = "muted";
      errEl.textContent = " · " + errMsg;
      el.append(errEl);
    }

    const logoutBtn = document.createElement("button");
    logoutBtn.className = "btn secondary";
    logoutBtn.textContent = t.login.logout;
    logoutBtn.onclick = async () => {
      try {
        await api.logout();
        onLogout();
      } catch (e) {
        // eslint-disable-next-line no-console
        console.error(errText(e));
      }
    };
    el.append(logoutBtn);
  }
}

export async function fetchConnectionStatus(): Promise<ConnectionStatus> {
  return api.getConnectionStatus();
}
