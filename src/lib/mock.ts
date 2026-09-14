/**
 * Browser-only mock backend. Produces plausible data and simulates a scan with progress events
 * so the UI can be exercised without Tauri. Never used inside the desktop app.
 */
import type { UnlistenFn } from "@tauri-apps/api/event";
import type {
  AppDetail,
  ApplicationInfo,
  CleanupPlan,
  CleanupResult,
  CleanupTarget,
  DiskNode,
  DiskNodeView,
  DiskSummary,
  LargeFile,
  OperationRecord,
  ProviderInfo,
  ScanResult,
  ScanSession,
  StartupItem,
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
const diskScans = new Map<string, { root: string; cancelled: boolean }>();

const MOCK_STARTUP: StartupItem[] = [
  {
    id: "su-docker",
    name: "Docker Desktop",
    label: "com.docker.helper",
    path: `${HOME}/Library/LaunchAgents/com.docker.helper.plist`,
    command:
      "/Applications/Docker.app/Contents/MacOS/Docker Desktop.app/Contents/MacOS/Docker Desktop",
    enabled: true,
    source: "launch_agent",
    scope: "user",
    trigger: "at_login",
    canToggle: true,
  },
  {
    id: "su-drive",
    name: "Google Drive",
    label: "com.google.drivefs",
    path: `${HOME}/Library/LaunchAgents/com.google.drivefs.plist`,
    command: "/Applications/Google Drive.app/Contents/MacOS/Google Drive",
    enabled: true,
    source: "launch_agent",
    scope: "user",
    trigger: "at_login",
    canToggle: true,
  },
  {
    id: "su-keystone",
    name: "Keystone Agent",
    label: "com.google.keystone.agent",
    path: `${HOME}/Library/LaunchAgents/com.google.keystone.agent.plist`,
    command: `${HOME}/Library/Google/GoogleSoftwareUpdate/GoogleSoftwareUpdate.bundle/Contents/MacOS/GoogleSoftwareUpdateAgent`,
    enabled: true,
    source: "launch_agent",
    scope: "user",
    trigger: "scheduled",
    canToggle: true,
  },
  {
    id: "su-spotify",
    name: "Spotify Web Helper",
    label: "com.spotify.webhelper",
    path: `${HOME}/Library/LaunchAgents/com.spotify.webhelper.plist`,
    command: "/Applications/Spotify.app/Contents/MacOS/Spotify --autostart",
    enabled: false,
    source: "launch_agent",
    scope: "user",
    trigger: "at_login",
    canToggle: true,
  },
  {
    id: "su-adobe",
    name: "Adobe Updater",
    label: "com.adobe.updater",
    path: `${HOME}/Library/LaunchAgents/com.adobe.updater.plist`,
    command: "/Library/Application Support/Adobe/Updater/Adobe Updater",
    enabled: true,
    source: "launch_agent",
    scope: "user",
    trigger: "keep_alive",
    canToggle: true,
  },
  {
    id: "su-vendor",
    name: "Vendor Daemon",
    label: "com.vendor.daemon",
    path: "/Library/LaunchDaemons/com.vendor.daemon.plist",
    command: "/usr/local/bin/vendord",
    enabled: true,
    source: "launch_daemon",
    scope: "system",
    trigger: "keep_alive",
    canToggle: false,
    reason: "Installed for all users; change it with administrator rights.",
  },
];

const MOCK_APPS: ApplicationInfo[] = [
  {
    id: "app-vscode",
    name: "Visual Studio Code",
    path: "/Applications/Visual Studio Code.app",
    version: "1.104.0",
    bundleId: "com.microsoft.VSCode",
    source: "applications",
    isSystem: false,
  },
  {
    id: "app-docker",
    name: "Docker",
    path: "/Applications/Docker.app",
    version: "4.44.1",
    bundleId: "com.docker.docker",
    source: "applications",
    isSystem: false,
  },
  {
    id: "app-xcode",
    name: "Xcode",
    path: "/Applications/Xcode.app",
    version: "26.0",
    bundleId: "com.apple.dt.Xcode",
    source: "applications",
    isSystem: true,
  },
  {
    id: "app-slack",
    name: "Slack",
    path: "/Applications/Slack.app",
    version: "4.45.64",
    bundleId: "com.tinyspeck.slackmacgap",
    source: "applications",
    isSystem: false,
  },
  {
    id: "app-spotify",
    name: "Spotify",
    path: "/Applications/Spotify.app",
    version: "1.2.70",
    bundleId: "com.spotify.client",
    source: "applications",
    isSystem: false,
  },
  {
    id: "app-figma",
    name: "Figma",
    path: `${HOME}/Applications/Figma.app`,
    version: "125.3",
    bundleId: "com.figma.Desktop",
    source: "user_applications",
    isSystem: false,
  },
  {
    id: "app-safari",
    name: "Safari",
    path: "/Applications/Safari.app",
    version: "26.0",
    bundleId: "com.apple.Safari",
    source: "applications",
    isSystem: true,
  },
];
const MOCK_APP_SIZES: Record<string, number> = {
  "app-vscode": 620e6,
  "app-docker": 1.9e9,
  "app-xcode": 31e9,
  "app-slack": 410e6,
  "app-spotify": 380e6,
  "app-figma": 290e6,
  "app-safari": 30e6,
};

function mockAppDetail(app: ApplicationInfo): AppDetail {
  const lib = `${HOME}/Library`;
  const bid = app.bundleId ?? app.name;
  const t = (
    kind: AppDetail["items"][number]["kind"],
    label: string,
    path: string,
    size: number,
    risk: CleanupTarget["risk"],
  ) => ({
    kind,
    kindLabel: label,
    target: {
      id: `${app.id}:${kind}`,
      providerId: "uninstaller",
      path,
      kind: "directory" as const,
      sizeBytes: size,
      fileCount: Math.max(1, Math.round(size / 60_000)),
      risk,
      label: path.split("/").pop() ?? path,
      description: label,
      permanentOnly: false,
      modifiedAt: new Date(Date.now() - 5 * 86400e3).toISOString(),
    },
  });
  const size = MOCK_APP_SIZES[app.id] ?? 200e6;
  const items = [
    t("application", "Application", app.path, size, app.isSystem ? "protected" : "medium"),
    t("caches", "Caches", `${lib}/Caches/${bid}`, size * 0.6, "safe"),
    t(
      "application_support",
      "Application Support",
      `${lib}/Application Support/${app.name === "Visual Studio Code" ? "Code" : bid}`,
      size * 0.9,
      "medium",
    ),
    t("preferences", "Preferences", `${lib}/Preferences/${bid}.plist`, 12e3, "low"),
    t(
      "saved_state",
      "Saved State",
      `${lib}/Saved Application State/${bid}.savedState`,
      240e3,
      "safe",
    ),
    t("web_kit", "WebKit Storage", `${lib}/WebKit/${bid}`, 8e6, "safe"),
    t("logs", "Logs", `${lib}/Logs/${app.name}`, 30e6, "safe"),
  ];
  const totalBytes = items.reduce((a, i) => a + i.target.sizeBytes, 0);
  return { app: { ...app, sizeBytes: size }, items, totalBytes, leftoverBytes: totalBytes - size };
}

function mockLargeFiles(root: string): LargeFile[] {
  const gb = 1e9;
  const mk = (
    rel: string,
    size: number,
    ext: string,
    risk: LargeFile["risk"] = "medium",
  ): LargeFile => ({
    targetId: `lf-${rel.replace(/[^a-z0-9]/gi, "_")}`,
    path: `${root}/${rel}`,
    name: rel.split("/").pop() ?? rel,
    sizeBytes: size,
    modifiedAt: new Date(Date.now() - 12 * 86400e3).toISOString(),
    extension: ext,
    risk,
  });
  return [
    mk("Virtual Machines/Windows 11.utm/Data/disk.qcow2", 12.4 * gb, "qcow2"),
    mk(
      "Library/Containers/com.docker.docker/Data/vms/0/data/Docker.raw",
      8.2 * gb,
      "raw",
      "protected",
    ),
    mk(
      "Library/Developer/Xcode/Archives/2026-08-27/Prune.xcarchive/dSYMs/all.dSYM",
      6.8 * gb,
      "dsym",
    ),
    mk("Movies/screen-recording-2026-07-01.mov", 4.2 * gb, "mov"),
    mk("Downloads/dataset-v3.zip", 3.8 * gb, "zip"),
    mk("Downloads/Xcode_26.xip", 3.1 * gb, "xip"),
    mk("Projects/ml/checkpoints/model-final.safetensors", 2.6 * gb, "safetensors"),
    mk("Downloads/ubuntu-24.04-live-server-arm64.iso", 1.9 * gb, "iso"),
    mk("Library/Caches/com.apple.dt.Xcode/Downloads/ios-simulator.dmg", 1.2 * gb, "dmg"),
    mk(
      "Pictures/Photos Library.photoslibrary/database/Photos.sqlite",
      620e6,
      "sqlite",
      "protected",
    ),
    mk("Documents/thesis/figures/render.blend", 340e6, "blend"),
  ];
}

function mockNode(root: string, path: string): DiskNodeView {
  const node = (
    name: string,
    p: string,
    size: number,
    files: number,
    dirs: number,
    childDirs = 4,
    own = 0,
  ): DiskNode => ({
    name,
    path: p,
    sizeBytes: size,
    fileCount: files,
    dirCount: dirs,
    childDirs,
    ownFileBytes: own,
  });
  const rel = path === root ? "" : path.slice(root.length + 1);
  const crumbs: DiskNode[] = [];
  let acc = root;
  if (rel) {
    crumbs.push(node(root.split("/").pop() ?? root, root, 382e9, 1_204_000, 88_000));
    const parts = rel.split("/");
    for (let i = 0; i < parts.length - 1; i++) {
      acc = `${acc}/${parts[i]}`;
      crumbs.push(node(parts[i] ?? "", acc, 184e9 / (i + 1), 500_000, 30_000));
    }
  }
  const depth = rel ? rel.split("/").length : 0;
  const scale = 1 / (depth + 1);
  const children =
    depth > 3
      ? []
      : [
          node("Projects", `${path}/Projects`, 184e9 * scale, 820_000, 40_000),
          node("Library", `${path}/Library`, 96e9 * scale, 300_000, 32_000),
          node("Downloads", `${path}/Downloads`, 42e9 * scale, 1_200, 40),
          node("Movies", `${path}/Movies`, 22e9 * scale, 140, 6),
          node("Documents", `${path}/Documents`, 22e9 * scale, 50_000, 4_000),
          node("Pictures", `${path}/Pictures`, 9e9 * scale, 30_000, 300),
          node(".cache", `${path}/.cache`, 4e9 * scale, 3_000, 200),
        ];
  const size = children.reduce((a, c) => a + c.sizeBytes, 0) + 3e9 * scale;
  return {
    node: node(
      path.split("/").pop() ?? path,
      path,
      size,
      1_204_000 * scale,
      88_000 * scale,
      children.length,
      3e9 * scale,
    ),
    children,
    breadcrumbs: crumbs,
  };
}
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
  async appGetPermissions() {
    return {
      fullDiskAccess: "denied" as const,
      blocked: ["Trash", "Safari data"],
      howToGrant:
        "System Settings → Privacy & Security → Full Disk Access, then add Prune and restart it.",
    };
  },
  async appOpenPrivacySettings() {
    console.info("[mock] open privacy settings");
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
  async diskStartScan(root) {
    const id = `disk-${Date.now()}`;
    const base = root && root.trim() ? root.trim() : HOME;
    diskScans.set(id, { root: base, cancelled: false });
    const total = 382e9;
    for (let s = 1; s <= 6; s++) {
      setTimeout(() => {
        const d = diskScans.get(id);
        if (!d || d.cancelled) return;
        emit("prune://disk-progress", {
          scanId: id,
          files: Math.round((1_204_000 * s) / 6),
          bytes: Math.round((total * s) / 6),
          dirs: Math.round((88_000 * s) / 6),
          currentPath: `${base}/Library/Developer/Xcode/DerivedData/item-${s}`,
        });
      }, 180 * s);
    }
    setTimeout(
      () => {
        const d = diskScans.get(id);
        if (!d || d.cancelled) return;
        const summary: DiskSummary = {
          scanId: id,
          root: base,
          status: "completed",
          totalBytes: total,
          fileCount: 1_204_000,
          dirCount: 88_000,
          durationMs: 1200,
          largeFileCount: mockLargeFiles(base).length,
          issueCount: 2,
          topExtensions: [
            { extension: "(none)", bytes: 96e9, count: 402_000 },
            { extension: "o", bytes: 61e9, count: 210_000 },
            { extension: "dmg", bytes: 24e9, count: 60 },
            { extension: "mov", bytes: 22e9, count: 140 },
            { extension: "js", bytes: 18e9, count: 380_000 },
            { extension: "png", bytes: 9e9, count: 88_000 },
            { extension: "zip", bytes: 7e9, count: 900 },
            { extension: "json", bytes: 5e9, count: 120_000 },
          ],
          startedAt: new Date().toISOString(),
        };
        // register large files as a session so cleanerPreview works
        const targets = mockLargeFiles(base).map((f): CleanupTarget => ({
          id: f.targetId,
          providerId: "large_files",
          path: f.path,
          kind: "file",
          sizeBytes: f.sizeBytes,
          fileCount: 1,
          risk: f.risk,
          label: f.name,
          description: f.extension,
          modifiedAt: f.modifiedAt,
          permanentOnly: false,
        }));
        sessions.set(id, {
          id,
          status: "completed",
          startedAt: summary.startedAt,
          finishedAt: summary.startedAt,
          results: [
            {
              providerId: "large_files",
              providerName: "Large Files",
              category: "large_files",
              targets,
              totalBytes: targets.reduce((a, t) => a + t.sizeBytes, 0),
              totalFiles: targets.length,
              durationMs: 0,
              issues: [],
            },
          ],
          totalBytes: 0,
          totalFiles: 0,
        });
        emit("prune://disk-completed", summary);
      },
      180 * 6 + 300,
    );
    return id;
  },
  async diskCancelScan(scanId) {
    const d = diskScans.get(scanId);
    if (d) {
      d.cancelled = true;
      emit("prune://disk-completed", {
        scanId,
        root: d.root,
        status: "cancelled",
        totalBytes: 120e9,
        fileCount: 300_000,
        dirCount: 20_000,
        durationMs: 400,
        largeFileCount: 0,
        issueCount: 0,
        topExtensions: [],
        startedAt: new Date().toISOString(),
      });
    }
  },
  async diskGetSummary(scanId) {
    const d = diskScans.get(scanId);
    if (!d) throw { code: "unknown_scan", message: scanId };
    return {
      scanId,
      root: d.root,
      status: "completed",
      totalBytes: 382e9,
      fileCount: 1_204_000,
      dirCount: 88_000,
      durationMs: 1200,
      largeFileCount: mockLargeFiles(d.root).length,
      issueCount: 2,
      topExtensions: [],
      startedAt: new Date().toISOString(),
    };
  },
  async diskGetNode(scanId, path) {
    const d = diskScans.get(scanId);
    if (!d) throw { code: "unknown_scan", message: scanId };
    return mockNode(d.root, path ?? d.root);
  },
  async diskLargeFiles(scanId, minBytes, limit = 200) {
    const d = diskScans.get(scanId);
    if (!d) throw { code: "unknown_scan", message: scanId };
    const removed = new Set(
      sessions.get(scanId)?.results.flatMap((r) => r.targets.map((t) => t.id)) ?? [],
    );
    return mockLargeFiles(d.root)
      .filter((f) => f.sizeBytes >= minBytes && removed.has(f.targetId))
      .slice(0, limit);
  },
  async appsStartScan() {
    const id = `apps-${Date.now()}`;
    MOCK_APPS.forEach((a, i) => {
      setTimeout(
        () => {
          emit("prune://apps-progress", {
            scanId: id,
            done: i + 1,
            total: MOCK_APPS.length,
            app: { ...a, sizeBytes: MOCK_APP_SIZES[a.id] },
          });
          if (i === MOCK_APPS.length - 1)
            emit(
              "prune://apps-completed",
              MOCK_APPS.map((x) => ({ ...x, sizeBytes: MOCK_APP_SIZES[x.id] })),
            );
        },
        150 * (i + 1),
      );
    });
    return MOCK_APPS;
  },
  async appsGetDetail(appId) {
    const app = MOCK_APPS.find((a) => a.id === appId);
    if (!app) throw { code: "unknown_app", message: appId };
    await new Promise((r) => setTimeout(r, 250));
    const detail = mockAppDetail(app);
    const sid = `app:${appId}`;
    const targets = detail.items.map((i) => i.target);
    sessions.set(sid, {
      id: sid,
      status: "completed",
      startedAt: new Date().toISOString(),
      finishedAt: new Date().toISOString(),
      results: [
        {
          providerId: "uninstaller",
          providerName: `Uninstall ${app.name}`,
          category: "applications",
          targets,
          totalBytes: detail.totalBytes,
          totalFiles: targets.length,
          durationMs: 0,
          issues: [],
        },
      ],
      totalBytes: detail.totalBytes,
      totalFiles: targets.length,
    });
    return detail;
  },
  async appsRunUninstaller() {
    throw { code: "not_implemented", message: "vendor uninstallers exist only on Windows" };
  },
  async startupList() {
    await new Promise((r) => setTimeout(r, 200));
    return MOCK_STARTUP;
  },
  async startupSetEnabled(itemId, enabled) {
    const item = MOCK_STARTUP.find((i) => i.id === itemId);
    if (!item) throw { code: "unknown_startup_item", message: itemId };
    if (!item.canToggle) throw { code: "other", message: item.reason ?? "cannot be changed" };
    await new Promise((r) => setTimeout(r, 150));
    item.enabled = enabled;
    return item;
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
