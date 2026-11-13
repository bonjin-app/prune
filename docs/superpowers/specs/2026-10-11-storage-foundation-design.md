# Prune production storage foundation

Status: written design for user review; not implemented or release-ready.
Date: 2026-10-11.

## Intent and product direction

The user wants an installable, open-source GUI that explains why a nearly full
2TB Mac is full and helps them make safe space-management decisions. The target
is Mole-level usability or better, especially for development and local AI data.
Prune remains independently implemented under MIT; no Mole code or artwork is copied.

The selected direction is storage-first, AI/developer-aware management rather
than adding every maintenance and monitoring feature at once. Existing project
artifact review, disk exploration and mandatory cleanup preview are retained.
“Better than Mole” is an acceptance ambition, not a verified marketing claim.
The current official reference is https://mole.fit/ (inspected 2026-10-11).

## Decomposition

These are separate, independently reviewable sub-projects:

1. **Storage foundation, specified here:** Git-aware artifact protection,
   user-protected paths, truthful review totals, and reliable folder-explorer tests.
2. **AI inventory:** read-only classification of Codex sessions/worktrees and
   Ollama/LM Studio model stores versus caches. Show ownership, evidence, unknown
   activity and impact. Never infer disposability from size, age or folder name.
   Session contents, credentials and conversation text are not read. Initially
   models/sessions are view-only; their removal needs a distinct design.
3. **Large-scan experience:** bounded persistent snapshots, explicit stale/partial
   results, targeted refresh, measured resource budgets and refined GUI navigation.
   A saved snapshot must never supply executable cleanup authority.

Projects 2 and 3 require their own written design and implementation plan. App
updates, privileged optimization, schedules, menu-bar monitoring and app-managed
Trash restoration are outside these projects. OS Trash remains the recovery path.

## Current evidence and constraints

- HEAD is `05a1c7f`; existing local 0.2.0 changes are uncommitted and must be preserved.
- The installed app identifies as 0.2.0; this is not evidence of a public release.
- On 2026-10-11, Rust 204 tests, TypeScript and ESLint passed. Frontend testing
  passed 275 tests and timed out in two `FolderExplorer.test.tsx` tests. The same
  two timeouts reproduced when that file ran alone. Their cause is not established.
- Project candidates currently depend on directory names and marker files.
  Unity source guards exist, but general Git-tracked/nested-repository checks do not.
- `PruneEngine::plan` and `execute` apply path policy; the new guard must apply
  there as well as during discovery. A frontend disabled checkbox is not protection.
- `Settings::load` currently falls back to defaults on corruption. Once settings
  contain protections, silently dropping them would be unsafe and must change.
- No user-data deletion, permissions changes, dependency installation, Git commit,
  push, release publication or installation replacement is authorized by this spec.

## Project 1 behavior

### Git-aware artifact guard

For targets from `project_artifacts`, determine whether the actual candidate can
be reviewed for removal. Retain the existing marker, source and path protections.

- A tracked file or Git submodule entry at or below a candidate blocks the entire
  candidate, including tracked-but-missing entries. Ignored is not proof of safety.
- A `.git` directory or pointer file inside the candidate blocks removal of that
  candidate. This prevents taking nested repositories and worktrees with it.
- Owner discovery understands normal repositories and `.git` pointer files.
  An unresolved pointer, unreadable metadata or malformed Git output is unknown,
  not “no repository.” A valid owner worktree is not itself a blanket prohibition
  on regenerable artifacts outside its tracked content.
- Candidates without an owner repository can remain eligible only after a complete
  nested-repository boundary check. They remain low/medium risk under existing rules.
- Missing Git, permission errors, cancellation, timeout and resource-limit exhaustion
  yield a protected/unknown outcome with an explanation, never an eligible outcome.
- Guards run at discovery, preview and immediately before each removal. Discovery
  caches are scoped to one scan; previews and execution do not trust cached eligibility.

Implement a focused core guard module consumed by the provider and engine. It
returns `eligible`, `blocked` or `unknown` with stable reason codes and display
text. Unknown is non-selectable. Keep existing cleanup target IDs and result
accounting; add optional protection evidence compatibly to Rust/TypeScript models.

Git inspection is local and read-only: no shell, checkout, fetch, index refresh,
hooks, filters or filesystem-monitor commands. Use NUL-delimited output and literal
path handling; do not split on whitespace or lossy Unicode. Disable optional locks
and executable monitoring/configuration side effects. Unsupported path encodings
must fail closed, not identify a different target.

Initial safety budgets are five seconds per candidate guard, 100,000 walked
entries and 4MiB of captured Git output. Budget exhaustion blocks that candidate.
These are proposed limits, not benchmark results. The runner drains pipes safely,
requests termination on timeout/cancellation and reaps its direct child without
an indefinite wait on the request path. Use bounded concurrency rather than a
subprocess for every file. No new dependencies are planned; if the existing runner
cannot satisfy this contract, request a design revision before adding one.

Proposed runner clarification for written review, 2026-10-15: one process-wide
runner has at most two lifetime slots, including quarantined jobs. Each query
uses one monotonic decision deadline and a shared 4MiB stdout/stderr capture
budget. A slot is released only after the direct child is reaped and both readers
finish; unsettled jobs keep their slots. Slot exhaustion returns unknown/busy,
never eligibility. Fixed read buffers and capped retention bound resource use.
This is a bounded-accumulation guarantee, not a guarantee that OS child/thread
teardown completes within five seconds. Only complete successful query output
can clear a target. This clarification has not yet been implemented or benchmarked.

The five-second filesystem-walk budget is checked between OS calls; an OS access
wait can delay it and cancellation. The GUI must show that wait rather than promise
a hard five-second completion. Git query decision deadlines are enforced by the runner;
unsettled resource teardown remains explicitly unknown rather than reported complete.

