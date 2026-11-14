# Prune Storage Foundation Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox syntax for tracking.

**Goal:** Make existing project-artifact cleanup enforce source/user protections at every removal boundary and explain those decisions honestly in the GUI.

**Architecture:** Keep Rust as the sole deletion authority. Add a focused Git artifact guard and a shared core CleanupAuthority that owns persisted protection state, policy stamps and an in-process execution lease; every replacement engine retains the same authority. React and the CLI consume the same decisions rather than implementing independent safety rules.

**Tech Stack:** Existing Rust 2021 workspace, Tauri 2, React 19, TypeScript, Zustand, Vitest and Rust integration fixtures; existing dependencies only.

**Spec:** ../specs/2026-10-11-storage-foundation-design.md. Product-quality target: ../../QUALITY-GOALS.md.

**Status:** Draft for written review. Neither this plan nor the Mole-class ambition is implementation, current feature parity, installation evidence or release approval. The existing written spec's review state is unchanged.

## Global Constraints

- “Prune remains independently implemented under MIT; no Mole code or artwork is copied.”
- “No new dependencies are planned; if the existing runner cannot satisfy this contract, request a design revision before adding one.”
- “Initial safety budgets are five seconds per candidate guard, 100,000 walked entries and 4MiB of captured Git output.”
- “Unknown is non-selectable.”
- “A saved snapshot must never supply executable cleanup authority.”
- Preserve the existing 1,001-folder/11-page regression, 100-row limit, 15-second large-fixture test limit and five-second default limit.
- Existing dirty tracked/untracked changes belong to the user. No stash, reset, whole-tree formatting, incidental version changes or commits in this scope.
- No real-user deletion, dependency installation, permission changes, Mole installation, global Codex configuration edits, application replacement or public release.
- Project-local Xcode rules, AI inventory, scan persistence, privileged maintenance and schedules are later designs, not hidden work in this milestone.
- Tests may remove only explicitly created disposable fixtures. Keep source/lockfiles, archives, dSYMs, credentials and QA evidence outside any real cleanup experiment.
- Use existing local toolchains. Record missing tools/dependencies or insufficient build space; do not install/upgrade or clean user data to make checks pass.

## Review Focus

1. Settings disappear or become corrupt after a valid preview: refuse new removal, retain prior effective protections, and preserve any already completed result. Tests: Tasks 4 and 5.
2. A stale engine Arc survives a settings change, or app/disk/CLI sessions enter cleanup by a different route: every route uses current core authority. Tests: Tasks 5 and 6.
3. Git emits hostile path bytes, malformed records, excessive output or holds output pipes open: never reinterpret a different path or accumulate unbounded workers. Tests: Tasks 2 and 3.
4. A protected path is missing, aliased or nested under a removable .app: neither lexical/canonical identity nor the bundle exception may bypass protection. Tests: Task 4.
5. A settings mutation races with cleanup completion, failed save or delayed GUI response: no deadlock, false success, lost protections or stale response replacement. Tests: Tasks 5 through 7.

## File and Interface Map

| Unit                     | Files                                                                                                                                                     | Responsibility                                                                          |
| ------------------------ | --------------------------------------------------------------------------------------------------------------------------------------------------------- | --------------------------------------------------------------------------------------- |
| Read-only Git execution  | New crates/prune-core/src/safety/git_query.rs                                                                                                             | Bounded local command execution, cancellation, byte-safe output                         |
| Artifact evidence        | New crates/prune-core/src/safety/artifact_guard.rs; models/scan.rs; safety/mod.rs                                                                         | Eligible/blocked/unknown decisions; nested repository and tracked-file checks           |
| Protection persistence   | settings/mod.rs; new settings/protected_paths.rs; safety/policy.rs                                                                                        | Strict loading, protected path identity, atomic persistence, path intersection          |
| Shared mutation boundary | New safety/authority.rs; engine.rs; error.rs; models/cleanup.rs                                                                                           | Policy stamps, fresh checks, execution/mutation exclusion, truthful interrupted results |
| Native and CLI adapters  | src-tauri/src/state.rs; commands/settings.rs; commands/cleaner.rs; lib.rs; crates/prune-cli/src/{main,lib}.rs                                             | Shared authority lifetime and transport, never an independent deletion policy           |
| Settings GUI             | src/types/models.ts; src/lib/{tauri,mock}.ts; stores/settings.ts; new features/settings/ProtectedPaths.tsx                                                | Typed protection management and visible configuration health                            |
| Review GUI               | stores/scan.ts; features/cleaner/{ProviderGroup,ProjectSection,PreviewDialog,ResultDialog,CleanerWorkspace}.tsx; dashboard/{DashboardView,insights}.ts[x] | Non-selectable evidence, accurate totals, stale-plan and interruption feedback          |
| Folder reliability       | features/disk/FolderExplorer.tsx and its existing tests                                                                                                   | Measured timeout diagnosis without weakened coverage                                    |
| Evidence                 | Core/CLI/Tauri/frontend tests; new docs/VERIFICATION-STORAGE-FOUNDATION.md                                                                                | Point-in-time results and limits, not parity claims                                     |

