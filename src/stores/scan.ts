import { create } from "zustand";
import { backend, errorMessage } from "@/lib/tauri";
import { useApps } from "./apps";
import { useDisk } from "./disk";
import type {
  Category,
  CleanupPlan,
  CleanupProgress,
  CleanupResult,
  CleanupTarget,
  DeleteMode,
  ProviderInfo,
  ScanProgress,
  ScanResult,
  ScanSession,
} from "@/types/models";

/**
 * Removal always starts out recoverable.
 *
 * "Delete permanently" used to be remembered between runs, so choosing it once turned every later
 * cleanup into one that cannot be undone, whenever Prune was next opened and whatever the person
 * had since forgotten they chose. It now holds for the run it was chosen in and no longer.
 *
 * This key was where the old choice was kept. It is still read once — to be deleted, so that
 * someone who had already switched is not carried into this run in the mode they left.
 */
const LEGACY_MODE_KEY = "prune.deleteMode";

function forgetLegacyMode() {
  try {
    localStorage.removeItem(LEGACY_MODE_KEY);
  } catch {
    /* storage unavailable */
  }
}
forgetLegacyMode();

/** Which risks the result list shows. */
export type RiskFilter = "all" | "safe";

interface ScanState {
  providers: ProviderInfo[];
  scanId: string | null;
  scanning: boolean;
  cancelling: boolean;
  /** Per provider progress while scanning. */
  progress: Record<string, ScanProgress>;
  session: ScanSession | null;
  selected: Set<string>;
  deleteMode: DeleteMode;
  /** Substring typed into the result search box. */
  filter: string;
  riskFilter: RiskFilter;
  /**
   * Which section the current filter was meant for.
   *
   * Cleaner and Developer share one filter, so arriving in one with the other's search still
   * applied would show "nothing matches". Each section clears a filter that was not meant for
   * it — but a filter set deliberately for where the user is being sent has to survive, which
   * is what this records.
   */
  filterScope: string | null;
  plan: CleanupPlan | null;
  previewing: boolean;
  executing: boolean;
  cleanupProgress: CleanupProgress | null;
  result: CleanupResult | null;
  error: string | null;

  loadProviders: () => Promise<void>;
  startScan: (providerIds?: string[]) => Promise<void>;
  cancelScan: () => Promise<void>;
  onScanProgress: (p: ScanProgress) => void;
  onScanCompleted: (s: ScanSession) => void;
  onCleanupProgress: (p: CleanupProgress) => void;

  toggleTarget: (id: string) => void;
  setTargets: (ids: string[], on: boolean) => void;
  selectRecommended: (categories?: Category[]) => void;
  clearSelection: () => void;
  setDeleteMode: (mode: DeleteMode) => void;
  setFilter: (filter: string, scope?: string) => void;
  setRiskFilter: (riskFilter: RiskFilter) => void;

  /**
   * Builds a plan for `targetIds`, or for everything selected when they are omitted.
   *
   * Views pass their own ids: the Cleaner and Developer sections share one selection, and a
   * plan must never contain items the button the user pressed did not count.
   */
  preview: (targetIds?: string[], sourceScanId?: string) => Promise<void>;
  closePreview: () => void;
  execute: () => Promise<void>;
  dismissResult: () => void;
  clearError: () => void;
}

