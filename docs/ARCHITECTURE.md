# Prune Architecture

This document is the design reference for contributors. It records the decisions made before
the first line of code (spec §39) and the reasoning behind them.

## 1. Principles

| Principle                   | Consequence in code                                                                                     |
| --------------------------- | ------------------------------------------------------------------------------------------------------- |
| Local-first / Privacy-first | The engine has no network crate at all; the app carries only what Tauri needs, and no HTTP client or TLS stack on any shipped target. Enforced by `scripts/check-no-network.sh` and `src/lib/no-network.test.ts` in CI. Operation log is JSON lines on disk. |
| Safe-by-default             | All deletion goes through `SafetyPolicy::validate` → `fs::remove`. Only `ValidatedPath` can be removed. |
| Developer-first             | Developer providers are the largest provider group and have their own view.                             |
| CLI-ready                   | `prune-core` has no Tauri dependency. `crates/prune-cli` is a second front end over the same engine.    |
| Native                      | Rust owns all OS access. React never touches the filesystem.                                            |

## 2. Workspace layout

```
prune/
├── Cargo.toml                 workspace: crates/prune-core, src-tauri
├── crates/prune-cli/          the `prune` command line
├── crates/prune-core/         the engine (pure Rust)
│   ├── src/
│   │   ├── engine.rs          PruneEngine: scan / plan / execute
│   │   ├── error.rs           PruneError with stable codes
│   │   ├── models/            serializable DTOs (camelCase) — mirrored in src/types/models.ts
│   │   ├── safety/            SafetyPolicy, ValidatedPath
│   │   ├── fs/                size calculation, trash / permanent removal
│   │   ├── providers/         CleanupProvider trait, SimpleProvider, registry
│   │   │   ├── system/        user caches, logs, temp, trash, old installers
│   │   │   ├── browser/       Chromium family, Firefox
│   │   │   └── developer/     tool caches (declarative) + project artifact walker
│   │   ├── scan/              parallel runner, progress, cancel, overlap de-dup
│   │   ├── settings/          user settings the engine needs (project roots)
│   │   ├── analyzer/          disk analyzer: directory tree, large files, extensions
│   │   ├── apps/              uninstaller: app detail + leftovers as cleanup targets
│   │   ├── docker/            Docker usage and the two prune commands
│   │   ├── ops/               OperationLog (JSONL)
│   │   ├── platform/          PlatformService trait; macos/, windows/, generic/, sandbox
│   │   └── system/            SystemMonitor (sysinfo)
│   └── tests/pipeline.rs      end-to-end against a sandboxed fake home
├── src-tauri/                 Tauri shell
│   ├── src/commands/          app, system, cleaner, disk, apps, startup, settings, ops, fs
│   ├── src/state.rs           AppState: engine, monitor, ops, scans, plans, disks, apps, startup
│   ├── tauri.conf.json        window, CSP, bundle
│   └── capabilities/          core permissions only (no shell / fs / http plugins)
└── src/                       React UI
    ├── lib/tauri.ts           the ONLY IPC entry point (+ browser mock in lib/mock.ts)
    ├── types/models.ts        TS mirror of Rust models
    ├── stores/                zustand: ui, system, scan, disk, apps, startup, settings
    ├── app/                   Shell, Sidebar, routes
    ├── components/ui/         primitives
    └── features/              dashboard, cleaner, developer, monitor, settings, palette, …
```

The spec suggested `src-tauri/src/{core,commands,models,services,platform}`. We moved
`core/models/services/platform` into a separate crate so the same engine can back a CLI
(spec §25) and be tested without a WebView.

## 3. Safety pipeline

```
Discovery            provider.scan()            → CleanupTarget (id, path, size, risk)
Path Validation      SafetyPolicy::validate     → ValidatedPath | SafetyViolation
Risk Classification  provider risk, escalated to Protected if validation fails
User Confirmation    UI selection by target id
Dry Run              PruneEngine::plan          → CleanupPlan (id, targets, blocked, totals)
Delete               PruneEngine::execute       re-validates each path, then fs::remove
Operation Log        OperationLog::append       JSONL in the app data dir
```