## Frozen Contracts for This Plan

- ProtectionStatus: eligible | blocked | unknown. ProtectionEvidence contains status, code and reason; optional CleanupTarget.protection is additive. Non-eligible evidence forces Protected risk.
- Protection codes: git_tracked_content, nested_repository, invalid_repository, unsupported_path, git_unavailable, git_query_failed, guard_timeout, guard_entry_limit, git_output_limit, git_busy, cancelled, inspection_failed. Absence of evidence is not proof of eligibility at preview/execute.
- ArtifactGuard::inspect(&self, candidate: &Path, cancel: &AtomicBool) -> ProtectionEvidence. GuardLimits::default() is 5 seconds, 100_000 entries, 4 * 1024 * 1024 combined captured stdout/stderr bytes.
- GitQueryRunner::query(&self, repo: &Path, relative_path: &Path, deadline: Instant, cancel: &AtomicBool) -> Result<Vec<u8>, GitQueryError>. GitQueryError maps to the stable evidence codes above.
- GitQueryError variants are Unavailable, Failed, Timeout, Cancelled, OutputLimit, Busy, UnsupportedPath and InvalidOutput. SystemGitQueryRunner::shared() -> Arc<Self> uses one process-wide two-slot pool, including quarantined jobs; replacement engines never create additional pools.
- ProtectedPath contains path: PathBuf and resolved_path: Option<PathBuf>. Both serialize camelCase. A missing path remains persisted; inability to establish necessary identity is reported and cleanup-blocking, not silently dropped.
- Settings gains protected_paths: Vec<ProtectedPath>. Settings::load(&Path) -> Result<Settings> replaces silent-default loading; only a genuinely absent initial file or a valid legacy document lacking protectedPaths supplies an empty list.
- PolicyStamp contains authority_id: String and revision: u64. CleanupPlan gains optional policy_stamp for serialization migration; executable plans must contain the exact current stamp.
- CleanupAuthority::persistent(data_dir: PathBuf, platform_policy: SafetyPolicy) -> Arc<Self>; CleanupAuthority::sandbox(settings: Settings, platform_policy: SafetyPolicy) -> Arc<Self> is explicit for fixtures/embedded callers, not a production fallback.
- CleanupAuthority::refresh(&self) -> Result<PolicyStamp>; try_update_settings(&self, update: impl FnOnce(&mut Settings) -> Result<()>) -> Result<Settings>; begin_execution(&self, expected: &PolicyStamp) -> Result<ExecutionLease<'_>>.
- ExecutionLease::refresh_before_target(&mut self) -> Result<()> reads persisted protections and checks the stamp without recursively acquiring the authority lock. The lease's current policy supplies path validation.
- PruneEngine::configured retains its existing settings/data-dir parameters and adds authority: Arc<CleanupAuthority>. Add PruneEngine::with_authority(platform, registry, authority) for explicit composition; production constructors resolve a real settings store or start diagnosis-only, never silently use a sandbox authority.
- PruneEngine::with_settings(&Settings, Arc<CleanupAuthority>) and with_parts(Box<dyn PlatformService>, ProviderRegistry, Arc<CleanupAuthority>) also require explicit authority; migrate all callers. PruneEngine::new() resolves the production store from KnownPaths.app_support/app.bonjin.prune or constructs a cleanup-blocked diagnosis-only authority when no store can be resolved.
- PruneEngine::plan(&ScanSession, &[String], DeleteMode) -> Result<CleanupPlan>. Keep execute's current result type. Its implementation acquires the execution lease internally.
- New core error codes: cleanup_busy, protection_config_error, policy_changed. A stale/missing/foreign policy stamp requires a fresh preview.
- CleanupResult gains optional interruption: CleanupInterruption { code: String, message: String }. Keep existing removed totals and failed-target accounting: every planned target not successfully removed, including unattempted targets, appears once in failed.
- SettingsView adds cleanupBlockedReason?: string and policyStamp?: PolicyStamp. The GUI can open and diagnose invalid settings but cannot treat the fallback view as an effective unprotected cleanup policy.
- New IPC settings_set_protected_paths(paths: Vec<String>) -> CommandResult<SettingsView>; Backend.settingsSetProtectedPaths(paths: string[]) -> Promise<SettingsView>. Inputs are absolute paths to protect, not arbitrary deletion paths.

