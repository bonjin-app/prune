import type { ScanResult, ScanSession } from "@/types/models";

/**
 * What a result could free: everything it found, less what is protected.
 *
 * Protected targets are listed so a person can see what is taking the space — a model's
 * conversation history, a git worktree with uncommitted work — but they can never be removed.
 * Counting them in a figure labelled "can be pruned" would promise space Prune will not give.
 * Computed from the targets themselves, so it holds for any result, including ones produced
 * before the engine reported a protected total of its own.
 */
export function reclaimableBytes(result: Pick<ScanResult, "targets">): number {
  return result.targets.reduce((sum, t) => (t.risk === "protected" ? sum : sum + t.sizeBytes), 0);
}

export function sessionReclaimableBytes(session: Pick<ScanSession, "results">): number {
  return session.results.reduce((sum, r) => sum + reclaimableBytes(r), 0);
}
