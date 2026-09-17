import { useEffect, useMemo } from "react";
import { AlertTriangle, Ban, RefreshCw, Search, Sparkles, X } from "lucide-react";
import { Button } from "@/components/ui/Button";
import { EmptyState } from "@/components/ui/EmptyState";
import { cn } from "@/lib/cn";
import { PermissionBanner } from "@/components/ui/PermissionBanner";
import { formatBytes } from "@/lib/format";
import { filterResults, selectedTargets, sumBytes, useScan } from "@/stores/scan";
import type { Category, ProviderInfo } from "@/types/models";
import { ProviderGroup } from "./ProviderGroup";
import { ScanProgressBar } from "./ScanProgressBar";

/**
 * Shared scan → review → clean workspace. The Cleaner and Developer views are the same
 * component scoped to different categories.
 */
export function CleanerWorkspace({
  scope,
  categories,
  emptyTitle,
  emptyDescription,
}: {
  /** Which section this is, so a filter meant for it is not cleared on arrival. */
  scope: string;
  categories: Category[];
  emptyTitle: string;
  emptyDescription: string;
}) {
  const providers = useScan((s) => s.providers);
  const session = useScan((s) => s.session);
  const cancelled = session?.status === "cancelled";
  const scanning = useScan((s) => s.scanning);
  const selected = useScan((s) => s.selected);
  const startScan = useScan((s) => s.startScan);
  const cancelScan = useScan((s) => s.cancelScan);
  const selectRecommended = useScan((s) => s.selectRecommended);
  const clearSelection = useScan((s) => s.clearSelection);
  const preview = useScan((s) => s.preview);
  const previewing = useScan((s) => s.previewing);
  const deleteMode = useScan((s) => s.deleteMode);
  const filter = useScan((s) => s.filter);
  const setFilter = useScan((s) => s.setFilter);
  const riskFilter = useScan((s) => s.riskFilter);
  const setRiskFilter = useScan((s) => s.setRiskFilter);
  const setTargets = useScan((s) => s.setTargets);

  const scoped: ProviderInfo[] = useMemo(
    () => providers.filter((p) => categories.includes(p.category)),
    [providers, categories],
  );
  const scopedIds = useMemo(() => scoped.map((p) => p.id), [scoped]);
  // The filter is shared state, so carrying one from Cleaner into Developer would greet the
  // user with "Nothing matches". Each section clears a filter that was not meant for it, and
  // leaves alone one that was — which is how following an insight arrives already narrowed.
  useEffect(() => {
    if (useScan.getState().filterScope === scope) return;
    setFilter("", scope);
    setRiskFilter("all");
  }, [scope, setFilter, setRiskFilter]);

  const scopedResults = useMemo(
    () => session?.results.filter((r) => categories.includes(r.category)) ?? [],
    [session, categories],
  );
  const results = useMemo(
    () => filterResults(scopedResults, filter, riskFilter),
    [scopedResults, filter, riskFilter],
  );
  const filtering = filter.trim().length > 0 || riskFilter !== "all";
  const shownTargets = useMemo(() => results.flatMap((r) => r.targets), [results]);
  const selectableShown = useMemo(
    () => shownTargets.filter((t) => t.risk !== "protected"),
    [shownTargets],
  );
  const scopedSelected = useMemo(() => {
    const all = selectedTargets({ session, selected });
    return all.filter((t) => scopedIds.includes(t.providerId));
  }, [session, selected, scopedIds]);
  const selectedBytes = sumBytes(scopedSelected);
  const scopedTotal = results.reduce((a, r) => a + r.totalBytes, 0);
  const hasResults = scopedResults.some((r) => r.targets.length > 0);
  const totalTargets = scopedResults.reduce((a, r) => a + r.targets.length, 0);

  return (
    <div className="flex h-full flex-col">
      <div className="flex items-center gap-2 px-7 pb-3">
        {scanning ? (
          <Button variant="secondary" onClick={() => void cancelScan()}>
            <Ban size={14} /> Cancel
          </Button>
        ) : (
          <Button
            variant={hasResults ? "secondary" : "primary"}
            onClick={() => void startScan(scopedIds)}
          >
            <RefreshCw size={14} /> {hasResults ? "Rescan" : "Scan"}
          </Button>
        )}
        {cancelled && !scanning && (
          <div className="mx-7 mb-3 flex items-center gap-2.5 rounded-lg border border-warn/30 bg-warn-soft/40 px-4 py-2.5 text-[12.5px]">
            <AlertTriangle size={14} className="shrink-0 text-warn" />
            <span>
              You stopped this scan, so these results are partial. The totals below are lower than
              what is really there.
            </span>
            <Button
              size="sm"
              variant="secondary"
              className="ml-auto"
              onClick={() => void startScan(scopedIds)}
            >
              Scan again
            </Button>
          </div>
        )}

        {hasResults && !scanning && (
          <>
            <Button
              variant="ghost"
              size="md"
              onClick={() =>
                filtering
                  ? setTargets(
                      selectableShown.map((t) => t.id),
                      true,
                    )
                  : selectRecommended(categories)
              }
              disabled={filtering && selectableShown.length === 0}
            >
              {filtering ? `Select ${selectableShown.length} shown` : "Select safe items"}
            </Button>
            {scopedSelected.length > 0 && (
              <Button variant="ghost" size="md" onClick={clearSelection}>
                Deselect all
              </Button>
            )}
          </>
        )}
        <div className="flex-1" />
        {hasResults && (
          <div className="text-[12px] text-fg-muted tnum">
            {scopedSelected.length > 0 ? (
              <>
                <span className="text-fg">{scopedSelected.length}</span> selected ·{" "}
                <span className="text-fg">{formatBytes(selectedBytes)}</span>
              </>
            ) : (
              <>
                Found <span className="text-fg">{formatBytes(scopedTotal)}</span>
              </>
            )}
          </div>
        )}
        {hasResults && (
          <Button
            variant={deleteMode === "permanent" ? "danger" : "primary"}
            disabled={scopedSelected.length === 0 || scanning}
            loading={previewing}
            onClick={() => void preview(scopedSelected.map((t) => t.id))}
          >
            Review {selectedBytes > 0 ? formatBytes(selectedBytes) : ""}
          </Button>
        )}
      </div>

      <PermissionBanner />

      {hasResults && !scanning && (
        <div className="flex items-center gap-2 px-7 pb-3">
          <div className="flex h-8 min-w-0 flex-1 items-center gap-2 rounded-md border border-line bg-surface px-2.5">
            <Search size={13} className="shrink-0 text-fg-faint" />
            <input
              value={filter}
              onChange={(e) => setFilter(e.target.value)}
              placeholder="Filter by name, path or kind"
              aria-label="Filter results"
              spellCheck={false}
              className="min-w-0 flex-1 bg-transparent text-[12.5px] outline-none placeholder:text-fg-faint"
            />
            {filter.length > 0 && (
              <button
                type="button"
                onClick={() => setFilter("")}
                aria-label="Clear filter"
                className="rounded p-0.5 text-fg-faint hover:text-fg"
              >
                <X size={13} />
              </button>
            )}
          </div>
          <div className="flex items-center gap-0.5 rounded-md border border-line bg-surface-2 p-0.5">
            {(["all", "safe"] as const).map((value) => (
              <button
                key={value}
                type="button"
                aria-pressed={riskFilter === value}
                onClick={() => setRiskFilter(value)}
                className={cn(
                  "h-6 rounded-[5px] px-2 text-[11.5px] font-medium transition-colors",
                  riskFilter === value
                    ? "bg-surface text-fg shadow-sm"
                    : "text-fg-muted hover:text-fg",
                )}
              >
                {value === "all" ? "All risks" : "Safe only"}
              </button>
            ))}
          </div>
          {filtering && (
            <span className="text-[11.5px] text-fg-faint tnum">
              {shownTargets.length} of {totalTargets}
            </span>
          )}
        </div>
      )}

      {scanning && <ScanProgressBar providerIds={scopedIds} />}

      <div className="min-h-0 flex-1 px-7 pb-8">
        {!hasResults && !scanning ? (
          <EmptyState
            icon={Sparkles}
            title={session ? "Nothing to prune here" : emptyTitle}
            description={session ? "Everything in this section is already tidy." : emptyDescription}
            action={
              !session && (
                <div className="flex flex-col items-center gap-3">
                  <Button variant="primary" onClick={() => void startScan(scopedIds)}>
                    <RefreshCw size={14} /> Scan now
                  </Button>
                  <ProviderChips providers={scoped} />
                </div>
              )
            }
          />
        ) : results.length === 0 ? (
          <EmptyState
            icon={Search}
            title="Nothing matches"
            description={`No items match that filter. ${totalTargets} were found in this section.`}
            action={
              <Button
                variant="secondary"
                onClick={() => {
                  setFilter("");
                  setRiskFilter("all");
                }}
              >
                Clear filter
              </Button>
            }
          />
        ) : (
          <div className="flex flex-col gap-3">
            {results.map((r) => (
              <ProviderGroup key={r.providerId} result={r} />
            ))}
          </div>
        )}
      </div>
    </div>
  );
}

function ProviderChips({ providers }: { providers: ProviderInfo[] }) {
  const available = providers.filter((p) => p.available);
  const unavailable = providers.filter((p) => !p.available);
  return (
    <div className="max-w-[520px] text-[11.5px] text-fg-faint">
      <div className="flex flex-wrap justify-center gap-1">
        {available.map((p) => (
          <span
            key={p.id}
            className="rounded-sm border border-line bg-surface px-1.5 py-0.5 text-fg-muted"
          >
            {p.name}
          </span>
        ))}
        {unavailable.map((p) => (
          <span
            key={p.id}
            className="rounded-sm border border-dashed border-line px-1.5 py-0.5 line-through"
          >
            {p.name}
          </span>
        ))}
      </div>
    </div>
  );
}
