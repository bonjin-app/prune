# Local verification: 0.2.0

Date: 2026-10-08, Apple Silicon macOS. Working-tree build, not a published release.

## Automated checks

- Frontend: ESLint, TypeScript, production Vite build and 277 tests passed.
- Rust: workspace format check, Clippy with warnings denied and all 204 workspace tests passed.
- Dependency policy: no-network and distinct-binary-name checks passed.
- Reproduced early completion and post-cleanup stale-response failures with controlled
  asynchronous tests before fixing them. Confirmed matching-ID replay and request invalidation.
- Native event-loss recovery is covered by timer-controlled summary reconciliation, including
  running scans, obsolete responses, duplicate completion and terminal query errors.
- Cleaner receives the same bounded early-completion and one-second reconciliation safeguards.
  Queued cancellation, duplicate requests, stale previews and malformed/failed cleanup feedback
  are covered without increasing the deletion policy's scope.
- Safety review reproduced a cross-screen late preview overwriting an executing plan, rescan
  discarding an active cleanup result, incomplete results pruning rows, and rejected results
  displaying success. All preview callers now share a guarded request lifecycle; execution
  locks rescan/dismissal and invalidates pending preview requests. Runtime result validation
  checks required fields and mode/status/failure consistency before showing or pruning results.
  Zero confirmed removals are never headlined as successful cleanup.
- Every failed path must match its planned target ID. Logical files/bytes must equal the sums
  of successful planned targets, as the native engine reports them. Contradictory zero totals,
  wrong failure paths and legitimate all-failed outcomes have dedicated regressions. Final
  read-only review passed this bounded lifecycle/feedback scope with no remaining Critical or
  Important findings; 92 focused tests and injected malformed/valid outcomes were checked.
- A 1,001-folder regression traverses all 11 pages, bounds each page to 100 rows, searches all
  children and checks focus, resets, shrink clamping and pending navigation controls.
- Result-dialog checks separate logical bytes from available-now capacity, retain cleanup
  status on capacity errors, retry only the reading and reject older-operation responses.
- All changed supported frontend/docs files pass Prettier. Whole-repository `format:check`
  still reports eight untouched files; each also fails the same check at HEAD `05a1c7f`:
  `docs/ARCHITECTURE.md`, two `packaging/winget` installer/locale manifests, `Card.tsx`,
  `ProjectSection.tsx`, `ProviderGroup.tsx`, `MonitorView.tsx`,
  and `src/lib/tauri.ts`. These unrelated baseline formatting differences were preserved.
- Unity fixtures check actual discovery/preview/deletion within disposable temp directories:
  generated Library is eligible; source/config, nested source scan roots, root symlink aliases
  and unrelated Libraries are protected from this artifact provider.

## Native read-only evidence

The installed `/Applications/Prune.app` read live capacity of roughly 2TB, 98.1% used,
with about 37GB available. These are point-in-time OS readings, not reclaimed-space promises.
During the final safety-pass app check, the live reading was about 29.3GB available,
98.5% used. Prune did not delete user data to improve this number.

A Documents analysis completed after reading 13,810,570 files. It measured about 1.45TB:
workspace about 1.19TB, Codex about 249GB. Six unreadable paths were explicitly marked as
partial; this is not an exact complete or atomic inventory. Drilldown into Codex worked,
including date-folder map and rankings. No user files were deleted.

After rebuilding the ad-hoc signed app, Documents access stalled inside `opendir` while macOS
TCC was evaluating folder access; this is not evidence of a completed final-app Documents
scan or a completion-event failure. No permissions were changed. A disposable `/tmp` fixture
completed natively (3 files, 174B), with map drilldown and the project-root suggestion handoff
verified. Storage now also explains blocked locations and OS access waits. The final signed
identity needs user-managed folder/Full Disk Access permissions for protected locations.

Values use decimal units and logical file lengths; hard-link deduplication, shared APFS
extents, sparse allocation and unreadable paths can differ from `du` or macOS Storage totals.

## Distribution boundary

The macOS app/DMG are local ad-hoc-signed review artifacts. Verify both the app and the app
inside the mounted DMG using `codesign --verify --deep --strict`; `hdiutil verify` checks the
disk image integrity. Ad-hoc signing is not notarization or Gatekeeper approval for downloads.
Public release, Developer ID signing/notarization, Windows/Intel builds, and actual Unity cache
regeneration are not evidenced here. See [the GUI feature mapping](MOLE-GUI.md) for remaining
Mole-inspired capabilities rather than treating 0.2.0 as complete feature parity.

Final local app and mounted DMG app both passed strict signature verification. The DMG
passed integrity verification; its app binary matched the installed app at:
`855163435adea94fa0b82a5831aab0998a3416d63b080bd1e973ba61a46ec53b` (SHA-256).
Final installed app re-tested the 3-file fixture and map drilldown, with Storage permission
guidance visible. A separate native fixture of 205 empty folders showed exactly 100, 100 and
5 list rows on its three pages. Searching for `folder-001` from the last page found it and
reset pagination. No file removal was performed in native QA. Native OS cancellation latency
and protected-folder access remain limitations, not claims proven by mocked lifecycle tests.

Browser-mock GUI checks at 880×600 verified dark preview/result dialogs and a light preview,
including visible confirmation/footer controls and the separated two-volume capacity reading.
The mock cleanup modifies only its in-memory dataset; it is not real-disk removal evidence.
Temporary viewport/theme overrides were restored and the preview server stopped.

Previous local app builds were moved to recoverable `/tmp/prune-*-install.*`
backups, not deleted. No commits, pushes, tags or GitHub releases were made.
