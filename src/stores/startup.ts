import { create } from "zustand";
import { backend, errorMessage } from "@/lib/tauri";
import type { StartupItem } from "@/types/models";

interface StartupState {
  items: StartupItem[];
  loading: boolean;
  /** Ids currently being toggled. */
  pending: Set<string>;
  error: string | null;

  load: () => Promise<void>;
  setEnabled: (id: string, enabled: boolean) => Promise<void>;
  clearError: () => void;
}

export const useStartup = create<StartupState>((set, get) => ({
  items: [],
  loading: false,
  pending: new Set(),
  error: null,

  load: async () => {
    if (get().loading) return;
    set({ loading: true, error: null });
    try {
      set({ items: await backend.startupList(), loading: false });
    } catch (e) {
      set({ loading: false, error: errorMessage(e) });
    }
  },

  setEnabled: async (id, enabled) => {
    set((s) => ({ pending: new Set(s.pending).add(id) }));
    try {
      const updated = await backend.startupSetEnabled(id, enabled);
      set((s) => ({ items: s.items.map((i) => (i.id === id ? { ...i, ...updated } : i)) }));
    } catch (e) {
      set({ error: errorMessage(e) });
    } finally {
      set((s) => {
        const pending = new Set(s.pending);
        pending.delete(id);
        return { pending };
      });
    }
  },

  clearError: () => set({ error: null }),
}));
