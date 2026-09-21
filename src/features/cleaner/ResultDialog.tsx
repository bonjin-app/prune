import { CheckCircle2, XCircle } from "lucide-react";
import { Button } from "@/components/ui/Button";
import { Dialog } from "@/components/ui/Dialog";
import { cn } from "@/lib/cn";
import { abbreviatePath, formatBytes, formatCount } from "@/lib/format";
import { useScan } from "@/stores/scan";
import { useSystem } from "@/stores/system";

export function ResultDialog() {
  const result = useScan((s) => s.result);
  const dismiss = useScan((s) => s.dismissResult);
  const home = useSystem((s) => s.info?.homeDir);
  if (!result) return null;
  const ok = result.status === "success";
  // The largest thing on screen should not say "success" when nothing was removed. It is the
  // first thing read and often the only thing read.
  const tone = ok
    ? "bg-accent-soft text-accent"
    : result.status === "partial"
      ? "bg-warn-soft text-warn"
      : "bg-danger-soft text-danger";
  return (
    <Dialog open onClose={dismiss} className="max-w-[460px]" label="Cleanup result">
      <div className="px-5 pt-6 pb-4 text-center">
        <div
          className={cn(
            "mx-auto flex h-12 w-12 items-center justify-center rounded-full",
            tone,
          )}
        >
          {ok ? <CheckCircle2 size={26} /> : <XCircle size={26} />}
        </div>
        <h2 className="mt-3 text-[16px] font-semibold tracking-tight">
          {ok ? "Pruned" : result.status === "partial" ? "Partially pruned" : "Nothing removed"}
        </h2>
        <div className="mt-1 text-[28px] font-semibold tracking-tight tnum">
          {formatBytes(result.removedBytes)}
        </div>
        <p className="text-[12.5px] text-fg-muted">
          {formatCount(result.removedFiles)} files in {result.removedTargets} item
          {result.removedTargets === 1 ? "" : "s"}{" "}
          {result.mode === "trash" ? "moved to the Trash" : "deleted permanently"}.
        </p>
        {result.logError && (
          <p className="mt-3 text-[12px] text-warn">
            The cleanup happened, but it could not be added to the operation log: {result.logError}
          </p>
        )}
        {result.failed.length > 0 && (
          <div className="mt-4 max-h-[160px] overflow-y-auto rounded-md border border-line text-left text-[11.5px]">
            {result.failed.map((f) => (
              <div
                key={f.targetId}
                className="border-t border-line/60 px-3 py-1.5 first:border-t-0"
              >
                <div className="truncate font-mono">{abbreviatePath(f.path, home)}</div>
                <div className="truncate text-danger">{f.error}</div>
              </div>
            ))}
          </div>
        )}
      </div>
      <div className="flex justify-center border-t border-line bg-surface-2/60 px-5 py-3">
        <Button variant="primary" onClick={dismiss} autoFocus>
          Done
        </Button>
      </div>
    </Dialog>
  );
}
