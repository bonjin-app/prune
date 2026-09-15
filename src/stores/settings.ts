import { create } from "zustand";
import { backend, errorMessage } from "@/lib/tauri";
import type { SettingsView } from "@/types/models";

interface SettingsState {
  view: SettingsView | null;
  loading: boolean;
  saving: boolean;
  error: string | null;

  load: () => Promise<void>;
  /** Replaces the whole list. Returns true when it was accepted. */
  setProjectRoots: (roots: string[]) => Promise<boolean>;
  addProjectRoot: (root: string) => Promise<boolean>;
  removeProjectRoot: (root: string) => Promise<boolean>;
  clearError: () => void;
}

export const useSettings = create<SettingsState>((set, get) => ({
  view: null,
  loading: false,
  saving: false,
  error: null,

  load: async () => {
    if (get().loading) return;
    set({ loading: true });
    try {
      set({ view: await backend.settingsGet(), loading: false, error: null });
    } catch (e) {
      set({ loading: false, error: errorMessage(e) });
    }
  },

  setProjectRoots: async (roots) => {
    set({ saving: true, error: null });
    try {
      set({ view: await backend.settingsSetProjectRoots(roots), saving: false });
      return true;
    } catch (e) {
      set({ saving: false, error: errorMessage(e) });
      return false;
    }
  },

  addProjectRoot: async (root) => {
    const trimmed = root.trim();
    if (!trimmed) return false;
    const current = get().view?.settings.projectRoots ?? [];
    if (current.includes(trimmed)) return true;
    return get().setProjectRoots([...current, trimmed]);
  },

  removeProjectRoot: async (root) => {
    const current = get().view?.settings.projectRoots ?? [];
    return get().setProjectRoots(current.filter((r) => r !== root));
  },

  clearError: () => set({ error: null }),
}));
