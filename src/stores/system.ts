import { create } from "zustand";
import { backend, errorMessage } from "@/lib/tauri";
import type {
  AppMeta,
  Permissions,
  ProcessInfo,
  StopMode,
  SystemInfo,
  SystemSnapshot,
} from "@/types/models";

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
  /** Asks a process to stop, or forces it. Returns true when the system accepted. */
  stopProcess: (pid: number, mode: StopMode) => Promise<boolean>;
  clearError: () => void;
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
  stopProcess: async (pid, mode) => {
    try {
      await backend.systemStopProcess(pid, mode);
      // A process asked to quit takes a moment to go; the next poll picks that up.
      set((s) => ({ processes: s.processes.filter((p) => p.pid !== pid) }));
      return true;
    } catch (e) {
      set({ error: errorMessage(e) });
      return false;
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

  clearError: () => set({ error: null }),
}));

/**
 * Runs `tick` on a timer while the window is actually being looked at.
 *
 * A snapshot is not free — it reads every process and, periodically, every mounted volume — and
 * a monitor nobody can see has nothing to report. Without this, leaving Prune open behind
 * another window costs the user battery for a view they are not reading, which is a poor
 * trade for a tool whose whole purpose is to leave the machine in better shape. Returning to
 * the window ticks immediately, so what appears is current rather than however old the last
 * reading was.
 */
export function startPolling(tick: () => void, intervalMs: number): () => void {
  let id: number | undefined;

  const stop = () => {
    if (id !== undefined) window.clearInterval(id);
    id = undefined;
  };
  const start = () => {
    if (id !== undefined) return;
    tick();
    id = window.setInterval(tick, intervalMs);
  };
  const onVisibility = () => (document.hidden ? stop() : start());

  if (!document.hidden) start();
  document.addEventListener("visibilitychange", onVisibility);
  return () => {
    document.removeEventListener("visibilitychange", onVisibility);
    stop();
  };
}

/** Polls the live snapshot while mounted and visible. */
export function startSnapshotPolling(intervalMs: number): () => void {
  return startPolling(() => void useSystem.getState().refreshSnapshot(), intervalMs);
}