An operation-log failure never fails a cleanup. By the time the log is written the files are
gone; returning an error there would tell the user the opposite of what happened and invite them
to run it again. `CleanupResult::log_error` carries the problem instead, and the result dialog
shows it.

### SafetyPolicy rules (in order)

1. Absolute path, no `.` / `..` components, not a filesystem root.
2. Must exist (`symlink_metadata`; the leaf symlink is never followed).
3. Must be strictly inside an **allowed root** (home, temp). Whitelist, not blacklist.
4. Must not equal an **exact protected** path or be an ancestor of one
   (`~`, `~/Library`, `~/Documents`, `~/Desktop`, `~/Downloads`, …).
5. Must not be inside a **protected tree**
   (`/System`, `/usr`, `~/.ssh`, `~/Library/Keychains`, `C:\Windows`, `%APPDATA%\Microsoft\Credentials`, …).

The lists live in `platform/{macos,windows}` and are canonicalized at policy creation.

Validation resolves the directories above the target, so a symlinked ancestor is followed once
and judged on where it actually points. That answer is recorded in the `ValidatedPath`, and
`fs::remove` checks it again immediately before removing: `still_resolves_where_it_did`
re-canonicalizes the parent and refuses if it now leads somewhere else. Without that, the gap
between the confirmation the user reads and the removal itself — seconds, or longer for a plan
with many items — is long enough for anything with write access to the home directory to
replace a folder in the path with a link, and `remove_dir_all` would walk through it. The check
cannot be made airtight without holding directory handles open for the whole operation, which
the standard library does not offer portably; what it does is reduce the window from "as long
as the dialog is open" to microseconds.

### IPC boundary

The frontend never sends a filesystem path for deletion:

```
cleaner_start_scan(provider_ids?)         → scan_id
cleaner_preview(scan_id, target_ids, mode) → CleanupPlan { id, … }
cleaner_execute(plan_id)                   → CleanupResult
```

Target ids are FNV-1a hashes of `provider_id + path`, resolved inside the engine. A plan can be
executed once; the scan it came from is dropped afterwards because its sizes are stale.

## 4. Providers

```rust
pub trait CleanupProvider: Send + Sync {
    fn id(&self) -> &'static str;
    fn name(&self) -> &'static str;
    fn category(&self) -> Category;
    fn description(&self) -> &'static str;
    fn default_risk(&self) -> RiskLevel;
    fn priority(&self) -> u8 { 50 }          // overlap resolution
    fn is_available(&self, known: &KnownPaths) -> bool;
    fn scan(&self, ctx: &ScanContext<'_>) -> ScanOutput;
}
```

Most providers are `SimpleProvider` constants: a list of roots resolved from `KnownPaths`,
a mode (`Whole` root = one target, or every `Child` = one target), an optional age filter and
exclusions. Custom providers exist for Chromium profiles, Firefox profiles, installers, and the
project artifact walker.

**User-defined providers.** `providers.json` in the application data directory is parsed at
start-up into `CustomProvider`s that join the registry. They sit at priority 90, above the
generic built-ins, so a location someone named by hand is reported under that name rather than
as an anonymous cache folder. A file chooses _where to look_ and nothing else: the paths it
produces go through `SafetyPolicy` exactly like a built-in provider's, so it cannot point Prune
at the system, and every target still needs a preview and a confirmation. Ids are rejected when
they collide with a built-in one, repeat, or contain anything but `[a-z0-9_]`; unknown fields
are rejected too, since a typo that changed nothing and said nothing would be worse than an
error. Problems are carried on the engine and shown, never swallowed — a provider that failed
to load looks exactly like one that found nothing.

**Overlap de-duplication.** A generic provider (`user_cache`, priority 10) may report
`~/Library/Caches/Homebrew` while `homebrew_cache` (priority 60) reports the same directory.
After all providers finish, `scan::dedupe_overlaps` keeps the higher-priority target for any
equal / ancestor / descendant pair, so no byte is offered twice. This is what lets a recognised
tool cache move out of the anonymous "Application Caches" pile and into the Developer view with
its own name, description and risk, without being counted twice — verified against a real
machine as well as in tests.

