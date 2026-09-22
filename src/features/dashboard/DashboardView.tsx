import { useEffect, useMemo } from "react";
import { Ban, RefreshCw } from "lucide-react";
import { Button } from "@/components/ui/Button";
import { PermissionBanner } from "@/components/ui/PermissionBanner";
import { Spinner } from "@/components/ui/Spinner";
import { cn } from "@/lib/cn";
import { InsightsCard } from "./InsightsCard";
import { formatBytes, formatPercent, formatRelative } from "@/lib/format";
import { useScan } from "@/stores/scan";
import { startSnapshotPolling, useSystem } from "@/stores/system";
import { useUi } from "@/stores/ui";
import { CATEGORY_LABEL, CLEANER_CATEGORIES, type Category } from "@/types/models";

/**
 * The screen Prune opens on.
 *
 * It answers one question — how much can I get back — and offers one thing to do about it.
 * The earlier version answered several at once: three equal panels of live system readings, a
 * list of categories, a history card, each in its own bordered box of the same weight, so
 * nothing was more important than anything else and the only action was a small button in a
 * corner. Everything here is arranged under the figure and the button, and anything that is
 * context rather than the point is quieter than both.
 */
export function DashboardView() {
  const snapshot = useSystem((s) => s.snapshot);
  const session = useScan((s) => s.session);
  const scanning = useScan((s) => s.scanning);
  const startScan = useScan((s) => s.startScan);
  const cancelScan = useScan((s) => s.cancelScan);
  const selectRecommended = useScan((s) => s.selectRecommended);
  const setView = useUi((s) => s.setView);

  useEffect(() => startSnapshotPolling(3000), []);

  const primary = snapshot?.disks.find((d) => d.isPrimary) ?? snapshot?.disks[0];
  const diskPct = primary
    ? ((primary.totalBytes - primary.availableBytes) / primary.totalBytes) * 100
    : 0;
  const memPct = snapshot ? (snapshot.memory.usedBytes / snapshot.memory.totalBytes) * 100 : 0;

  const byCategory = useMemo(() => {
    const map = new Map<Category, number>();
    session?.results.forEach((r) => map.set(r.category, (map.get(r.category) ?? 0) + r.totalBytes));
    return [...map.entries()].filter(([, b]) => b > 0).sort((a, b) => b[1] - a[1]);
  }, [session]);

  const total = session?.totalBytes ?? 0;
  const [amount, unit] = splitBytes(total);

  const review = () => {
    selectRecommended(CLEANER_CATEGORIES);
    setView("cleaner");
  };

  return (
    <div className="mx-auto flex min-h-full w-full max-w-[720px] flex-col px-6 pt-12 pb-10">
      <div className="flex flex-col items-center text-center">
        {session ? (
          <>
            <div className="flex items-baseline gap-2">
              <span className="text-[68px] leading-none font-semibold tracking-[-0.03em] tnum">
                {amount}
              </span>
              <span className="text-[24px] leading-none font-medium text-fg-muted">{unit}</span>
            </div>
            <p className="mt-3 text-[13px] text-fg-muted">
              {total > 0 ? "can be pruned" : "nothing to prune — this machine is tidy"}
            </p>
            {session.finishedAt && (
              <p className="mt-1 text-[11.5px] text-fg-faint">
                scanned {formatRelative(session.finishedAt)}
              </p>
            )}
          </>
        ) : (
          <>
            <div className="text-[68px] leading-none font-semibold tracking-[-0.03em] text-fg-faint/40 tnum">
              —
            </div>
            <p className="mt-3 max-w-[38ch] text-[13px] text-fg-muted">
              Prune looks for caches, logs and build output that programs make again by themselves.
              Nothing is removed until you have read the list.
            </p>
          </>
        )}

        <div className="mt-7 flex items-center gap-2">
          {scanning ? (
            <>
              <span className="flex items-center gap-2 text-[13px] text-fg-muted">
                <Spinner size={14} className="text-accent" /> Looking through your system…
              </span>
              <Button variant="ghost" onClick={() => void cancelScan()}>
                <Ban size={14} /> Stop
              </Button>
            </>
          ) : (
            <>
              <Button size="lg" variant="primary" onClick={() => void startScan()}>
                {session ? <RefreshCw size={15} /> : null}
                {session ? "Scan again" : "Scan this machine"}
              </Button>
              {total > 0 && (
                <Button size="lg" variant="secondary" onClick={review}>
                  Review what can go
                </Button>
              )}
            </>
          )}
        </div>
      </div>

      {byCategory.length > 0 && (
        <ul className="mt-10">
          {byCategory.map(([category, bytes]) => {
            const largest = byCategory[0]?.[1] ?? bytes;
            return (
              <li key={category}>
                <button
                  type="button"
                  onClick={() => setView(category === "developer_files" ? "developer" : "cleaner")}
                  className="group relative flex w-full items-baseline gap-4 overflow-hidden rounded-md border-t border-line/70 px-3 py-2.5 text-left first:border-t-0"
                >
                  <span
                    aria-hidden="true"
                    className="absolute inset-y-0 left-0 bg-accent/[0.07] transition-colors group-hover:bg-accent/[0.13]"
                    style={{ width: `${Math.max(1.5, (bytes / largest) * 100)}%` }}
                  />
                  <span className="relative text-[13.5px]">{CATEGORY_LABEL[category]}</span>
                  <span className="relative flex-1" />
                  <span className="relative w-[84px] shrink-0 text-right text-[13.5px] font-medium tnum">
                    {formatBytes(bytes)}
                  </span>
                </button>
              </li>
            );
          })}
        </ul>
      )}

      {session && <InsightsCard />}

      <div className="flex-1" />

      <div className="mt-8">
        <PermissionBanner />
      </div>

      <dl className="mt-10 flex items-center justify-center gap-7 text-[12px] text-fg-muted">
        <Reading label="Disk" value={primary ? formatPercent(diskPct) : "—"} pct={diskPct} />
        <Reading label="Memory" value={snapshot ? formatPercent(memPct) : "—"} pct={memPct} />
        <Reading
          label="CPU"
          value={snapshot ? formatPercent(snapshot.cpu.usagePercent) : "—"}
          pct={snapshot?.cpu.usagePercent ?? 0}
        />
      </dl>
    </div>
  );
}

/** One live reading. Context for the figure above, not a headline of its own. */
function Reading({ label, value, pct }: { label: string; value: string; pct: number }) {
  return (
    <div className="flex items-center gap-2">
      <dt>{label}</dt>
      <dd className="flex items-center gap-1.5 tnum">
        <span className="text-fg">{value}</span>
        <span className="h-1 w-10 overflow-hidden rounded-full bg-fg-faint/20">
          <span
            className={cn("block h-full rounded-full", pct >= 90 ? "bg-danger" : "bg-fg-faint")}
            style={{ width: `${Math.min(100, pct)}%` }}
          />
        </span>
      </dd>
    </div>
  );
}

/** "44.0 GB" → ["44.0", "GB"], so the unit can sit at its own size beside the figure. */
function splitBytes(bytes: number): [string, string] {
  const text = formatBytes(bytes);
  const at = text.lastIndexOf(" ");
  return at === -1 ? [text, ""] : [text.slice(0, at), text.slice(at + 1)];
}
