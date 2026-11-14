import { useEffect, useMemo } from "react";
import { Ban, RefreshCw } from "lucide-react";
import { Button } from "@/components/ui/Button";
import { PermissionBanner } from "@/components/ui/PermissionBanner";
import { Spinner } from "@/components/ui/Spinner";
import { InsightsCard } from "./InsightsCard";
import { SystemStrip } from "./SystemStrip";
import { StorageOverview } from "@/features/disk/StorageOverview";
import { useDisk } from "@/stores/disk";
import { formatBytes, formatRelative, splitBytes } from "@/lib/format";
import { reclaimableBytes, sessionReclaimableBytes } from "@/lib/reclaimable";
import { useScan } from "@/stores/scan";
import { startSnapshotPolling, useSystem } from "@/stores/system";
import { useUi } from "@/stores/ui";
import {
  CATEGORY_LABEL,
  CLEANER_CATEGORIES,
  DEVELOPER_CATEGORIES,
  type Category,
} from "@/types/models";

/** Category rows shown before the rest are folded into "and N more". */
const MAX_ROWS = 6;

/** What a scan looks through, shown before there is a scan so the space is not empty. */
const LOOKS_THROUGH: Category[] = [
  "application_cache",
  "developer_files",
  "ai_data",
  "browser_cache",
  "logs",
  "temporary_files",
  "trash",
  "old_installers",
];

/**
 * The screen Prune opens on.
 *
 * It answers one question — how much can I get back — and offers one thing to do about it, on
 * one screen with nothing below the fold. The disk is a line of context above; the figure and
 * where it lives sit side by side in one panel; the few observations worth acting on take one
 * row under it; live readings are a footnote. Anything that is context rather than the point is
 * quieter than the figure and the button.
 */
