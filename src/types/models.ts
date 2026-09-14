/**
 * TypeScript mirror of `crates/prune-core/src/models`. Keep in sync by hand; every Rust struct
 * serializes with `camelCase` field names and `snake_case` enum variants.
 */

export type RiskLevel = "safe" | "low" | "medium" | "high" | "protected";

export type Category =
  | "system_cache"
  | "application_cache"
  | "browser_cache"
  | "logs"
  | "temporary_files"
  | "trash"
  | "developer_files"
  | "old_installers";

export type DeleteMode = "trash" | "permanent";

export type TargetKind = "file" | "directory";

export interface CleanupTarget {
  id: string;
  providerId: string;
  path: string;
  kind: TargetKind;
  sizeBytes: number;
  fileCount: number;
  risk: RiskLevel;
  label: string;
  description?: string;
  modifiedAt?: string;
  permanentOnly: boolean;
}

export interface ScanIssue {
  path: string;
  message: string;
}

export interface ScanResult {
  providerId: string;
  providerName: string;
  category: Category;
  targets: CleanupTarget[];
  totalBytes: number;
  totalFiles: number;
  durationMs: number;
  issues: ScanIssue[];
}

export type ScanStatus = "running" | "completed" | "cancelled" | "failed";

export interface ScanProgress {
  scanId: string;
  providerId: string;
  scannedFiles: number;
  discoveredBytes: number;
  currentPath?: string;
  providersDone: number;
  providersTotal: number;
}

export interface ScanSession {
  id: string;
  status: ScanStatus;
  startedAt: string;
  finishedAt?: string;
  results: ScanResult[];
  totalBytes: number;
  totalFiles: number;
}

export interface ProviderInfo {
  id: string;
  name: string;
  category: Category;
  description: string;
  defaultRisk: RiskLevel;
  available: boolean;
}

export interface BlockedTarget {
  targetId: string;
  path: string;
  reason: string;
}

export interface CleanupPlan {
  id: string;
  scanId: string;
  mode: DeleteMode;
  createdAt: string;
  targets: CleanupTarget[];
  blocked: BlockedTarget[];
  totalBytes: number;
  fileCount: number;
  directoryCount: number;
}

export interface CleanupProgress {
  planId: string;
  done: number;
  total: number;
  currentPath: string;
  removedBytes: number;
}

export interface FailedTarget {
  targetId: string;
  path: string;
  error: string;
}

export type CleanupStatus = "success" | "partial" | "failed";

export interface CleanupResult {
  operationId: string;
  planId: string;
  mode: DeleteMode;
  status: CleanupStatus;
  startedAt: string;
  finishedAt: string;
  removedTargets: number;
  removedFiles: number;
  removedBytes: number;
  failed: FailedTarget[];
}

export interface OperationRecord {
  id: string;
  at: string;
  title: string;
  mode: DeleteMode;
  status: CleanupStatus;
  removedTargets: number;
  removedFiles: number;
  removedBytes: number;
  failedCount: number;
  providers: string[];
}

export type Platform = "macos" | "windows" | "linux" | "unknown";

export interface SystemInfo {
  platform: Platform;
  osName: string;
  osVersion: string;
  kernelVersion: string;
  hostname: string;
  arch: string;
  cpuBrand: string;
  physicalCores: number | null;
  logicalCores: number;
  totalMemoryBytes: number;
  homeDir: string;
}

export interface CpuStatus {
  usagePercent: number;
  perCorePercent: number[];
}

export interface MemoryStatus {
  totalBytes: number;
  usedBytes: number;
  availableBytes: number;
  swapTotalBytes: number;
  swapUsedBytes: number;
}

export interface DiskStatus {
  name: string;
  mountPoint: string;
  fileSystem: string;
  totalBytes: number;
  availableBytes: number;
  isRemovable: boolean;
  isPrimary: boolean;
}

export interface SystemSnapshot {
  cpu: CpuStatus;
  memory: MemoryStatus;
  disks: DiskStatus[];
  uptimeSeconds: number;
  processCount: number;
}

export interface ProcessInfo {
  pid: number;
  name: string;
  cpuPercent: number;
  memoryBytes: number;
  user?: string;
  parentPid?: number;
}

export interface AppMeta {
  name: string;
  version: string;
  coreVersion: string;
  platform: Platform;
  arch: string;
  debug: boolean;
  operationLogPath: string;
}

export interface CommandError {
  code: string;
  message: string;
}

export const CATEGORY_LABEL: Record<Category, string> = {
  system_cache: "System Cache",
  application_cache: "Application Cache",
  browser_cache: "Browser Cache",
  logs: "Logs",
  temporary_files: "Temporary Files",
  trash: "Trash",
  developer_files: "Developer Files",
  old_installers: "Old Installers",
};

export const RISK_LABEL: Record<RiskLevel, string> = {
  safe: "Safe",
  low: "Low Risk",
  medium: "Medium Risk",
  high: "High Risk",
  protected: "Protected",
};

/** Categories shown in the Cleaner view; Developer Files has its own view. */
export const CLEANER_CATEGORIES: Category[] = [
  "system_cache",
  "application_cache",
  "browser_cache",
  "logs",
  "temporary_files",
  "trash",
  "old_installers",
];

export const DEVELOPER_CATEGORIES: Category[] = ["developer_files"];
