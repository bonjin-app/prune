import { useEffect, useState } from "react";
import { Card, CardHeader } from "@/components/ui/Card";
import { PageHeader } from "@/components/ui/PageHeader";
import { cn } from "@/lib/cn";
import { formatBytes, formatCount, formatDateTime } from "@/lib/format";
import { backend, isTauri } from "@/lib/tauri";
import { useScan } from "@/stores/scan";
import { useSystem } from "@/stores/system";
import { useUi, type ThemePreference } from "@/stores/ui";
import type { DeleteMode, OperationRecord } from "@/types/models";

export function SettingsView() {
  const theme = useUi((s) => s.theme);
  const setTheme = useUi((s) => s.setTheme);
  const mode = useScan((s) => s.deleteMode);
  const setMode = useScan((s) => s.setDeleteMode);
  const meta = useSystem((s) => s.meta);
  const info = useSystem((s) => s.info);
  const permissions = useSystem((s) => s.permissions);
  const [ops, setOps] = useState<OperationRecord[]>([]);

  useEffect(() => {
    void backend
      .opsList(100)
      .then(setOps)
      .catch(() => setOps([]));
  }, []);

  return (
    <div className="pb-8">
      <PageHeader title="Settings" />
      <div className="flex flex-col gap-3 px-7">
        <Card>
          <CardHeader title="Appearance" />
          <div className="px-4 pb-4">
            <Segmented<ThemePreference>
              value={theme}
              onChange={setTheme}
              options={[
                ["system", "System"],
                ["light", "Light"],
                ["dark", "Dark"],
              ]}
            />
          </div>
        </Card>

        <Card>
          <CardHeader
            title="Removal"
            subtitle="How selected items are removed. Items already in the Trash are always deleted permanently."
          />
          <div className="px-4 pb-4">
            <Segmented<DeleteMode>
              value={mode}
              onChange={setMode}
              options={[
                ["trash", "Move to Trash (recoverable)"],
                ["permanent", "Delete permanently"],
              ]}
              danger="permanent"
            />
          </div>
        </Card>

        <Card>
          <CardHeader title="Operation Log" subtitle={meta?.operationLogPath ?? ""} />
          <div className="px-4 pb-4">
            {ops.length === 0 ? (
              <p className="text-[12.5px] text-fg-faint">No operations recorded yet.</p>
            ) : (
              <table className="w-full text-[12px]">
                <thead className="text-[11px] text-fg-faint">
                  <tr>
                    <th className="py-1 text-left font-medium">When</th>
                    <th className="py-1 text-left font-medium">Operation</th>
                    <th className="py-1 text-left font-medium">Mode</th>
                    <th className="py-1 text-right font-medium">Files</th>
                    <th className="py-1 text-right font-medium">Size</th>
                    <th className="py-1 text-right font-medium">Status</th>
                  </tr>
                </thead>
                <tbody>
                  {ops.map((op) => (
                    <tr key={op.id} className="border-t border-line/50">
                      <td className="py-1.5 text-fg-muted tnum">{formatDateTime(op.at)}</td>
                      <td className="py-1.5">{op.title}</td>
                      <td className="py-1.5 text-fg-muted">
                        {op.mode === "trash" ? "Trash" : "Permanent"}
                      </td>
                      <td className="py-1.5 text-right tnum">{formatCount(op.removedFiles)}</td>
                      <td className="py-1.5 text-right font-medium tnum">
                        {formatBytes(op.removedBytes)}
                      </td>
                      <td
                        className={cn(
                          "py-1.5 text-right capitalize",
                          op.status !== "success" && "text-warn",
                        )}
                      >
                        {op.status}
                      </td>
                    </tr>
                  ))}
                </tbody>
              </table>
            )}
          </div>
        </Card>

        <Card>
          <CardHeader title="About" />
          <dl className="grid grid-cols-[140px_1fr] gap-y-1 px-4 pb-4 text-[12.5px]">
            <dt className="text-fg-muted">Version</dt>
            <dd className="tnum">
              {meta?.version ?? "—"}{" "}
              <span className="text-fg-faint">(core {meta?.coreVersion ?? "—"})</span>
              {!isTauri && (
                <span className="ml-2 rounded-sm bg-warn-soft px-1 text-[10.5px] text-warn">
                  browser mock
                </span>
              )}
            </dd>
            <dt className="text-fg-muted">Platform</dt>
            <dd>{info ? `${info.osName} ${info.osVersion} · ${info.arch}` : "—"}</dd>
            <dt className="text-fg-muted">Home</dt>
            <dd className="font-mono text-[11.5px]">{info?.homeDir ?? "—"}</dd>
            <dt className="text-fg-muted">Full Disk Access</dt>
            <dd>
              {permissions?.fullDiskAccess === "granted"
                ? "Granted"
                : permissions?.fullDiskAccess === "denied"
                  ? `Not granted — ${permissions.blocked.join(", ")} are hidden from scans`
                  : "Not required on this platform"}
            </dd>
            <dt className="text-fg-muted">Privacy</dt>
            <dd>100% local. No account, no telemetry, no network access.</dd>
            <dt className="text-fg-muted">Source</dt>
            <dd className="text-fg-muted">github.com/bonjin-app/prune · MIT License</dd>
          </dl>
        </Card>
      </div>
    </div>
  );
}

function Segmented<T extends string>({
  value,
  onChange,
  options,
  danger,
}: {
  value: T;
  onChange: (v: T) => void;
  options: [T, string][];
  danger?: T;
}) {
  return (
    <div className="inline-flex items-center gap-0.5 rounded-md border border-line bg-surface-2 p-0.5">
      {options.map(([v, label]) => (
        <button
          key={v}
          type="button"
          aria-pressed={value === v}
          onClick={() => onChange(v)}
          className={cn(
            "h-7 rounded-[5px] px-3 text-[12px] font-medium transition-colors",
            value === v
              ? v === danger
                ? "bg-danger text-white"
                : "bg-surface text-fg shadow-sm"
              : "text-fg-muted hover:text-fg",
          )}
        >
          {label}
        </button>
      ))}
    </div>
  );
}
