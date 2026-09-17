import type { CleanupTarget, TargetGroup } from "@/types/models";

/** One project, with everything it is holding. */
export interface Project {
  group: TargetGroup;
  targets: CleanupTarget[];
  sizeBytes: number;
}

/** Old enough that nobody is working on it, recent enough to still be a live project. */
export const STALE_DAYS = 90;

/**
 * Groups a provider's targets by the project they belong to, biggest project first.
 *
 * A real machine produced 797 artifact directories across 112 projects. A list that long
 * cannot be judged item by item; one row per project can be.
 */
export function groupByProject(targets: CleanupTarget[]): {
  projects: Project[];
  ungrouped: CleanupTarget[];
} {
  const byKey = new Map<string, Project>();
  const ungrouped: CleanupTarget[] = [];

  for (const target of targets) {
    if (!target.group) {
      ungrouped.push(target);
      continue;
    }
    const existing = byKey.get(target.group.key);
    if (existing) {
      existing.targets.push(target);
      existing.sizeBytes += target.sizeBytes;
    } else {
      byKey.set(target.group.key, {
        group: target.group,
        targets: [target],
        sizeBytes: target.sizeBytes,
      });
    }
  }

  const projects = [...byKey.values()].sort((a, b) => b.sizeBytes - a.sizeBytes);
  for (const project of projects) {
    project.targets.sort((a, b) => b.sizeBytes - a.sizeBytes);
  }
  return { projects, ungrouped };
}

/** Whole days since a project was last worked on, or null when there is nothing to read. */
export function daysSince(iso: string | undefined, now = Date.now()): number | null {
  if (!iso) return null;
  const then = new Date(iso).getTime();
  if (Number.isNaN(then)) return null;
  return Math.max(0, Math.floor((now - then) / 86_400_000));
}

/**
 * How long since anyone worked on a project, in words.
 *
 * "No repository" is deliberately not "never": an unknown date must not make a project look
 * abandoned when Prune simply had nothing to read.
 */
export function describeActivity(days: number | null): string {
  if (days === null) return "no repository";
  if (days === 0) return "worked on today";
  if (days === 1) return "worked on yesterday";
  if (days < 30) return `${days} days since work`;
  if (days < 365) return `${Math.round(days / 30)} months since work`;
  return `over a year since work`;
}
