import { useScan } from "@/stores/scan";
import { formatBytes, formatCount } from "@/lib/format";
import { Spinner } from "@/components/ui/Spinner";

export function ScanProgressBar({ providerIds }: { providerIds: string[] }) {
  const progress = useScan((s) => s.progress);
  const providers = useScan((s) => s.providers);
  const entries = Object.values(progress).filter((p) => providerIds.includes(p.providerId));
  const files = entries.reduce((a, p) => a + p.scannedFiles, 0);
  const bytes = entries.reduce((a, p) => a + p.discoveredBytes, 0);
  const latest = entries.find((p) => p.currentPath);
  const total = entries[0]?.providersTotal ?? providerIds.length;
  const done = Math.max(...entries.map((p) => p.providersDone), 0);
  const pct = total > 0 ? Math.min(100, (done / total) * 100) : 0;
  const name = latest ? providers.find((p) => p.id === latest.providerId)?.name : undefined;

  return (
    <div className="mx-7 mb-3 rounded-lg border border-line bg-surface px-4 py-3">
      <div className="flex items-center gap-2 text-[12.5px]">
        <Spinner size={13} className="text-accent" />
        <span className="font-medium">Scanning{name ? ` ${name}` : ""}…</span>
        <span className="ml-auto text-fg-muted tnum">
          {formatCount(files)} files · {formatBytes(bytes)}
        </span>
      </div>
      <div
        role="progressbar"
        aria-label="Scan progress"
        aria-valuemin={0}
        aria-valuemax={total}
        aria-valuenow={done}
        aria-valuetext={`${done} of ${total} places searched`}
        className="mt-2 h-1 w-full overflow-hidden rounded-full bg-black/[0.07] dark:bg-white/[0.08]"
      >
        <div
          className="h-full rounded-full bg-accent transition-[width] duration-300"
          style={{ width: `${Math.max(3, pct)}%` }}
        />
      </div>
      <div className="mt-1.5 truncate font-mono text-[11px] text-fg-faint">
        {latest?.currentPath ?? " "}
      </div>
      {/*
        A scan can run for two minutes. Announce which place is being searched, and nothing
        else: the file count and the current path change several times a second, and a live
        region fed from those would talk over everything the user does.
      */}
      <span className="sr-only" aria-live="polite">
        {name ? `Searching ${name}` : "Searching"}
      </span>
    </div>
  );
}
