import { useEffect, useMemo, useState } from "react";
import { ArrowRight, Cpu, HardDrive, MemoryStick, RefreshCw, Sparkles } from "lucide-react";
import { Button } from "@/components/ui/Button";
import { Card, CardHeader } from "@/components/ui/Card";
import { Meter } from "@/components/ui/Meter";
import { PageHeader } from "@/components/ui/PageHeader";
import { PermissionBanner } from "@/components/ui/PermissionBanner";
import { formatBytes, formatDuration, formatPercent, formatRelative } from "@/lib/format";
import { useScan } from "@/stores/scan";
import { startSnapshotPolling, useSystem } from "@/stores/system";
import { useUi } from "@/stores/ui";
import { CATEGORY_LABEL, type Category } from "@/types/models";
import { backend } from "@/lib/tauri";
import type { OperationRecord } from "@/types/models";

export function DashboardView() {
  const info = useSystem((s) => s.info);
  const snapshot = useSystem((s) => s.snapshot);
  const session = useScan((s) => s.session);
  const scanning = useScan((s) => s.scanning);
  const startScan = useScan((s) => s.startScan);
  const setView = useUi((s) => s.setView);

  useEffect(() => startSnapshotPolling(3000), []);

  const primary = snapshot?.disks.find((d) => d.isPrimary) ?? snapshot?.disks[0];
  const diskUsed = primary ? primary.totalBytes - primary.availableBytes : 0;
  const diskPct = primary ? (diskUsed / primary.totalBytes) * 100 : 0;
  const memPct = snapshot ? (snapshot.memory.usedBytes / snapshot.memory.totalBytes) * 100 : 0;
  const cpuPct = snapshot?.cpu.usagePercent ?? 0;

  const byCategory = useMemo(() => {
    const map = new Map<Category, number>();
    session?.results.forEach((r) => map.set(r.category, (map.get(r.category) ?? 0) + r.totalBytes));
    return [...map.entries()].filter(([, b]) => b > 0).sort((a, b) => b[1] - a[1]);
  }, [session]);

  const headline = !snapshot
    ? "Reading your system…"
    : diskPct >= 90
      ? "Your disk is almost full."
      : session && session.totalBytes > 20e9
        ? "There's a lot to prune."
        : "Your system is healthy.";

  return (
    <div className="pb-8">
      <PageHeader
        title={headline}
        description={
          info
            ? `${info.hostname} · ${info.osName} ${info.osVersion} · ${info.cpuBrand || info.arch}${
                snapshot ? ` · up ${formatDuration(snapshot.uptimeSeconds)}` : ""
              }`
            : undefined
        }
      />

      <PermissionBanner />

      <div className="grid grid-cols-3 gap-3 px-7">
        <MetricCard
          icon={HardDrive}
          label="Storage"
          value={primary ? formatBytes(diskUsed) : "—"}
          suffix={primary ? `/ ${formatBytes(primary.totalBytes)}` : ""}
          pct={diskPct}
          footer={
            primary
              ? `${primary.name || primary.mountPoint} · ${formatBytes(primary.availableBytes)} free`
              : ""
          }
        />
        <MetricCard
          icon={MemoryStick}
          label="Memory"
          value={snapshot ? formatBytes(snapshot.memory.usedBytes) : "—"}
          suffix={snapshot ? `/ ${formatBytes(snapshot.memory.totalBytes)}` : ""}
          pct={memPct}
          footer={
            snapshot
              ? `${formatBytes(snapshot.memory.availableBytes)} available${
                  snapshot.memory.swapUsedBytes > 0
                    ? ` · swap ${formatBytes(snapshot.memory.swapUsedBytes)}`
                    : ""
                }`
              : ""
          }
        />
        <MetricCard
          icon={Cpu}
          label="CPU"
          value={snapshot ? formatPercent(cpuPct) : "—"}
          suffix=""
          pct={cpuPct}
          footer={
            snapshot
              ? `${snapshot.cpu.perCorePercent.length} cores · ${snapshot.processCount} processes`
              : ""
          }
        />
      </div>

      <div className="mt-4 grid grid-cols-[1.6fr_1fr] gap-3 px-7">
        <Card>
          <CardHeader
            title="Potential Cleanup"
            subtitle={
              session
                ? `Last scan ${session.finishedAt ? formatRelative(session.finishedAt) : ""} · ${formatBytes(session.totalBytes)} reclaimable`
                : "Run a scan to see what can be pruned."
            }
            action={
              <Button
                size="sm"
                variant="secondary"
                loading={scanning}
                onClick={() => void startScan()}
              >
                <RefreshCw size={12} /> {session ? "Rescan" : "Scan"}
              </Button>
            }
          />
          <div className="px-4 pb-4">
            {byCategory.length === 0 ? (
              <div className="flex items-center gap-3 rounded-md border border-dashed border-line px-4 py-5 text-[12.5px] text-fg-muted">
                <Sparkles size={16} className="text-fg-faint" />
                {scanning
                  ? "Scanning…"
                  : session
                    ? "Nothing to clean right now."
                    : "No scan yet. Scans are read-only."}
              </div>
            ) : (
              <ul className="divide-y divide-line/60">
                {byCategory.map(([cat, bytes]) => (
                  <li key={cat} className="flex items-center gap-3 py-2">
                    <span className="flex-1 text-[12.5px]">{CATEGORY_LABEL[cat]}</span>
                    <Meter
                      value={(bytes / (byCategory[0]?.[1] ?? 1)) * 100}
                      tone="accent"
                      className="w-[120px]"
                      height={4}
                    />
                    <span className="w-[72px] text-right text-[12.5px] font-medium tnum">
                      {formatBytes(bytes)}
                    </span>
                  </li>
                ))}
              </ul>
            )}
            {session && session.totalBytes > 0 && (
              <div className="mt-3 flex justify-end">
                <Button variant="primary" size="sm" onClick={() => setView("cleaner")}>
                  Review & Clean <ArrowRight size={12} />
                </Button>
              </div>
            )}
          </div>
        </Card>

        <RecentOperations />
      </div>
    </div>
  );
}

