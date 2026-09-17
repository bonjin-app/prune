import { useMemo } from "react";
import { ArrowRight, Lightbulb } from "lucide-react";
import { Card, CardHeader } from "@/components/ui/Card";
import { cn } from "@/lib/cn";
import { useScan } from "@/stores/scan";
import { useUi } from "@/stores/ui";
import { deriveInsights, type Insight } from "./insights";

/**
 * What the scan is actually telling you.
 *
 * A total is not advice: "504 GB reclaimable" only repeats what the disk gauge already said.
 * These are the few observations that point somewhere — dormant projects, what carries no risk,
 * a single directory that dwarfs the rest — each opening the view where it can be acted on,
 * already filtered. Nothing here removes anything.
 */
export function InsightsCard() {
  const session = useScan((s) => s.session);
  const setView = useUi((s) => s.setView);
  const setFilter = useScan((s) => s.setFilter);
  const setRiskFilter = useScan((s) => s.setRiskFilter);
  const selectRecommended = useScan((s) => s.selectRecommended);

  const insights = useMemo(() => deriveInsights(session), [session]);
  if (insights.length === 0) return null;

  const follow = (insight: Insight) => {
    setFilter(insight.filter ?? "", insight.view);
    setRiskFilter(insight.safeOnly ? "safe" : "all");
    if (insight.safeOnly) selectRecommended();
    setView(insight.view);
  };

  return (
    <Card>
      <CardHeader
        title={
          <span className="flex items-center gap-1.5">
            <Lightbulb size={14} /> Worth a look
          </span>
        }
        subtitle="What this scan found that a total cannot tell you."
      />
      <div className="px-4 pb-4">
        <ul className="divide-y divide-line/60">
          {insights.map((insight) => (
            <li key={insight.id}>
              <button
                type="button"
                onClick={() => follow(insight)}
                className="group flex w-full items-center gap-3 py-2.5 text-left hover:bg-surface-2/50"
              >
                <span
                  className={cn(
                    "mt-[3px] h-1.5 w-1.5 shrink-0 self-start rounded-full",
                    insight.tone === "opportunity" ? "bg-accent" : "bg-fg-faint",
                  )}
                />
                <span className="min-w-0 flex-1">
                  <span className="block text-[12.5px] font-medium">{insight.headline}</span>
                  <span className="block truncate text-[11.5px] text-fg-muted">
                    {insight.detail}
                  </span>
                </span>
                <ArrowRight
                  size={13}
                  className="shrink-0 text-fg-faint transition-transform group-hover:translate-x-0.5 group-hover:text-fg"
                />
              </button>
            </li>
          ))}
        </ul>
      </div>
    </Card>
  );
}
