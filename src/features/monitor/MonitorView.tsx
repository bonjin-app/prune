import { useEffect } from "react";
import { Card, CardHeader } from "@/components/ui/Card";
import { Meter } from "@/components/ui/Meter";
import { PageHeader } from "@/components/ui/PageHeader";
import { formatBytes, formatPercent } from "@/lib/format";
import { startSnapshotPolling, useSystem } from "@/stores/system";

export function MonitorView() {
  const snapshot = useSystem((s) => s.snapshot);
  const processes = useSystem((s) => s.processes);
  const refreshProcesses = useSystem((s) => s.refreshProcesses);

  useEffect(() => {
    const stop = startSnapshotPolling(2000);
    void refreshProcesses(40);
    const id = window.setInterval(() => void refreshProcesses(40), 3000);
    return () => {
      stop();
      window.clearInterval(id);
    };
  }, [refreshProcesses]);

  const mem = snapshot?.memory;
  return (
    <div className="pb-8">
      <PageHeader
        title="Monitor"
        description="Live CPU, memory, disk and process activity. Read-only."
      />
      <div className="grid grid-cols-2 gap-3 px-7">
        <Card>
          <CardHeader
            title="CPU"
            action={
              <span className="text-[18px] font-semibold tnum">
                {formatPercent(snapshot?.cpu.usagePercent ?? 0)}
              </span>
            }
          />
          <div className="grid grid-cols-8 gap-1.5 px-4 pb-4">
            {(snapshot?.cpu.perCorePercent ?? []).map((v, i) => (
              <div key={i} className="flex flex-col items-center gap-1">
                <div className="flex h-14 w-full items-end overflow-hidden rounded-sm bg-black/[0.06] dark:bg-white/[0.07]">
                  <div
                    className="w-full rounded-sm bg-accent transition-[height] duration-500"
                    style={{
                      height: `${Math.max(2, Math.min(100, v))}%`,
                      opacity: 0.5 + Math.min(100, v) / 200,
                    }}
                  />
                </div>
                <span className="text-[10px] text-fg-faint tnum">{i + 1}</span>
              </div>
            ))}
          </div>
        </Card>

        <Card>
          <CardHeader
            title="Memory"
            action={
              <span className="text-[18px] font-semibold tnum">
                {mem ? formatPercent((mem.usedBytes / mem.totalBytes) * 100) : "—"}
              </span>
            }
          />
          <div className="space-y-3 px-4 pb-4">
            <Row
              label="Used"
              value={mem ? formatBytes(mem.usedBytes) : "—"}
              pct={mem ? (mem.usedBytes / mem.totalBytes) * 100 : 0}
            />
            <Row
              label="Available"
              value={mem ? formatBytes(mem.availableBytes) : "—"}
              pct={mem ? (mem.availableBytes / mem.totalBytes) * 100 : 0}
              tone="info"
            />
            <Row
              label="Swap"
              value={
                mem ? `${formatBytes(mem.swapUsedBytes)} / ${formatBytes(mem.swapTotalBytes)}` : "—"
              }
              pct={
                mem && mem.swapTotalBytes > 0 ? (mem.swapUsedBytes / mem.swapTotalBytes) * 100 : 0
              }
              tone="warn"
            />
          </div>
        </Card>
      </div>

      <div className="mt-3 grid grid-cols-[1fr_1.6fr] gap-3 px-7">
        <Card>
          <CardHeader title="Disks" />
          <div className="space-y-3 px-4 pb-4">
            {(snapshot?.disks ?? []).map((d) => {
              const used = d.totalBytes - d.availableBytes;
              return (
                <Row
                  key={d.mountPoint}
                  label={`${d.name || d.mountPoint}${d.isRemovable ? " · removable" : ""}`}
                  value={`${formatBytes(used)} / ${formatBytes(d.totalBytes)}`}
                  pct={(used / d.totalBytes) * 100}
                />
              );
            })}
          </div>
        </Card>

        <Card>
          <CardHeader title="Top Processes" subtitle={`${snapshot?.processCount ?? 0} running`} />
          <div className="max-h-[420px] overflow-y-auto px-2 pb-2">
            <table className="w-full text-[12px]">
              <thead className="sticky top-0 bg-surface text-[11px] text-fg-faint">
                <tr>
                  <th className="px-2 py-1 text-left font-medium">Process</th>
                  <th className="px-2 py-1 text-right font-medium">CPU</th>
                  <th className="px-2 py-1 text-right font-medium">Memory</th>
                  <th className="px-2 py-1 text-right font-medium">PID</th>
                  <th className="px-2 py-1 text-left font-medium">User</th>
                </tr>
              </thead>
              <tbody>
                {processes.map((p) => (
                  <tr key={p.pid} className="border-t border-line/50 hover:bg-surface-2/60">
                    <td className="max-w-[220px] truncate px-2 py-1">{p.name}</td>
                    <td className="px-2 py-1 text-right tnum">{formatPercent(p.cpuPercent, 1)}</td>
                    <td className="px-2 py-1 text-right tnum">{formatBytes(p.memoryBytes)}</td>
                    <td className="px-2 py-1 text-right text-fg-faint tnum">{p.pid}</td>
                    <td className="px-2 py-1 text-fg-faint">{p.user ?? ""}</td>
                  </tr>
                ))}
              </tbody>
            </table>
          </div>
        </Card>
      </div>
    </div>
  );
}

function Row({
  label,
  value,
  pct,
  tone,
}: {
  label: string;
  value: string;
  pct: number;
  tone?: "accent" | "info" | "warn";
}) {
  return (
    <div>
      <div className="flex justify-between text-[12px]">
        <span className="text-fg-muted">{label}</span>
        <span className="tnum">{value}</span>
      </div>
      <Meter value={pct} className="mt-1" tone={tone ?? "auto"} height={5} />
    </div>
  );
}