## Task 1: Reproduce and Resolve Folder-Explorer Reliability Failures

**Files:** Modify src/features/disk/FolderExplorer.tsx and src/features/disk/FolderExplorer.test.tsx only if measurement identifies their bottleneck; record evidence in docs/VERIFICATION-STORAGE-FOUNDATION.md.

**Interfaces:** Preserve FolderExplorer() and existing useDisk.openNode behavior. No new backend contract.

- [ ] Run the existing FolderExplorer test file alone, recording wall time, Node version and the exact failing test names: pnpm exec vitest run src/features/disk/FolderExplorer.test.tsx.
- [ ] Distinguish DOM-query time, React render time and unfinished async work with disposable instrumentation. Do not assume the October 11 failures still reproduce; record a current pass if they do not.
- [ ] Add a focused failing regression for the measured cause before any fix; retain all 1,001 folders, all 11 pages, focus, search, shrink/path-reset and pending-request checks.
- [ ] Apply only the measured fix; do not raise timeouts, skip coverage or replace production accessibility queries with weaker assertions.
- [ ] Run that file three consecutive times with unchanged thresholds; then run pnpm test. Expected: all passes. If the cause is not established, stop this task with measurements rather than claim a fix.
- [ ] Review the scoped diff; do not commit unrelated existing work.

## Task 2: Add a Bounded Read-Only Git Query Adapter

**Files:** Create safety/git_query.rs and crates/prune-core/tests/git_query.rs; register through safety/mod.rs. Leave the Docker runner's behavior unchanged.

**Interfaces:** Produce GitQueryRunner, GitQueryError and a production SystemGitQueryRunner; use GuardLimits from the frozen contract. Provide an injectable fake runner for Task 3.

- [ ] Write failing tests named output_is_nul_preserving, combined_output_limit_is_4mib, deadline_requests_termination_and_reaps_settled_child, cancellation_is_unknown, inherited_git_environment_is_ignored and held_pipe_never_accumulates_workers.
- [ ] Pin exact assertions: 4MiB is the combined stdout/stderr capture cap; the deadline is five seconds; timeout/cancellation never return partial bytes as a successful query. Use short injected deadlines for tests, not five-second sleeps.
- [ ] Run cargo test -p prune-core --test git_query. Expected initially: missing adapter/tests fail to compile or the new assertions fail for the documented reason.
- [ ] Implement shell-free git ls-files --stage -z with literal pathspec handling. Preserve raw path bytes until validated; pass OS-native arguments. Disable optional index locks, executable fsmonitor and inherited GIT_DIR/GIT_WORK_TREE/GIT_INDEX_FILE/GIT_CONFIG overrides; do not run status, hooks, filters, checkout or network commands.
- [ ] Do not reuse Docker SystemRunner: it captures unbounded output and may leave reader threads behind after a timeout. Acquire a lifetime slot nonblocking before spawning; use at most two jobs process-wide. Use fixed read buffers and one synchronized 4MiB capture budget per job; two jobs retain at most 8MiB of captured output plus bounded buffers/parser state.
- [ ] At deadline/cancellation/overflow request termination and reaping without an indefinite wait or join on the request path. Release the slot only after the direct child is reaped AND both readers finish; unsettled child/reader/reaper jobs retain that same slot. Later requests return unknown/git_busy rather than creating replacement jobs. Five seconds is a decision deadline, not a teardown guarantee.
- [ ] Return successful bytes only after exit success, complete collection of both streams, no exceeded budget/deadline/cancellation and complete valid NUL records. Treat truncated/error/unknown output as no clearance. Use a resolved trusted Git executable, never a repository-controlled executable choice.
- [ ] Add a helper fixture that holds a pipe after the direct child exits and test bounded query return/slot exhaustion. If the std-only adapter cannot satisfy these limits on a supported platform, stop for a design revision; do not introduce a dependency, unsafe unreviewed platform code or an unbounded fallback.
- [ ] Run the test file and cargo clippy -p prune-core --all-targets -- -D warnings. Expected: passes with no output truncation treated as success.
- [ ] Review the scoped diff without committing user changes.

