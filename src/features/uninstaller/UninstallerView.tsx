import { useEffect, useMemo } from "react";
import {
  AlertTriangle,
  AppWindow,
  FolderOpen,
  PackageMinus,
  RefreshCw,
  Search,
  ShieldAlert,
} from "lucide-react";
import { Button } from "@/components/ui/Button";
import { Checkbox } from "@/components/ui/Checkbox";
import { EmptyState } from "@/components/ui/EmptyState";
import { PageHeader } from "@/components/ui/PageHeader";
import { RiskBadge } from "@/components/ui/RiskBadge";
import { Spinner } from "@/components/ui/Spinner";
import { cn } from "@/lib/cn";
import { abbreviatePath, formatBytes, formatCount } from "@/lib/format";
import { backend, errorMessage } from "@/lib/tauri";
import { useApps } from "@/stores/apps";
import { useScan } from "@/stores/scan";
import { useSystem } from "@/stores/system";
import type { ApplicationInfo } from "@/types/models";
import { DeleteModeToggle } from "../cleaner/DeleteModeToggle";

export function UninstallerView() {
  const apps = useApps((s) => s.apps);
  const loading = useApps((s) => s.loading);
  const measuring = useApps((s) => s.measuring);
  const progress = useApps((s) => s.progress);
  const query = useApps((s) => s.query);
  const setQuery = useApps((s) => s.setQuery);
  const load = useApps((s) => s.load);
  const selectedId = useApps((s) => s.selectedId);
  const select = useApps((s) => s.select);

  useEffect(() => {
    if (apps.length === 0 && !loading) void load();
  }, [apps.length, loading, load]);

  const filtered = useMemo(() => {
    const q = query.trim().toLowerCase();
    const list = q
      ? apps.filter(
          (a) => a.name.toLowerCase().includes(q) || a.bundleId?.toLowerCase().includes(q),
        )
      : apps;
    return [...list].sort(
      (a, b) => (b.sizeBytes ?? -1) - (a.sizeBytes ?? -1) || a.name.localeCompare(b.name),
    );
  }, [apps, query]);

  const totalBytes = apps.reduce((a, x) => a + (x.sizeBytes ?? 0), 0);

  return (
    <div className="flex h-full flex-col">
      <PageHeader
        title="Uninstaller"
        description="Remove an application together with the caches, preferences and containers it leaves behind."
        actions={<DeleteModeToggle />}
      />
      <div className="flex min-h-0 flex-1 gap-3 px-7 pb-8">
        <div className="flex w-[300px] shrink-0 flex-col overflow-hidden rounded-lg border border-line bg-surface">
          <div className="flex items-center gap-2 border-b border-line px-3 py-2">
            <Search size={13} className="text-fg-faint" />
            <input
              value={query}
              onChange={(e) => setQuery(e.target.value)}
              placeholder="Search applications"
              className="min-w-0 flex-1 bg-transparent text-[12.5px] outline-none placeholder:text-fg-faint"
            />
            <button
              type="button"
              onClick={() => void load()}
              disabled={loading || measuring}
              className="rounded p-1 text-fg-faint hover:bg-surface-2 hover:text-fg disabled:opacity-40"
              title="Refresh"
            >
              {loading || measuring ? <Spinner size={13} /> : <RefreshCw size={13} />}
            </button>
          </div>
          <div className="flex items-center justify-between px-3 py-1.5 text-[11px] text-fg-faint tnum">
            <span>{formatCount(apps.length)} applications</span>
            <span>
              {measuring && progress
                ? `measuring ${progress.done}/${progress.total}`
                : formatBytes(totalBytes)}
            </span>
          </div>
          <div className="min-h-0 flex-1 overflow-y-auto">
            {filtered.map((a) => (
              <AppRow
                key={a.id}
                app={a}
                active={a.id === selectedId}
                onSelect={() => void select(a.id)}
              />
            ))}
            {filtered.length === 0 && !loading && (
              <div className="px-3 py-8 text-center text-[12.5px] text-fg-faint">
                No applications match.
              </div>
            )}
          </div>
        </div>
        <div className="min-w-0 flex-1">
          <AppDetailPane />
        </div>
      </div>
    </div>
  );
}