**Project grouping.** Each artifact carries the project it belongs to: the nearest ancestor
holding a `.git` directory, bounded by the scan root. Without walking up, a monorepo's
`apps/desktop/src-tauri/target` looks like a project called `src-tauri`, and one repository
appears as a dozen unrelated rows. The group also carries when that project was last worked on,
taken from `.git/index` (which moves on every stage, commit or checkout) and falling back to
`.git/HEAD`. A project with no repository reports an unknown date rather than a very old one,
because guessing there would age a live project wrongly. This is what turns 797 artifact
directories into 112 project rows that can actually be judged.

**Project artifacts.** The walker only reports a directory when a marker proves it is a build
artifact (`node_modules` next to `package.json`, `target` next to `Cargo.toml`, `.venv`
containing `pyvenv.cfg`, …) and never descends into a reported directory, `.git`, or hidden
folders.

## 5. Scan engine

- Providers run in parallel with `rayon`; each gets its own throttled progress callback
  (≤10 events/s) that the Tauri layer forwards as `prune://scan-progress`.
- Cancellation is an `Arc<AtomicBool>` checked every 64 entries during traversal.
- Sizes are logical (`metadata.len()`), hard links counted once per `(dev, ino)`, symlinks not
  followed. This matches Finder's reported size.
- Discovery and measurement are separate phases in the project artifact walker. The walk finds
  candidate directories (cheap; it never descends into one it has reported), then all of them are
  measured with rayon. Most of the hundreds found on a real machine are small, and measuring them
  one after another left most cores idle even though each measurement is itself parallel. Results
  are sorted by path afterwards, because parallel completion order is arbitrary and the list must
  be stable.
- Sizing one tree is itself parallel. `fs::dir_stats` descends up to four levels until it has at
  least 32 independent subtrees, then walks them with rayon; the directories and files it passes
  on the way are counted during the descent. Two consequences worth remembering when editing it:
  the descent must honour cancellation and emit progress (a wide, shallow tree can be consumed
  entirely before any subtree walk starts), and every file — including those found during the
  descent — must go through the shared inode set, or a file and a hard link to it elsewhere are
  both counted. Both cases are covered by tests.
- Non-fatal errors (permission denied, vanished files) become `ScanIssue`s and are shown, not
  hidden.

## 5b. Disk analyzer

`analyzer::analyze` walks a root once (DFS via `walkdir`) and keeps:

- every **directory** as a node (name, path, size, files, dirs, own-file bytes, children) —
  files are not stored, so memory stays proportional to the directory count;
- the **largest files** in a bounded min-heap (2000 entries);
- **per-extension** byte/count totals (top 40).

Sizes propagate to all ancestors on every file (depth is small), hard links count once, symlinks
are not followed. Scanning `/` skips other mounts and APFS firmlink mirrors (`/Volumes`,
`/System/Volumes`) to avoid double counting.

The analysis stays in `AppState.disks` for drill-down queries (`disk_get_node`). Large files are
also registered as a synthetic `ScanSession` (provider `large_files`, category `LargeFiles`,
risk `Medium`, or `Protected` when the policy refuses the path) so `cleaner_preview` /
`cleaner_execute` handle them with no special casing. After an execution the Tauri layer calls
`DiskAnalysis::forget_removed` so folder sizes and the large-file list reflect reality.

## 5c. Uninstaller

`platform::applications` lists installed software: `.app` bundles under `/Applications`
(one level of subfolders) and `~/Applications` on macOS, the `Uninstall` registry keys on
Windows. Sizes are measured in parallel afterwards and streamed to the UI, so the list appears
instantly.

`platform::app_related_paths` returns the existing locations an application writes outside its
bundle. Matching by folder name is what makes this work for Electron apps, and also what makes
it dangerous: a vendor folder such as `~/Library/Application Support/Google` is shared by
Chrome, Drive and the updater, so no single application may claim it. Those names are on a
denylist. The product folder _inside_ one is a different matter — `Google Chrome` does own
`Google/Chrome` — so a name of the form `Vendor Product` is also looked up as `Vendor/Product`.
Without both halves of that rule the uninstaller either offers a sibling's data or misses the
application's own.

