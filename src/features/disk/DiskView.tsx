import { useEffect, useMemo, useState } from "react";
import { Ban, FolderOpen, HardDrive, Home, RefreshCw } from "lucide-react";
import { Button } from "@/components/ui/Button";
import { Checkbox } from "@/components/ui/Checkbox";
import { EmptyState } from "@/components/ui/EmptyState";
import { PageHeader } from "@/components/ui/PageHeader";
import { PermissionBanner } from "@/components/ui/PermissionBanner";
import { RiskBadge } from "@/components/ui/RiskBadge";
import { Spinner } from "@/components/ui/Spinner";
import { cn } from "@/lib/cn";
import { abbreviatePath, formatBytes, formatRelative, plural } from "@/lib/format";
import { backend, isTauri } from "@/lib/tauri";
import { SIZE_FILTERS, useDisk, type DiskTab } from "@/stores/disk";
import { useScan } from "@/stores/scan";
import { startSnapshotPolling, useSystem } from "@/stores/system";
import { useSettings } from "@/stores/settings";
import { DeleteModeToggle } from "../cleaner/DeleteModeToggle";
import { StorageOverview } from "./StorageOverview";
import { FolderExplorer } from "./FolderExplorer";

export function DiskView() {
  const scanning = useDisk((s) => s.scanning);
  const cancelling = useDisk((s) => s.cancelling);
  const summary = useDisk((s) => s.summary);
  const progress = useDisk((s) => s.progress);
  const root = useDisk((s) => s.root);
  const setRoot = useDisk((s) => s.setRoot);
  const startScan = useDisk((s) => s.startScan);
  const cancelScan = useDisk((s) => s.cancelScan);
  const tab = useDisk((s) => s.tab);
  const setTab = useDisk((s) => s.setTab);
  // The tab counts what the tab shows. The analyzer's own count is taken at a different
  // threshold from the size filter, so it said 11 above a list of 9.
  const largeFileCount = useDisk((s) => s.largeFiles.length);
  const info = useSystem((s) => s.info);
  const permissions = useSystem((s) => s.permissions);
  const home = info?.homeDir;
  const disks = useSystem((s) => s.snapshot?.disks);
  const settings = useSettings((s) => s.view);
  const loadSettings = useSettings((s) => s.load);
  useEffect(() => startSnapshotPolling(5000), []);
  useEffect(() => {
    if (!settings) void loadSettings();
  }, [settings, loadSettings]);
  const separator = home?.includes("\\") ? "\\" : "/";
  const scopes = home
    ? [
        { label: "This disk", path: /^[A-Za-z]:/.test(home) ? `${home.slice(0, 2)}\\` : "/" },
        { label: "Home", path: home },
        { label: "Documents", path: `${home}${separator}Documents` },
        { label: "Downloads", path: `${home}${separator}Downloads` },
        ...(settings?.effectiveProjectRoots ?? []).slice(0, 3).map((path) => ({
          label: path.split(/[\\/]/).filter(Boolean).at(-1) || "Projects",
          path,
        })),
        ...(disks ?? [])
          .filter((disk) => disk.isRemovable)
          .map((disk) => ({ label: disk.name || "External drive", path: disk.mountPoint })),
      ].filter((scope, index, all) => all.findIndex((other) => other.path === scope.path) === index)
    : [];

  return (
    <div className="flex h-full flex-col">
      <PageHeader
        title="Storage"
        description="See what takes up space. Explore folders, then review what can go."
        actions={tab === "large" ? <DeleteModeToggle /> : undefined}
      />

      <div className="px-7 pb-5">
        <StorageOverview path={summary?.root || root || home} />
      </div>
      {!isTauri && (
        <p className="px-7 pb-3 text-[11px] text-fg-faint">
          Sample data · Open the desktop app to analyze your computer.
        </p>
      )}
      <PermissionBanner />

      <div className="flex items-center gap-2 px-7 pb-3">
        <div className="field flex h-8 min-w-0 flex-1 items-center gap-2 rounded-md border border-line bg-surface px-2.5">
          <HardDrive size={13} className="shrink-0 text-fg-faint" />
          <input
            value={root}
            disabled={scanning}
            onChange={(e) => setRoot(e.target.value)}
            onKeyDown={(e) => e.key === "Enter" && !scanning && void startScan()}
            placeholder={home ? `${home}  (default: home directory)` : "Directory to analyze"}
            aria-label="Directory to analyze"
            spellCheck={false}
            className="min-w-0 flex-1 bg-transparent font-mono text-[12px] outline-none placeholder:text-fg-faint"
          />
          {home && (
            <button
              type="button"
              title="Use home directory"
              onClick={() => setRoot(home)}
              disabled={scanning}
              className="rounded p-1 text-fg-faint hover:bg-surface-2 hover:text-fg"
            >
              <Home size={13} />
            </button>
          )}
        </div>
        {scanning ? (
          <Button variant="secondary" disabled={cancelling} onClick={() => void cancelScan()}>
            <Ban size={14} /> {cancelling ? "Cancelling…" : "Cancel"}
          </Button>
        ) : (
          <Button variant={summary ? "secondary" : "primary"} onClick={() => void startScan()}>
            <RefreshCw size={14} /> {summary ? "Rescan" : "Analyze"}
          </Button>
        )}
      </div>
      {scopes.length > 0 && (
        <div className="flex flex-wrap items-center gap-1.5 px-7 pb-4">
          <span className="mr-1 text-[11px] text-fg-faint">Analyze</span>
          {scopes.map((scope) => (
            <button
              key={scope.path}
              type="button"
              disabled={scanning}
              onClick={() => {
                setRoot(scope.path);
                void startScan(scope.path);
              }}
              title={scope.path}
              className="max-w-[200px] truncate rounded-full border border-line bg-surface px-3 py-1 text-[11px] text-fg-muted hover:border-accent/40 hover:bg-accent-soft hover:text-accent disabled:opacity-40"
            >
              {scope.label}
            </button>
          ))}
        </div>
      )}

      {scanning && (
        <div className="mx-7 mb-3 rounded-lg border border-line bg-surface px-4 py-3">
          <div className="flex items-center gap-2 text-[12.5px]">
            <Spinner size={13} className="text-accent" />
            <span className="font-medium">
              {cancelling ? "Stopping analysis of " : "Analyzing "}
              {abbreviatePath(root || home || "", home)}…
            </span>
            <span className="ml-auto text-fg-muted tnum">
              {plural(progress?.files ?? 0, "file")} · {plural(progress?.dirs ?? 0, "folder")}·{" "}
              {formatBytes(progress?.bytes ?? 0)}
            </span>
          </div>
          <div className="mt-1.5 truncate font-mono text-[11px] text-fg-faint">
            {progress?.currentPath ?? " "}
          </div>
          {cancelling && (
            <p role="status" className="mt-2 text-[11px] text-fg-muted">
              Cancellation requested. Waiting for file access to return; partial results will be
              shown when the scan stops.
            </p>
          )}
          {!progress && permissions?.fullDiskAccess === "denied" && (
            <p className="mt-2 text-[11px] text-fg-muted">
              If progress stays at zero, check for a macOS folder access request. The OS can wait
              for your decision before file access returns, including after Cancel.
            </p>
          )}
        </div>
      )}

      {!summary && !scanning ? (
        <EmptyState
          icon={HardDrive}
          title="Analyze a directory"
          description="Prune walks the directory once and shows folder sizes, the largest files and which file types take the most space. Nothing is modified."
          action={
            <Button variant="primary" onClick={() => void startScan()}>
              <RefreshCw size={14} /> Analyze {root ? "this directory" : "home directory"}
            </Button>
          }
        />
      ) : summary ? (
        <>
          <div className="flex flex-wrap items-center gap-x-1 gap-y-2 px-7 pb-3">
            <Tab id="tree" current={tab} onSelect={setTab} label="Folders" />
            <Tab
              id="large"
              current={tab}
              onSelect={setTab}
              label="Large Files"
              count={largeFileCount}
            />
            <Tab id="types" current={tab} onSelect={setTab} label="File Types" />
            {/*
              The total is the answer; the counts are how it was arrived at; how long the walk
              took is neither. Strung together with middle dots they all read as equally
              important, so the figure leads and the rest follows it quietly.
            */}
            <div className="ml-auto flex items-baseline gap-2 text-[12px] tnum">
              <span className="text-[13px] font-medium text-fg">
                {formatBytes(summary.totalBytes)}
              </span>
              <span className="text-fg-faint" title={summary.root}>
                measured · {plural(summary.fileCount, "file")}
              </span>
              {summary.status === "cancelled" && (
                <span className="text-warn">stopped early, partial</span>
              )}
              {summary.issueCount > 0 && (
                <span className="text-warn">{summary.issueCount} skipped</span>
              )}
            </div>
          </div>
          {(summary.issueCount > 0 || summary.status === "cancelled") && (
            <div
              role="status"
              className="mx-7 mb-3 rounded-lg border border-warn/25 bg-warn/5 px-3 py-2 text-[12px] text-fg-muted"
            >
              {summary.status === "cancelled" ? "Scan stopped early. " : ""}
              {summary.issueCount > 0
                ? `${plural(summary.issueCount, "path")} could not be read. `
                : ""}
              These are partial results; unreadable folders may contain more data.
            </div>
          )}
          <div className="min-h-0 flex-1 px-7 pb-8">
            {tab === "tree" && <FolderExplorer />}
            {tab === "large" && <LargeFiles />}
            {tab === "types" && <FileTypes />}
          </div>
        </>
      ) : null}
    </div>
  );
}