function AppRow({
  app,
  active,
  onSelect,
}: {
  app: ApplicationInfo;
  active: boolean;
  onSelect: () => void;
}) {
  return (
    <button
      type="button"
      onClick={onSelect}
      className={cn(
        "flex w-full items-center gap-2.5 border-t border-line/60 px-3 py-2 text-left first:border-t-0",
        active ? "bg-accent-soft/50" : "hover:bg-surface-2/60",
      )}
    >
      <div className="flex h-7 w-7 shrink-0 items-center justify-center rounded-md border border-line bg-surface-2 text-[12px] font-semibold text-fg-muted">
        {app.name.slice(0, 1).toUpperCase()}
      </div>
      <div className="min-w-0 flex-1">
        <div className="flex items-center gap-1.5">
          <span className="truncate text-[12.5px] font-medium">{app.name}</span>
          {app.isSystem && <ShieldAlert size={11} className="shrink-0 text-fg-faint" />}
        </div>
        <div className="truncate text-[11px] text-fg-faint">
          {app.version ?? app.bundleId ?? app.source}
        </div>
      </div>
      <div className="text-[12px] text-fg-muted tnum">
        {app.sizeBytes !== undefined ? formatBytes(app.sizeBytes) : "…"}
      </div>
    </button>
  );
}

