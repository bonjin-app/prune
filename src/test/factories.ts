/**
 * Builders for the models the stores work with. Only the fields a test cares about need to be
 * passed; everything else gets a plausible default.
 */
import type {
  AppDataItem,
  AppDetail,
  ApplicationInfo,
  Category,
  CleanupPlan,
  CleanupResult,
  CleanupTarget,
  LargeFile,
  RiskLevel,
  ScanResult,
  ScanSession,
  StartupItem,
} from "@/types/models";

let counter = 0;
const nextId = () => `id${(counter += 1)}`;

export function resetIds() {
  counter = 0;
}

export function target(overrides: Partial<CleanupTarget> = {}): CleanupTarget {
  const id = overrides.id ?? nextId();
  return {
    id,
    providerId: "user_cache",
    path: `/home/u/Library/Caches/${id}`,
    kind: "directory",
    sizeBytes: 1_000_000,
    fileCount: 10,
    risk: "safe",
    label: id,
    permanentOnly: false,
    ...overrides,
  };
}

export function result(
  providerId: string,
  category: Category,
  targets: CleanupTarget[],
): ScanResult {
  return {
    providerId,
    providerName: providerId,
    category,
    targets,
    totalBytes: targets.reduce((a, t) => a + t.sizeBytes, 0),
    totalFiles: targets.reduce((a, t) => a + t.fileCount, 0),
    durationMs: 1,
    issues: [],
  };
}

export function session(results: ScanResult[], id = "scan1"): ScanSession {
  return {
    id,
    status: "completed",
    startedAt: new Date().toISOString(),
    finishedAt: new Date().toISOString(),
    results,
    totalBytes: results.reduce((a, r) => a + r.totalBytes, 0),
    totalFiles: results.reduce((a, r) => a + r.totalFiles, 0),
  };
}

export function plan(targets: CleanupTarget[], overrides: Partial<CleanupPlan> = {}): CleanupPlan {
  return {
    id: "plan1",
    scanId: "scan1",
    mode: "trash",
    createdAt: new Date().toISOString(),
    targets,
    blocked: [],
    totalBytes: targets.reduce((a, t) => a + t.sizeBytes, 0),
    fileCount: targets.reduce((a, t) => a + t.fileCount, 0),
    directoryCount: targets.filter((t) => t.kind === "directory").length,
    ...overrides,
  };
}

export function cleanupResult(overrides: Partial<CleanupResult> = {}): CleanupResult {
  return {
    operationId: "op1",
    planId: "plan1",
    mode: "trash",
    status: "success",
    startedAt: new Date().toISOString(),
    finishedAt: new Date().toISOString(),
    removedTargets: 1,
    removedFiles: 10,
    removedBytes: 1_000_000,
    failed: [],
    ...overrides,
  };
}

export function largeFile(overrides: Partial<LargeFile> = {}): LargeFile {
  const id = overrides.targetId ?? nextId();
  return {
    targetId: id,
    path: `/home/u/Movies/${id}.mov`,
    name: `${id}.mov`,
    sizeBytes: 2_000_000_000,
    extension: "mov",
    risk: "medium",
    ...overrides,
  };
}

export function application(overrides: Partial<ApplicationInfo> = {}): ApplicationInfo {
  const id = overrides.id ?? nextId();
  return {
    id,
    name: `App ${id}`,
    path: `/Applications/${id}.app`,
    source: "applications",
    isSystem: false,
    ...overrides,
  };
}

export function appItem(kind: AppDataItem["kind"], risk: RiskLevel, size = 1_000): AppDataItem {
  return {
    kind,
    kindLabel: kind,
    target: target({
      id: kind,
      providerId: "uninstaller",
      risk,
      sizeBytes: size,
      kind: "directory",
    }),
  };
}

export function appDetail(
  app: ApplicationInfo,
  items: AppDataItem[],
  overrides: Partial<AppDetail> = {},
): AppDetail {
  const totalBytes = items.reduce((a, i) => a + i.target.sizeBytes, 0);
  return {
    app,
    items,
    totalBytes,
    leftoverBytes: items
      .filter((i) => i.kind !== "application")
      .reduce((a, i) => a + i.target.sizeBytes, 0),
    ...overrides,
  };
}

export function startupItem(overrides: Partial<StartupItem> = {}): StartupItem {
  const id = overrides.id ?? nextId();
  return {
    id,
    name: `Item ${id}`,
    path: `/Users/u/Library/LaunchAgents/${id}.plist`,
    enabled: true,
    source: "launch_agent",
    scope: "user",
    trigger: "at_login",
    canToggle: true,
    ...overrides,
  };
}
