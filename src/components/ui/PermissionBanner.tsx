import { useState } from "react";
import { Lock, X } from "lucide-react";
import { Button } from "./Button";
import { backend } from "@/lib/tauri";
import { useSystem } from "@/stores/system";

/**
 * Shown when the OS is hiding locations from Prune. Without this the affected scans simply
 * report a smaller number with no explanation, which is the one thing a cleanup tool must
 * never do.
 */
export function PermissionBanner() {
  const permissions = useSystem((s) => s.permissions);
  const [dismissed, setDismissed] = useState(false);

  if (dismissed || !permissions || permissions.fullDiskAccess !== "denied") return null;

  return (
    <div className="mx-7 mb-3 flex items-start gap-3 rounded-lg border border-warn/30 bg-warn-soft/40 px-4 py-3">
      <Lock size={15} className="mt-0.5 shrink-0 text-warn" />
      <div className="min-w-0 flex-1 text-[12.5px]">
        <div className="font-medium">Some locations are hidden from Prune</div>
        <p className="mt-0.5 text-fg-muted">
          macOS is blocking {permissions.blocked.join(", ")}, so those are missing from every scan.{" "}
          {permissions.howToGrant}
        </p>
      </div>
      <Button size="sm" variant="secondary" onClick={() => void backend.appOpenPrivacySettings()}>
        Open Settings
      </Button>
      <button
        type="button"
        onClick={() => setDismissed(true)}
        className="rounded p-1 text-fg-faint hover:text-fg"
        aria-label="Dismiss"
      >
        <X size={13} />
      </button>
    </div>
  );
}
