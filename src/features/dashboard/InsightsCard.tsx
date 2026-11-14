import { useMemo } from "react";
import { ArrowRight, Lightbulb } from "lucide-react";
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

  // Side by side, one per column, so three observations cost one row of the screen. Each is a
  // way in, not a box of its own: one frame, divided.
  return (
    <section aria-labelledby="insights-title">
      <h2
        id="insights-title"
        className="mb-2 flex items-center gap-1.5 text-[12px] font-medium text-fg-muted"
      >
        <Lightbulb size={13} /> Worth a look
      </h2>
      <ul
        className={cn(
          "grid overflow-hidden rounded-xl border border-line/70 bg-surface",
          "divide-y divide-line/60 md:divide-x md:divide-y-0",
          insights.length === 3 && "md:grid-cols-3",
          insights.length === 2 && "md:grid-cols-2",
        )}
      >
        {insights.map((insight) => (
          <li key={insight.id} className="min-w-0">
            <button
              type="button"
              onClick={() => follow(insight)}
              className="group flex h-full w-full items-start gap-2.5 px-4 py-3 text-left hover:bg-surface-2/50"
            >
              <span
                className={cn(
                  "mt-[5px] h-1.5 w-1.5 shrink-0 rounded-full",
                  insight.tone === "opportunity" ? "bg-accent" : "bg-fg-faint",
                )}
              />
              <span className="min-w-0 flex-1">
                <span className="block text-[12.5px] leading-snug font-medium">
                  {insight.headline}
                </span>
                <span className="mt-0.5 block truncate text-[11.5px] text-fg-muted">
                  {insight.detail}
                </span>
              </span>
              <ArrowRight
                size={13}
                className="mt-[3px] shrink-0 text-fg-faint transition-transform group-hover:translate-x-0.5 group-hover:text-fg"
              />
            </button>
          </li>
        ))}
      </ul>
    </section>
  );
}
