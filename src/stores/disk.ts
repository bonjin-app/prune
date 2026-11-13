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
  cancelling: boolean;
  progress: DiskProgress | null;
  summary: DiskSummary | null;
  root: string;
  node: DiskNodeView | null;
  nodeLoading: boolean;
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

export const useDisk = create<DiskState>((set, get) => {
  let scanRequest = 0;
  let nodeRequest = 0;
  let largeFilesRequest = 0;
  let cleanupRequest = 0;
  let pendingNodePath: string | undefined;
  let pendingNodeScan: string | null = null;
  const earlyCompletions = new Map<string, DiskSummary>();
  const sendCancellation = async (scanId: string, request: number) => {
    try {
      await backend.diskCancelScan(scanId);
    } catch (e) {
      if (request === scanRequest && scanId === get().scanId && get().scanning)
        set({ cancelling: false, error: errorMessage(e) });
    }
  };
  let reconciliationTimer: ReturnType<typeof setTimeout> | undefined;
  const stopReconciliation = () => {
    clearTimeout(reconciliationTimer);
    reconciliationTimer = undefined;
  };
  const reconcile = (scanId: string, request: number) => {
    const current = () => request === scanRequest && get().scanId === scanId && get().scanning;
    reconciliationTimer = setTimeout(() => {
      if (!current()) return;
      void backend.diskGetSummary(scanId).then(
        (summary) => {
          if (current() && summary) void get().onCompleted(summary);
        },
        (error: unknown) => {
          if (!current()) return;
          if (
            error &&
            typeof error === "object" &&
            "code" in error &&
            error.code === "scan_running"
          )
            reconcile(scanId, request);
          else set({ scanning: false, cancelling: false, error: errorMessage(error) });
        },
      );
    }, 1_000);
  };

  return {
    scanId: null,
    scanning: false,
    cancelling: false,
    progress: null,
    summary: null,
    root: "",
    node: null,
    nodeLoading: false,
    largeFiles: [],
    minBytes: 1_000_000_000,
    selected: new Set(),
    tab: "tree",
    error: null,

    setRoot: (root) => set({ root }),
    setTab: (tab) => set({ tab }),

    startScan: async (root) => {
      if (get().scanning) return;
      const request = ++scanRequest;
      stopReconciliation();
      earlyCompletions.clear();
      ++nodeRequest;
      ++largeFilesRequest;
      set({
        scanId: null,
        scanning: true,
        cancelling: false,
        progress: null,
        summary: null,
        node: null,
        nodeLoading: false,
        largeFiles: [],
        selected: new Set(),
        error: null,
      });
      try {
        const scanId = await backend.diskStartScan(root ?? get().root);
        if (request !== scanRequest) return;
        set({ scanId });
        const early = earlyCompletions.get(scanId);
        earlyCompletions.clear();
        if (early) await get().onCompleted(early);
        else if (get().scanning) {
          reconcile(scanId, request);
          if (get().cancelling) await sendCancellation(scanId, request);
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
      if (scanId) await sendCancellation(scanId, scanRequest);
    },

    onProgress: (p) => {
      if (p.scanId !== get().scanId) return;
      set({ progress: p });
    },

    onCompleted: async (summary) => {
      if (get().scanning && get().scanId === null) {
        // A small directory may finish before the start command returns its ID.
        // Keep a bounded buffer, then accept only the ID returned by that launch.
        if (earlyCompletions.size >= 8)
          earlyCompletions.delete(earlyCompletions.keys().next().value ?? "");
        earlyCompletions.set(summary.scanId, summary);
        return;
      }
      if (summary.scanId !== get().scanId) return;
      if (get().summary?.scanId === summary.scanId) return;
      stopReconciliation();
      set({ summary, scanning: false, cancelling: false, root: summary.root });
      await Promise.all([get().openNode(), get().refreshLargeFiles()]);
    },

    openNode: async (path) => {
      const { scanId } = get();
      if (!scanId) return;
      const request = ++nodeRequest;
      pendingNodePath = path;
      pendingNodeScan = scanId;
      const scan = scanRequest;
      const isCurrent = () =>
        request === nodeRequest && scan === scanRequest && scanId === get().scanId;
      set({ nodeLoading: true });
      try {
        const node = await backend.diskGetNode(scanId, path);
        if (!isCurrent()) return;
        set({ node });
      } catch (e) {
        if (!isCurrent()) return;
        set({ error: errorMessage(e) });
      } finally {
        if (isCurrent()) set({ nodeLoading: false });
      }
    },

    setMinBytes: async (minBytes) => {
      set({ minBytes });
      await get().refreshLargeFiles();
    },

    refreshLargeFiles: async () => {
      const { scanId, minBytes } = get();
      if (!scanId) return;
      const request = ++largeFilesRequest;
      const scan = scanRequest;
      const isCurrent = () =>
        request === largeFilesRequest &&
        scan === scanRequest &&
        scanId === get().scanId &&
        minBytes === get().minBytes;
      try {
        const largeFiles = await backend.diskLargeFiles(scanId, minBytes, 500);
        if (!isCurrent()) return;
        const ids = new Set(largeFiles.map((f) => f.targetId));
        set((s) => ({
          largeFiles,
          selected: new Set([...s.selected].filter((id) => ids.has(id))),
        }));
      } catch (e) {
        if (!isCurrent()) return;
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
      const destination =
        get().nodeLoading && pendingNodeScan === scanId ? pendingNodePath : node?.node.path;
      // Cleanup changes the underlying snapshot: invalidate every earlier read, including
      // pending navigation, then re-fetch its intended destination and active file filter.
      ++nodeRequest;
      ++largeFilesRequest;
      const cleanup = ++cleanupRequest;
      const scan = scanRequest;
      await Promise.all([
        backend
          .diskGetSummary(scanId)
          .then((summary) => {
            if (cleanup === cleanupRequest && scan === scanRequest && scanId === get().scanId)
              set({ summary });
          })
          .catch(() => {
            /* Folder/file refresh reports any actionable query errors. */
          }),
        get().openNode(destination),
        get().refreshLargeFiles(),
      ]);
    },

    clearError: () => set({ error: null }),
  };
});

/** Subscribes the store to disk events. Call once at app start. */
export async function bindDiskEvents(): Promise<() => void> {
  const s = useDisk.getState();
  const unlisteners = await Promise.all([
    backend.on("prune://disk-progress", s.onProgress),
    backend.on("prune://disk-completed", (summary) => void s.onCompleted(summary)),
  ]);
  return () => unlisteners.forEach((u) => u());
}