### User-protected paths

Extend native settings with `protectedPaths`, defaulting to an empty list only
for a missing settings file or a valid legacy file lacking that field. Each
protection covers the path, its descendants and cleanup candidates containing it.
It does not hide the location from read-only storage analysis.

The GUI adds a protection action to a reviewed target and a Settings list with
explicit add/remove actions. Saving requires an absolute normalized path. Store
both the user-facing path and resolved identity when available; retaining a
temporarily missing path is intentional. Reject relative/traversal inputs and
report unresolved protection identities rather than silently dropping them.
Existing OS-protected paths cannot be removed through this list.

The existing project-roots setter reconstructs `Settings`; change it to preserve
all protection fields. Updating either setting must not erase the other.

Apply protections to every cleanup source: project artifacts, custom providers,
large files and app leftovers. Protection of a descendant blocks parent removal.
Do not allow the app-bundle exception to bypass user protections.

Atomic persistence failure leaves the last effective policy intact. A corrupt or
unreadable existing protection configuration permits diagnosis but blocks cleanup
until the user repairs it; it never silently becomes an empty policy. Legacy
project roots and appearance preferences are preserved.

The backend maintains a policy revision. Preview records its revision; executing
with a different revision returns a re-preview-required error before removing
anything. Refresh persisted protections at the execution boundary. Settings
mutations are rejected while execution is active. Re-check each target's path and
guard immediately before removal. Portable path validation is not an atomic
filesystem snapshot: retain and document the existing residual race limitation.

### Honest GUI decisions

- Dashboard separates “found”, “available for review” and “protected/unchecked”.
  The existing session total must not be advertised as bytes definitely reclaimable.
- Protected and unknown targets remain visible with their reasons; neither group
  participates in selection, recommended totals or executable plan totals.
- Review actions lead to the relevant category. A cancelled/partial scan never
  describes the machine as tidy merely because it found zero candidates.
- Preview shows newly blocked targets and protection changes without converting
  them into successful removals. Keep existing fail-closed result validation.
- Preserve logical-size versus OS-available-capacity labels and Trash caveats.
- Maintain keyboard navigation, focus, accessible names and light/dark contrast.
  Do not introduce a second navigation system or replace the current design tokens.

### Folder-explorer reliability

Investigate the two observed timeouts before making changes. Instrument disposable
test runs to distinguish DOM-query cost, render cost and unfinished async work.
Do not blanket-raise timeouts, skip tests, reduce fixture coverage or claim native
UI slowness from jsdom timing alone.

Keep the 1,001-folder/11-page regression, 100-row limit, whole-folder search,
focus preservation, shrink clamping, path-reset and pending-navigation behavior.
The unchanged 15-second large-fixture test limit and five-second default limit
are the initial acceptance thresholds. Optimize the actual measured bottleneck;
if representative hardware cannot meet them, bring evidence back for review.

## Existing code boundaries

| Responsibility                  | Existing integration                                                        |
| ------------------------------- | --------------------------------------------------------------------------- |
| Candidate evidence              | `crates/prune-core/src/providers/developer/projects.rs`                     |
| Mandatory enforcement           | `crates/prune-core/src/engine.rs`, `crates/prune-core/src/safety/policy.rs` |
| Protection persistence          | `crates/prune-core/src/settings/mod.rs`, native settings commands           |
| API/model compatibility         | `src/types/models.ts`, `src/lib/tauri.ts`, native command registration      |
| Review and selection            | Cleaner/project components and `src/stores/scan.ts`                         |
| Honest first screen             | `src/features/dashboard/DashboardView.tsx`                                  |
| Protection management           | Settings components and `src/stores/settings.ts`                            |
| Regression/performance evidence | FolderExplorer tests, core safety/pipeline tests, Tauri IPC tests           |

Implementation planning will name exact new files and signatures after this
written design is approved. No unrelated refactoring or formatting cleanup.

## Acceptance and verification

1. Reproduce then resolve both folder-explorer failures; three consecutive isolated
   passes and one full frontend pass without increasing limits or dropping coverage.
2. Git fixtures cover tracked output, ignored output, tracked missing files,
   nested repositories, submodules, linked worktrees, spaces/newlines/non-ASCII
   paths, unavailable Git, malformed output, unreadable paths and budget exhaustion.
3. Execution fixtures add a tracked file or nested repository after scanning and
   after preview: removal must fail without touching it. These tests use only
   named disposable temporary directories.
4. Protection fixtures cover descendants, parents, aliases, temporary absence,
   legacy migration, corrupt settings, full-disk save failure and stale policy
   revisions. All entry points enforce the same native decision.
5. Frontend tests verify visible reasons, truthful totals, disabled selection,
   re-preview requirements and preservation of existing cleanup-result invariants.
6. TypeScript, ESLint, frontend tests, Rust workspace tests, formatting of touched
   files, Rust Clippy and dependency-policy checks pass. Baseline unrelated format
   warnings remain separately identified, not erased.
7. Native read-only fixture QA verifies selection, reasons, focus and both themes.
   Real-user deletions are excluded. Record timing, environment and limitations
   separately from mocked tests; no Mole speed claim without comparable measurements.
8. A built local artifact, installed-app evidence, signing/notarization, publication
   and actual deletion are separate milestones. No public production claim follows
   merely from tests or an ad-hoc app build.

## Review decision

The conversational direction is approved. This written Project 1 design still
needs user review. Next is a written implementation plan for Project 1, followed
by its review before product-code execution. Delegation is chosen automatically:
read-only evidence uses `fast_scan`, bounded coding uses `routine_worker`, and
high-risk safety review uses `deep_worker`; no recursive agents.
