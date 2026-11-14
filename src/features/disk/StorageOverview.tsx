import { ArrowUpRight, HardDrive } from "lucide-react";
import { cn } from "@/lib/cn";
import { formatBytes } from "@/lib/format";
import { useSystem } from "@/stores/system";
import { capacity, diskForPath } from "./storage";

export function StorageOverview({
  path = "",
  onExplore,
  compact = false,
}: {
  path?: string;
  onExplore?: () => void;
  /** One line, for the dashboard, where the disk is context for the figure below it. */
  compact?: boolean;
}) {
  const disks = useSystem((s) => s.snapshot?.disks);
  const disk = diskForPath(disks ?? [], path);
  const usage = disk ? capacity(disk) : undefined;
  const tight = (usage?.percent ?? 0) >= 90;
  if (compact) {
    return (
      <section aria-label="Disk capacity" className="flex items-center gap-4 text-[12px]">
        <span className="flex shrink-0 items-center gap-2 font-medium">
          <HardDrive size={15} className="text-fg-muted" />
          {disk?.name || "Your storage"}
        </span>
        <span className="shrink-0 text-fg-muted tnum">
          {usage ? (
            <>
              <span className={cn("font-semibold text-fg", tight && "text-danger")}>
                {formatBytes(usage.available)}
              </span>{" "}
              free of {formatBytes(usage.total)}
            </>
          ) : (
            "Reading disk capacity…"
          )}
        </span>
        <div
          role="progressbar"
          aria-label="Disk space used"
          aria-valuemin={0}
          aria-valuemax={100}
          aria-valuenow={usage ? Math.round(usage.percent) : 0}
          aria-valuetext={
            usage
              ? `${usage.percent.toFixed(1)}% used; ${formatBytes(usage.available)} available`
              : "Unknown"
          }
          className="h-1.5 min-w-[60px] flex-1 overflow-hidden rounded-full bg-fg-faint/15"
        >
          {usage && (
            <div
              className={cn(
                "h-full rounded-full transition-[width] duration-500 motion-reduce:transition-none",
                tight ? "bg-danger/80" : "bg-accent/70",
              )}
              style={{ width: `${usage.percent}%` }}
            />
          )}
        </div>
        {usage && (
          <span className={cn("shrink-0 text-fg-muted tnum", tight && "text-danger")}>
            {Math.round(usage.percent)}% used
          </span>
        )}
        {onExplore && (
          <button
            type="button"
            onClick={onExplore}
            className="flex shrink-0 items-center gap-0.5 font-medium text-accent hover:underline"
          >
            Explore storage <ArrowUpRight size={13} />
          </button>
        )}
      </section>
    );
  }
  return (
    <section aria-label="Disk capacity" className="rounded-xl border border-line bg-surface p-5">
      <div className="flex items-start justify-between gap-4">
        <div className="flex items-center gap-2.5">
          <span className="rounded-lg border border-line bg-surface-2 p-2 text-fg-muted">
            <HardDrive size={18} />
          </span>
          <div>
            <h2 className="text-[13px] font-semibold">{disk?.name || "Your storage"}</h2>
            <p className="mt-0.5 text-[11px] text-fg-faint">
              {disk
                ? `${disk.fileSystem} · ${disk.isRemovable ? "External drive" : "Local drive"}`
                : "Reading disk capacity…"}
            </p>
          </div>
        </div>
        {onExplore && (
          <button
            type="button"
            onClick={onExplore}
            className="flex items-center gap-1 text-[12px] font-medium text-accent hover:underline"
          >
            Explore storage <ArrowUpRight size={14} />
          </button>
        )}
      </div>
      <div className="mt-5 flex flex-wrap items-baseline justify-between gap-x-5 gap-y-1">
        <div className="flex items-baseline gap-1.5 tnum">
          <span
            className={cn(
              "text-[30px] leading-none font-semibold tracking-tight",
              tight && "text-danger",
            )}
          >
            {usage ? formatBytes(usage.available) : "—"}
          </span>
          <span className="text-[12px] text-fg-muted">available</span>
        </div>
        <span className="text-[12px] text-fg-muted tnum">
          {usage
            ? `${formatBytes(usage.used)} of ${formatBytes(usage.total)} used`
            : "Waiting for this computer"}
        </span>
      </div>
      {usage && (
        <>
          <div
            role="progressbar"
            aria-label="Disk space used"
            aria-valuemin={0}
            aria-valuemax={100}
            aria-valuenow={Math.round(usage.percent)}
            aria-valuetext={`${usage.percent.toFixed(1)}% used; ${formatBytes(usage.available)} available`}
            className="mt-3 h-3 overflow-hidden rounded-full bg-surface-2 ring-1 ring-inset ring-line"
          >
            <div
              className={cn(
                "h-full rounded-full transition-[width] duration-500 motion-reduce:transition-none",
                tight ? "bg-danger/80" : "bg-accent/80",
              )}
              style={{ width: `${usage.percent}%` }}
            />
          </div>
          <div className="mt-2 flex flex-wrap items-center justify-between gap-2 text-[11px] text-fg-muted">
            <span className="tnum">{usage.percent.toFixed(1)}% used</span>
            <span className={cn(tight && "text-danger")}>
              {tight
                ? "Space is running low. Explore large folders to decide what can go."
                : "Capacity is reported live by your operating system."}
            </span>
          </div>
        </>
      )}
    </section>
  );
}
