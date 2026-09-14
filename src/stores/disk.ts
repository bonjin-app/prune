import { create } from "zustand";
import { backend, errorMessage } from "@/lib/tauri";
import type { DiskNodeView, DiskProgress, DiskSummary, LargeFile } from "@/types/models";

export const SIZE_FILTERS: { label: string; bytes: number }[] = [
  { label: "> 100 MB", bytes: 100_000_000 },
  { label: "> 1 GB", bytes: 1_000_000_000 },
  { label: "> 5 GB", bytes: 5_000_000_000 },
  { label: "> 10 GB", bytes: 10_000_000_000 },
];

export type DiskTab = "tree" | "large" | "types";

interface DiskState {
  scanId: string | null;
  scanning: boolean;
  progress: DiskProgress | null;
  summary: DiskSummary | null;
  root: string;
  node: DiskNodeView | null;
  largeFiles: LargeFile[];
  minBytes: number;
  selected: Set<string>;
  tab: DiskTab;
  error: string | null;

  setRoot: (root: string) => void;
  setTab: (tab: DiskTab) => void;
  startScan: (root?: string) => Promise<void>;
  cancelScan: () => Promise<void>;
  onProgress: (p: DiskProgress) => void;
  onCompleted: (s: DiskSummary) => Promise<void>;
  openNode: (path?: string) => Promise<void>;
  setMinBytes: (bytes: number) => Promise<void>;
  refreshLargeFiles: () => Promise<void>;
  toggle: (id: string) => void;
  clearSelection: () => void;
  forgetRemoved: (ids: string[]) => Promise<void>;
  clearError: () => void;
}

export const useDisk = create<DiskState>((set, get) => ({
  scanId: null,
  scanning: false,
  progress: null,
  summary: null,
  root: "",
  node: null,
  largeFiles: [],
  minBytes: 1_000_000_000,
  selected: new Set(),
  tab: "tree",
  error: null,

  setRoot: (root) => set({ root }),
  setTab: (tab) => set({ tab }),

  startScan: async (root) => {
    if (get().scanning) return;
    set({
      scanning: true,
      progress: null,
      summary: null,
      node: null,
      largeFiles: [],
      selected: new Set(),
      error: null,
    });
    try {
      const scanId = await backend.diskStartScan(root ?? get().root);
      set({ scanId });
    } catch (e) {
      set({ scanning: false, error: errorMessage(e) });
    }
  },

  cancelScan: async () => {
    const { scanId } = get();
    if (!scanId) return;
    try {
      await backend.diskCancelScan(scanId);
    } catch (e) {
      set({ error: errorMessage(e) });
    }
  },

  onProgress: (p) => {
    if (p.scanId !== get().scanId) return;
    set({ progress: p });
  },

  onCompleted: async (summary) => {
    if (summary.scanId !== get().scanId) return;
    set({ summary, scanning: false, root: summary.root });
    await Promise.all([get().openNode(), get().refreshLargeFiles()]);
  },

  openNode: async (path) => {
    const { scanId } = get();
    if (!scanId) return;
    try {
      const node = await backend.diskGetNode(scanId, path);
      set({ node });
    } catch (e) {
      set({ error: errorMessage(e) });
    }
  },

  setMinBytes: async (minBytes) => {
    set({ minBytes });
    await get().refreshLargeFiles();
  },

  refreshLargeFiles: async () => {
    const { scanId, minBytes } = get();
    if (!scanId) return;
    try {
      const largeFiles = await backend.diskLargeFiles(scanId, minBytes, 500);
      const ids = new Set(largeFiles.map((f) => f.targetId));
      set((s) => ({ largeFiles, selected: new Set([...s.selected].filter((id) => ids.has(id))) }));
    } catch (e) {
      set({ error: errorMessage(e) });
    }
  },

  toggle: (id) =>
    set((s) => {
      const selected = new Set(s.selected);
      if (selected.has(id)) selected.delete(id);
      else selected.add(id);
      return { selected };
    }),

  clearSelection: () => set({ selected: new Set() }),

  forgetRemoved: async (ids) => {
    set((s) => ({
      largeFiles: s.largeFiles.filter((f) => !ids.includes(f.targetId)),
      selected: new Set([...s.selected].filter((id) => !ids.includes(id))),
    }));
    const { scanId, node } = get();
    if (!scanId) return;
    try {
      const [summary, fresh] = await Promise.all([
        backend.diskGetSummary(scanId),
        backend.diskGetNode(scanId, node?.node.path),
      ]);
      set({ summary, node: fresh });
    } catch {
      /* stale view is acceptable */
    }
  },

  clearError: () => set({ error: null }),
}));

/** Subscribes the store to disk events. Call once at app start. */
export async function bindDiskEvents(): Promise<() => void> {
  const s = useDisk.getState();
  const unlisteners = await Promise.all([
    backend.on("prune://disk-progress", s.onProgress),
    backend.on("prune://disk-completed", (summary) => void s.onCompleted(summary)),
  ]);
  return () => unlisteners.forEach((u) => u());
}