export const useScan = create<ScanState>((set, get) => {
  let scanRequest = 0;
  let previewRequest = 0;
  const earlyCompletions = new Map<string, ScanSession>();
  let timer: ReturnType<typeof setTimeout> | undefined;
  const stopPolling = () => {
    clearTimeout(timer);
    timer = undefined;
  };
  const cancel = async (scanId: string, request: number) => {
    try {
      await backend.cleanerCancelScan(scanId);
    } catch (e) {
      if (request === scanRequest && get().scanId === scanId && get().scanning)
        set({ cancelling: false, error: errorMessage(e) });
    }
  };
  const reconcile = (scanId: string, request: number) => {
    const current = () => request === scanRequest && get().scanId === scanId && get().scanning;
    timer = setTimeout(async () => {
      if (!current()) return;
      try {
        const session = await backend.cleanerGetScan(scanId);
        if (!current()) return;
        get().onScanCompleted(session);
        if (current()) reconcile(scanId, request);
      } catch (e) {
        if (!current()) return;
        if (e && typeof e === "object" && "code" in e && e.code === "scan_running")
          reconcile(scanId, request);
        else set({ scanning: false, cancelling: false, error: errorMessage(e) });
      }
    }, 1_000);
  };
  return {
    providers: [],
    scanId: null,
    scanning: false,
    cancelling: false,
    progress: {},
    session: null,
    selected: new Set(),
    deleteMode: "trash",
    filter: "",
    riskFilter: "all",
    filterScope: null,
    plan: null,
    previewing: false,
    executing: false,
    cleanupProgress: null,
    result: null,
    error: null,

    loadProviders: async () => {
      try {
        set({ providers: await backend.cleanerListProviders() });
      } catch (e) {
        set({ error: errorMessage(e) });
      }
    },

    startScan: async (providerIds) => {
      if (get().scanning || get().executing) return;
      const request = ++scanRequest;
      ++previewRequest;
      stopPolling();
      earlyCompletions.clear();
      set({
        scanId: null,
        scanning: true,
        cancelling: false,
        progress: {},
        error: null,
        result: null,
        plan: null,
        previewing: false,
        executing: false,
        cleanupProgress: null,
      });
      try {
        const scanId = await backend.cleanerStartScan(providerIds);
        if (request !== scanRequest || !get().scanning) return;
        set({ scanId });
        const early = earlyCompletions.get(scanId);
        earlyCompletions.clear();
        if (early) get().onScanCompleted(early);
        if (get().scanning) {
          reconcile(scanId, request);
          if (get().cancelling) await cancel(scanId, request);
        }
      } catch (e) {
        if (request !== scanRequest) return;
        earlyCompletions.clear();
        set({ scanning: false, cancelling: false, error: errorMessage(e) });
      }
    },

    cancelScan: async () => {
      const { scanId, scanning, cancelling } = get();
      if (!scanning || cancelling) return;
      set({ cancelling: true });
      if (scanId) await cancel(scanId, scanRequest);
    },

    onScanProgress: (p) => {
      if (!get().scanning || p.scanId !== get().scanId) return;
      set((s) => ({ progress: { ...s.progress, [p.providerId]: p } }));
    },

    onScanCompleted: (session) => {
      if (session.status === "running") return;
      if (get().scanning && get().scanId === null) {
        if (earlyCompletions.size >= 8)
          earlyCompletions.delete(earlyCompletions.keys().next().value ?? "");
        earlyCompletions.set(session.id, session);
        return;
      }
      if (session.id !== get().scanId) return;
      if (!get().scanning) return;
      stopPolling();
      // Keep selection for targets that still exist.
      const ids = new Set(session.results.flatMap((r) => r.targets.map((t) => t.id)));
      const selected = new Set([...get().selected].filter((id) => ids.has(id)));
      set({ session, scanning: false, cancelling: false, selected });
    },

    onCleanupProgress: (p) => {
      if (!get().executing || p.planId !== get().plan?.id) return;
      set({ cleanupProgress: p });
    },

    toggleTarget: (id) =>
      set((s) => {
        const selected = new Set(s.selected);
        if (selected.has(id)) selected.delete(id);
        else selected.add(id);
        return { selected };
      }),

    setTargets: (ids, on) =>
      set((s) => {
        const selected = new Set(s.selected);
        ids.forEach((id) => (on ? selected.add(id) : selected.delete(id)));
        return { selected };
      }),

    selectRecommended: (categories) =>
      set((s) => {
        const selected = new Set(s.selected);
        // Some targets can only be deleted outright — items already in the Trash, for instance —
        // and are removed permanently whatever mode is chosen. Ticking those for someone who
        // asked to move things to the Trash puts one irreversible item among a list that reads as
        // recoverable. They are still offered; they are just not chosen on the user's behalf
        // unless permanent removal is what was asked for. `prune clean` applies the same rule.
        const permanent = s.deleteMode === "permanent";
        s.session?.results
          .filter((r) => !categories || categories.includes(r.category))
          .flatMap((r) => r.targets)
          .filter((t) => t.risk === "safe")
          .filter((t) => permanent || !t.permanentOnly)
          .forEach((t) => selected.add(t.id));
        return { selected };
      }),

    clearSelection: () => set({ selected: new Set() }),

    setDeleteMode: (deleteMode) => set({ deleteMode }),

    setFilter: (filter, scope) => set((s) => ({ filter, filterScope: scope ?? s.filterScope })),
    setRiskFilter: (riskFilter) => set({ riskFilter }),

    preview: async (targetIds, sourceScanId) => {
      const { scanId: activeScanId, selected, deleteMode, scanning, executing } = get();
      const scanId = sourceScanId ?? activeScanId;
      const ids = targetIds ?? [...selected];
      if (!scanId || ids.length === 0 || scanning || executing) return;
      const request = ++previewRequest;
      const scan = scanRequest;
      const current = () =>
        request === previewRequest &&
        scan === scanRequest &&
        get().scanId === activeScanId &&
        !get().executing;
      const sourceCurrent = () =>
        !sourceScanId ||
        (sourceScanId.startsWith("app:")
          ? useApps.getState().selectedId === sourceScanId.slice(4)
          : useDisk.getState().scanId === sourceScanId && !useDisk.getState().scanning);
      set({ previewing: true, plan: null, error: null });
      try {
        const plan = await backend.cleanerPreview(scanId, ids, deleteMode);
        if (!current() || !sourceCurrent()) return;
        if (plan.scanId !== scanId)
          throw new Error("Preview belongs to another scan. Review again.");
        set({ plan, previewing: false, cleanupProgress: null });
      } catch (e) {
        if (!current() || !sourceCurrent()) return;
        set({ previewing: false, error: errorMessage(e) });
      } finally {
        if (current()) set({ previewing: false });
      }
    },

    closePreview: () => {
      if (get().executing) return;
      ++previewRequest;
      set({ plan: null, previewing: false, cleanupProgress: null });
    },

    execute: async () => {
      const { plan, scanId, executing, scanning, previewing } = get();
      if (!plan || plan.targets.length === 0 || executing || scanning || previewing) return;
      ++previewRequest;
      const scan = scanRequest;
      const current = () => scan === scanRequest && get().scanId === scanId && get().plan === plan;
      set({ executing: true, error: null });
      try {
        const result = await backend.cleanerExecute(plan.id);
        if (!current()) return;
        // Remove cleaned targets from the local session so the list reflects reality.
        const verified = resultForPlan(result, plan);
        if (!verified || result.status === "failed") {
          set({
            executing: false,
            plan: null,
            cleanupProgress: null,
            result: verified ? result : null,
            error:
              "Cleanup could not be verified. Items have been kept in the list; rescan before trying again.",
          });
          return;
        }
        const removed = new Set(plan.targets.map((t) => t.id));
        result.failed.forEach((f) => removed.delete(f.targetId));
        set((s) => {
          const ownsSession = s.session?.id === plan.scanId;
          const session = ownsSession && s.session ? pruneSession(s.session, removed) : s.session;
          const selected = ownsSession
            ? new Set([...s.selected].filter((id) => !removed.has(id)))
            : s.selected;
          return { result, executing: false, plan: null, cleanupProgress: null, session, selected };
        });
        if (plan.scanId === useDisk.getState().scanId) {
          void useDisk.getState().forgetRemoved([...removed]);
        }
        if (plan.scanId.startsWith("app:")) {
          void useApps.getState().forgetRemoved([...removed]);
        }
      } catch (e) {
        if (!current()) return;
        set({ executing: false, error: errorMessage(e) });
      }
    },

    dismissResult: () => set({ result: null }),
    clearError: () => set({ error: null }),
  };
});