function AppDetailPane() {
  const detail = useApps((s) => s.detail);
  const detailLoading = useApps((s) => s.detailLoading);
  const selectedId = useApps((s) => s.selectedId);
  const selectedItems = useApps((s) => s.selectedItems);
  const toggleItem = useApps((s) => s.toggleItem);
  const setItems = useApps((s) => s.setItems);
  const home = useSystem((s) => s.info?.homeDir);
  const platform = useSystem((s) => s.meta?.platform);
  const deleteMode = useScan((s) => s.deleteMode);
  const previewing = useScan((s) => s.previewing);

  if (!selectedId) {
    return (
      <div className="flex h-full items-center justify-center rounded-lg border border-dashed border-line">
        <EmptyState
          icon={PackageMinus}
          title="Select an application"
          description="Prune shows the bundle and every related folder it can find, with a risk level for each. Nothing is removed until you review and confirm."
        />
      </div>
    );
  }
  if (detailLoading || !detail) {
    return (
      <div className="flex h-full items-center justify-center gap-2 rounded-lg border border-line bg-surface text-[12.5px] text-fg-muted">
        <Spinner size={14} className="text-accent" /> Measuring related data…
      </div>
    );
  }

  const removable = detail.items.filter((i) => i.target.risk !== "protected");
  const chosen = detail.items.filter((i) => selectedItems.has(i.target.id));
  const chosenBytes = chosen.reduce((a, i) => a + i.target.sizeBytes, 0);
  const includesBundle = chosen.some((i) => i.kind === "application");
  const allOn = removable.length > 0 && removable.every((i) => selectedItems.has(i.target.id));

  const review = async () => {
    if (chosen.length === 0) return;
    useScan.setState({ previewing: true });
    try {
      const plan = await backend.cleanerPreview(
        `app:${detail.app.id}`,
        chosen.map((i) => i.target.id),
        deleteMode,
      );
      useScan.setState({ plan, previewing: false, cleanupProgress: null });
    } catch (e) {
      useScan.setState({ previewing: false });
      useApps.setState({ error: errorMessage(e) });
    }
  };

  return (
    <div className="flex h-full flex-col overflow-hidden rounded-lg border border-line bg-surface">
      <div className="flex items-start gap-3 border-b border-line px-4 py-3">
        <div className="flex h-11 w-11 shrink-0 items-center justify-center rounded-lg border border-line bg-surface-2 text-[18px] font-semibold text-fg-muted">
          {detail.app.name.slice(0, 1).toUpperCase()}
        </div>
        <div className="min-w-0 flex-1">
          <div className="flex items-center gap-2">
            <h2 className="truncate text-[15px] font-semibold tracking-tight">{detail.app.name}</h2>
            {detail.app.version && (
              <span className="text-[12px] text-fg-muted tnum">{detail.app.version}</span>
            )}
            {detail.app.isSystem && (
              <span className="rounded-sm bg-surface-2 px-1.5 text-[10.5px] text-fg-faint">
                system
              </span>
            )}
          </div>
          <div className="truncate font-mono text-[11px] text-fg-faint">
            {detail.app.bundleId ?? abbreviatePath(detail.app.path, home)}
          </div>
          <div className="mt-1 text-[12px] text-fg-muted tnum">
            <span className="text-fg">{formatBytes(detail.totalBytes)}</span> total ·{" "}
            {formatBytes(detail.leftoverBytes)} outside the app
          </div>
        </div>
        <button
          type="button"
          onClick={() => void backend.fsReveal(detail.app.path)}
          className="rounded p-1.5 text-fg-faint hover:bg-surface-2 hover:text-fg"
          title="Reveal"
          disabled={!detail.app.path}
        >
          <FolderOpen size={14} />
        </button>
      </div>

      {(detail.sharedWith?.length ?? 0) > 0 && (
        <div className="flex items-start gap-2.5 border-b border-line bg-warn-soft/40 px-4 py-2.5 text-[12.5px]">
          <AlertTriangle size={14} className="mt-0.5 shrink-0 text-warn" />
          <div className="min-w-0">
            Another copy of this application is installed, and both read the same data:
            <ul className="mt-1 space-y-0.5 font-mono text-[11.5px] text-fg-muted">
              {detail.sharedWith?.map((path) => (
                <li key={path} className="truncate">
                  {abbreviatePath(path, home)}
                </li>
              ))}
            </ul>
            <p className="mt-1 text-fg-muted">
              So the data below is left alone: removing it here would take that copy's settings
              with it. Remove the other copy first, then scan again.
            </p>
          </div>
        </div>
      )}

      <div className="flex items-center gap-2 border-b border-line px-4 py-2 text-[12px]">
        <Checkbox
          checked={allOn}
          indeterminate={chosen.length > 0 && !allOn}
          disabled={removable.length === 0}
          onChange={(next) =>
            setItems(
              removable.map((i) => i.target.id),
              next,
            )
          }
          label="Select all"
        />
        <span className="text-fg-muted">
          {chosen.length} of {detail.items.length} items · {formatBytes(chosenBytes)}
        </span>
        <div className="flex-1" />
        {platform === "windows" && detail.app.uninstallCommand && (
          <Button
            size="sm"
            variant="secondary"
            onClick={() =>
              void backend
                .appsRunUninstaller(detail.app.id)
                .catch((e) => useApps.setState({ error: errorMessage(e) }))
            }
          >
            <AppWindow size={12} /> Run vendor uninstaller
          </Button>
        )}
        <Button
          size="sm"
          variant={deleteMode === "permanent" ? "danger" : "primary"}
          disabled={chosen.length === 0}
          loading={previewing}
          onClick={() => void review()}
        >
          <PackageMinus size={12} /> {includesBundle ? "Uninstall" : "Remove leftovers"}{" "}
          {chosenBytes > 0 ? formatBytes(chosenBytes) : ""}
        </Button>
      </div>

      <div className="min-h-0 flex-1 overflow-y-auto">
        {detail.items.map((item) => {
          const t = item.target;
          const checked = selectedItems.has(t.id);
          const isProtected = t.risk === "protected";
          return (
            <div
              key={t.id}
              onClick={() => !isProtected && toggleItem(t.id)}
              className={cn(
                "flex items-center gap-2.5 border-t border-line/60 px-4 py-[7px] first:border-t-0",
                isProtected ? "opacity-60" : "hover:bg-surface-2/60",
                checked && "bg-accent-soft/40",
              )}
            >
              <Checkbox
                checked={checked}
                disabled={isProtected}
                onChange={() => toggleItem(t.id)}
                label={t.label}
              />
              <div className="min-w-0 flex-1">
                <div className="flex items-baseline gap-1.5">
                  <span className="truncate text-[12.5px] font-medium">{item.kindLabel}</span>
                  <span className="shrink-0 text-[11px] text-fg-faint tnum">
                    {t.kind === "directory" ? `${formatCount(t.fileCount)} files` : "file"}
                  </span>
                </div>
                <div className="truncate font-mono text-[11px] text-fg-faint">
                  {abbreviatePath(t.path, home)}
                </div>
              </div>
              <RiskBadge risk={t.risk} />
              <div className="w-[72px] shrink-0 text-right text-[12.5px] font-medium tnum">
                {formatBytes(t.sizeBytes)}
              </div>
            </div>
          );
        })}
      </div>
      {detail.app.isSystem && (
        <div className="border-t border-line bg-surface-2/60 px-4 py-2 text-[11.5px] text-fg-muted">
          This is a system application. Its bundle is protected; only its caches and leftover data
          can be removed.
        </div>
      )}
    </div>
  );
}
