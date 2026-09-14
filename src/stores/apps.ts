import { create } from "zustand";
import { backend, errorMessage } from "@/lib/tauri";
import type { AppDetail, ApplicationInfo, AppsProgress } from "@/types/models";

interface AppsState {
  apps: ApplicationInfo[];
  loading: boolean;
  measuring: boolean;
  progress: { done: number; total: number } | null;
  query: string;
  selectedId: string | null;
  detail: AppDetail | null;
  detailLoading: boolean;
  selectedItems: Set<string>;
  error: string | null;

  load: () => Promise<void>;
  onProgress: (p: AppsProgress) => void;
  onCompleted: (apps: ApplicationInfo[]) => void;
  setQuery: (q: string) => void;
  select: (id: string | null) => Promise<void>;
  toggleItem: (targetId: string) => void;
  setItems: (ids: string[], on: boolean) => void;
  forgetRemoved: (ids: string[]) => Promise<void>;
  clearError: () => void;
}

export const useApps = create<AppsState>((set, get) => ({
  apps: [],
  loading: false,
  measuring: false,
  progress: null,
  query: "",
  selectedId: null,
  detail: null,
  detailLoading: false,
  selectedItems: new Set(),
  error: null,

  load: async () => {
    if (get().loading) return;
    set({ loading: true, error: null, progress: null });
    try {
      const apps = await backend.appsStartScan();
      set({ apps, loading: false, measuring: apps.length > 0 });
    } catch (e) {
      set({ loading: false, error: errorMessage(e) });
    }
  },

  onProgress: (p) =>
    set((s) => ({
      progress: { done: p.done, total: p.total },
      apps: s.apps.map((a) => (a.id === p.app.id ? { ...a, sizeBytes: p.app.sizeBytes } : a)),
    })),

  onCompleted: (apps) => set({ apps, measuring: false, progress: null }),

  setQuery: (query) => set({ query }),

  select: async (id) => {
    set({ selectedId: id, detail: null, selectedItems: new Set() });
    if (!id) return;
    set({ detailLoading: true });
    try {
      const detail = await backend.appsGetDetail(id);
      if (get().selectedId !== id) return;
      // Pre-select everything removable; the user reviews before anything happens.
      const selectedItems = new Set(
        detail.items.filter((i) => i.target.risk !== "protected").map((i) => i.target.id),
      );
      set({ detail, detailLoading: false, selectedItems });
    } catch (e) {
      set({ detailLoading: false, error: errorMessage(e) });
    }
  },

  toggleItem: (targetId) =>
    set((s) => {
      const selectedItems = new Set(s.selectedItems);
      if (selectedItems.has(targetId)) selectedItems.delete(targetId);
      else selectedItems.add(targetId);
      return { selectedItems };
    }),

  setItems: (ids, on) =>
    set((s) => {
      const selectedItems = new Set(s.selectedItems);
      ids.forEach((id) => (on ? selectedItems.add(id) : selectedItems.delete(id)));
      return { selectedItems };
    }),

  forgetRemoved: async (ids) => {
    const { detail, selectedId } = get();
    if (!detail) return;
    const bundleRemoved = detail.items.some(
      (i) => i.kind === "application" && ids.includes(i.target.id),
    );
    const items = detail.items.filter((i) => !ids.includes(i.target.id));
    const totalBytes = items.reduce((a, i) => a + i.target.sizeBytes, 0);
    set((s) => ({
      detail: {
        ...detail,
        items,
        totalBytes,
        leftoverBytes: items
          .filter((i) => i.kind !== "application")
          .reduce((a, i) => a + i.target.sizeBytes, 0),
      },
      selectedItems: new Set([...s.selectedItems].filter((id) => !ids.includes(id))),
      apps: bundleRemoved ? s.apps.filter((a) => a.id !== selectedId) : s.apps,
      selectedId: bundleRemoved ? null : s.selectedId,
    }));
    if (bundleRemoved) set({ detail: null });
  },

  clearError: () => set({ error: null }),
}));

export async function bindAppsEvents(): Promise<() => void> {
  const s = useApps.getState();
  const unlisteners = await Promise.all([
    backend.on("prune://apps-progress", s.onProgress),
    backend.on("prune://apps-completed", s.onCompleted),
  ]);
  return () => unlisteners.forEach((u) => u());
}
