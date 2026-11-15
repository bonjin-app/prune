import { useEffect, useState } from "react";
import { History, RefreshCw } from "lucide-react";
import { Button } from "@/components/ui/Button";
import { abbreviatePath, formatBytes, formatRelative, plural } from "@/lib/format";
import { backend } from "@/lib/tauri";
import type { SavedAnalysis } from "@/types/models";

/**
 * What earlier analyses found, so the Storage view is not blank while a new one runs for
 * minutes. A record only: it says when it is from, and offers nothing but analysing again,
 * because anything removed must come from a fresh analysis and its preview.
 */
export function SavedAnalyses({
  home,
  disabled,
  onAnalyze,
}: {
  home: string | undefined;
  disabled: boolean;
  onAnalyze: (root: string) => void;
}) {
  const [saved, setSaved] = useState<SavedAnalysis[]>([]);

  useEffect(() => {
    let live = true;
    // History is a convenience: if it cannot be read, there is simply none to show.
    backend
      .diskListSaved()
      .then((list) => live && setSaved(list))
      .catch(() => undefined);
    return () => {
      live = false;
    };
  }, []);

  if (saved.length === 0) return null;
  return (
    <section aria-label="Earlier analyses" className="mx-7 mb-8 flex flex-col gap-3">
      <h3 className="flex items-center gap-1.5 text-[12px] font-medium text-fg-muted">
        <History size={13} /> Earlier analyses
      </h3>
      {saved.map((a) => {
        const largest = a.folders[0]?.sizeBytes ?? 0;
        return (
          <article
            key={a.root}
            aria-label={`Earlier analysis of ${a.root}`}
            className="rounded-lg border border-line bg-surface px-4 py-3"
          >
            <header className="flex items-baseline gap-2">
              <span className="min-w-0 truncate font-mono text-[12px]" title={a.root}>
                {abbreviatePath(a.root, home)}
              </span>
              <span className="text-[13px] font-medium tnum">{formatBytes(a.totalBytes)}</span>
              <span className="text-[11.5px] text-fg-faint tnum">
                {plural(a.fileCount, "file")}, analyzed {formatRelative(a.finishedAt)}
              </span>
              <Button
                size="sm"
                variant="secondary"
                className="ml-auto shrink-0"
                disabled={disabled}
                onClick={() => onAnalyze(a.root)}
              >
                <RefreshCw size={12} /> Analyze again
              </Button>
            </header>
            {a.issueCount > 0 && (
              <p className="mt-1 text-[11.5px] text-warn">
                {plural(a.issueCount, "path")} could not be read then, so this may be low.
              </p>
            )}
            <ul className="mt-2 flex flex-col gap-1">
              {a.folders.map((f) => (
                <li
                  key={f.name}
                  className="grid grid-cols-[minmax(0,1fr)_120px_70px] items-center gap-3 text-[12px]"
                >
                  <span className="truncate">{f.name}</span>
                  <span className="h-1.5 overflow-hidden rounded-full bg-black/[0.06] dark:bg-white/[0.07]">
                    <span
                      className="block h-full rounded-full bg-fg-faint/60"
                      style={{
                        width: `${largest ? Math.max(2, (f.sizeBytes / largest) * 100) : 0}%`,
                      }}
                    />
                  </span>
                  <span className="text-right text-fg-muted tnum">{formatBytes(f.sizeBytes)}</span>
                </li>
              ))}
            </ul>
          </article>
        );
      })}
      <p className="text-[11px] text-fg-faint">
        Sizes as they were then. To remove anything, analyze again: removal always works from a
        fresh analysis and its preview.
      </p>
    </section>
  );
}
