// Windows 軟體登入畫面（T031，依 tauri-commands.md 契約）。

import { api, type LoginError } from "../api";
import { t } from "../i18n/zh-TW";

export function renderLogin(root: HTMLElement, onLoggedIn: () => void): void {
  root.innerHTML = "";
  const center = document.createElement("div");
  center.className = "center";
  const box = document.createElement("div");
  box.className = "unlock-box";

  const title = document.createElement("h1");
  title.textContent = t.login.title;
  const hint = document.createElement("p");
  hint.className = "muted";
  hint.textContent = t.login.hint;

  const identifierInput = document.createElement("input");
  identifierInput.type = "text";
  identifierInput.placeholder = t.login.identifier;
  identifierInput.style.width = "100%";
  identifierInput.style.margin = "8px 0";

  const passwordInput = document.createElement("input");
  passwordInput.type = "password";
  passwordInput.placeholder = t.login.password;
  passwordInput.style.width = "100%";
  passwordInput.style.margin = "8px 0";

  const btn = document.createElement("button");
  btn.className = "btn";
  btn.textContent = t.login.login;

  const err = document.createElement("p");
  err.className = "muted";

  const submit = async (): Promise<void> => {
    err.textContent = "";
    btn.disabled = true;
    try {
      await api.login(identifierInput.value, passwordInput.value);
      onLoggedIn();
    } catch (e) {
      err.textContent = loginErrorText(e as LoginError);
    } finally {
      btn.disabled = false;
    }
  };
  btn.onclick = () => void submit();
  passwordInput.onkeydown = (e) => {
    if (e.key === "Enter") void submit();
  };

  box.append(title, hint, identifierInput, passwordInput, btn, err);
  center.append(box);
  root.append(center);
}

function loginErrorText(e: LoginError): string {
  if (!e || typeof e !== "object") return t.login.networkError;
  switch (e.code) {
    case "InvalidCredentials":
      return t.login.invalidCredentials;
    case "AccountLocked":
      return t.login.accountLocked(e.retry_after_seconds ?? 0);
    default:
      return t.login.networkError;
  }
}
