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

const MODE_KEY = "prune.deleteMode";

function loadMode(): DeleteMode {
  try {
    const v = localStorage.getItem(MODE_KEY);
    if (v === "trash" || v === "permanent") return v;
  } catch {
    /* ignore */
  }
  return "trash";
}

/** Which risks the result list shows. */
export type RiskFilter = "all" | "safe";

interface ScanState {
  providers: ProviderInfo[];
  scanId: string | null;
  scanning: boolean;
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
  preview: (targetIds?: string[]) => Promise<void>;
  closePreview: () => void;
  execute: () => Promise<void>;
  dismissResult: () => void;
  clearError: () => void;
}

export const useScan = create<ScanState>((set, get) => ({
  providers: [],
  scanId: null,
  scanning: false,
  progress: {},
  session: null,
  selected: new Set(),
  deleteMode: loadMode(),
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
    if (get().scanning) return;
    set({ scanning: true, progress: {}, error: null, result: null, plan: null });
    try {
      const scanId = await backend.cleanerStartScan(providerIds);
      set({ scanId });
    } catch (e) {
      set({ scanning: false, error: errorMessage(e) });
    }
  },

  cancelScan: async () => {
    const { scanId } = get();
    if (!scanId) return;
    try {
      await backend.cleanerCancelScan(scanId);
    } catch (e) {
      set({ error: errorMessage(e) });
    }
  },

  onScanProgress: (p) => {
    if (p.scanId !== get().scanId) return;
    set((s) => ({ progress: { ...s.progress, [p.providerId]: p } }));
  },

  onScanCompleted: (session) => {
    if (session.id !== get().scanId) return;
    // Keep selection for targets that still exist.
    const ids = new Set(session.results.flatMap((r) => r.targets.map((t) => t.id)));
    const selected = new Set([...get().selected].filter((id) => ids.has(id)));
    set({ session, scanning: false, selected });
  },

  onCleanupProgress: (p) => {
    if (p.planId !== get().plan?.id) return;
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
      s.session?.results
        .filter((r) => !categories || categories.includes(r.category))
        .flatMap((r) => r.targets)
        .filter((t) => t.risk === "safe")
        .forEach((t) => selected.add(t.id));
      return { selected };
    }),

  clearSelection: () => set({ selected: new Set() }),

  setDeleteMode: (deleteMode) => {
    try {
      localStorage.setItem(MODE_KEY, deleteMode);
    } catch {
      /* ignore */
    }
    set({ deleteMode });
  },

  setFilter: (filter, scope) => set((s) => ({ filter, filterScope: scope ?? s.filterScope })),
  setRiskFilter: (riskFilter) => set({ riskFilter }),

  preview: async (targetIds) => {
    const { scanId, selected, deleteMode } = get();
    const ids = targetIds ?? [...selected];
    if (!scanId || ids.length === 0) return;
    set({ previewing: true, error: null });
    try {
      const plan = await backend.cleanerPreview(scanId, ids, deleteMode);
      set({ plan, previewing: false, cleanupProgress: null });
    } catch (e) {
      set({ previewing: false, error: errorMessage(e) });
    }
  },

  closePreview: () => set({ plan: null, cleanupProgress: null }),

  execute: async () => {
    const { plan, scanId } = get();
    if (!plan) return;
    set({ executing: true, error: null });
    try {
      const result = await backend.cleanerExecute(plan.id);
      // Remove cleaned targets from the local session so the list reflects reality.
      const removed = new Set(plan.targets.map((t) => t.id));
      result.failed.forEach((f) => removed.delete(f.targetId));
      set((s) => {
        const session = s.session ? pruneSession(s.session, removed) : null;
        const selected = new Set([...s.selected].filter((id) => !removed.has(id)));
        return { result, executing: false, plan: null, cleanupProgress: null, session, selected };
      });
      if (scanId === useDisk.getState().scanId || plan.scanId === useDisk.getState().scanId) {
        void useDisk.getState().forgetRemoved([...removed]);
      }
      if (plan.scanId.startsWith("app:")) {
        void useApps.getState().forgetRemoved([...removed]);
      }
    } catch (e) {
      set({ executing: false, error: errorMessage(e) });
    }
  },

  dismissResult: () => set({ result: null }),
  clearError: () => set({ error: null }),
}));

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
