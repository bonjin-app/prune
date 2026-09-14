import { create } from "zustand";
import { backend, errorMessage } from "@/lib/tauri";
import type {
  Category,
  CleanupPlan,
  CleanupProgress,
  CleanupResult,
  CleanupTarget,
  DeleteMode,
  ProviderInfo,
  ScanProgress,
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

interface ScanState {
  providers: ProviderInfo[];
  scanId: string | null;
  scanning: boolean;
  /** Per provider progress while scanning. */
  progress: Record<string, ScanProgress>;
  session: ScanSession | null;
  selected: Set<string>;
  deleteMode: DeleteMode;
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

  preview: () => Promise<void>;
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

  preview: async () => {
    const { scanId, selected, deleteMode } = get();
    if (!scanId || selected.size === 0) return;
    set({ previewing: true, error: null });
    try {
      const plan = await backend.cleanerPreview(scanId, [...selected], deleteMode);
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
      // The backend drops the scan after execution; keep the id only for display.
      void scanId;
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
