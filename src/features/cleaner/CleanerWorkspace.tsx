import { useMemo } from "react";
import { Ban, RefreshCw, Sparkles } from "lucide-react";
import { Button } from "@/components/ui/Button";
import { EmptyState } from "@/components/ui/EmptyState";
import { PermissionBanner } from "@/components/ui/PermissionBanner";
import { formatBytes } from "@/lib/format";
import { selectedTargets, sumBytes, useScan } from "@/stores/scan";
import type { Category, ProviderInfo } from "@/types/models";
import { ProviderGroup } from "./ProviderGroup";
import { ScanProgressBar } from "./ScanProgressBar";

/**
 * Shared scan → review → clean workspace. The Cleaner and Developer views are the same
 * component scoped to different categories.
 */
export function CleanerWorkspace({
  categories,
  emptyTitle,
  emptyDescription,
}: {
  categories: Category[];
  emptyTitle: string;
  emptyDescription: string;
}) {
  const providers = useScan((s) => s.providers);
  const session = useScan((s) => s.session);
  const scanning = useScan((s) => s.scanning);
  const selected = useScan((s) => s.selected);
  const startScan = useScan((s) => s.startScan);
  const cancelScan = useScan((s) => s.cancelScan);
  const selectRecommended = useScan((s) => s.selectRecommended);
  const clearSelection = useScan((s) => s.clearSelection);
  const preview = useScan((s) => s.preview);
  const previewing = useScan((s) => s.previewing);
  const deleteMode = useScan((s) => s.deleteMode);

  const scoped: ProviderInfo[] = useMemo(
    () => providers.filter((p) => categories.includes(p.category)),
    [providers, categories],
  );
  const scopedIds = useMemo(() => scoped.map((p) => p.id), [scoped]);
  const results = useMemo(
    () => session?.results.filter((r) => categories.includes(r.category)) ?? [],
    [session, categories],
  );
  const scopedSelected = useMemo(() => {
    const all = selectedTargets({ session, selected });
    return all.filter((t) => scopedIds.includes(t.providerId));
  }, [session, selected, scopedIds]);
  const selectedBytes = sumBytes(scopedSelected);
  const scopedTotal = results.reduce((a, r) => a + r.totalBytes, 0);
  const hasResults = results.some((r) => r.targets.length > 0);

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
        {hasResults && !scanning && (
          <>
            <Button variant="ghost" size="md" onClick={() => selectRecommended(categories)}>
              Select safe items
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
            onClick={() => void preview()}
          >
            Review {selectedBytes > 0 ? formatBytes(selectedBytes) : ""}
          </Button>
        )}
      </div>

      <PermissionBanner />

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
        ) : (
          <div className="flex flex-col gap-3">
            {results
              .filter((r) => r.targets.length > 0 || r.issues.length > 0)
              .map((r) => (
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