/** IPC types are compile-time promises; validate the actual reply before changing rows or tone. */
function resultForPlan(value: unknown, plan: CleanupPlan): value is CleanupResult {
  if (!value || typeof value !== "object") return false;
  const result = value as Partial<CleanupResult>;
  const text = (value: unknown): value is string => typeof value === "string" && value.length > 0;
  const count = (value: unknown): value is number =>
    typeof value === "number" && Number.isSafeInteger(value) && value >= 0;
  if (
    !text(result.operationId) ||
    result.planId !== plan.id ||
    result.mode !== plan.mode ||
    !text(result.startedAt) ||
    !text(result.finishedAt) ||
    !Number.isFinite(Date.parse(result.startedAt)) ||
    !Number.isFinite(Date.parse(result.finishedAt)) ||
    Date.parse(result.finishedAt) < Date.parse(result.startedAt) ||
    !count(result.removedTargets) ||
    !count(result.removedFiles) ||
    !count(result.removedBytes) ||
    !Array.isArray(result.failed) ||
    (result.logError !== undefined && typeof result.logError !== "string")
  )
    return false;
  const known = new Set(plan.targets.map((target) => target.id));
  const targets = new Map(plan.targets.map((target) => [target.id, target]));
  const failed = new Set<string>();
  for (const failure of result.failed) {
    if (
      !failure ||
      typeof failure !== "object" ||
      !text(failure.targetId) ||
      typeof failure.path !== "string" ||
      !text(failure.error) ||
      !known.has(failure.targetId) ||
      targets.get(failure.targetId)?.path !== failure.path ||
      failed.has(failure.targetId)
    )
      return false;
    failed.add(failure.targetId);
  }
  if (result.removedTargets !== known.size - failed.size) return false;
  const successful = plan.targets.filter((target) => !failed.has(target.id));
  const logicalBytes = successful.reduce((total, target) => total + target.sizeBytes, 0);
  const logicalFiles = successful.reduce((total, target) => total + target.fileCount, 0);
  if (
    !count(logicalBytes) ||
    !count(logicalFiles) ||
    result.removedBytes !== logicalBytes ||
    result.removedFiles !== logicalFiles
  )
    return false;
  if (result.status === "success") return result.removedTargets > 0 && failed.size === 0;
  if (result.status === "partial") return result.removedTargets > 0 && failed.size > 0;
  return (
    result.status === "failed" &&
    result.removedTargets === 0 &&
    result.removedFiles === 0 &&
    result.removedBytes === 0
  );
}