macOS matches on the bundle identifier (`~/Library/Caches/<id>`, `Containers`, `WebKit`,
`HTTPStorages`, `Application Scripts`, and `Preferences` / `Saved Application State` entries
prefixed with the id) **and** on folder names, using both the bundle file name and
`CFBundleName` — Electron apps store data under the latter (`~/Library/Application Support/Code`
for Visual Studio Code). Names shorter than three characters are ignored so generic folders are
never swept in. Windows matches `%LOCALAPPDATA%` / `%APPDATA%` folders named after the
application and its publisher.

Each location becomes a `CleanupTarget` with a risk derived from `AppDataKind` (caches and logs
`Safe`, preferences `Low`, the bundle and application data `Medium`). The detail is registered as
a scan session `app:<id>`, so `cleaner_preview` / `cleaner_execute` remove it with no special
casing. Apple and Microsoft system applications are listed, but their bundle is forced to
`Protected` — only their leftovers can be removed. On Windows `apps_run_uninstaller` launches the
vendor's own uninstaller; Prune never runs it silently.

### The `.app` bundle exception in the safety policy

`/Applications` is a protected tree, yet uninstalling must be possible. `ProtectedPaths` gained
`app_bundle_roots`, and a path passes validation when it is a directory ending in `.app` whose
**parent is exactly** one of those roots. `/Applications` itself, `/Applications/Utilities`,
nested bundles and anything _inside_ a bundle stay refused.

## 5d. Startup manager

macOS reads `~/Library/LaunchAgents`, `/Library/LaunchAgents` and `/Library/LaunchDaemons`.
The plist supplies the label, the program and the trigger (`RunAtLoad`, `KeepAlive`,
`StartInterval` → at login / always running / scheduled / on demand), but the _effective_ state
comes from launchd's per-user override database, read with
`launchctl print-disabled gui/<uid>`. Toggling runs `launchctl enable|disable`, which writes
only to that database: the plist is never edited, nothing is deleted, running processes are not
killed, and the change applies at the next login. Applications registered through
`SMAppService` (macOS Login Items) live in a SIP-protected database and are deliberately not
reported rather than shown with a state Prune cannot verify.

Windows reads the `Run` keys (HKCU, HKLM, and the WOW6432Node mirror) and the per-user and
common Start Menu `Startup` folders. The enabled flag and the toggle both use
`StartupApproved`, the same per-user database Task Manager writes: the original `Run` value or
shortcut is untouched.

Items installed for all users (system launch agents and daemons, HKLM keys, the common Startup
folder) need administrator rights, so they are listed with `can_toggle: false` and a reason
instead of prompting for a password.

## 5e. Permissions

macOS refuses `~/.Trash`, `~/Library/Safari`, `~/Library/Mail` and `~/Library/Messages` to any
process without Full Disk Access. A scan that hits them records an issue and moves on, which
means the reported total is quietly smaller than the truth — unacceptable for a tool whose whole
value is an accurate number.

`PlatformService::permissions` probes those four locations with a plain `read_dir` (no prompt,
no content read) and reports which are blocked plus how to grant access.
`open_privacy_settings` opens the relevant pane. The UI shows a dismissible banner on the
dashboard and above every scan, and Settings lists the state. Windows has no equivalent gate, so
it reports `NotApplicable`.

Both platforms match leftovers by folder name as well, and both refuse to treat a name as proof
of ownership: a `SHARED_FOLDERS` list keeps one application from claiming a vendor umbrella
(`Application Support/Google`, `%APPDATA%\Microsoft`) or a folder the system shares. The
Windows list has to cover one case macOS does not — `%LOCALAPPDATA%\Programs` holds every
per-user installation and cannot be protected, because those applications are what the
uninstaller exists to remove, so the name rule is the only thing standing between it and a
DisplayName that happens to match. Those rules live in `platform/windows/rules.rs`, which is
compiled on every platform (`platform::windows_rules` off Windows) so the half of the product
that never runs during development is not also the untested half.

Leftovers are matched by bundle identifier, which is also why two installed copies of one
application cannot each claim them. `AppDetail::shared_with` names any other installed copy with
the same identifier, and while it is non-empty every leftover is reported `Protected`: the data
belongs to no single copy, and the copy the user asked to remove can still go. Removing the
others and scanning again offers the data normally, so nothing is permanently unreachable.

