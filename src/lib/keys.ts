import { useSystem } from "@/stores/system";
import type { Platform } from "@/types/models";

/**
 * How to write a shortcut on this machine: "⌘1" on a Mac, "Ctrl+1" everywhere else.
 *
 * The shortcuts themselves accept either modifier, but the hints were all written with ⌘ — a
 * key Windows keyboards do not have.
 */
export function shortcutLabel(platform: Platform | undefined, key: string): string {
  return platform === "windows" ? `Ctrl+${key.toUpperCase()}` : `⌘${key.toUpperCase()}`;
}

/** The same shortcut for `aria-keyshortcuts`, which spells modifiers out. */
export function shortcutAria(platform: Platform | undefined, key: string): string {
  return `${platform === "windows" ? "Control" : "Meta"}+${key.toUpperCase()}`;
}

/** `shortcutLabel` for the machine Prune is running on. */
export function useShortcutLabel(): (key: string) => string {
  const platform = useSystem((s) => s.meta?.platform);
  return (key) => shortcutLabel(platform, key);
}