function pruneSession(session: ScanSession, removed: Set<string>): ScanSession {
  const results = session.results.map((r) => {
    const targets = r.targets.filter((t) => !removed.has(t.id));
    return {
      ...r,
      targets,
      totalBytes: targets.reduce((a, t) => a + t.sizeBytes, 0),
      totalFiles: targets.reduce((a, t) => a + t.fileCount, 0),
    };
  });
  return {
    ...session,
    results,
    totalBytes: results.reduce((a, r) => a + r.totalBytes, 0),
    totalFiles: results.reduce((a, r) => a + r.totalFiles, 0),
  };
}

/**
 * Applies the search box and risk filter to one provider's results.
 *
 * Targets come back biggest first: with hundreds of artifacts, size is the only order that
 * puts the items worth deciding about at the top. Groups that end up empty are dropped, so a
 * search narrows the page rather than leaving a wall of empty headings.
 */
export function filterResults(
  results: ScanResult[],
  filter: string,
  riskFilter: RiskFilter,
): ScanResult[] {
  const needle = filter.trim().toLowerCase();
  return results
    .map((r) => {
      const matchesProvider = needle.length > 0 && r.providerName.toLowerCase().includes(needle);
      const targets = r.targets
        .filter((t) => riskFilter === "all" || t.risk === "safe")
        .filter(
          (t) =>
            needle.length === 0 ||
            matchesProvider ||
            t.label.toLowerCase().includes(needle) ||
            t.path.toLowerCase().includes(needle) ||
            (t.description ?? "").toLowerCase().includes(needle),
        )
        .slice()
        .sort((a, b) => b.sizeBytes - a.sizeBytes);
      return {
        ...r,
        targets,
        totalBytes: targets.reduce((a, t) => a + t.sizeBytes, 0),
        totalFiles: targets.reduce((a, t) => a + t.fileCount, 0),
        // Issues explain gaps in the numbers, so keep them unless the user is searching.
        issues: needle.length === 0 && riskFilter === "all" ? r.issues : [],
      };
    })
    .filter((r) => r.targets.length > 0 || r.issues.length > 0);
}

/** Selected targets resolved from the session. */
export function selectedTargets(state: Pick<ScanState, "session" | "selected">): CleanupTarget[] {
  if (!state.session) return [];
  return state.session.results.flatMap((r) => r.targets).filter((t) => state.selected.has(t.id));
}

export function sumBytes(targets: CleanupTarget[]): number {
  return targets.reduce((a, t) => a + t.sizeBytes, 0);
}

/** Subscribes the store to backend events. Call once at app start. */
export async function bindScanEvents(): Promise<() => void> {
  const s = useScan.getState();
  const unlisteners = await Promise.all([
    backend.on("prune://scan-progress", s.onScanProgress),
    backend.on("prune://scan-completed", s.onScanCompleted),
    backend.on("prune://cleanup-progress", s.onCleanupProgress),
  ]);
  return () => unlisteners.forEach((u) => u());
}
