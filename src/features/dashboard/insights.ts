import { groupByProject, STALE_DAYS, daysSince } from "@/features/cleaner/projects";
import { formatBytes, formatCount } from "@/lib/format";
import { DEVELOPER_CATEGORIES, type Category, type ScanSession } from "@/types/models";
import type { ViewId } from "@/stores/ui";

/**
 * Something worth telling the user about what the scan found.
 *
 * A total is not advice. "504 GB reclaimable" tells someone their machine is full, which they
 * already knew; "21 projects you have not touched in three months are holding 9.45 GB" tells
 * them where to start. Every insight names a real number from the scan and sends the user to
 * the place where they can act on it — nothing here removes anything.
 */
export interface Insight {
  id: string;
  headline: string;
  detail: string;
  bytes: number;
  /** Where acting on this starts. */
  view: ViewId;
  /** Applied to the result list when the user follows it. */
  filter?: string;
  /** Only show safe items when following this. */
  safeOnly?: boolean;
  tone: "neutral" | "opportunity";
}

/** Which section shows a given category. */
function viewFor(category: Category | undefined): ViewId {
  return category && DEVELOPER_CATEGORIES.includes(category) ? "developer" : "cleaner";
}

/** How many to show. More than this and it stops being advice. */
const MAX_INSIGHTS = 3;

/**
 * Reads a finished scan and picks the few observations worth acting on.
 *
 * Ordered by how much space each represents, because that is what the user is here for.
 */
export function deriveInsights(session: ScanSession | null, now = Date.now()): Insight[] {
  if (!session) return [];
  const targets = session.results.flatMap((r) => r.targets);
  if (targets.length === 0) return [];

  const insights: Insight[] = [];

  // Projects nobody has worked on for months. The one thing a size-sorted list cannot show.
  const { projects } = groupByProject(targets);
  const stale = projects.filter((p) => {
    const days = daysSince(p.group.lastActiveAt, now);
    return days !== null && days >= STALE_DAYS;
  });
  const staleBytes = stale.reduce((a, p) => a + p.sizeBytes, 0);
  if (stale.length > 0 && staleBytes > 0) {
    insights.push({
      id: "stale-projects",
      headline: `${formatCount(stale.length)} dormant ${stale.length === 1 ? "project is" : "projects are"} holding ${formatBytes(staleBytes)}`,
      detail:
        stale.length === 1
          ? `Nothing has touched ${stale[0]!.group.label} in over three months.`
          : `Nothing has touched them in over three months. The largest is ${stale[0]!.group.label}.`,
      bytes: staleBytes,
      view: "developer",
      tone: "opportunity",
    });
  }

  // Everything that can go without a second thought.
  //
  // Safe items live in both sections, and an insight has to send the user to one of them. It
  // therefore counts only what the destination actually shows: a headline promising more than
  // the page it opens would be the same broken promise as a button that under-counts its plan.
  const safeByView = new Map<ViewId, number>();
  for (const result of session.results) {
    const bytes = result.targets
      .filter((t) => t.risk === "safe")
      .reduce((a, t) => a + t.sizeBytes, 0);
    if (bytes === 0) continue;
    const view = viewFor(result.category);
    safeByView.set(view, (safeByView.get(view) ?? 0) + bytes);
  }
  const [safeView, safeBytes] = [...safeByView.entries()].sort((a, b) => b[1] - a[1])[0] ?? [];
  if (safeView && safeBytes) {
    insights.push({
      id: "safe",
      headline: `${formatBytes(safeBytes)} can go with nothing at stake`,
      detail:
        safeView === "developer"
          ? "Tool caches your toolchain rebuilds on its own."
          : "Caches and logs your applications rebuild on their own.",
      bytes: safeBytes,
      view: safeView,
      safeOnly: true,
      tone: "opportunity",
    });
  }

  // One directory that dwarfs the rest is worth naming on its own.
  const biggest = targets.reduce((a, b) => (b.sizeBytes > a.sizeBytes ? b : a));
  const rest = targets.reduce((a, t) => a + t.sizeBytes, 0) - biggest.sizeBytes;
  if (biggest.sizeBytes > 1_000_000_000 && biggest.sizeBytes > rest * 0.15) {
    insights.push({
      id: "biggest",
      headline: `One item alone holds ${formatBytes(biggest.sizeBytes)}`,
      detail: `${biggest.label}${biggest.description ? ` — ${biggest.description}` : ""}.`,
      bytes: biggest.sizeBytes,
      view: viewFor(
        session.results.find((r) => r.targets.some((t) => t.id === biggest.id))?.category,
      ),
      filter: biggest.label,
      tone: "neutral",
    });
  }

  return insights.sort((a, b) => b.bytes - a.bytes).slice(0, MAX_INSIGHTS);
}
