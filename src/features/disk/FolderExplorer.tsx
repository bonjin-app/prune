import { useState } from "react";
import { ArrowUp, ChevronRight, FileBox, FolderOpen, Search } from "lucide-react";
import { Spinner } from "@/components/ui/Spinner";
import { Button } from "@/components/ui/Button";
import { cn } from "@/lib/cn";
import { abbreviatePath, formatBytes, plural } from "@/lib/format";
import { backend } from "@/lib/tauri";
import { useDisk } from "@/stores/disk";
import { useSystem } from "@/stores/system";
import { useUi } from "@/stores/ui";
import type { DiskNodeView } from "@/types/models";
import { layoutMap, mapEntries } from "./storage";

const COLORS = [
  "bg-emerald-100/80 dark:bg-emerald-900/40",
  "bg-blue-100/80 dark:bg-blue-900/40",
  "bg-amber-100/80 dark:bg-amber-900/40",
  "bg-violet-100/80 dark:bg-violet-900/40",
  "bg-rose-100/80 dark:bg-rose-900/40",
  "bg-cyan-100/80 dark:bg-cyan-900/40",
];
const PAGE_SIZE = 100;

export function FolderExplorer() {
  const node = useDisk((s) => s.node);
  const loading = useDisk((s) => s.nodeLoading);
  const openNode = useDisk((s) => s.openNode);
  if (!node)
    return loading ? (
      <div className="flex items-center justify-center gap-2 py-16 text-[12px] text-fg-muted">
        <Spinner size={14} /> Loading folders…
      </div>
    ) : (
      <div className="py-12 text-center text-[12px] text-fg-muted">
        <p className="mb-3">Folders could not be loaded.</p>
        <Button variant="secondary" onClick={() => void openNode()}>
          Retry loading folders
        </Button>
      </div>
    );
  // Reset search and pagination at each destination, including when navigating back.
  return <Explorer key={node.node.path} view={node} loading={loading} />;
}

