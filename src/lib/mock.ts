/**
 * Browser-only mock backend. Produces plausible data and simulates a scan with progress events
 * so the UI can be exercised without Tauri. Never used inside the desktop app.
 */
import type { UnlistenFn } from "@tauri-apps/api/event";
import type {
  CleanupPlan,
  CleanupResult,
  CleanupTarget,
  OperationRecord,
  ProviderInfo,
  ScanResult,
  ScanSession,
} from "@/types/models";
import type { Backend, EventMap } from "./tauri";

type Listener = (payload: unknown) => void;
const listeners = new Map<string, Set<Listener>>();

function emit<K extends keyof EventMap>(event: K, payload: EventMap[K]) {
  listeners.get(event)?.forEach((cb) => cb(payload));
}

const HOME = "/Users/developer";

const PROVIDERS: ProviderInfo[] = [
  {
    id: "user_cache",
    name: "Application Caches",
    category: "application_cache",
    description: "Per-application cache folders. Apps rebuild them on next launch.",
    defaultRisk: "safe",
    available: true,
  },
  {
    id: "user_logs",
    name: "Logs",
    category: "logs",
    description: "Diagnostic logs written by applications.",
    defaultRisk: "safe",
    available: true,
  },
  {
    id: "temp_files",
    name: "Temporary Files",
    category: "temporary_files",
    description: "Temporary files older than a day.",
    defaultRisk: "low",
    available: true,
  },
  {
    id: "trash",
    name: "Trash",
    category: "trash",
    description: "Items already in the Trash.",
    defaultRisk: "safe",
    available: true,
  },
  {
    id: "old_installers",
    name: "Old Installers",
    category: "old_installers",
    description: "Installer images in Downloads older than a week.",
    defaultRisk: "low",
    available: true,
  },
  {
    id: "chromium_cache",
    name: "Chromium Browsers",
    category: "browser_cache",
    description: "HTTP and code caches of Chrome, Brave, Edge, Arc.",
    defaultRisk: "safe",
    available: true,
  },
  {
    id: "firefox_cache",
    name: "Firefox",
    category: "browser_cache",
    description: "Firefox HTTP cache per profile.",
    defaultRisk: "safe",
    available: false,
  },
  {
    id: "npm_cache",
    name: "npm cache",
    category: "developer_files",
    description: "Downloaded package tarballs.",
    defaultRisk: "safe",
    available: true,
  },
  {
    id: "pnpm_store",
    name: "pnpm store",
    category: "developer_files",
    description: "Content-addressable pnpm store.",
    defaultRisk: "low",
    available: true,
  },
  {
    id: "gradle_cache",
    name: "Gradle caches",
    category: "developer_files",
    description: "Dependency and build caches in ~/.gradle.",
    defaultRisk: "low",
    available: true,
  },
  {
    id: "cargo_cache",
    name: "Cargo registry",
    category: "developer_files",
    description: "Downloaded crates and git checkouts.",
    defaultRisk: "low",
    available: true,
  },
  {
    id: "xcode_derived_data",
    name: "Xcode DerivedData",
    category: "developer_files",
    description: "Intermediate build products, per project.",
    defaultRisk: "safe",
    available: true,
  },
  {
    id: "xcode_archives",
    name: "Xcode Archives",
    category: "developer_files",
    description: "App archives. Needed to symbolicate crash logs.",
    defaultRisk: "medium",
    available: true,
  },
  {
    id: "project_artifacts",
    name: "Project Artifacts",
    category: "developer_files",
    description: "node_modules, target/, build/, dist/ inside your projects.",
    defaultRisk: "low",
    available: true,
  },
];

let idCounter = 0;
function target(
  providerId: string,
  path: string,
  sizeBytes: number,
  risk: CleanupTarget["risk"],
  extra?: Partial<CleanupTarget>,
): CleanupTarget {
  idCounter += 1;
  return {
    id: `t${idCounter}`,
    providerId,
    path,
    kind: "directory",
    sizeBytes,
    fileCount: Math.max(1, Math.round(sizeBytes / 48_000)),
    risk,
    label: path.split("/").slice(-1)[0] ?? path,
    permanentOnly: providerId === "trash",
    modifiedAt: new Date(Date.now() - Math.random() * 30 * 86400e3).toISOString(),
    ...extra,
  };
}