## Task 3: Classify and Enforce Artifact Evidence

**Files:** Create safety/artifact_guard.rs and tests/artifact_guard.rs; modify models/scan.rs, models/mod.rs, safety/mod.rs, providers/developer/projects.rs and providers/context.rs only as required for evidence transport; update target constructors across existing fixtures.

**Interfaces:** Consume Task 2's runner; produce ArtifactGuard::inspect and optional CleanupTarget.protection. Preview/execution enforcement is wired in Task 5.

- [ ] Write failing fixtures for ignored_generated_output_is_eligible, tracked_missing_file_blocks, submodule_entry_blocks, nested_git_file_blocks, linked_worktree_owner_is_valid, no_owner_requires_complete_boundary_walk and unknown_is_never_eligible.
- [ ] Add spaces, newlines, non-ASCII, pathspec-looking names, malformed NUL records, broken .git pointers, permission failures, unavailable Git and a pre-cancelled guard. A complete 100_000-entry walk may pass; a required 100_001st entry returns guard_entry_limit. Unsupported path encodings must return unknown/unsupported_path, not a lossy path or different target.
- [ ] Run cargo test -p prune-core --test artifact_guard. Expected: new missing behavior fails.
- [ ] Implement owner discovery for normal repositories and .git pointer files; parse staged index records including mode 160000 and tracked-but-missing paths. Inspect nested repository boundaries without following directory symlinks; keep existing Unity source/marker rules.
- [ ] Enforce the combined five-second candidate budget across Git and filesystem inspection. Check cancellation/time between filesystem calls; an OS access wait may delay return. No cross-scan eligibility cache; no cache at all is required in the first implementation.
- [ ] Invoke the guard during project discovery, attach explicit blocked/unknown evidence and Protected risk, and retain visible reasons even for non-selectable candidates. A failure to measure size remains a ScanIssue, not zero verified bytes.
- [ ] Run cargo test -p prune-core --test artifact_guard and cargo test -p prune-core --test developer_projects. Expected: passes; old Unity source/symlink tests remain intact.
- [ ] Review the scoped diff and constructors, including CLI/Tauri fixture compatibility.

## Task 4: Persist User Protections Without Silent Fallback

**Files:** Modify settings/mod.rs and safety/policy.rs; create settings/protected_paths.rs, tests/settings_protections.rs and tests/protected_paths.rs.

**Interfaces:** Produce strict Settings::load, ProtectedPath and protected-path intersection helpers. Consume existing SafetyPolicy, preserving base platform restrictions and its residual filesystem-race disclaimer.

- [ ] Add failing tests for missing_initial_file_is_legacy_default, corrupt_existing_file_is_error, invalid_protection_field_is_error, unknown_settings_fields_are_not_overwritten, missing_protected_path_is_retained, protection_alias_is_blocked and protected_descendant_blocks_parent.
- [ ] Add a protected file inside an otherwise removable .app bundle; assert the bundle exception cannot bypass it. Cover relative/traversal inputs, Unicode/spaces, aliases and platform-specific canonical prefixes. A temporarily missing path keeps its original and last resolved identities.
- [ ] Run cargo test -p prune-core --test settings_protections and cargo test -p prune-core --test protected_paths. Expected: red before implementation.
- [ ] Implement strict loading and protection validation. Reject malformed/unsupported existing documents rather than overwrite unrecognized settings; valid legacy projectRoots persists unchanged. Do not trim meaningful path characters or silently discard unresolved identities.
- [ ] Update policy evaluation so user protections apply before platform bundle exceptions and cover exact paths, descendants and containing candidates. Unknown identity must prevent unsafe cleanup while read-only analysis remains available.
- [ ] Replace fixed settings.json.writing with a unique create-new temporary sibling; preserve permissions and atomically rename. Inject persistence faults to test a full-disk write/rename failure without filling the real disk.
- [ ] Run the two test files plus cargo test -p prune-core settings. Expected: last effective settings survive failed persistence; no orphan temporary file or silent protection reset.
- [ ] Review the scoped diff; leave authority-mediated saves to Task 5.