## 5f. Settings

`settings.json` in the application data directory holds only what the engine needs; theme and
delete mode stay in the frontend because the engine has no use for them. Today that is the list
of folders searched for project artifacts.

Three rules make this safe to get wrong:

- A candidate folder must be absolute, exist, be a directory, and sit **inside the home
  directory** — outside it, every target found would be refused by the safety layer anyway, so
  accepting such a folder would only produce a list of things that cannot be cleaned.
- Nested folders collapse into their parent, so no tree is walked twice.
- Roots that no longer validate are dropped at load time, and if nothing is left the engine
  keeps the guessed folders. A stale or mistyped setting therefore degrades to the previous
  behaviour instead of silently finding nothing.

`AppState` keeps the engine behind an `RwLock<Arc<PruneEngine>>`: changing settings rebuilds it,
while a scan already in flight keeps the `Arc` it started with and finishes against consistent
paths.

## 5g. Command line

The two binaries are named apart on purpose. The command line is `prune`, because that is what
a person types; the desktop app's binary is `prune-desktop`, set in both `src-tauri/Cargo.toml`
and `tauri.conf.json`. When both were called `prune` they wrote to the same
`target/<profile>/prune` and the last build silently won — `pnpm tauri build` followed by
`./target/release/prune scan` opened the GUI and ignored the arguments, and a release cut from
one directory could have shipped the desktop app under the command line's name. Cargo warns
about it, but a warning in the middle of a five-minute build is not a check, so
`scripts/check-binary-names.sh` runs in CI. Nothing a user sees changed: the bundle is still
`Prune.app` and the command is still `prune`.

`crates/prune-cli` is a second front end over the same `PruneEngine`, not a reimplementation: it
calls `scan`, `plan` and `execute` exactly as the desktop app does, so the safety pipeline,
protected paths and operation log are shared.

Two choices belong to the CLI itself, because a terminal has no confirmation dialog:

- `clean` builds a plan and prints it, and only removes anything when `--yes` is given. A dry
  run is the default, not an option.
- `clean` auto-selects `Safe` targets only, or `Low` as well with `--include-low`. Medium and
  higher are reported by `scan` but can only be chosen individually in the desktop app, where
  each path and risk is visible. This is why there is no `--all`.

