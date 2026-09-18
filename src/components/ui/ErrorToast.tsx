import { useEffect, useState } from "react";
import { AlertTriangle, X } from "lucide-react";
import { useApps } from "@/stores/apps";
import { useDisk } from "@/stores/disk";
import { useScan } from "@/stores/scan";
import { useStartup } from "@/stores/startup";
import { useSystem } from "@/stores/system";

/**
 * The one place a failed command becomes visible.
 *
 * `role="alert"` matters more here than anywhere else in the interface: without it a screen
 * reader user is told nothing at all when an action fails, and the only signal that the thing
 * they asked for did not happen is its absence.
 *
 * Dismissal remembers the message rather than only clearing it, because some errors come from
 * a timer. The Monitor polls every two seconds, so if the backend is unreachable the store sets
 * the same message again immediately and a toast that only cleared state could never be closed.
 */
export function ErrorToast() {
  const scanError = useScan((s) => s.error);
  const clearScan = useScan((s) => s.clearError);
  const systemError = useSystem((s) => s.error);
  const clearSystem = useSystem((s) => s.clearError);
  const diskError = useDisk((s) => s.error);
  const clearDisk = useDisk((s) => s.clearError);
  const appsError = useApps((s) => s.error);
  const clearApps = useApps((s) => s.clearError);
  const startupError = useStartup((s) => s.error);
  const clearStartup = useStartup((s) => s.clearError);

  const message = scanError ?? diskError ?? appsError ?? startupError ?? systemError;
  const [dismissed, setDismissed] = useState<string | null>(null);
  const showing = message !== null && message !== undefined && message !== dismissed;

  const dismiss = () => {
    if (message) setDismissed(message);
    clearScan();
    clearDisk();
    clearApps();
    clearStartup();
    clearSystem();
  };

  // Escape is where people reach first to make something go away.
  useEffect(() => {
    if (!showing) return;
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape") dismiss();
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  });

  if (!showing) return null;
  return (
    <div
      role="alert"
      className="fade-in fixed right-4 bottom-4 z-50 flex max-w-[420px] items-start gap-2.5 rounded-lg border border-danger/30 bg-surface px-3 py-2.5 shadow-lg"
    >
      <AlertTriangle size={15} className="mt-0.5 shrink-0 text-danger" />
      <div className="min-w-0 flex-1 text-[12.5px]">
        <div className="font-medium">Something went wrong</div>
        <div className="mt-0.5 break-words text-fg-muted">{message}</div>
      </div>
      <button
        type="button"
        onClick={dismiss}
        className="rounded p-0.5 text-fg-faint hover:text-fg"
        aria-label="Dismiss"
      >
        <X size={14} />
      </button>
    </div>
  );
}
