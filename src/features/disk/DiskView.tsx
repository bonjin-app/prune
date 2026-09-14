import { useMemo, useState } from "react";
import { Ban, ChevronRight, FileBox, FolderOpen, HardDrive, Home, RefreshCw } from "lucide-react";
import { Button } from "@/components/ui/Button";
import { Checkbox } from "@/components/ui/Checkbox";
import { EmptyState } from "@/components/ui/EmptyState";
import { PageHeader } from "@/components/ui/PageHeader";
import { RiskBadge } from "@/components/ui/RiskBadge";
import { Spinner } from "@/components/ui/Spinner";
import { cn } from "@/lib/cn";
import { abbreviatePath, formatBytes, formatCount, formatRelative } from "@/lib/format";
import { backend } from "@/lib/tauri";
import { SIZE_FILTERS, useDisk, type DiskTab } from "@/stores/disk";
import { useScan } from "@/stores/scan";
import { useSystem } from "@/stores/system";
import { DeleteModeToggle } from "../cleaner/DeleteModeToggle";

export function DiskView() {
  const scanning = useDisk((s) => s.scanning);
  const summary = useDisk((s) => s.summary);
  const progress = useDisk((s) => s.progress);
  const root = useDisk((s) => s.root);
  const setRoot = useDisk((s) => s.setRoot);
  const startScan = useDisk((s) => s.startScan);
  const cancelScan = useDisk((s) => s.cancelScan);
  const tab = useDisk((s) => s.tab);
  const setTab = useDisk((s) => s.setTab);
  const info = useSystem((s) => s.info);
  const home = info?.homeDir;

  return (
    <div className="flex h-full flex-col">
      <PageHeader
        title="Disk"
        description="Where your space goes. Scans are read-only; large files can be reviewed and removed."
        actions={tab === "large" ? <DeleteModeToggle /> : undefined}
      />

      <div className="flex items-center gap-2 px-7 pb-3">
        <div className="flex h-8 min-w-0 flex-1 items-center gap-2 rounded-md border border-line bg-surface px-2.5">
          <HardDrive size={13} className="shrink-0 text-fg-faint" />
          <input
            value={root}
            onChange={(e) => setRoot(e.target.value)}
            onKeyDown={(e) => e.key === "Enter" && !scanning && void startScan()}
            placeholder={home ? `${home}  (default: home directory)` : "Directory to analyze"}
            spellCheck={false}
            className="min-w-0 flex-1 bg-transparent font-mono text-[12px] outline-none placeholder:text-fg-faint"
          />
          {home && (
            <button
              type="button"
              title="Use home directory"
              onClick={() => setRoot(home)}
              className="rounded p-1 text-fg-faint hover:bg-surface-2 hover:text-fg"
            >
              <Home size={13} />
            </button>
          )}
        </div>
        {scanning ? (
          <Button variant="secondary" onClick={() => void cancelScan()}>
            <Ban size={14} /> Cancel
          </Button>
        ) : (
          <Button variant={summary ? "secondary" : "primary"} onClick={() => void startScan()}>
            <RefreshCw size={14} /> {summary ? "Rescan" : "Analyze"}
          </Button>
        )}
      </div>

      {scanning && (
        <div className="mx-7 mb-3 rounded-lg border border-line bg-surface px-4 py-3">
          <div className="flex items-center gap-2 text-[12.5px]">
            <Spinner size={13} className="text-accent" />
            <span className="font-medium">
              Analyzing {abbreviatePath(root || home || "", home)}…
            </span>
            <span className="ml-auto text-fg-muted tnum">
              {formatCount(progress?.files ?? 0)} files · {formatCount(progress?.dirs ?? 0)} folders
              · {formatBytes(progress?.bytes ?? 0)}
            </span>
          </div>
          <div className="mt-1.5 truncate font-mono text-[11px] text-fg-faint">
            {progress?.currentPath ?? " "}
          </div>
        </div>
      )}

      {!summary && !scanning ? (
        <EmptyState
          icon={HardDrive}
          title="Analyze a directory"
          description="Prune walks the directory once and shows folder sizes, the largest files and which file types take the most space. Nothing is modified."
          action={
            <Button variant="primary" onClick={() => void startScan()}>
              <RefreshCw size={14} /> Analyze home directory
            </Button>
          }
        />
      ) : summary ? (
        <>
          <div className="flex items-center gap-1 px-7 pb-3">
            <Tab id="tree" current={tab} onSelect={setTab} label="Folders" />
            <Tab
              id="large"
              current={tab}
              onSelect={setTab}
              label={`Large Files`}
              count={summary.largeFileCount}
            />
            <Tab id="types" current={tab} onSelect={setTab} label="File Types" />
            <div className="ml-auto text-[12px] text-fg-muted tnum">
              {formatBytes(summary.totalBytes)} · {formatCount(summary.fileCount)} files ·{" "}
              {formatCount(summary.dirCount)} folders · {(summary.durationMs / 1000).toFixed(1)}s
              {summary.status === "cancelled" && <span className="ml-2 text-warn">cancelled</span>}
              {summary.issueCount > 0 && (
                <span className="ml-2 text-warn">{summary.issueCount} skipped</span>
              )}
            </div>
          </div>
          <div className="min-h-0 flex-1 px-7 pb-8">
            {tab === "tree" && <FolderTree />}
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

function FolderTree() {
  const node = useDisk((s) => s.node);
  const openNode = useDisk((s) => s.openNode);
  const home = useSystem((s) => s.info?.homeDir);
  if (!node) return null;
  const max = Math.max(node.children[0]?.sizeBytes ?? 0, node.node.ownFileBytes, 1);
  return (
    <div className="overflow-hidden rounded-lg border border-line bg-surface">
      <div className="flex items-center gap-1 border-b border-line px-3 py-2 text-[12px]">
        {node.breadcrumbs.map((b) => (
          <span key={b.path} className="flex items-center gap-1">
            <button
              type="button"
              onClick={() => void openNode(b.path)}
              className="rounded px-1 text-fg-muted hover:bg-surface-2 hover:text-fg"
            >
              {b.name || b.path}
            </button>
            <ChevronRight size={12} className="text-fg-faint" />
          </span>
        ))}
        <span className="rounded px-1 font-medium">{node.node.name || node.node.path}</span>
        <span className="ml-auto text-fg-muted tnum">{formatBytes(node.node.sizeBytes)}</span>
        <button
          type="button"
          onClick={() => void backend.fsReveal(node.node.path)}
          className="rounded p-1 text-fg-faint hover:bg-surface-2 hover:text-fg"
          title="Reveal"
        >
          <FolderOpen size={13} />
        </button>
      </div>
      {node.children.length === 0 && node.node.ownFileBytes === 0 && (
        <div className="px-3 py-6 text-center text-[12.5px] text-fg-faint">Empty folder</div>
      )}
      {node.children.map((c) => (
        <button
          key={c.path}
          type="button"
          onClick={() => void openNode(c.path)}
          className="group flex w-full items-center gap-3 border-t border-line/60 px-3 py-[7px] text-left hover:bg-surface-2/60"
        >
          <div className="w-[38%] min-w-0">
            <div className="truncate text-[12.5px] font-medium">{c.name}</div>
            <div className="truncate text-[11px] text-fg-faint tnum">
              {formatCount(c.fileCount)} files · {formatCount(c.dirCount)} folders
            </div>
          </div>
          <div className="flex-1">
            <div className="h-[8px] w-full overflow-hidden rounded-sm bg-black/[0.06] dark:bg-white/[0.07]">
              <div
                className="h-full rounded-sm bg-accent/80 transition-[width] duration-300"
                style={{ width: `${Math.max(0.5, (c.sizeBytes / max) * 100)}%` }}
              />
            </div>
          </div>
          <div className="w-[52px] text-right text-[11px] text-fg-faint tnum">
            {node.node.sizeBytes > 0
              ? `${((c.sizeBytes / node.node.sizeBytes) * 100).toFixed(0)}%`
              : ""}
          </div>
          <div className="w-[76px] text-right text-[12.5px] font-medium tnum">
            {formatBytes(c.sizeBytes)}
          </div>
          <ChevronRight size={13} className="text-fg-faint" />
        </button>
      ))}
      {node.node.ownFileBytes > 0 && (
        <div className="flex items-center gap-3 border-t border-line/60 px-3 py-[7px] text-fg-muted">
          <div className="w-[38%] min-w-0">
            <div className="flex items-center gap-1.5 truncate text-[12.5px]">
              <FileBox size={12} /> Files in this folder
            </div>
            <div className="truncate text-[11px] text-fg-faint">
              {abbreviatePath(node.node.path, home)}
            </div>
          </div>
          <div className="flex-1">
            <div className="h-[8px] w-full overflow-hidden rounded-sm bg-black/[0.06] dark:bg-white/[0.07]">
              <div
                className="h-full rounded-sm bg-fg-faint/50"
                style={{ width: `${Math.max(0.5, (node.node.ownFileBytes / max) * 100)}%` }}
              />
            </div>
          </div>
          <div className="w-[52px] text-right text-[11px] text-fg-faint tnum">
            {node.node.sizeBytes > 0
              ? `${((node.node.ownFileBytes / node.node.sizeBytes) * 100).toFixed(0)}%`
              : ""}
          </div>
          <div className="w-[76px] text-right text-[12.5px] font-medium tnum">
            {formatBytes(node.node.ownFileBytes)}
          </div>
          <div className="w-[13px]" />
        </div>
      )}
    </div>
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
  const [previewing, setPreviewing] = useState(false);

  const selectedFiles = useMemo(
    () => files.filter((f) => selected.has(f.targetId)),
    [files, selected],
  );
  const selectedBytes = selectedFiles.reduce((a, f) => a + f.sizeBytes, 0);
  const isPreset = SIZE_FILTERS.some((f) => f.bytes === minBytes);

  const review = async () => {
    if (!scanId || selectedFiles.length === 0) return;
    setPreviewing(true);
    try {
      const plan = await backend.cleanerPreview(
        scanId,
        selectedFiles.map((f) => f.targetId),
        deleteMode,
      );
      useScan.setState({ plan, cleanupProgress: null });
    } catch (e) {
      useDisk.setState({ error: String((e as { message?: string }).message ?? e) });
    } finally {
      setPreviewing(false);
    }
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
            "flex h-7 items-center gap-1 rounded-md border px-2 text-[12px]",
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
            `${files.length} files`
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
            {formatCount(e.count)} files
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
