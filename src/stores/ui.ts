import { create } from "zustand";

export type ViewId =
  | "dashboard"
  | "cleaner"
  | "uninstaller"
  | "disk"
  | "developer"
  | "monitor"
  | "startup"
  | "settings";

export type ThemePreference = "system" | "light" | "dark";

const THEME_KEY = "prune.theme";

function loadTheme(): ThemePreference {
  try {
    const v = localStorage.getItem(THEME_KEY);
    if (v === "light" || v === "dark" || v === "system") return v;
  } catch {
    /* storage unavailable */
  }
  return "system";
}

export function applyTheme(pref: ThemePreference) {
  const root = document.documentElement;
  const dark =
    pref === "dark" ||
    (pref === "system" && window.matchMedia("(prefers-color-scheme: dark)").matches);
  root.classList.toggle("dark", dark);
  root.style.colorScheme = dark ? "dark" : "light";
}

interface UiState {
  view: ViewId;
  theme: ThemePreference;
  paletteOpen: boolean;
  setView: (view: ViewId) => void;
  setTheme: (theme: ThemePreference) => void;
  setPaletteOpen: (open: boolean) => void;
}

export const useUi = create<UiState>((set) => ({
  view: "dashboard",
  theme: loadTheme(),
  paletteOpen: false,
  setView: (view) => set({ view }),
  setTheme: (theme) => {
    try {
      localStorage.setItem(THEME_KEY, theme);
    } catch {
      /* ignore */
    }
    applyTheme(theme);
    set({ theme });
  },
  setPaletteOpen: (paletteOpen) => set({ paletteOpen }),
}));