function MetricCard({
  icon: Icon,
  label,
  value,
  suffix,
  pct,
  footer,
}: {
  icon: typeof HardDrive;
  label: string;
  value: string;
  suffix: string;
  pct: number;
  footer: string;
}) {
  const [num = value, unit = ""] = value.includes(" ") ? value.split(" ") : [value, ""];
  return (
    <Card className="px-4 py-3.5">
      <div className="flex items-center justify-between text-[12px] text-fg-muted">
        <span className="flex items-center gap-1.5">
          <Icon size={13} /> {label}
        </span>
        <span className="tnum">{formatPercent(pct)}</span>
      </div>
      <div className="mt-2 flex items-baseline gap-1.5">
        <span className="text-[24px] font-semibold tracking-tight tnum">{num}</span>
        {unit && <span className="text-[13px] text-fg-muted">{unit}</span>}
        {suffix && <span className="text-[12px] text-fg-faint tnum">{suffix}</span>}
      </div>
      <Meter value={pct} className="mt-3" />
      <div className="mt-2 truncate text-[11.5px] text-fg-faint">{footer || " "}</div>
    </Card>
  );
}

function RecentOperations() {
  const [ops, setOps] = useState<OperationRecord[]>([]);
  const result = useScan((s) => s.result);
  useEffect(() => {
    void backend
      .opsList(6)
      .then(setOps)
      .catch(() => setOps([]));
  }, [result]);
  return (
    <Card>
      <CardHeader title="Recent Activity" subtitle="Local operation log" />
      <div className="px-4 pb-4">
        {ops.length === 0 ? (
          <p className="py-3 text-[12.5px] text-fg-faint">No cleanups yet.</p>
        ) : (
          <ul className="divide-y divide-line/60">
            {ops.map((op) => (
              <li key={op.id} className="flex items-center gap-2 py-2 text-[12.5px]">
                <div className="min-w-0 flex-1">
                  <div className="truncate">{op.title}</div>
                  <div className="text-[11px] text-fg-faint">
                    {formatRelative(op.at)} · {op.mode === "trash" ? "to Trash" : "permanent"}
                    {op.status !== "success" ? ` · ${op.status}` : ""}
                  </div>
                </div>
                <span className="font-medium tnum">{formatBytes(op.removedBytes)}</span>
              </li>
            ))}
          </ul>
        )}
      </div>
    </Card>
  );
}
