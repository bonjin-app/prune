import { useEffect, useState } from "react";
import { AlertTriangle, FileJson } from "lucide-react";
import { Card, CardHeader } from "@/components/ui/Card";
import { backend } from "@/lib/tauri";
import { useScan } from "@/stores/scan";
import { useSystem } from "@/stores/system";
import type { CustomIssue } from "@/types/models";

/**
 * Cache locations the user added themselves.
 *
 * The point of showing this is the problems. A provider that failed to load looks exactly like
 * one that found nothing, so a mistake in the file would otherwise read as "that cache is
 * already clean".
 */
export function CustomProviders() {
  const providers = useScan((s) => s.providers);
  const meta = useSystem((s) => s.meta);
  const [issues, setIssues] = useState<CustomIssue[]>([]);

  useEffect(() => {
    void backend
      .providersCustomIssues()
      .then(setIssues)
      .catch(() => setIssues([]));
  }, [providers]);

  const custom = providers.filter((p) => p.custom);
  const file = meta?.operationLogPath.replace(/operations\.jsonl$/, "providers.json");

  return (
    <Card>
      <CardHeader
        title={
          <span className="flex items-center gap-1.5">
            <FileJson size={14} /> Your own cache locations
          </span>
        }
        subtitle="Anything listed in providers.json is scanned alongside the built-in locations."
      />
      <div className="px-4 pb-4">
        {custom.length === 0 && issues.length === 0 ? (
          <p className="text-[12.5px] text-fg-muted">
            None yet. Create the file below to have Prune look somewhere it does not know about.
          </p>
        ) : (
          <ul className="mb-2 flex flex-col gap-1">
            {custom.map((provider) => (
              <li
                key={provider.id}
                className="flex items-center gap-2 rounded-md border border-line bg-surface-2/50 px-2.5 py-1.5"
              >
                <span className="min-w-0 flex-1 truncate text-[12.5px]">{provider.name}</span>
                <span className="shrink-0 text-[11px] text-fg-faint">
                  {provider.available ? "found" : "nothing there"}
                </span>
              </li>
            ))}
            {issues.map((issue, i) => (
              <li
                key={`${issue.id ?? "file"}-${i}`}
                className="flex items-start gap-2 rounded-md border border-warn/30 bg-warn-soft/40 px-2.5 py-1.5"
              >
                <AlertTriangle size={13} className="mt-[2px] shrink-0 text-warn" />
                <span className="min-w-0 flex-1 text-[12px]">
                  {issue.id && <span className="font-medium">{issue.id}: </span>}
                  {issue.message}
                </span>
              </li>
            ))}
          </ul>
        )}
        {file && <p className="font-mono text-[11px] text-fg-faint">{file}</p>}
        <p className="mt-2 text-[11.5px] text-fg-faint">
          Changes are picked up when Prune restarts. A location you add is scanned under the same
          rules as the built-in ones: nothing is removed without a preview.
        </p>
      </div>
    </Card>
  );
}
