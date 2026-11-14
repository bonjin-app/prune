import { Lock, X } from "lucide-react";
import { Button } from "./Button";
import { backend } from "@/lib/tauri";
import { useSystem } from "@/stores/system";
import { useUi } from "@/stores/ui";

/**
 * Shown when the OS is hiding locations from Prune. Without this the affected scans simply
 * report a smaller number with no explanation, which is the one thing a cleanup tool must
 * never do.
 */
export function PermissionBanner({ compact = false }: { compact?: boolean }) {
  const permissions = useSystem((s) => s.permissions);
  const dismissed = useUi((s) => s.permissionNoticeDismissed);
  const dismiss = useUi((s) => s.dismissPermissionNotice);

  if (dismissed || !permissions || permissions.fullDiskAccess !== "denied") return null;

  // One line, where the screen has no room to spare. The fix is the button; how to grant
  // access is in its title and on every other screen.
  if (compact) {
    return (
      <div className="flex items-center gap-2.5 rounded-lg border border-warn/30 bg-warn-soft/40 px-3 py-1.5 text-[12px]">
        <Lock size={13} className="shrink-0 text-warn" />
        <span className="min-w-0 flex-1 truncate" title={permissions.howToGrant}>
          <span className="font-medium">Hidden from Prune:</span>{" "}
          <span className="text-fg-muted">{permissions.blocked.join(", ")}</span>
        </span>
        <Button size="sm" variant="ghost" onClick={() => void backend.appOpenPrivacySettings()}>
          Grant access
        </Button>
        <button
          type="button"
          onClick={dismiss}
          className="rounded p-1 text-fg-faint hover:text-fg"
          aria-label="Dismiss"
        >
          <X size={13} />
        </button>
      </div>
    );
  }

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
        onClick={dismiss}
        className="rounded p-1 text-fg-faint hover:text-fg"
        aria-label="Dismiss"
      >
        <X size={13} />
      </button>
    </div>
  );
}
