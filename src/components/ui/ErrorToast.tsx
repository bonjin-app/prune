import { AlertTriangle, X } from "lucide-react";
import { useApps } from "@/stores/apps";
import { useDisk } from "@/stores/disk";
import { useScan } from "@/stores/scan";
import { useStartup } from "@/stores/startup";
import { useSystem } from "@/stores/system";

export function ErrorToast() {
  const scanError = useScan((s) => s.error);
  const clearScan = useScan((s) => s.clearError);
  const systemError = useSystem((s) => s.error);
  const diskError = useDisk((s) => s.error);
  const clearDisk = useDisk((s) => s.clearError);
  const appsError = useApps((s) => s.error);
  const clearApps = useApps((s) => s.clearError);
  const startupError = useStartup((s) => s.error);
  const clearStartup = useStartup((s) => s.clearError);
  const message = scanError ?? diskError ?? appsError ?? startupError ?? systemError;
  if (!message) return null;
  return (
    <div className="fade-in fixed right-4 bottom-4 z-50 flex max-w-[420px] items-start gap-2.5 rounded-lg border border-danger/30 bg-surface px-3 py-2.5 shadow-lg">
      <AlertTriangle size={15} className="mt-0.5 shrink-0 text-danger" />
      <div className="min-w-0 flex-1 text-[12.5px]">
        <div className="font-medium">Something went wrong</div>
        <div className="mt-0.5 break-words text-fg-muted">{message}</div>
      </div>
      <button
        type="button"
        onClick={() => {
          clearScan();
          clearDisk();
          clearApps();
          clearStartup();
          useSystem.setState({ error: null });
        }}
        className="rounded p-0.5 text-fg-faint hover:text-fg"
        aria-label="Dismiss"
      >
        <X size={14} />
      </button>
    </div>
  );
}
