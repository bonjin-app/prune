import { useState } from "react";
import { ChevronRight, GitBranch } from "lucide-react";
import { Checkbox } from "@/components/ui/Checkbox";
import { cn } from "@/lib/cn";
import { formatBytes, formatCount } from "@/lib/format";
import { useScan } from "@/stores/scan";
import { TargetRow } from "./ProviderGroup";
import { daysSince, describeActivity, STALE_DAYS, type Project } from "./projects";

/**
 * One project's artifacts, collapsed into a single row.
 *
 * The decision this is meant to support is the one a developer can actually make: this project
 * has not been touched in eight months and is holding 11 GB.
 */
export function ProjectSection({ project }: { project: Project }) {
  const [open, setOpen] = useState(false);
  const selected = useScan((s) => s.selected);
  const setTargets = useScan((s) => s.setTargets);

  const selectable = project.targets.filter((t) => t.risk !== "protected");
  const chosen = selectable.filter((t) => selected.has(t.id));
  const all = selectable.length > 0 && chosen.length === selectable.length;
  const some = chosen.length > 0 && !all;
  const days = daysSince(project.group.lastActiveAt);
  const stale = days !== null && days >= STALE_DAYS;

  return (
    <div className="border-t border-line/60 first:border-t-0">
      <div
        role="button"
        tabIndex={0}
        aria-expanded={open}
        aria-label={`${open ? "Collapse" : "Expand"} ${project.group.label}`}
        onClick={() => setOpen((o) => !o)}
        onKeyDown={(e) => {
          if (e.key === "Enter" || e.key === " ") {
            e.preventDefault();
            setOpen((o) => !o);
          }
        }}
        className={cn(
          "flex cursor-default items-center gap-2.5 px-3 py-[7px]",
          chosen.length > 0 ? "bg-accent-soft/40" : "hover:bg-surface-2/60",
        )}
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
          label={`Select everything in ${project.group.label}`}
        />
        <ChevronRight
          size={13}
          className={cn("text-fg-faint transition-transform", open && "rotate-90")}
        />
        <div className="min-w-0 flex-1">
          <div className="flex items-center gap-2">
            <span className="truncate text-[12.5px] font-medium">{project.group.label}</span>
            <span
              className={cn(
                "flex shrink-0 items-center gap-1 text-[11px]",
                stale ? "text-warn" : "text-fg-faint",
              )}
              title={project.group.key}
            >
              <GitBranch size={10} />
              {describeActivity(days)}
            </span>
          </div>
          <div className="truncate text-[11px] text-fg-faint">
            {project.targets
              .map((t) => t.description ?? t.label)
              .slice(0, 4)
              .join(", ")}
            {project.targets.length > 4 && ` +${project.targets.length - 4}`}
          </div>
        </div>
        <div className="w-[60px] text-right text-[11px] text-fg-faint tnum">
          {formatCount(project.targets.length)} items
        </div>
        <div className="w-[76px] text-right text-[12.5px] font-medium tnum">
          {formatBytes(project.sizeBytes)}
        </div>
      </div>
      {open && (
        <div className="bg-surface-2/30">
          {project.targets.map((target) => (
            <TargetRow key={target.id} target={target} indent />
          ))}
        </div>
      )}
    </div>
  );
}
