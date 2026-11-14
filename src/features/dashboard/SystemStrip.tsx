import type { ReactNode } from "react";
import { cn } from "@/lib/cn";
import { formatBytes, formatDuration, formatPercent } from "@/lib/format";
import { useSystem } from "@/stores/system";
import { useUi } from "@/stores/ui";

/**
 * How the machine is doing right now, in one row under what can be pruned.
 *
 * Context, not the point: each reading is small, and the row as a whole opens the Monitor,
 * where the same numbers have room. CPU is drawn per core, because one core pinned at 100%
 * reads as "4%" on an average and is the thing worth seeing. In a window too short to show it
 * without scrolling it is left out, since the Monitor is one click away.
 */
export function SystemStrip() {
  const snapshot = useSystem((s) => s.snapshot);
  const setView = useUi((s) => s.setView);

  const memory = snapshot?.memory;
  const memPct = memory ? (memory.usedBytes / memory.totalBytes) * 100 : 0;
  const cores = snapshot?.cpu.perCorePercent ?? [];

  return (
    <button
      type="button"
      onClick={() => setView("monitor")}
      aria-label="Open the Monitor"
      className="grid w-full grid-cols-2 overflow-hidden [@media(max-height:679px)]:hidden rounded-xl border border-line/70 bg-surface text-left transition-colors hover:border-line md:grid-cols-4"
    >
      <Tile label="CPU" value={snapshot ? formatPercent(snapshot.cpu.usagePercent) : "—"}>
        <span aria-hidden="true" className="flex h-5 items-end gap-[2px]">
          {cores.map((pct, i) => (
            <span
              key={i}
              className={cn(
                "w-full max-w-[6px] min-w-[2px] rounded-[1px]",
                pct >= 90 ? "bg-danger/80" : "bg-accent/60",
              )}
              style={{ height: `${Math.max(8, Math.min(100, pct))}%` }}
            />
          ))}
        </span>
      </Tile>
      <Tile label="Memory" value={memory ? formatPercent(memPct) : "—"}>
        <Meter pct={memPct} />
        <Detail>
          {memory
            ? `${formatBytes(memory.usedBytes)} of ${formatBytes(memory.totalBytes)}${
                memory.swapUsedBytes > 0 ? `, ${formatBytes(memory.swapUsedBytes)} swap` : ""
              }`
            : "—"}
        </Detail>
      </Tile>
      <Tile
        label="Network"
        value={snapshot ? `${formatBytes(snapshot.network.downBytesPerSec)}/s` : "—"}
      >
        <Detail>
          {snapshot ? `down, ${formatBytes(snapshot.network.upBytesPerSec)}/s up` : "—"}
        </Detail>
      </Tile>
      <Tile label="Up" value={snapshot ? formatDuration(snapshot.uptimeSeconds) : "—"}>
        <Detail>{snapshot ? `${snapshot.processCount} processes running` : "—"}</Detail>
      </Tile>
    </button>
  );
}

function Tile({ label, value, children }: { label: string; value: string; children: ReactNode }) {
  return (
    <span className="block min-w-0 border-line/60 px-4 py-3 [&:not(:first-child)]:border-l max-md:[&:nth-child(3)]:border-l-0 max-md:[&:nth-child(n+3)]:border-t">
      <span className="flex items-baseline justify-between gap-2">
        <span className="text-[11.5px] text-fg-muted">{label}</span>
        <span className="shrink-0 text-[13px] font-semibold whitespace-nowrap tnum">{value}</span>
      </span>
      <span className="mt-2 block">{children}</span>
    </span>
  );
}

function Meter({ pct }: { pct: number }) {
  return (
    <span aria-hidden="true" className="block h-1 overflow-hidden rounded-full bg-fg-faint/15">
      <span
        className={cn("block h-full rounded-full", pct >= 90 ? "bg-danger/80" : "bg-accent/60")}
        style={{ width: `${Math.min(100, pct)}%` }}
      />
    </span>
  );
}

function Detail({ children }: { children: ReactNode }) {
  return <span className="mt-1.5 block truncate text-[11px] text-fg-faint tnum">{children}</span>;
}