function Tab({
  id,
  current,
  onSelect,
  label,
  count,
}: {
  id: DiskTab;
  current: DiskTab;
  onSelect: (t: DiskTab) => void;
  label: string;
  count?: number;
}) {
  return (
    <button
      type="button"
      onClick={() => onSelect(id)}
      aria-pressed={current === id}
      className={cn(
        "h-7 rounded-md px-2.5 text-[12.5px] font-medium transition-colors",
        current === id
          ? "bg-surface text-fg shadow-sm ring-1 ring-line"
          : "text-fg-muted hover:text-fg",
      )}
    >
      {label}
      {count !== undefined && (
        <span className="ml-1.5 text-[11px] text-fg-faint tnum">{count}</span>
      )}
    </button>
  );
}

function LargeFiles() {
  const files = useDisk((s) => s.largeFiles);
  const minBytes = useDisk((s) => s.minBytes);
  const setMinBytes = useDisk((s) => s.setMinBytes);
  const selected = useDisk((s) => s.selected);
  const toggle = useDisk((s) => s.toggle);
  const clearSelection = useDisk((s) => s.clearSelection);
  const scanId = useDisk((s) => s.scanId);
  const home = useSystem((s) => s.info?.homeDir);
  const deleteMode = useScan((s) => s.deleteMode);
  const [custom, setCustom] = useState("");
  const previewing = useScan((s) => s.previewing);
  const preview = useScan((s) => s.preview);

  const selectedFiles = useMemo(
    () => files.filter((f) => selected.has(f.targetId)),
    [files, selected],
  );
  const selectedBytes = selectedFiles.reduce((a, f) => a + f.sizeBytes, 0);
  const isPreset = SIZE_FILTERS.some((f) => f.bytes === minBytes);

  const review = async () => {
    if (!scanId || selectedFiles.length === 0) return;
    await preview(
      selectedFiles.map((f) => f.targetId),
      scanId,
    );
  };

  return (
    <div className="flex flex-col gap-3">
      <div className="flex items-center gap-1.5">
        {SIZE_FILTERS.map((f) => (
          <button
            key={f.bytes}
            type="button"
            aria-pressed={minBytes === f.bytes}
            onClick={() => void setMinBytes(f.bytes)}
            className={cn(
              "h-7 rounded-md border px-2.5 text-[12px] font-medium tnum",
              minBytes === f.bytes
                ? "border-accent bg-accent-soft text-accent"
                : "border-line bg-surface text-fg-muted hover:text-fg",
            )}
          >
            {f.label}
          </button>
        ))}
        <div
          className={cn(
            "field flex h-7 items-center gap-1 rounded-md border px-2 text-[12px]",
            !isPreset ? "border-accent bg-accent-soft" : "border-line bg-surface",
          )}
        >
          <span className="text-fg-faint">&gt;</span>
          <input
            value={custom}
            onChange={(e) => setCustom(e.target.value)}
            onKeyDown={(e) => {
              if (e.key === "Enter") {
                const mb = Number.parseFloat(custom);
                if (Number.isFinite(mb) && mb > 0) void setMinBytes(Math.round(mb * 1_000_000));
              }
            }}
            placeholder="custom"
            aria-label="Custom minimum size in megabytes"
            className="w-[56px] bg-transparent text-right outline-none tnum placeholder:text-fg-faint"
          />
          <span className="text-fg-faint">MB</span>
        </div>
        <div className="ml-auto text-[12px] text-fg-muted tnum">
          {selectedFiles.length > 0 ? (
            <>
              <span className="text-fg">{selectedFiles.length}</span> selected ·{" "}
              <span className="text-fg">{formatBytes(selectedBytes)}</span>
              <button
                type="button"
                onClick={clearSelection}
                className="ml-2 text-fg-faint hover:text-fg"
              >
                clear
              </button>
            </>
          ) : (
            plural(files.length, "file")
          )}
        </div>
        <Button
          variant={deleteMode === "permanent" ? "danger" : "primary"}
          size="sm"
          disabled={selectedFiles.length === 0}
          loading={previewing}
          onClick={() => void review()}
        >
          Review {selectedBytes > 0 ? formatBytes(selectedBytes) : ""}
        </Button>
      </div>

      <div className="overflow-hidden rounded-lg border border-line bg-surface">
        {files.length === 0 ? (
          <div className="px-3 py-8 text-center text-[12.5px] text-fg-faint">
            No files above {formatBytes(minBytes)}.
          </div>
        ) : (
          files.map((f) => {
            const checked = selected.has(f.targetId);
            const isProtected = f.risk === "protected";
            return (
              <div
                key={f.targetId}
                onClick={() => !isProtected && toggle(f.targetId)}
                className={cn(
                  "group flex items-center gap-2.5 border-t border-line/60 px-3 py-[7px] first:border-t-0",
                  isProtected ? "opacity-60" : "hover:bg-surface-2/60",
                  checked && "bg-accent-soft/40",
                )}
              >
                <Checkbox
                  checked={checked}
                  disabled={isProtected}
                  onChange={() => toggle(f.targetId)}
                  label={f.name}
                />
                <div className="min-w-0 flex-1">
                  <div className="flex items-center gap-2">
                    <span className="truncate text-[12.5px] font-medium">{f.name}</span>
                    {f.extension && (
                      <span className="shrink-0 rounded-sm bg-surface-2 px-1 text-[10.5px] uppercase text-fg-muted">
                        {f.extension}
                      </span>
                    )}
                  </div>
                  <div className="truncate font-mono text-[11px] text-fg-faint">
                    {abbreviatePath(f.path, home)}
                  </div>
                </div>
                <button
                  type="button"
                  onClick={(e) => {
                    e.stopPropagation();
                    void backend.fsReveal(f.path);
                  }}
                  className="invisible rounded p-1 text-fg-faint hover:bg-surface-2 hover:text-fg group-hover:visible"
                  title="Reveal"
                >
                  <FolderOpen size={13} />
                </button>
                <RiskBadge risk={f.risk} />
                <div className="w-[64px] text-right text-[11px] text-fg-faint tnum">
                  {f.modifiedAt ? formatRelative(f.modifiedAt) : ""}
                </div>
                <div className="w-[76px] text-right text-[12.5px] font-medium tnum">
                  {formatBytes(f.sizeBytes)}
                </div>
              </div>
            );
          })
        )}
      </div>
      <p className="text-[11.5px] text-fg-faint">
        Large files are your own data, so they are rated Medium Risk and never pre-selected. Review
        each one before removing.
      </p>
    </div>
  );
}