function fakeResults(providerIds?: string[]): ScanResult[] {
  const gb = 1_000_000_000;
  const all: Record<string, CleanupTarget[]> = {
    user_cache: [
      target("user_cache", `${HOME}/Library/Caches/com.apple.dt.Xcode`, 3.2 * gb, "safe"),
      target("user_cache", `${HOME}/Library/Caches/com.spotify.client`, 1.1 * gb, "safe"),
      target("user_cache", `${HOME}/Library/Caches/com.microsoft.VSCode`, 640e6, "safe"),
      target("user_cache", `${HOME}/Library/Caches/com.apple.bird`, 120e6, "protected"),
    ],
    user_logs: [
      target("user_logs", `${HOME}/Library/Logs/DiagnosticReports`, 210e6, "safe", {
        kind: "directory",
      }),
    ],
    temp_files: [target("temp_files", `/var/folders/ab/T/TemporaryItems`, 88e6, "low")],
    trash: [
      target("trash", `${HOME}/.Trash/old-project.zip`, 2.4 * gb, "safe", {
        kind: "file",
        fileCount: 1,
      }),
    ],
    old_installers: [
      target("old_installers", `${HOME}/Downloads/Docker.dmg`, 610e6, "low", {
        kind: "file",
        fileCount: 1,
      }),
    ],
    chromium_cache: [
      target(
        "chromium_cache",
        `${HOME}/Library/Caches/Google/Chrome/Default/Cache`,
        1.9 * gb,
        "safe",
        { label: "Google Chrome · Default · Cache" },
      ),
      target(
        "chromium_cache",
        `${HOME}/Library/Application Support/Arc/User Data/Default/Code Cache`,
        320e6,
        "safe",
        { label: "Arc · Default · Code Cache" },
      ),
    ],
    npm_cache: [target("npm_cache", `${HOME}/.npm/_cacache`, 2.1 * gb, "safe")],
    pnpm_store: [target("pnpm_store", `${HOME}/Library/pnpm/store`, 4.6 * gb, "low")],
    gradle_cache: [
      target("gradle_cache", `${HOME}/.gradle/caches`, 3.8 * gb, "low", { label: "Gradle caches" }),
    ],
    cargo_cache: [
      target("cargo_cache", `${HOME}/.cargo/registry/cache`, 1.4 * gb, "low", {
        label: "Crate archives",
      }),
      target("cargo_cache", `${HOME}/.cargo/registry/src`, 2.2 * gb, "low", {
        label: "Extracted crate sources",
      }),
    ],
    xcode_derived_data: [
      target(
        "xcode_derived_data",
        `${HOME}/Library/Developer/Xcode/DerivedData/Prune-abcdef`,
        5.1 * gb,
        "safe",
      ),
      target(
        "xcode_derived_data",
        `${HOME}/Library/Developer/Xcode/DerivedData/ModuleCache.noindex`,
        1.3 * gb,
        "safe",
      ),
    ],
    xcode_archives: [
      target(
        "xcode_archives",
        `${HOME}/Library/Developer/Xcode/Archives/2026-08-12`,
        900e6,
        "medium",
      ),
    ],
    project_artifacts: [
      target("project_artifacts", `${HOME}/Projects/web/node_modules`, 1.7 * gb, "low", {
        label: "web/node_modules",
        description: "node_modules",
      }),
      target("project_artifacts", `${HOME}/Projects/prune/target`, 6.2 * gb, "low", {
        label: "prune/target",
        description: "Rust target",
      }),
      target("project_artifacts", `${HOME}/Projects/api/.venv`, 480e6, "medium", {
        label: "api/.venv",
        description: "Python virtualenv",
      }),
    ],
  };
  return PROVIDERS.filter((p) => p.available)
    .filter((p) => !providerIds || providerIds.includes(p.id))
    .map((p) => {
      const targets = all[p.id] ?? [];
      return {
        providerId: p.id,
        providerName: p.name,
        category: p.category,
        targets,
        totalBytes: targets.reduce((a, t) => a + t.sizeBytes, 0),
        totalFiles: targets.reduce((a, t) => a + t.fileCount, 0),
        durationMs: 120,
        issues:
          p.id === "user_cache"
            ? [
                {
                  path: `${HOME}/Library/Caches/com.apple.Safari`,
                  message: "Operation not permitted",
                },
              ]
            : [],
      };
    });
}

