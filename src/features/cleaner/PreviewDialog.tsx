import { AlertTriangle, ShieldCheck, Trash2 } from "lucide-react";
import { Button } from "@/components/ui/Button";
import { Dialog } from "@/components/ui/Dialog";
import { Meter } from "@/components/ui/Meter";
import { RiskBadge } from "@/components/ui/RiskBadge";
import { abbreviatePath, formatBytes, formatCount } from "@/lib/format";
import { useScan } from "@/stores/scan";
import { useSystem } from "@/stores/system";

export function PreviewDialog() {
  const plan = useScan((s) => s.plan);
  const closePreview = useScan((s) => s.closePreview);
  const execute = useScan((s) => s.execute);
  const executing = useScan((s) => s.executing);
  const progress = useScan((s) => s.cleanupProgress);
  const home = useSystem((s) => s.info?.homeDir);

  if (!plan) return null;
  const permanent = plan.mode === "permanent";
  const permanentOnly = plan.targets.filter((t) => t.permanentOnly).length;
  const riskiest = plan.targets.some((t) => t.risk === "medium" || t.risk === "high");
  const pct = progress && progress.total > 0 ? (progress.done / progress.total) * 100 : 0;

  return (
    <Dialog open onClose={() => !executing && closePreview()} closeOnBackdrop={!executing}>
      <div className="px-5 pt-5 pb-4">
        <div className="flex items-start gap-3">
          <div className="flex h-9 w-9 shrink-0 items-center justify-center rounded-lg bg-accent-soft text-accent">
            {permanent ? <AlertTriangle size={18} /> : <ShieldCheck size={18} />}
          </div>
          <div className="min-w-0 flex-1">
            <h2 className="text-[15px] font-semibold tracking-tight">Preview Cleanup</h2>
            <p className="mt-0.5 text-[12.5px] text-fg-muted">
              Nothing has been removed yet. Review what Prune would{" "}
              {permanent ? "delete permanently" : "move to the Trash"}.
            </p>
          </div>
        </div>

        <div className="mt-4 grid grid-cols-3 gap-2">
          <Stat label="Would remove" value={formatBytes(plan.totalBytes)} strong />
          <Stat label="Files" value={formatCount(plan.fileCount)} />
          <Stat label="Directories" value={formatCount(plan.directoryCount)} />
        </div>

        {(riskiest || permanentOnly > 0 || permanent) && (
          <ul className="mt-3 space-y-1 text-[12px]">
            {permanent && (
              <li className="flex items-center gap-2 text-danger">
                <AlertTriangle size={13} /> Permanent deletion cannot be undone.
              </li>
            )}
            {!permanent && permanentOnly > 0 && (
              <li className="flex items-center gap-2 text-fg-muted">
                <Trash2 size={13} /> {permanentOnly} item{permanentOnly > 1 ? "s" : ""} already in
                the Trash will be deleted permanently.
              </li>
            )}
            {riskiest && (
              <li className="flex items-center gap-2 text-warn">
                <AlertTriangle size={13} /> Includes medium-risk items. Make sure you don't need
                them.
              </li>
            )}
          </ul>
        )}

        <div className="mt-4 max-h-[260px] overflow-y-auto rounded-md border border-line">
          {plan.targets.map((t) => (
            <div
              key={t.id}
              className="flex items-center gap-2 border-t border-line/60 px-3 py-1.5 text-[12px] first:border-t-0"
            >
              <div className="min-w-0 flex-1">
                <div className="truncate font-medium">{t.label}</div>
                <div className="truncate font-mono text-[11px] text-fg-faint">
                  {abbreviatePath(t.path, home)}
                </div>
              </div>
              <RiskBadge risk={t.risk} />
              <div className="w-[72px] text-right tnum">{formatBytes(t.sizeBytes)}</div>
            </div>
          ))}
          {plan.blocked.map((b) => (
            <div
              key={b.targetId}
              className="flex items-center gap-2 border-t border-line/60 bg-danger-soft/40 px-3 py-1.5 text-[12px]"
            >
              <div className="min-w-0 flex-1">
                <div className="truncate font-mono text-[11px]">
                  {abbreviatePath(b.path, home) || b.targetId}
                </div>
                <div className="truncate text-[11px] text-danger">Blocked: {b.reason}</div>
              </div>
              <RiskBadge risk="protected" />
            </div>
          ))}
        </div>

        {executing && progress && (
          <div className="mt-4">
            <div className="flex justify-between text-[12px] text-fg-muted tnum">
              <span>
                Removing {progress.done} / {progress.total}
              </span>
              <span>{formatBytes(progress.removedBytes)}</span>
            </div>
            <Meter value={pct} tone="accent" className="mt-1.5" />
            <div className="mt-1 truncate font-mono text-[11px] text-fg-faint">
              {abbreviatePath(progress.currentPath, home)}
            </div>
          </div>
        )}
      </div>

      <div className="flex items-center justify-end gap-2 border-t border-line bg-surface-2/60 px-5 py-3">
        <Button variant="ghost" onClick={closePreview} disabled={executing}>
          Cancel
        </Button>
        <Button
          variant={permanent ? "danger" : "primary"}
          onClick={() => void execute()}
          loading={executing}
          disabled={plan.targets.length === 0}
        >
          {permanent ? "Delete" : "Move to Trash"} {formatBytes(plan.totalBytes)}
        </Button>
      </div>
    </Dialog>
  );
}

function Stat({ label, value, strong }: { label: string; value: string; strong?: boolean }) {
  return (
    <div className="rounded-md border border-line bg-surface-2/60 px-3 py-2">
      <div className="text-[11px] text-fg-faint">{label}</div>
      <div
        className={
          strong ? "text-[18px] font-semibold tracking-tight tnum" : "text-[15px] font-medium tnum"
        }
      >
        {value}
      </div>
    </div>
  );
}