function FileTypes() {
  const summary = useDisk((s) => s.summary);
  if (!summary) return null;
  const max = summary.topExtensions[0]?.bytes ?? 1;
  return (
    <div className="overflow-hidden rounded-lg border border-line bg-surface">
      {summary.topExtensions.map((e) => (
        <div
          key={e.extension}
          className="flex items-center gap-3 border-t border-line/60 px-3 py-[7px] first:border-t-0"
        >
          <div className="w-[120px] truncate font-mono text-[12.5px]">
            {e.extension === "(none)" ? (
              <span className="text-fg-faint">no extension</span>
            ) : (
              `.${e.extension}`
            )}
          </div>
          <div className="flex-1">
            <div className="h-[8px] w-full overflow-hidden rounded-sm bg-black/[0.06] dark:bg-white/[0.07]">
              <div
                className="h-full rounded-sm bg-info/70"
                style={{ width: `${Math.max(0.5, (e.bytes / max) * 100)}%` }}
              />
            </div>
          </div>
          <div className="w-[80px] text-right text-[11px] text-fg-faint tnum">
            {plural(e.count, "file")}
          </div>
          <div className="w-[52px] text-right text-[11px] text-fg-faint tnum">
            {summary.totalBytes > 0 ? `${((e.bytes / summary.totalBytes) * 100).toFixed(1)}%` : ""}
          </div>
          <div className="w-[76px] text-right text-[12.5px] font-medium tnum">
            {formatBytes(e.bytes)}
          </div>
        </div>
      ))}
    </div>
  );
}
