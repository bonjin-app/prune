import { useSystem } from "@/stores/system";
import type { Platform } from "@/types/models";

/**
 * What this machine calls the place removed items go: the Trash on a Mac, the Recycle Bin on
 * Windows.
 *
 * The interface said "Move to Trash" everywhere, Windows included, where the same button puts
 * things in the Recycle Bin — the one word a person reads right before confirming a removal.
 * Before the platform is known it falls back to the Mac word, the same default the shortcut hints
 * use.
 */
export function trashName(platform: Platform | undefined): string {
  return platform === "windows" ? "Recycle Bin" : "Trash";
}

/** `trashName` for the machine Prune is running on. */
export function useTrashName(): string {
  return trashName(useSystem((s) => s.meta?.platform));
}

/**
 * What the cleaner says it looks through.
 *
 * The Trash is on the list only where Prune actually reads it. On Windows the Recycle Bin is
 * per-volume and is not scanned, so naming it would promise a search that never happens.
 */
export function cleanerSources(platform: Platform | undefined): string {
  const items = [
    "application caches",
    "logs",
    "temporary files",
    "browser caches",
    ...(platform === "windows" ? [] : [`the ${trashName(platform)}`]),
    "old installers",
  ];
  return `${items.slice(0, -1).join(", ")} and ${items[items.length - 1]}`;
}
