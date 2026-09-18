/**
 * The only place the frontend talks to the backend. Every backend call is a typed function
 * here; components never import `@tauri-apps/api` directly.
 *
 * When the page is opened in a plain browser (e.g. `pnpm dev` without Tauri) a small in-memory
 * mock backend is used so the UI can be developed and tested without native code.
 */
import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import type {
  AppDetail,
  ApplicationInfo,
  AppMeta,
  AppsProgress,
  CleanupPlan,
  CleanupProgress,
  CleanupResult,
  CustomIssue,
  DeleteMode,
  DiskNodeView,
  DockerAction,
  DockerPruneResult,
  DockerState,
  DiskProgress,
  DiskSummary,
  LargeFile,
  OperationRecord,
  Permissions,
  ProcessInfo,
  ProviderInfo,
  ScanProgress,
  ScanSession,
  SettingsView,
  StopMode,
  StartupItem,
  SystemInfo,
  SystemSnapshot,
} from "@/types/models";
import { mockBackend } from "./mock";

export const isTauri: boolean = typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;

export const EVENTS = {
  scanProgress: "prune://scan-progress",
  scanCompleted: "prune://scan-completed",
  cleanupProgress: "prune://cleanup-progress",
  diskProgress: "prune://disk-progress",
  diskCompleted: "prune://disk-completed",
  appsProgress: "prune://apps-progress",
  appsCompleted: "prune://apps-completed",
} as const;

export interface EventMap {
  [EVENTS.scanProgress]: ScanProgress;
  [EVENTS.scanCompleted]: ScanSession;
  [EVENTS.cleanupProgress]: CleanupProgress;
  [EVENTS.diskProgress]: DiskProgress;
  [EVENTS.diskCompleted]: DiskSummary;
  [EVENTS.appsProgress]: AppsProgress;
  [EVENTS.appsCompleted]: ApplicationInfo[];
}

export interface Backend {
  appGetMeta(): Promise<AppMeta>;
  appGetPermissions(): Promise<Permissions>;
  appOpenPrivacySettings(): Promise<void>;
  providersCustomIssues(): Promise<CustomIssue[]>;
  systemGetInfo(): Promise<SystemInfo>;
  systemGetSnapshot(): Promise<SystemSnapshot>;
  systemListProcesses(limit?: number): Promise<ProcessInfo[]>;
  systemStopProcess(pid: number, mode: StopMode): Promise<void>;
  cleanerListProviders(): Promise<ProviderInfo[]>;
  cleanerStartScan(providerIds?: string[]): Promise<string>;
  cleanerCancelScan(scanId: string): Promise<void>;
  cleanerGetScan(scanId: string): Promise<ScanSession>;
  cleanerPreview(scanId: string, targetIds: string[], mode: DeleteMode): Promise<CleanupPlan>;
  cleanerExecute(planId: string): Promise<CleanupResult>;
  diskStartScan(root?: string): Promise<string>;
  diskCancelScan(scanId: string): Promise<void>;
  diskGetSummary(scanId: string): Promise<DiskSummary>;
  diskGetNode(scanId: string, path?: string): Promise<DiskNodeView>;
  diskLargeFiles(scanId: string, minBytes: number, limit?: number): Promise<LargeFile[]>;
  dockerStatus(): Promise<DockerState>;
  dockerPrune(action: DockerAction): Promise<DockerPruneResult>;
  appsStartScan(): Promise<ApplicationInfo[]>;
  appsGetDetail(appId: string): Promise<AppDetail>;
  appsRunUninstaller(appId: string): Promise<void>;
  settingsGet(): Promise<SettingsView>;
  settingsSetProjectRoots(roots: string[]): Promise<SettingsView>;
  startupList(): Promise<StartupItem[]>;
  startupSetEnabled(itemId: string, enabled: boolean): Promise<StartupItem>;
  opsList(limit?: number): Promise<OperationRecord[]>;
  fsReveal(path: string): Promise<void>;
  on<K extends keyof EventMap>(event: K, cb: (payload: EventMap[K]) => void): Promise<UnlistenFn>;
}

const tauriBackend: Backend = {
  appGetMeta: () => invoke<AppMeta>("app_get_meta"),
  appGetPermissions: () => invoke<Permissions>("app_get_permissions"),
  appOpenPrivacySettings: () => invoke<void>("app_open_privacy_settings"),
  providersCustomIssues: () => invoke<CustomIssue[]>("providers_custom_issues"),
  systemGetInfo: () => invoke<SystemInfo>("system_get_info"),
  systemGetSnapshot: () => invoke<SystemSnapshot>("system_get_snapshot"),
  systemListProcesses: (limit) => invoke<ProcessInfo[]>("system_list_processes", { limit }),
  systemStopProcess: (pid, mode) => invoke<void>("system_stop_process", { pid, mode }),
  cleanerListProviders: () => invoke<ProviderInfo[]>("cleaner_list_providers"),
  cleanerStartScan: (providerIds) => invoke<string>("cleaner_start_scan", { providerIds }),
  cleanerCancelScan: (scanId) => invoke<void>("cleaner_cancel_scan", { scanId }),
  cleanerGetScan: (scanId) => invoke<ScanSession>("cleaner_get_scan", { scanId }),
  cleanerPreview: (scanId, targetIds, mode) =>
    invoke<CleanupPlan>("cleaner_preview", { scanId, targetIds, mode }),
  cleanerExecute: (planId) => invoke<CleanupResult>("cleaner_execute", { planId }),
  diskStartScan: (root) => invoke<string>("disk_start_scan", { root }),
  diskCancelScan: (scanId) => invoke<void>("disk_cancel_scan", { scanId }),
  diskGetSummary: (scanId) => invoke<DiskSummary>("disk_get_summary", { scanId }),
  diskGetNode: (scanId, path) => invoke<DiskNodeView>("disk_get_node", { scanId, path }),
  diskLargeFiles: (scanId, minBytes, limit) =>
    invoke<LargeFile[]>("disk_large_files", { scanId, minBytes, limit }),
  dockerStatus: () => invoke<DockerState>("docker_status"),
  dockerPrune: (action) => invoke<DockerPruneResult>("docker_prune", { action }),
  appsStartScan: () => invoke<ApplicationInfo[]>("apps_start_scan"),
  appsGetDetail: (appId) => invoke<AppDetail>("apps_get_detail", { appId }),
  appsRunUninstaller: (appId) => invoke<void>("apps_run_uninstaller", { appId }),
  settingsGet: () => invoke<SettingsView>("settings_get"),
  settingsSetProjectRoots: (roots) => invoke<SettingsView>("settings_set_project_roots", { roots }),
  startupList: () => invoke<StartupItem[]>("startup_list"),
  startupSetEnabled: (itemId, enabled) =>
    invoke<StartupItem>("startup_set_enabled", { itemId, enabled }),
  opsList: (limit) => invoke<OperationRecord[]>("ops_list", { limit }),
  fsReveal: (path) => invoke<void>("fs_reveal", { path }),
  on: (event, cb) => listen<EventMap[typeof event]>(event, (e) => cb(e.payload)),
};

export const backend: Backend = isTauri ? tauriBackend : mockBackend;

/** Normalizes anything thrown by `invoke` into a readable message. */
export function errorMessage(err: unknown): string {
  if (err && typeof err === "object" && "message" in err) {
    const e = err as { code?: string; message: string };
    return e.code ? `${e.message} (${e.code})` : e.message;
  }
  return String(err);
}