## Task 5: Make All Engines Share One Cleanup Authority

**Files:** Create safety/authority.rs and tests/cleanup_authority.rs; modify engine.rs, error.rs, models/cleanup.rs, safety/mod.rs, settings/mod.rs, src-tauri/src/state.rs, commands/cleaner.rs, commands/settings.rs, crates/prune-cli/src/main.rs and crates/prune-cli/src/lib.rs; adapt existing core constructors/tests.

**Interfaces:** Produce the authority, policy stamp, execution lease, interruption metadata and engine signatures from the frozen contract. Consume Tasks 3 and 4.

- [ ] Write stale_engine_cannot_bypass_new_protection, foreign_or_missing_stamp_requires_preview, deleted_settings_after_preview_blocks, persistent_authorities_for_same_store_share_lease and settings_change_during_cleanup_is_busy tests. Canonical aliases of the same settings store must share authority rather than obtain independent mutation leases.
- [ ] Write tracked_content_added_after_scan_blocks_preview and nested_repository_added_after_preview_blocks_execution fixtures. Assert every source file survives, including directly invoked engine calls.
- [ ] Write policy_change_after_first_removal_preserves_partial_result: a controlled callback changes fixture settings after one success. Assert no later target is touched; removedTargets == 1; all other planned IDs appear exactly once in failed, marked unattempted; interruption.code == policy_changed.
- [ ] Write corrupt_config_after_first_removal_reports_partial, failed_save_keeps_effective_policy and poisoned_authority_blocks_cleanup tests. Do not use lock-recovery defaults for poisoned safety state.
- [ ] Run cargo test -p prune-core --test cleanup_authority. Expected: new safety assertions fail before wiring.
- [ ] Implement one authority per live persisted store: persistent() reuses a process-wide registry keyed by canonical store identity with weak authority references. For a genuinely new store, canonicalize its nearest existing ancestor and append validated missing components; test startup without an existing app-data directory. Unresolvable ancestor identity is diagnosis-only. Give replacements the same Arc; include a unique authority ID plus a monotonic protection revision. Reject a configured engine/store mismatch. Compare normalized persisted protection contents and resolution state, not only mtime or an editable revision field. Removing a previously observed settings file is not initial startup.
- [ ] Keep startup diagnosis available on invalid configuration, with an explicit cleanup-blocked status and last known effective protections. Production constructors and CLI main never turn an unavailable store into unrestricted sandbox cleanup.
- [ ] Acquire the lease inside synchronous execute on its blocking worker. Acquire authority before engine/settings locks; never acquire it recursively during refresh or from a progress callback. Settings mutations use try_lock and return cleanup_busy.
- [ ] At preview, refresh persisted protection state and run fresh artifact/path guards. Stamp every executable plan. At execution start reject stale/foreign/missing stamps before removing anything; refresh and re-check immediately before each target.
- [ ] On a policy/configuration change after work began, stop further removal and return truthful partial/failed results with interruption metadata and all unattempted IDs in failed. Never return a bare error that conceals completed deletion.
- [ ] Route CLI construction/clean through the same authority; map policy errors to EXIT_ERROR without deleting. Read-only commands still work with visible diagnostic status. Keep target-ID and preview semantics unchanged.
- [ ] Update settings setters by cloning the latest strictly loaded settings and changing only their owned field; verify persisted content has not changed before saving. Unique temp names avoid collision, but document that this is not a cross-process lock/CAS guarantee.
- [ ] Run cargo test -p prune-core --test cleanup_authority, cargo test -p prune-core --test pipeline and cargo test -p prune-cli. Expected: passes, preserving accounting and no mutation through stale Arcs.
- [ ] Review locking and every production constructor independently before integration. Do not claim protection against an arbitrary external editor changing files between the final check and removal.

## Task 6: Expose and Manage Protection State Through IPC and Settings

**Files:** Modify src-tauri/src/commands/settings.rs, commands/mod.rs, lib.rs and tests/ipc.rs; modify src/types/models.ts, src/lib/{tauri,mock}.ts, stores/settings.ts, stores/settings.test.ts, src/test/{factories,reset}.ts and features/settings/SettingsView.tsx; create features/settings/ProtectedPaths.tsx and ProtectedPaths.test.tsx.