export function DashboardView() {
  const session = useScan((s) => s.session);
  const scanning = useScan((s) => s.scanning);
  const startScan = useScan((s) => s.startScan);
  const cancelScan = useScan((s) => s.cancelScan);
  const selectRecommended = useScan((s) => s.selectRecommended);
  const setView = useUi((s) => s.setView);
  const home = useSystem((s) => s.info?.homeDir);

  useEffect(() => startSnapshotPolling(3000), []);

  const byCategory = useMemo(() => {
    const map = new Map<Category, number>();
    session?.results.forEach((r) =>
      map.set(r.category, (map.get(r.category) ?? 0) + reclaimableBytes(r)),
    );
    return [...map.entries()].filter(([, b]) => b > 0).sort((a, b) => b[1] - a[1]);
  }, [session]);

  // What can be pruned, not everything found: protected items are listed elsewhere, never offered.
  const total = session ? sessionReclaimableBytes(session) : 0;
  const protectedTotal = useMemo(
    () =>
      session?.results
        .flatMap((r) => r.targets)
        .reduce((a, t) => (t.risk === "protected" ? a + t.sizeBytes : a), 0) ?? 0,
    [session],
  );
  const [amount, unit] = splitBytes(total);
  const rows = byCategory.slice(0, MAX_ROWS);
  const folded = byCategory.length - rows.length;
  const largest = rows[0]?.[1] ?? 0;

  const review = () => {
    selectRecommended(CLEANER_CATEGORIES);
    setView("cleaner");
  };
  const open = (category: Category) =>
    setView(DEVELOPER_CATEGORIES.includes(category) ? "developer" : "cleaner");

  return (
    <div className="mx-auto flex w-full max-w-[960px] flex-col gap-5 px-7 pt-4 pb-5">
      <StorageOverview
        compact
        path={home}
        onExplore={() => {
          if (home) useDisk.getState().setRoot(home);
          setView("disk");
        }}
      />

      <section className="grid overflow-hidden rounded-xl border border-line/70 bg-surface md:grid-cols-2 lg:grid-cols-[minmax(0,5fr)_minmax(0,6fr)]">
        <div className="relative flex flex-col justify-center px-6 py-6 [@media(min-height:700px)]:py-7">
          {/* Out of the way of the one thing to do, and never wrapped under it. */}
          {session && !scanning && total > 0 && (
            <Button
              size="sm"
              variant="ghost"
              className="absolute top-3 right-3 whitespace-nowrap text-fg-muted"
              onClick={() => void startScan()}
            >
              <RefreshCw size={12} /> Scan again
            </Button>
          )}
          {session ? (
            <>
              <div className="flex items-baseline gap-2">
                <span className="text-[60px] leading-none font-semibold tracking-[-0.03em] tnum">
                  {amount}
                </span>
                <span className="text-[22px] leading-none font-medium text-fg-muted">{unit}</span>
              </div>
              <p className="mt-2.5 text-[13px] text-fg-muted">
                {total > 0 ? "can be pruned" : "nothing to prune — this machine is tidy"}
              </p>
              <p className="mt-1 text-[11.5px] text-fg-faint">
                {session.finishedAt && `scanned ${formatRelative(session.finishedAt)}`}
                {protectedTotal > 0 && (
                  <>
                    {session.finishedAt && "; "}
                    {formatBytes(protectedTotal)} more is protected and only shown
                  </>
                )}
              </p>
            </>
          ) : (
            <>
              <div className="text-[60px] leading-none font-semibold tracking-[-0.03em] text-fg-faint/40 tnum">
                —
              </div>
              <p className="mt-2.5 max-w-[34ch] text-[13px] text-fg-muted">
                Prune looks for caches, logs and build output that programs make again by
                themselves. Nothing is removed until you have read the list.
              </p>
            </>
          )}

          <div className="mt-5 flex flex-wrap items-center gap-1.5">
            {scanning ? (
              <>
                <span className="flex items-center gap-2 text-[13px] text-fg-muted">
                  <Spinner size={14} className="text-accent" /> Looking through your system…
                </span>
                <Button variant="ghost" onClick={() => void cancelScan()}>
                  <Ban size={14} /> Stop
                </Button>
              </>
            ) : total > 0 ? (
              <Button size="lg" variant="primary" onClick={review}>
                Review what can go
              </Button>
            ) : (
              <Button size="lg" variant="primary" onClick={() => void startScan()}>
                {session ? <RefreshCw size={15} /> : null}
                {session ? "Scan again" : "Scan this machine"}
              </Button>
            )}
          </div>
        </div>

        <div className="border-t border-line/70 px-3 py-3 md:border-t-0 md:border-l">
          {rows.length > 0 ? (
            <ul aria-label="By category">
              {rows.map(([category, bytes]) => (
                <li key={category}>
                  <button
                    type="button"
                    onClick={() => open(category)}
                    className="group relative flex w-full items-baseline gap-4 overflow-hidden rounded-md px-3 py-[9px] text-left"
                  >
                    <span
                      aria-hidden="true"
                      className="absolute inset-y-[3px] left-0 rounded-md bg-accent/[0.08] transition-colors group-hover:bg-accent/[0.15]"
                      style={{ width: `${Math.max(1.5, (bytes / largest) * 100)}%` }}
                    />
                    <span className="relative truncate text-[13px]">
                      {CATEGORY_LABEL[category]}
                    </span>
                    <span className="relative flex-1" />
                    <span className="relative w-[76px] shrink-0 text-right text-[13px] font-medium tnum">
                      {formatBytes(bytes)}
                    </span>
                  </button>
                </li>
              ))}
              {folded > 0 && (
                <li>
                  <button
                    type="button"
                    onClick={() => setView("cleaner")}
                    className="w-full px-3 pt-1.5 text-left text-[12px] text-fg-muted hover:text-fg"
                  >
                    and {folded} more {folded === 1 ? "category" : "categories"}
                  </button>
                </li>
              )}
            </ul>
          ) : (
            <div className="flex h-full flex-col justify-center px-4 py-3">
              <p className="text-[12px] text-fg-muted">
                {session ? "Looked through" : "A scan looks through"}
              </p>
              <ul className="mt-2.5 grid grid-cols-2 gap-x-4 gap-y-1.5 text-[12.5px] text-fg-faint">
                {LOOKS_THROUGH.map((category) => (
                  <li key={category}>{CATEGORY_LABEL[category]}</li>
                ))}
              </ul>
            </div>
          )}
        </div>
      </section>

      {session && <InsightsCard />}

      <PermissionBanner compact />

      <SystemStrip />
    </div>
  );
}