`run()` takes the engine and a `Write`, which is what lets the tests drive whole commands
against a [`SandboxPlatform`](#) and assert on both the output and the filesystem — including
that a dry run changes nothing and that `node_modules` survives a plain `clean`.

## 5h. Stopping a process

Stopping the wrong process logs the user out or takes the machine down, and unlike a deleted
cache there is no trash to recover from. `system::protection::classify` is therefore a single
pure function, exhaustively tested, that refuses:

- pid 0 and 1 — the kernel and init on every supported platform;
- a list of names that hold the session together (`WindowServer`, `loginwindow`, `launchd`,
  `Finder`, `lsass.exe`, `csrss.exe`, `explorer.exe`, `systemd`, …), matched case-insensitively;
- anything owned by another user, or whose owner cannot be read, because stopping it would need
  rights Prune never asks for;
- Prune itself.

`SystemMonitor::stop_process` applies those rules again at the moment of stopping rather than
trusting the caller: the UI sends a process id, and by the time it arrives that id may belong to
something else. Quitting sends `SIGTERM` so the program can save; forcing sends `SIGKILL`, and
the dialog says which is which.

`crates/prune-core/tests/processes.rs` verifies the whole path for real — it starts a process,
stops it both ways, and checks that pid 1, Prune itself and an unknown id are refused. The only
process those tests stop is the one they started.

## 5i. Turning a scan into advice

`features/dashboard/insights.ts` reads a finished scan and picks at most three observations.
The rule it follows is that a total is not advice: "504 GB reclaimable" only repeats what the
disk gauge already showed, while "21 dormant projects are holding 9.45 GB" says where to start.

Each insight names a view to open and, optionally, a filter to apply there. Two constraints
keep it honest:

- An insight counts only what its destination will show. Safe items exist in both the Cleaner
  and Developer sections, so the safe-space insight picks whichever section holds more of them
  and reports that number — a headline promising more than the page it opens is the same broken
  promise as a button that under-counts its plan.
- A project with no readable activity date is never called dormant.

Following one sets `filter` together with a `filterScope`. The shared filter is otherwise
cleared when a section loads, so that a search typed in Developer does not greet the user with
"nothing matches" in Cleaner; the scope is what lets a filter set deliberately for a
destination survive the trip.

## 6. Tauri layer

Command naming: `<domain>_<verb>_<object>` in `snake_case`.

| Command                                                             | Purpose                                        |
| ------------------------------------------------------------------- | ---------------------------------------------- |
| `app_get_meta`                                                      | version, platform, log path                    |
| `system_get_info` / `system_get_snapshot` / `system_list_processes` | read-only system data                          |
| `cleaner_list_providers`                                            | provider catalogue with availability           |
| `cleaner_start_scan` / `cleaner_cancel_scan` / `cleaner_get_scan`   | scan lifecycle                                 |
| `cleaner_preview` / `cleaner_execute`                               | dry run and removal                            |
| `ops_list`                                                          | operation log                                  |
| `fs_reveal`                                                         | reveal a path in Finder / Explorer (read-only) |

Events: `prune://scan-progress`, `prune://scan-completed`, `prune://cleanup-progress`,
`prune://disk-progress`, `prune://disk-completed`, `prune://apps-progress`,
`prune://apps-completed`.

Two plugins make Prune behave like an application rather than a process: window state, so it
reopens where it was left, and single instance, so a second copy hands over to the first rather
than scanning the same machine twice and disagreeing about what it found. Neither can see
anything of the user's. Single instance is registered only in `run`, never in
`register_plugins`, because it exits the process when another copy exists — which a test
harness must never do.

Nothing is allowed to break permanently either. A thread that panics while holding a `Mutex`
poisons it, and `lock().unwrap()` would then panic on every later use — one bug in one scan
leaving the whole application dead until the user quits and reopens. `sync::LockExt` recovers
instead. That is only sound because of where the locks are: everything they guard is a cache of
scan results, and nothing about removal depends on it, since `SafetyPolicy` re-validates every
path at the moment of deletion rather than trusting anything cached. The frontend has the same
guard in `ErrorBoundary`, because a thrown render error otherwise unmounts the tree and leaves
an empty window that looks, to the person in front of it, exactly like a crash.

Nothing in start-up is allowed to refuse. A bundled application that fails to open has no
console to explain itself, so a data directory that cannot be used costs the user their
settings and history, not the application: the operation log creates its directory on first
write, an unreadable history reads as empty, and `AppState::new` cannot fail.

Long-lived results are bounded. `Recent<T>` keeps only the newest few scan sessions, plans and
disk analyses: an analysis holds one node per directory, so keeping every one of them for the
life of the process was a slow leak. Asking for an evicted scan returns `unknown_scan`, which
the UI already handles. The operation log is bounded the same way but on disk: past
`MAX_BYTES` it is rewritten with the newest `KEEP_RECORDS` entries, so a machine cleaned
regularly for years does not carry a history that has to be read in full every time.

Both files Prune owns are written whole or not at all. Writing in place truncates first, so a
crash or a full disk between the truncate and the write leaves an empty file where the user's
settings or history were — and an empty settings file reads as "no project folders", which
silently changes what a scan covers. `Settings::save` and the log's compaction both write a
`.writing` sibling and rename it over the original, which is atomic on both platforms, and
remove the sibling if the rename fails so the next run has nothing half-written to trip over.

Errors cross IPC as `{ code, message }` (`CommandError`), with stable codes from
`PruneError::code()`.

Long-running work uses `tauri::async_runtime::spawn_blocking`; the UI thread is never blocked.

## 7. Frontend

- **State:** seven zustand stores — `ui` (view, theme, palette), `system` (meta, info,
  snapshot, processes), `scan` (providers, session, selection, plan, result), `disk`, `apps`,
  `startup`, `settings`.
- **IPC:** `lib/tauri.ts` exposes a typed `Backend`; in a plain browser `lib/mock.ts` is used so
  the UI is developable and testable without native code. `?scale=real` makes that mock produce
  what a real machine does — a hundred projects, five hundred artifacts, a hundred caches —
  which is the only way to see the list behave at the size users will meet.
- **Theme:** Tailwind v4 with CSS variable tokens; `.dark` class toggled from the
  system / light / dark preference stored in `localStorage`.
- **Views:** Cleaner and Developer share `CleanerWorkspace`, scoped by category.
- **Rendering at scale:** the results list is the one place where the frontend can be slow, and
  the filter field sits directly above it. Three things keep typing responsive: the filter is
  read through `useDeferredValue`, so the field updates immediately and the list follows;
  `ProviderGroup`, `ProjectSection` and `TargetRow` are memoised, so a render whose scan results
  are unchanged does no work at all; and each group subscribes to its own selected count and
  bytes rather than to the selection itself, so ticking one checkbox does not re-render every
  other group. Without these the median keystroke was 35ms and the worst 157ms at real scale;
  with them, 2ms and 7ms.

## 8. Testing

- Unit tests: safety policy, size calculation, deletion, overlap de-dup, project rules,
  operation log, provider id uniqueness.
- Integration (`tests/pipeline.rs`): a `SandboxPlatform` points `KnownPaths` at a temp dir with
  fake caches, projects, trash, and protected data; the full scan → plan → execute → log flow is
  verified, including tamper resistance (a forged path into `.ssh` is blocked).
- IPC (`src-tauri/tests/ipc.rs`): the real `#[tauri::command]` functions run on Tauri's mock
  runtime, covering argument deserialization, managed state, the async runtime and the JSON
  responses. One test drives a full disk scan → drill-down → preview → execute → operation log
  cycle inside a temp directory; others assert that unknown ids, bad roots and paths outside the
  user's directories are refused. This is why the commands are generic over `Runtime` and why
  command registration lives in `register_commands`, separate from `run`.
- Frontend (vitest + Testing Library): every store's logic — selection, recommendations scoped
  by category, stale-event rejection, preview arguments, and the bookkeeping that removes cleaned
  targets from the session, the disk analysis and the app detail. Component tests cover the two
  behaviours a user's data depends on: a protected target cannot be selected by any route, and a
  dialog traps Tab and restores focus to whatever opened it.

Never use real system paths in tests.

## 9. Roadmap mapping

| Phase               | Spec                                                                    | Status                                                     |
| ------------------- | ----------------------------------------------------------------------- | ---------------------------------------------------------- |
| 1 Foundation        | Tauri 2, React, TS, Vite, Tailwind, navigation, theme, icon, versioning | done                                                       |
| 2 Dashboard         | CPU, memory, disk, OS info, process list                                | done                                                       |
| 3 Cleaner           | caches, logs, temp, trash, scan, preview, safe cleanup, log             | done (system-wide `/Library/Caches` deferred: needs admin) |
| 4 Developer         | tool caches + project artifacts                                         | done, including Docker (§10)                               |
| 5 Disk Analyzer     | tree, large files, visual usage                                         | done                                                       |
| 6 Uninstaller       | apps + related data                                                     | done                                                       |
| 7 Monitor / Startup | monitor + startup items                                                 | done                                                       |

Beyond the phases: a command line over the same engine (§5g), cache locations added from
`providers.json` without rebuilding, and the operation log. What is *not* here is listed under
§11.

## 10. Docker, the one cleanup outside the pipeline

Docker is often the largest single thing on a developer's disk, and the only target Prune
cannot treat as files: images, containers and build cache live inside a disk image the daemon
owns, and deleting that file is a factory reset rather than a cleanup. So `docker/` does not use
`SafetyPolicy`, `CleanupPlan` or the trash at all. It asks the Docker CLI what is reclaimable
and, on confirmation, asks Docker to reclaim it.

Because the usual protections do not apply, three others take their place:

- **Only two commands are reachable**, `docker system prune --force` and
  `docker builder prune --force`. No `-a`, so images in use stay; never `--volumes`, because
  volumes hold databases and other state a developer expects to survive a cleanup. A test
  asserts those flags never appear.
- **The exact command is shown before it runs**, and Docker's own output afterwards, because
  Prune cannot verify or undo what Docker did.
- Volumes appear in the table with their size, marked as kept, and are excluded from the
  headline "reclaimable" figure so that number never promises something Prune will not do.

Command execution goes through a `CommandRunner` trait. The tests inject a fake that records
the argv and replies with real `docker system df` output, so parsing, argument construction,
the missing-Docker case and the stopped-daemon case are all covered without Docker installed.
The same path was exercised against a stub `docker` on `PATH` to confirm the real runner sends
exactly those arguments. **It has not been run against a real Docker daemon**, which is the one
gap left in this feature.

### Being usable without a mouse or a screen

Interactive elements all carry an accessible name (checked across every view), dialogs trap and
restore focus, and the sidebar is a labelled `nav` whose current item is marked `aria-current`.

Three things are announced, and only three, because a live region that reports everything
reports nothing: a failure (`role="alert"` on the error toast — otherwise a failed command is
completely silent), which place a running scan is searching, and what a finished scan found.
The scan's file counter and current path are deliberately left out; they change several times a
second and would talk over the user.

The palette is checked rather than eyeballed. `src/contrast.test.ts` reads the tokens out of
`src/index.css` and asserts that every text colour clears WCAG AA (4.5:1) against every surface
it can appear on, in both themes, and that the three levels of text stay far enough apart to
still read as a hierarchy. The faintest level carries file paths and dates, so it is content and
gets the content threshold.

### Cost of being open

A system monitor that is expensive to run is self-defeating, so the live readings are priced
individually rather than taken together every tick:

| reading | cost on a 2,600-process machine | how often |
| --- | --- | --- |
| CPU, memory, network | under 1ms | every poll |
| process table walk | ~60ms | at most every 4s, and reused by the process list |
| mounted volumes | ~140ms | at most every 10s, or at once after a removal |
| process list with details | ~55ms | only while the Monitor view is open |

That takes a snapshot from 439ms to under a millisecond in the steady state. On top of it,
`startPolling` in the frontend stops the timers entirely while the window is hidden and takes a
fresh reading when it returns, because a monitor nobody can see has nothing to report.

Measured on the built application on the same 2,600-process machine: 0.0% behind another
window, a few percent on the Dashboard, and 0.7–9.9% with the Monitor view open and listing
forty processes — which includes the WebView redrawing live meters, not only the readings.

## 11. Known limitations

- macOS: `~/.Trash`, Safari, Mail and Messages are TCC-protected. Prune detects this and says so
  (see §5e), but cannot read them until the user grants Full Disk Access and restarts the app.
- Windows: Recycle Bin size is not path-based and is not reported yet.
- Sizes are logical bytes, not on-disk blocks (APFS clones and compression make "space freed"
  slightly lower than shown).
- The disk analyzer walks a single thread, at roughly `du`'s speed (10.5s against 8.1s on a
  190k-file tree, while also building a drill-down tree, a largest-files list and per-extension
  statistics). A 7.2M-file, 994k-folder tree takes ~590s and peaks at 142 MB. The tree itself is
  what is held afterwards, so it stores no paths: a node knows its name and its parent, and a
  full path is rebuilt by walking up when one is needed. Keeping a path on every node and a
  `HashMap<PathBuf, usize>` beside it cost 564 MB on that same tree — four times as much, for an
  answer the tree already contained.
- A full scan of this developer machine (6.4M files, 536 GB reclaimable across 114 projects)
  takes 189s and peaks at 31 MB resident. The time is nearly all kernel time spent on
  `readdir`/`stat` — 452s of system time against 22s of user time, so the work is spread over
  the cores and then waits on the filesystem. That is close to the floor: on one 190k-file tree
  `du -sh` takes 8.1s and `prune disk`, which also builds a directory tree, a largest-files list
  and per-extension statistics, takes 10.5s. Further speedups would have to come from not
  walking, i.e. caching, which would trade accuracy for time — the wrong trade for a tool whose
  value is an accurate number. Memory is not the constraint and is not expected to become one:
  nothing holds a file list, only running totals per directory.