function Explorer({ view, loading }: { view: DiskNodeView; loading: boolean }) {
  const openNode = useDisk((s) => s.openNode);
  const home = useSystem((s) => s.info?.homeDir);
  const setTab = useDisk((s) => s.setTab);
  const openProjectReview = useUi((s) => s.openProjectReview);
  const [query, setQuery] = useState("");
  const [page, setPage] = useState(0);
  const [showMap, setShowMap] = useState(true);
  const filtered = view.children.filter((child) =>
    child.name.toLocaleLowerCase().includes(query.toLocaleLowerCase()),
  );
  const pageCount = Math.max(1, Math.ceil(filtered.length / PAGE_SIZE));
  const currentPage = Math.min(page, pageCount - 1);
  // Persist the clamp so growing results cannot restore a stale page.
  if (page !== currentPage) setPage(currentPage);
  const start = currentPage * PAGE_SIZE;
  const end = Math.min(start + PAGE_SIZE, filtered.length);
  const tiles = layoutMap(mapEntries(view.children, view.node.ownFileBytes));
  const max = Math.max(
    ...view.children.slice(0, 1).map((child) => child.sizeBytes),
    view.node.ownFileBytes,
    1,
  );
  const parent = view.breadcrumbs.at(-1);
  const reveal = async () => {
    try {
      await backend.fsReveal(view.node.path);
    } catch (error) {
      useDisk.setState({ error: String((error as { message?: string }).message ?? error) });
    }
  };
  return (
    <section
      aria-label="Folder explorer"
      aria-busy={loading}
      className="overflow-hidden rounded-xl border border-line bg-surface"
    >
      <div className="flex flex-wrap items-center gap-1 border-b border-line px-4 py-3 text-[12px]">
        <button
          type="button"
          disabled={!parent || loading}
          aria-label="Go to parent folder"
          onClick={() => parent && void openNode(parent.path)}
          className="mr-1 rounded p-1.5 text-fg-muted hover:bg-surface-2 disabled:opacity-30"
        >
          <ArrowUp size={14} />
        </button>
        <nav
          aria-label="Folder breadcrumbs"
          className="flex min-w-0 flex-1 flex-wrap items-center gap-1"
        >
          {view.breadcrumbs.map((crumb) => (
            <span key={crumb.path} className="flex min-w-0 items-center gap-1">
              <button
                type="button"
                disabled={loading}
                onClick={() => void openNode(crumb.path)}
                title={crumb.path}
                className="max-w-[180px] truncate rounded px-1 py-1 text-fg-muted hover:bg-surface-2 hover:text-fg"
              >
                {crumb.name || crumb.path}
              </button>
              <ChevronRight size={12} className="shrink-0 text-fg-faint" />
            </span>
          ))}
          <span
            aria-current="page"
            title={view.node.path}
            className="max-w-[220px] truncate px-1 font-semibold"
          >
            {view.node.name || view.node.path}
          </span>
        </nav>
        {loading && <Spinner size={13} className="text-accent" />}
        <span className="ml-auto px-2 font-semibold tnum">{formatBytes(view.node.sizeBytes)}</span>
        <button
          type="button"
          onClick={() => void reveal()}
          title="Show in file manager"
          aria-label="Show folder in file manager"
          className="rounded p-1.5 text-fg-faint hover:bg-surface-2 hover:text-fg"
        >
          <FolderOpen size={14} />
        </button>
      </div>

      {view.partial && (
        <div
          role="status"
          className="border-b border-warn/20 bg-warn/5 px-4 py-3 text-[12px] text-fg-muted"
        >
          <p>
            Partial folder totals ·{" "}
            {(view.issueCount ?? 0) > 0
              ? `${plural(view.issueCount ?? 0, "unreadable path")}.`
              : "Scan stopped early."}{" "}
            This folder may hold more data than shown.
          </p>
          {(view.issues?.length ?? 0) > 0 && (
            <details className="mt-2">
              <summary className="cursor-pointer text-[11px] text-warn">Show skipped paths</summary>
              <ul className="mt-2 space-y-2">
                {view.issues?.map((issue, index) => (
                  <li key={`${issue.path}-${index}`}>
                    <p className="break-all font-mono text-[10.5px]">
                      {issue.path || "Unknown path"}
                    </p>
                    <p className="text-[11px] text-fg-faint">{issue.message}</p>
                  </li>
                ))}
              </ul>
              {(view.issueCount ?? 0) > (view.issues?.length ?? 0) && (
                <p className="mt-2 text-[11px]">
                  Showing the first {view.issues?.length} skipped paths.
                </p>
              )}
            </details>
          )}
        </div>
      )}
      {showMap && tiles.length > 0 && (
        <div className="p-4 pb-0">
          <div className="mb-2 flex items-center justify-between text-[11px] text-fg-muted">
            <span>Space map</span>
            <span>Click a folder to look inside</span>
          </div>
          <div
            className="relative h-[250px] overflow-hidden rounded-lg bg-surface-2"
            aria-label="Folder sizes map"
          >
            {tiles.map((tile, index) => {
              const small = tile.width < 22 || tile.height < 36;
              const tiny = tile.width < 9 || tile.height < 9;
              const content = (
                <>
                  <span
                    className={cn(
                      "block truncate font-semibold",
                      small ? "text-[11px]" : "text-[13px]",
                    )}
                  >
                    {tile.name}
                  </span>
                  {!small && (
                    <>
                      <span className="mt-1 block text-[12px] tnum">{formatBytes(tile.bytes)}</span>
                      <span className="mt-1 block text-[10.5px] text-fg-muted tnum">
                        {view.node.sizeBytes > 0
                          ? `${((tile.bytes / view.node.sizeBytes) * 100).toFixed(1)}%`
                          : ""}
                      </span>
                    </>
                  )}
                </>
              );
              const style = {
                left: `${tile.x}%`,
                top: `${tile.y}%`,
                width: `${tile.width}%`,
                height: `${tile.height}%`,
              };
              const title = `${tile.name} · ${formatBytes(tile.bytes)}${tile.path ? ` · ${tile.path}` : ""}`;
              return tile.path ? (
                <button
                  key={tile.key}
                  type="button"
                  disabled={loading}
                  onClick={() => void openNode(tile.path)}
                  title={title}
                  aria-label={`Explore ${tile.name}, ${formatBytes(tile.bytes)}`}
                  style={style}
                  className={cn(
                    "absolute overflow-hidden rounded-md border-[3px] border-surface text-left transition-[filter] hover:brightness-95 focus-visible:z-10 focus-visible:outline-2 focus-visible:outline-accent",
                    small ? "p-1.5" : "p-3",
                    COLORS[index % COLORS.length],
                  )}
                >
                  {!tiny && content}
                </button>
              ) : (
                <div
                  key={tile.key}
                  title={title}
                  style={style}
                  className={cn(
                    "absolute overflow-hidden rounded-md border-[3px] border-surface bg-surface-2",
                    small ? "p-1.5" : "p-3",
                  )}
                >
                  {!tiny && content}
                </div>
              );
            })}
          </div>
          <p className="mt-2 text-[10.5px] text-fg-faint">
            Folder totals are file sizes. Shared or sparse files can use less space on disk.
          </p>
        </div>
      )}

      <div className="flex flex-wrap items-center gap-3 px-4 py-3">
        <label className="field flex h-8 min-w-[180px] flex-1 items-center gap-2 rounded-md border border-line bg-surface px-2.5">
          <Search size={13} className="text-fg-faint" />
          <input
            aria-label="Filter folders"
            value={query}
            onChange={(e) => {
              setQuery(e.target.value);
              setPage(0);
            }}
            placeholder="Find a folder…"
            className="min-w-0 flex-1 bg-transparent text-[12px] outline-none"
          />
        </label>
        <span className="text-[11px] text-fg-faint tnum">{plural(filtered.length, "folder")}</span>
        <button
          type="button"
          aria-pressed={showMap}
          onClick={() => setShowMap(!showMap)}
          className="rounded px-2 py-1 text-[11px] text-fg-muted hover:bg-surface-2"
        >
          {showMap ? "Hide map" : "Show map"}
        </button>
      </div>
      <div className="flex flex-wrap items-center justify-between gap-2 border-t border-line/60 bg-surface-2/30 px-4 py-3">
        <p className="text-[11px] text-fg-muted">
          Project copies or AI worktrees? Review rebuildable artifacts separately from source files.
        </p>
        <button
          type="button"
          onClick={() => openProjectReview(view.node.path)}
          className="shrink-0 rounded px-2 py-1 text-[11px] font-medium text-accent hover:bg-accent-soft"
        >
          Find build artifacts here
        </button>
      </div>
      {filtered.slice(start, end).map((child) => (
        <button
          key={child.path}
          type="button"
          disabled={loading}
          onClick={() => void openNode(child.path)}
          className="group flex w-full items-center gap-3 border-t border-line/60 px-4 py-3 text-left hover:bg-surface-2/60 disabled:opacity-60"
        >
          <FolderOpen size={16} className="shrink-0 text-fg-faint" />
          <div className="w-[38%] min-w-0">
            <div className="truncate text-[12.5px] font-medium">{child.name}</div>
            <div className="truncate text-[11px] text-fg-faint tnum">
              {plural(child.fileCount, "file")} · {plural(child.dirCount, "folder")}
            </div>
          </div>
          <div className="min-w-0 flex-1">
            <div className="h-1.5 overflow-hidden rounded-full bg-surface-2">
              <div
                className="h-full rounded-full bg-accent/70"
                style={{ width: `${Math.max(0.5, (child.sizeBytes / max) * 100)}%` }}
              />
            </div>
          </div>
          <span className="w-[40px] text-right text-[11px] text-fg-faint tnum">
            {view.node.sizeBytes > 0
              ? `${((child.sizeBytes / view.node.sizeBytes) * 100).toFixed(1)}%`
              : ""}
          </span>
          <span className="w-[78px] text-right text-[12.5px] font-medium tnum">
            {formatBytes(child.sizeBytes)}
          </span>
          <ChevronRight size={13} className="shrink-0 text-fg-faint" />
        </button>
      ))}
      <nav
        aria-label="Folder pages"
        className="flex flex-wrap items-center justify-between gap-3 border-t border-line px-4 py-3"
      >
        <p role="status" aria-live="polite" className="text-[11px] text-fg-muted tnum">
          Page {currentPage + 1} of {pageCount} · {filtered.length ? start + 1 : 0}–{end} of{" "}
          {plural(filtered.length, "folder")}
        </p>
        <div className="flex items-center gap-2">
          <Button
            size="sm"
            variant="secondary"
            aria-label="Previous page of folders"
            disabled={loading || currentPage === 0}
            onClick={() => setPage(currentPage - 1)}
          >
            Previous
          </Button>
          <Button
            size="sm"
            variant="secondary"
            aria-label="Next page of folders"
            disabled={loading || currentPage === pageCount - 1}
            onClick={() => setPage(currentPage + 1)}
          >
            Next
          </Button>
        </div>
      </nav>
      {filtered.length === 0 && (
        <p className="border-t border-line py-8 text-center text-[12px] text-fg-faint">
          {query ? "No folders match your search." : "No subfolders here."}
        </p>
      )}
      {view.node.ownFileBytes > 0 && (
        <div className="flex items-center gap-3 border-t border-line px-4 py-3">
          <FileBox size={16} className="shrink-0 text-fg-faint" />
          <div className="min-w-0 flex-1">
            <p className="text-[12px] font-medium">Files in this folder</p>
            <p className="truncate text-[11px] text-fg-faint">
              {abbreviatePath(view.node.path, home)}
            </p>
          </div>
          <span className="text-[12px] font-medium tnum">
            {formatBytes(view.node.ownFileBytes)}
          </span>
          <button
            type="button"
            onClick={() => setTab("large")}
            className="rounded px-2 py-1 text-[11px] text-accent hover:bg-accent-soft"
          >
            All large files
          </button>
        </div>
      )}
    </section>
  );
}
