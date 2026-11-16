import { Card, CardHeader } from "@/components/ui/Card";
import { Meter } from "@/components/ui/Meter";
import { formatBytes, formatPercent } from "@/lib/format";
import type { GpuStatus } from "@/types/models";

/**
 * Graphics processors and how busy they are, read-only. What the operating system does not
 * report is shown as unknown: Windows does not name an adapter in its counters, and a machine
 * without a modern graphics driver reports none at all.
 */
export function GpuCard({ gpus }: { gpus: GpuStatus[] | undefined }) {
  const list = gpus ?? [];
  return (
    <Card>
      <CardHeader title="Graphics" />
      <div className="space-y-3 px-4 pb-4 text-[12px]">
        {list.length === 0 ? (
          <p className="text-fg-faint">
            This system does not report its graphics processor to Prune.
          </p>
        ) : (
          list.map((gpu, i) => <GpuRow key={i} gpu={gpu} fallbackName={`GPU ${i + 1}`} />)
        )}
      </div>
    </Card>
  );
}

function GpuRow({ gpu, fallbackName }: { gpu: GpuStatus; fallbackName: string }) {
  const name = gpu.name ?? fallbackName;
  const memory =
    gpu.memoryUsedBytes != null
      ? gpu.memoryTotalBytes != null
        ? `${formatBytes(gpu.memoryUsedBytes)} of ${formatBytes(gpu.memoryTotalBytes)} memory`
        : `${formatBytes(gpu.memoryUsedBytes)} memory in use`
      : null;
  return (
    <div>
      <div className="flex justify-between gap-2">
        <span className="truncate text-fg-muted">
          {name}
          {gpu.coreCount != null ? `, ${gpu.coreCount} cores` : ""}
        </span>
        <span className="tnum">
          {gpu.usagePercent != null ? formatPercent(gpu.usagePercent) : "load unknown"}
        </span>
      </div>
      {gpu.usagePercent != null && (
        <Meter value={gpu.usagePercent} className="mt-1" tone="info" height={5} />
      )}
      {memory && <div className="mt-0.5 text-right text-[11px] text-fg-faint tnum">{memory}</div>}
    </div>
  );
}
