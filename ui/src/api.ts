// 與 Rust 後端的 IPC 封裝（對應 contracts/tauri-commands.md）。
// 使用 withGlobalTauri 提供的 window.__TAURI__.core.invoke。

type Invoke = <T>(cmd: string, args?: Record<string, unknown>) => Promise<T>;

const invoke: Invoke = (cmd, args) => {
  const tauri = (window as unknown as { __TAURI__?: { core?: { invoke?: Invoke } } }).__TAURI__;
  if (!tauri?.core?.invoke) {
    return Promise.reject({ code: "NO_TAURI", message_zh: "無法連線到後端" });
  }
  return tauri.core.invoke(cmd, args);
};

export interface NamedTotal {
  name: string;
  active_ms: number;
}
export interface DaySummary {
  local_date: string;
  total_active_ms: number;
  apps: NamedTotal[];
  websites: NamedTotal[];
}
export interface RangeSummary {
  from: string;
  to: string;
  total_active_ms: number;
  apps: NamedTotal[];
  websites: NamedTotal[];
}
export interface TrackingStatus {
  paused: boolean;
  current_app: string | null;
  current_website: string | null;
  since_utc: number | null;
}
export interface Settings {
  idle_threshold_sec: number;
  tracking_paused: boolean;
  autostart_enabled: boolean;
  master_password_set: boolean;
  ui_language: string;
}
export interface SettingsPatch {
  idle_threshold_sec?: number;
  tracking_paused?: boolean;
  autostart_enabled?: boolean;
  ui_language?: string;
}
export interface Exclusion {
  id: number;
  kind: string;
  pattern: string;
}
export interface LockState {
  locked: boolean;
  password_protected: boolean;
}
export interface ExportResult {
  written_path: string;
  row_count: number;
}
export interface CommandError {
  code: string;
  message_zh: string;
}

export const api = {
  getTrackingStatus: () => invoke<TrackingStatus>("get_tracking_status"),
  getLockState: () => invoke<LockState>("get_lock_state"),
  getTodaySummary: () => invoke<DaySummary>("get_today_summary"),
  getSummaryByDate: (date: string) => invoke<DaySummary>("get_summary_by_date", { date }),
  getSummaryRange: (from: string, to: string) =>
    invoke<RangeSummary>("get_summary_range", { from, to }),
  pause: () => invoke<TrackingStatus>("pause_tracking"),
  resume: () => invoke<TrackingStatus>("resume_tracking"),
  getSettings: () => invoke<Settings>("get_settings"),
  updateSettings: (patch: SettingsPatch) => invoke<Settings>("update_settings", { patch }),
  listExclusions: () => invoke<Exclusion[]>("list_exclusions"),
  addExclusion: (kind: "app" | "website", pattern: string) =>
    invoke<Exclusion>("add_exclusion", { kind, pattern }),
  removeExclusion: (id: number) => invoke<void>("remove_exclusion", { id }),
  exportData: (opts: {
    format: "csv" | "json";
    scope: "all" | "range";
    from?: string;
    to?: string;
    target_path: string;
  }) => invoke<ExportResult>("export_data", { opts }),
  clearData: (scope: "all" | "range", from?: string, to?: string) =>
    invoke<{ deleted_rows: number }>("clear_data", { scope, from, to }),
  unlock: (password: string) => invoke<boolean>("unlock", { password }),
  setMasterPassword: (newPassword: string, currentPassword?: string) =>
    invoke<void>("set_master_password", {
      new_password: newPassword,
      current_password: currentPassword ?? null,
    }),
};

export function errText(e: unknown): string {
  if (e && typeof e === "object" && "message_zh" in e) {
    return String((e as CommandError).message_zh);
  }
  return String(e);
}
