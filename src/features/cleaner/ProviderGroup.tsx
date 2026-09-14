import { useState } from "react";
import { AlertCircle, ChevronRight, FolderOpen } from "lucide-react";
import { Checkbox } from "@/components/ui/Checkbox";
import { RiskBadge } from "@/components/ui/RiskBadge";
import { cn } from "@/lib/cn";
import { abbreviatePath, formatBytes, formatCount, formatRelative } from "@/lib/format";
import { backend } from "@/lib/tauri";
import { useScan } from "@/stores/scan";
import { useSystem } from "@/stores/system";
import type { CleanupTarget, ScanResult } from "@/types/models";
import { CATEGORY_LABEL } from "@/types/models";

export function ProviderGroup({ result }: { result: ScanResult }) {
  const [open, setOpen] = useState(true);
  const [showIssues, setShowIssues] = useState(false);
  const selected = useScan((s) => s.selected);
  const setTargets = useScan((s) => s.setTargets);
  const providers = useScan((s) => s.providers);
  const description = providers.find((p) => p.id === result.providerId)?.description;

  const selectable = result.targets.filter((t) => t.risk !== "protected");
  const selectedCount = selectable.filter((t) => selected.has(t.id)).length;
  const all = selectable.length > 0 && selectedCount === selectable.length;
  const some = selectedCount > 0 && !all;
  const selectedBytes = selectable
    .filter((t) => selected.has(t.id))
    .reduce((a, t) => a + t.sizeBytes, 0);

  return (
    <section className="overflow-hidden rounded-lg border border-line bg-surface">
      <header
        role="button"
        tabIndex={0}
        aria-expanded={open}
        aria-label={`${open ? "Collapse" : "Expand"} ${result.providerName}`}
        className="flex cursor-default items-center gap-2.5 px-3 py-2.5 hover:bg-surface-2/60"
        onClick={() => setOpen((o) => !o)}
        onKeyDown={(e) => {
          if (e.key === "Enter" || e.key === " ") {
            e.preventDefault();
            setOpen((o) => !o);
          }
        }}
      >
        <Checkbox
          checked={all}
          indeterminate={some}
          disabled={selectable.length === 0}
          onChange={(next) =>
            setTargets(
              selectable.map((t) => t.id),
              next,
            )
          }
          label={`Select all in ${result.providerName}`}
        />
        <ChevronRight
          size={14}
          className={cn("text-fg-faint transition-transform", open && "rotate-90")}
        />
        <div className="min-w-0 flex-1">
          <div className="flex items-baseline gap-2">
            <h3 className="text-[13px] font-semibold">{result.providerName}</h3>
            <span className="text-[11px] text-fg-faint">{CATEGORY_LABEL[result.category]}</span>
          </div>
          {description && <p className="truncate text-[11.5px] text-fg-muted">{description}</p>}
        </div>
        {result.issues.length > 0 && (
          <button
            type="button"
            onClick={(e) => {
              e.stopPropagation();
              setShowIssues((v) => !v);
              setOpen(true);
            }}
            className="flex items-center gap-1 rounded px-1.5 py-0.5 text-[11px] text-warn hover:bg-warn-soft"
            title="Some locations could not be read"
          >
            <AlertCircle size={12} /> {result.issues.length}
          </button>
        )}
        <div className="text-right tnum">
          <div className="text-[13px] font-semibold">{formatBytes(result.totalBytes)}</div>
          <div className="text-[11px] text-fg-faint">
            {selectedCount > 0
              ? `${formatBytes(selectedBytes)} selected`
              : `${formatCount(result.targets.length)} items · ${formatCount(result.totalFiles)} files`}
          </div>
        </div>
      </header>

      {open && (
        <div className="border-t border-line">
          {showIssues && result.issues.length > 0 && (
            <ul className="border-b border-line bg-warn-soft/40 px-3 py-2 text-[11.5px]">
              {result.issues.slice(0, 8).map((i) => (
                <li key={i.path} className="flex gap-2 truncate">
                  <span className="shrink-0 text-warn">skipped</span>
                  <span className="truncate font-mono text-fg-muted">{i.path}</span>
                  <span className="ml-auto shrink-0 text-fg-faint">{i.message}</span>
                </li>
              ))}
              {result.issues.length > 8 && (
                <li className="text-fg-faint">…and {result.issues.length - 8} more</li>
              )}
            </ul>
          )}
          {result.targets.map((t) => (
            <TargetRow key={t.id} target={t} />
          ))}
        </div>
      )}
    </section>
  );
}

function TargetRow({ target }: { target: CleanupTarget }) {
  const checked = useScan((s) => s.selected.has(target.id));
  const toggle = useScan((s) => s.toggleTarget);
  const home = useSystem((s) => s.info?.homeDir);
  const protectedItem = target.risk === "protected";

  return (
    <div
      className={cn(
        "group flex items-center gap-2.5 border-t border-line/60 px-3 py-[7px] first:border-t-0",
        protectedItem ? "opacity-60" : "hover:bg-surface-2/60",
        checked && "bg-accent-soft/40",
      )}
      onClick={() => !protectedItem && toggle(target.id)}
    >
      <Checkbox
        checked={checked}
        disabled={protectedItem}
        onChange={() => toggle(target.id)}
        label={target.label}
      />
      <div className="w-[14px]" />
      <div className="min-w-0 flex-1">
        <div className="flex items-center gap-2">
          <span className="truncate text-[12.5px] font-medium">{target.label}</span>
          {target.description && (
            <span className="shrink-0 rounded-sm bg-surface-2 px-1 text-[10.5px] text-fg-muted">
              {target.description}
            </span>
          )}
          {target.permanentOnly && (
            <span className="shrink-0 text-[10.5px] text-fg-faint">permanent</span>
          )}
        </div>
        <div className="truncate font-mono text-[11px] text-fg-faint">
          {abbreviatePath(target.path, home)}
        </div>
      </div>
      <button
        type="button"
        onClick={(e) => {
          e.stopPropagation();
          void backend.fsReveal(target.path);
        }}
        className="invisible rounded p-1 text-fg-faint hover:bg-surface-2 hover:text-fg group-hover:visible"
        title="Reveal"
        aria-label="Reveal in file manager"
      >
        <FolderOpen size={13} />
      </button>
      <RiskBadge risk={target.risk} />
      <div className="w-[64px] text-right text-[11px] text-fg-faint tnum">
        {target.modifiedAt ? formatRelative(target.modifiedAt) : ""}
      </div>
      <div className="w-[60px] text-right text-[11px] text-fg-faint tnum">
        {target.kind === "directory" ? `${formatCount(target.fileCount)} files` : "file"}
      </div>
      <div className="w-[76px] text-right text-[12.5px] font-medium tnum">
        {formatBytes(target.sizeBytes)}
      </div>
    </div>
  );
}