**Interfaces:** Consume Task 5's shared authority. Expose SettingsView fields and settings_set_protected_paths / Backend.settingsSetProtectedPaths from the frozen contract; add useSettings.setProtectedPaths(paths: string[]) -> Promise<boolean>.

- [ ] Add IPC regressions for legacy settings migration, roots/protections surviving reciprocal edits, rejected save preserving state, cleanup-busy mutation and a stale policy stamp.
- [ ] Add IPC fixtures covering cleaner scans, app:<id> sessions and disk large-file sessions. Protect a candidate after each session's discovery; assert shared preview refuses it. No browser-only disabled checkbox can satisfy this test.
- [ ] Add frontend tests for adding/removing an explicit protection, missing-path visibility, duplicate save suppression, stale response rejection, save failure and retaining the last visible effective state.
- [ ] Run cargo test -p prune --test ipc and pnpm exec vitest run src/stores/settings.test.ts src/features/settings/ProtectedPaths.test.tsx. Expected: red for the new behavior.
- [ ] Register and implement the single typed IPC setter. Preserve latest project roots and return policy health; all persistence remains in Rust. Keep paths literal and do not add a broad filesystem/shell plugin.
- [ ] Add a Settings protection list with explicit add/remove controls, visible unresolved state and clear distinction from non-removable built-in OS protections. Follow existing tokens and accessible focus/error patterns.
- [ ] Add “Protect this path” to reviewed cleanup targets without executing cleanup. On protection changes invalidate the old preview and prune only now-ineligible selections; preserve the scan and its evidence.
- [ ] Run the IPC/frontend commands again plus pnpm typecheck. Expected: passes; mock behavior is labelled mock evidence, not native deletion evidence.
- [ ] Review changed IPC names and Rust/TypeScript model parity.

## Task 7: Show Honest Review Totals and Protection Reasons

**Files:** Modify stores/scan.ts and its tests; features/cleaner/{ProviderGroup,ProjectSection,PreviewDialog,ResultDialog,CleanerWorkspace}.tsx and focused tests; dashboard/DashboardView.tsx, dashboard/insights.ts and insights.test.ts; add features/cleaner/eligibility.ts and eligibility.test.ts; add dashboard/DashboardView.test.tsx if absent.

**Interfaces:** eligibility.ts exports isReviewable(target: CleanupTarget): boolean and summarizeTargets(targets: CleanupTarget[]): { foundBytes: number; reviewableBytes: number; blockedBytes: number }. It rejects Protected risk or non-eligible evidence. These are display/selection helpers, never backend authorization.

- [ ] Add a 60-byte eligible, 30-byte blocked and 10-byte unknown fixture; assert foundBytes == 100, reviewableBytes == 60 and blockedBytes == 40. Unknown and blocked targets remain visible with reasons.
- [ ] Add regressions proving row selection, project “select all”, safe recommendation and plan totals exclude blocked/unknown IDs, even after a delayed scan response or failed settings save.
- [ ] Add cancelled/partial zero-result dashboard tests: neither says the machine is tidy or verified empty. Keep logical bytes distinct from available-now capacity and retain the current Trash caveat.
- [ ] Add preview regressions for newly blocked targets and policy_changed/protection_config_error; preserve scan rows and require re-preview, without calling the result a successful cleanup.
- [ ] Add interrupted-result tests using Task 5's contract: successes alone contribute to removed totals; every failed/unattempted planned ID is retained; invalid duplicate IDs, contradictory totals or inconsistent interruption/status are rejected.
- [ ] Run pnpm exec vitest run src/stores/scan.test.ts src/features/cleaner src/features/dashboard. Expected: newly specified assertions fail before the UI changes.
- [ ] Implement shared eligibility display helpers and integrate them with all selection entrypoints and dashboards. Qualify generic “rebuilt on demand” wording; old repository metadata is not proof that the artifact is unused.
- [ ] Distinguish repository-activity metadata from artifact-modification time in the existing project grouping. No automatic age-based deletion or new navigation system.
- [ ] Run the same tests, pnpm typecheck and pnpm lint. Expected: passes with keyboard/focus, compact-window and theme behavior retained.
- [ ] Review the scoped diff and confirm cleanup-result capacity-read errors cannot alter the actual cleanup outcome.

## Task 8: Verify the Whole Foundation and Record Release Boundaries

**Files:** Tests from Tasks 1–7; new docs/VERIFICATION-STORAGE-FOUNDATION.md; scoped updates to docs/ARCHITECTURE.md, SECURITY.md and docs/QUALITY-GOALS.md only when their statements are now backed by results.

