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
  addProtectedPath: (path: string) => Promise<boolean>;
  removeProtectedPath: (path: string) => Promise<boolean>;
  /** Lets the user choose a path in the system panel, then protects it. False if cancelled. */
  pickProtectedPath: (kind: "folder" | "file") => Promise<boolean>;
  clearError: () => void;
}

type Set = (partial: Partial<SettingsState>) => void;
type Get = () => SettingsState;

/** Runs one settings change. The reply is the settings as now saved, list and all. */
async function saveWith(set: Set, get: Get, save: () => Promise<SettingsView>): Promise<boolean> {
  if (get().saving) return false;
  set({ saving: true, error: null });
  try {
    set({ view: await save(), saving: false });
    return true;
  } catch (e) {
    set({ saving: false, error: errorMessage(e) });
    return false;
  }
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
    if (get().saving) return false;
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
    const { view, saving } = get();
    if (saving || !view) return false;
    const trimmed = root.trim();
    if (!trimmed) return false;
    const current = view.projectRootsConfigured
      ? view.settings.projectRoots
      : view.effectiveProjectRoots;
    if (current.includes(trimmed)) return true;
    return get().setProjectRoots([...current, trimmed]);
  },

  removeProjectRoot: async (root) => {
    const current = get().view?.settings.projectRoots ?? [];
    return get().setProjectRoots(current.filter((r) => r !== root));
  },

  addProtectedPath: async (path) => {
    const { view, saving } = get();
    if (saving || !view) return false;
    let trimmed = path.trim();
    if (!trimmed) return false;
    // `~/…` is how people write a path in their home folder; the backend wants it absolute.
    if (trimmed === "~" || trimmed.startsWith("~/")) {
      trimmed = view.homeDir + trimmed.slice(1);
    }
    return saveWith(set, get, () => backend.settingsAddProtectedPath(trimmed));
  },

  removeProtectedPath: async (path) => {
    if (get().saving || !get().view) return false;
    return saveWith(set, get, () => backend.settingsRemoveProtectedPath(path));
  },

  pickProtectedPath: async (kind) => {
    if (get().saving || !get().view) return false;
    let picked: string | null;
    try {
      picked = await backend.settingsPickProtectedPath(kind);
    } catch (e) {
      set({ error: errorMessage(e) });
      return false;
    }
    return picked ? get().addProtectedPath(picked) : false;
  },

  clearError: () => set({ error: null }),
}));
