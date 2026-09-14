import { create } from "zustand";
import { backend, errorMessage } from "@/lib/tauri";
import type { AppMeta, Permissions, ProcessInfo, SystemInfo, SystemSnapshot } from "@/types/models";

interface SystemState {
  meta: AppMeta | null;
  info: SystemInfo | null;
  permissions: Permissions | null;
  snapshot: SystemSnapshot | null;
  processes: ProcessInfo[];
  error: string | null;
  loadStatic: () => Promise<void>;
  refreshSnapshot: () => Promise<void>;
  refreshProcesses: (limit?: number) => Promise<void>;
}

export const useSystem = create<SystemState>((set) => ({
  meta: null,
  info: null,
  permissions: null,
  snapshot: null,
  processes: [],
  error: null,
  loadStatic: async () => {
    try {
      const [meta, info, permissions] = await Promise.all([
        backend.appGetMeta(),
        backend.systemGetInfo(),
        backend.appGetPermissions(),
      ]);
      set({ meta, info, permissions, error: null });
    } catch (e) {
      set({ error: errorMessage(e) });
    }
  },
  refreshSnapshot: async () => {
    try {
      const snapshot = await backend.systemGetSnapshot();
      set({ snapshot, error: null });
    } catch (e) {
      set({ error: errorMessage(e) });
    }
  },
  refreshProcesses: async (limit = 40) => {
    try {
      const processes = await backend.systemListProcesses(limit);
      set({ processes });
    } catch (e) {
      set({ error: errorMessage(e) });
    }
  },
}));

/** Polls the live snapshot while mounted. */
export function startSnapshotPolling(intervalMs: number): () => void {
  const tick = () => void useSystem.getState().refreshSnapshot();
  tick();
  const id = window.setInterval(tick, intervalMs);
  return () => window.clearInterval(id);
}