const sessions = new Map<string, ScanSession>();
const plans = new Map<string, CleanupPlan>();
const ops: OperationRecord[] = [];
let cpu = 18;

export const mockBackend: Backend = {
  async appGetMeta() {
    return {
      name: "Prune",
      version: "0.1.0-mock",
      coreVersion: "0.1.0",
      platform: "macos",
      arch: "aarch64",
      debug: true,
      operationLogPath: `${HOME}/Library/Application Support/app.bonjin.prune/operations.jsonl`,
    };
  },
  async systemGetInfo() {
    return {
      platform: "macos",
      osName: "macOS",
      osVersion: "26.0",
      kernelVersion: "25.0.0",
      hostname: "dev-mac",
      arch: "aarch64",
      cpuBrand: "Apple M4 Pro",
      physicalCores: 12,
      logicalCores: 12,
      totalMemoryBytes: 32 * 1024 ** 3,
      homeDir: HOME,
    };
  },
  async systemGetSnapshot() {
    cpu = Math.min(95, Math.max(3, cpu + (Math.random() - 0.5) * 12));
    return {
      cpu: {
        usagePercent: cpu,
        perCorePercent: Array.from({ length: 12 }, () =>
          Math.max(0, cpu + (Math.random() - 0.5) * 40),
        ),
      },
      memory: {
        totalBytes: 32 * 1024 ** 3,
        usedBytes: 12.4 * 1024 ** 3,
        availableBytes: 19.6 * 1024 ** 3,
        swapTotalBytes: 2 * 1024 ** 3,
        swapUsedBytes: 0.3 * 1024 ** 3,
      },
      disks: [
        {
          name: "Macintosh HD",
          mountPoint: "/",
          fileSystem: "apfs",
          totalBytes: 1_000_000_000_000,
          availableBytes: 618_000_000_000,
          isRemovable: false,
          isPrimary: true,
        },
        {
          name: "Backup",
          mountPoint: "/Volumes/Backup",
          fileSystem: "apfs",
          totalBytes: 2_000_000_000_000,
          availableBytes: 900_000_000_000,
          isRemovable: true,
          isPrimary: false,
        },
      ],
      uptimeSeconds: 3 * 86400 + 4 * 3600,
      processCount: 612,
    };
  },
  async systemListProcesses(limit = 50) {
    const names = [
      "WindowServer",
      "Google Chrome Helper",
      "Xcode",
      "node",
      "cargo",
      "Docker",
      "Spotify",
      "Finder",
      "kernel_task",
      "Code Helper (Plugin)",
    ];
    return Array.from({ length: limit }, (_, i) => ({
      pid: 100 + i * 7,
      name: names[i % names.length] ?? "proc",
      cpuPercent: Math.max(0, 40 - i * 3 + Math.random() * 3),
      memoryBytes: (900 - i * 40) * 1e6,
      user: "developer",
      parentPid: 1,
    }));
  },
  async cleanerListProviders() {
    return PROVIDERS;
  },
  async cleanerStartScan(providerIds) {
    const id = `scan-${Date.now()}`;
    const results = fakeResults(providerIds);
    sessions.set(id, {
      id,
      status: "running",
      startedAt: new Date().toISOString(),
      results: [],
      totalBytes: 0,
      totalFiles: 0,
    });
    let done = 0;
    results.forEach((r, i) => {
      const steps = 4;
      for (let s = 1; s <= steps; s++) {
        setTimeout(
          () => {
            const session = sessions.get(id);
            if (!session || session.status === "cancelled") return;
            emit("prune://scan-progress", {
              scanId: id,
              providerId: r.providerId,
              scannedFiles: Math.round((r.totalFiles * s) / steps),
              discoveredBytes: Math.round((r.totalBytes * s) / steps),
              currentPath: r.targets[0]?.path,
              providersDone: done,
              providersTotal: results.length,
            });
            if (s === steps) done += 1;
          },
          150 * i + 120 * s,
        );
      }
    });
    setTimeout(
      () => {
        const session = sessions.get(id);
        if (!session || session.status === "cancelled") return;
        const finished: ScanSession = {
          ...session,
          status: "completed",
          finishedAt: new Date().toISOString(),
          results,
          totalBytes: results.reduce((a, r) => a + r.totalBytes, 0),
          totalFiles: results.reduce((a, r) => a + r.totalFiles, 0),
        };
        sessions.set(id, finished);
        emit("prune://scan-completed", finished);
      },
      150 * results.length + 700,
    );
    return id;
  },
  async cleanerCancelScan(scanId) {
    const s = sessions.get(scanId);
    if (s) {
      const cancelled: ScanSession = {
        ...s,
        status: "cancelled",
        finishedAt: new Date().toISOString(),
      };
      sessions.set(scanId, cancelled);
      emit("prune://scan-completed", cancelled);
    }
  },
  async cleanerGetScan(scanId) {
    const s = sessions.get(scanId);
    if (!s) throw { code: "unknown_scan", message: scanId };
    return s;
  },
  async cleanerPreview(scanId, targetIds, mode) {
    const s = sessions.get(scanId);
    if (!s) throw { code: "unknown_scan", message: scanId };
    const all = s.results.flatMap((r) => r.targets);
    const targets = all.filter((t) => targetIds.includes(t.id) && t.risk !== "protected");
    const blocked = all
      .filter((t) => targetIds.includes(t.id) && t.risk === "protected")
      .map((t) => ({ targetId: t.id, path: t.path, reason: "protected" }));
    const plan: CleanupPlan = {
      id: `plan-${Date.now()}`,
      scanId,
      mode,
      createdAt: new Date().toISOString(),
      targets,
      blocked,
      totalBytes: targets.reduce((a, t) => a + t.sizeBytes, 0),
      fileCount: targets.reduce((a, t) => a + t.fileCount, 0),
      directoryCount: targets.filter((t) => t.kind === "directory").length,
    };
    plans.set(plan.id, plan);
    return plan;
  },
  async cleanerExecute(planId) {
    const plan = plans.get(planId);
    if (!plan) throw { code: "unknown_plan", message: planId };
    plans.delete(planId);
    let removed = 0;
    for (let i = 0; i < plan.targets.length; i++) {
      await new Promise((r) => setTimeout(r, 120));
      removed += plan.targets[i]?.sizeBytes ?? 0;
      emit("prune://cleanup-progress", {
        planId,
        done: i + 1,
        total: plan.targets.length,
        currentPath: plan.targets[i]?.path ?? "",
        removedBytes: removed,
      });
    }
    const session = sessions.get(plan.scanId);
    if (session) {
      const ids = new Set(plan.targets.map((t) => t.id));
      session.results = session.results
        .map((r) => ({ ...r, targets: r.targets.filter((t) => !ids.has(t.id)) }))
        .map((r) => ({
          ...r,
          totalBytes: r.targets.reduce((a, t) => a + t.sizeBytes, 0),
          totalFiles: r.targets.reduce((a, t) => a + t.fileCount, 0),
        }));
      session.totalBytes = session.results.reduce((a, r) => a + r.totalBytes, 0);
      session.totalFiles = session.results.reduce((a, r) => a + r.totalFiles, 0);
    }
    const result: CleanupResult = {
      operationId: `op-${Date.now()}`,
      planId,
      mode: plan.mode,
      status: "success",
      startedAt: plan.createdAt,
      finishedAt: new Date().toISOString(),
      removedTargets: plan.targets.length,
      removedFiles: plan.fileCount,
      removedBytes: plan.totalBytes,
      failed: [],
    };
    ops.unshift({
      id: result.operationId,
      at: result.finishedAt,
      title:
        plan.targets.length === 1
          ? `Clean ${plan.targets[0]?.label}`
          : `Clean ${new Set(plan.targets.map((t) => t.providerId)).size} categories`,
      mode: plan.mode,
      status: "success",
      removedTargets: result.removedTargets,
      removedFiles: result.removedFiles,
      removedBytes: result.removedBytes,
      failedCount: 0,
      providers: [...new Set(plan.targets.map((t) => t.providerId))],
    });
    return result;
  },
  async opsList(limit = 50) {
    return ops.slice(0, limit);
  },
  async fsReveal(path) {
    console.info("[mock] reveal", path);
  },
  async on(event, cb) {
    const set = listeners.get(event) ?? new Set();
    set.add(cb as Listener);
    listeners.set(event, set);
    const unlisten: UnlistenFn = () => {
      set.delete(cb as Listener);
    };
    return unlisten;
  },
};