**Interfaces:** No new runtime API. Consume the complete contracts above and the existing no-network/distinct-binary-name policy scripts.

- [ ] Check toolchain versions and available storage. Record the exact source revision plus dirty diff/file provenance rather than implying HEAD alone represents this work.
- [ ] Run pnpm typecheck, pnpm lint, pnpm test and pnpm build. Run the FolderExplorer file three times separately. Expected: all pass with original timeout/fixture thresholds.
- [ ] Run cargo test --workspace and cargo clippy --workspace --all-targets -- -D warnings. Expected: passes. Missing dependencies are a reported blocker, not permission to download them.
- [ ] Check formatting only for touched supported files; record unrelated HEAD failures separately. Run bash scripts/check-no-network.sh and bash scripts/check-binary-names.sh.
- [ ] Use existing native/test tooling against named disposable fixtures to verify selection, blocked reasons, focus, both themes and protection changes through the actual IPC surface. Browser mocks and Tauri mock runtime do not substitute for native WebView evidence.
- [ ] If a fresh local native build is feasible with existing dependencies and available space, run it from the build output without replacing /Applications/Prune.app. Any destructive fixture exercise must be confined to a newly created, named temporary root; do not invoke real cleaner providers on the user's folders.
- [ ] Record current tests/timings, peak-resource measurements where actually obtained, native fixture results, inability to run a check and remaining OS/permission waits. Do not reuse October 8 or October 11 results as current passes.
- [ ] Have an independent high-risk review cover authority lifetime, locks, Git parsing, settings corruption and partial-result truthfulness. Resolve correctness/data-loss findings before calling the foundation complete.
- [ ] State separately: source implementation, automated checks, native QA, local artifact, installed app, signing/notarization and publication. No Mole parity/speed claim from a passing fixture suite.
- [ ] Preserve all existing unrelated edits and provide the reviewed diff/test evidence; no commit, push, tag or installer replacement without separate direction.

## Spec Coverage and Sequencing

- Git-aware guard: Tasks 2, 3 and 5. User-protected paths: Tasks 4, 5 and 6.
- Honest GUI decisions: Tasks 6 and 7. Folder-explorer reliability: Task 1.
- All entrypoints and protection-change races: Tasks 5, 6 and 8.
- Automated/native evidence and distribution boundaries: Task 8.
- Tasks 1 and 2 can be investigated independently. Tasks 3 and 4 can follow in parallel once Task 2's runner contract is fixed; Task 5 integrates them. Tasks 6 and 7 share DTOs and must integrate in order, not race on the same files.
- Match the user's automatic-routing preference: main agent owns integration; fast_scan gathers bounded read-only evidence, routine_worker may implement bounded UI/tests, and deep_worker performs the safety review. Never recursively spawn workers or ask the user to pick a model.
- Keep each task's red/green/checkpoint cycle. Since this tree is already dirty and commits are outside scope, checkpoint by reviewing scoped diffs, not by committing another contributor's changes.

## Architecture Consultation and Known Limits

A read-only independent design consultation recommended the shared-core-authority approach over AppState-only checks or caller-supplied immutable snapshots. It identified stale Arcs, locking order, external settings edits, .app exceptions and truthful partial results as critical. This was source inspection, not a verified implementation.

The existing Docker query adapter does not satisfy Git's output/cancellation constraints. Task 2 therefore has a specific safe-stop gate; no new dependency or privilege is implicitly approved. Separate processes can still edit configuration or start another cleanup, and path validation plus a final Git/policy check is not an atomic filesystem operation. A portable OS-wide lock/handle-based deletion design is outside this milestone; do not describe in-process exclusion as cross-process serialization.

The real RiderLink folders and the recently measured storage values are motivating evidence only. All new safety/removal tests use synthetic fixtures. This plan does not grant eligibility to RiderLink/.local or to any model/session/archive folder.

## Execution Handoff

Review this plan together with the written spec before product-code execution. The user has already requested automatic delegation decisions; keep that method and do not ask for model selection. Recommended execution is main-agent integration with bounded workers and a fresh independent high-risk review, respecting the task dependencies above.

A review approval is for this storage-foundation milestone, not for automatic cleanup, a new Xcode provider, model/session deletion or public distribution. Those capabilities require their own designs and evidence.
