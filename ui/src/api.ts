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

/** 事件解除訂閱函式。 */
export type UnlistenFn = () => void;

type Listen = <T>(event: string, handler: (e: { payload: T }) => void) => Promise<UnlistenFn>;

/** 訂閱後端事件（withGlobalTauri 之 window.__TAURI__.event.listen）。 */
function listen<T>(event: string, handler: (payload: T) => void): Promise<UnlistenFn> {
  const tauri = (window as unknown as { __TAURI__?: { event?: { listen?: Listen } } }).__TAURI__;
  if (!tauri?.event?.listen) {
    return Promise.reject({ code: "NO_TAURI", message_zh: "無法連線到後端事件" });
  }
  return tauri.event.listen<T>(event, (e) => handler(e.payload));
}

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

// ---- 系統指標（002-system-metrics，對應 contracts/tauri-commands.md）----

export interface DiskReading {
  id: number;
  name: string | null;
  readBps: number | null;
  writeBps: number | null;
}
export interface NetReading {
  id: number;
  name: string | null;
  rxBps: number | null;
  txBps: number | null;
}
export interface GpuReading {
  id: number;
  name: string | null;
  utilPct: number | null;
  memUsedBytes: number | null;
  memTotalBytes: number | null;
}
export interface MetricSnapshot {
  tsUtc: number;
  cpuPct: number | null;
  memUsedBytes: number | null;
  memTotalBytes: number | null;
  disks: DiskReading[];
  nets: NetReading[];
  gpus: GpuReading[];
}
export interface DeviceInfo {
  id: number;
  name: string | null;
}
export interface DeviceLists {
  disks: DeviceInfo[];
  nets: DeviceInfo[];
  gpus: DeviceInfo[];
}
export interface MetricsSettings {
  sampleIntervalSec: number;
  retentionDays: number;
  enabled: boolean;
}
export interface MetricsSettingsPatch {
  sampleIntervalSec?: number;
  retentionDays?: number;
  enabled?: boolean;
}
export interface TrendDisk {
  id: number;
  name: string | null;
  readAvg: number | null;
  readMax: number | null;
  writeAvg: number | null;
  writeMax: number | null;
}
export interface TrendNet {
  id: number;
  name: string | null;
  rxAvg: number | null;
  rxMax: number | null;
  txAvg: number | null;
  txMax: number | null;
}
export interface TrendGpu {
  id: number;
  name: string | null;
  utilAvg: number | null;
  utilMax: number | null;
  memUsedAvg: number | null;
  memTotalBytes: number | null;
}
export interface TrendPoint {
  tsUtc: number;
  cpuAvg: number | null;
  cpuMax: number | null;
  memUsedAvg: number | null;
  memTotalBytes: number | null;
  disks: TrendDisk[];
  nets: TrendNet[];
  gpus: TrendGpu[];
}
export interface TrendSeries {
  bucketMs: number;
  points: TrendPoint[];
}

// ---- 遠端同步與登入（003-remote-sync-web-login，對應 contracts/tauri-commands.md）----

export interface ConnectionStatus {
  logged_in: boolean;
  account_hint: string | null;
  last_successful_sync_at: string | null;
  pending_queue_count: number;
  last_sync_error: string | null;
}
export interface LoginError {
  code: "InvalidCredentials" | "AccountLocked" | "NetworkError";
  retry_after_seconds?: number;
  message?: string;
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

  // ---- 系統指標 ----
  getCurrentMetrics: () => invoke<MetricSnapshot | null>("get_current_metrics"),
  getMetricsRange: (fromUtc: number, toUtc: number) =>
    invoke<TrendSeries>("get_metrics_range", { query: { fromUtc, toUtc } }),
  listMetricDevices: () => invoke<DeviceLists>("list_metric_devices"),
  getMetricsSettings: () => invoke<MetricsSettings>("get_metrics_settings"),
  setMetricsSettings: (patch: MetricsSettingsPatch) =>
    invoke<MetricsSettings>("set_metrics_settings", { patch }),
  clearMetricsData: (beforeUtc?: number) =>
    invoke<{ deletedSamples: number }>("clear_metrics_data", { before_utc: beforeUtc ?? null }),
  onMetricsSample: (cb: (snap: MetricSnapshot) => void) =>
    listen<MetricSnapshot>("metrics://sample", cb),

  // ---- 遠端同步與登入 ----
  login: (identifier: string, password: string) =>
    invoke<ConnectionStatus>("login", { identifier, password }),
  logout: () => invoke<void>("logout"),
  getConnectionStatus: () => invoke<ConnectionStatus>("get_connection_status"),
};

export function errText(e: unknown): string {
  if (e && typeof e === "object" && "message_zh" in e) {
    return String((e as CommandError).message_zh);
  }
  return String(e);
}
