# Prune Architecture

This document is the design reference for contributors. It records the decisions made before
the first line of code (spec §39) and the reasoning behind them.

## 1. Principles

| Principle                   | Consequence in code                                                                                     |
| --------------------------- | ------------------------------------------------------------------------------------------------------- |
| Local-first / Privacy-first | No network crate anywhere in the workspace. Operation log is JSON lines on disk.                        |
| Safe-by-default             | All deletion goes through `SafetyPolicy::validate` → `fs::remove`. Only `ValidatedPath` can be removed. |
| Developer-first             | Developer providers are the largest provider group and have their own view.                             |
| CLI-ready                   | `prune-core` has no Tauri dependency. `examples/scan.rs` is the CLI prototype.                          |
| Native                      | Rust owns all OS access. React never touches the filesystem.                                            |

## 2. Workspace layout

```
prune/
├── Cargo.toml                 workspace: crates/prune-core, src-tauri
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
│   │   ├── analyzer/          disk analyzer: directory tree, large files, extensions
│   │   ├── apps/              uninstaller: app detail + leftovers as cleanup targets
│   │   ├── ops/               OperationLog (JSONL)
│   │   ├── platform/          PlatformService trait; macos/, windows/, generic/
│   │   └── system/            SystemMonitor (sysinfo)
│   ├── examples/{scan,disk,apps,startup}.rs  read-only CLI prototypes
│   └── tests/pipeline.rs      end-to-end against a sandboxed fake home
├── src-tauri/                 Tauri shell
│   ├── src/commands/          app, system, cleaner, disk, apps, startup, ops, fs
│   ├── src/state.rs           AppState: engine, monitor, ops, scans, plans, disks, apps, startup
│   ├── tauri.conf.json        window, CSP, bundle
│   └── capabilities/          core permissions only (no shell / fs / http plugins)
└── src/                       React UI
    ├── lib/tauri.ts           the ONLY IPC entry point (+ browser mock in lib/mock.ts)
    ├── types/models.ts        TS mirror of Rust models
    ├── stores/                zustand: ui, system, scan
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

### SafetyPolicy rules (in order)

1. Absolute path, no `.` / `..` components, not a filesystem root.
2. Must exist (`symlink_metadata`; the leaf symlink is never followed).
3. Must be strictly inside an **allowed root** (home, temp). Whitelist, not blacklist.
4. Must not equal an **exact protected** path or be an ancestor of one
   (`~`, `~/Library`, `~/Documents`, `~/Desktop`, `~/Downloads`, …).
5. Must not be inside a **protected tree**
   (`/System`, `/usr`, `~/.ssh`, `~/Library/Keychains`, `C:\Windows`, `%APPDATA%\Microsoft\Credentials`, …).

The lists live in `platform/{macos,windows}` and are canonicalized at policy creation.

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

**Overlap de-duplication.** A generic provider (`user_cache`, priority 10) may report
`~/Library/Caches/Homebrew` while `homebrew_cache` (priority 60) reports the same directory.
After all providers finish, `scan::dedupe_overlaps` keeps the higher-priority target for any
equal / ancestor / descendant pair, so no byte is offered twice.

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
bundle. macOS matches on the bundle identifier (`~/Library/Caches/<id>`, `Containers`, `WebKit`,
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

Errors cross IPC as `{ code, message }` (`CommandError`), with stable codes from
`PruneError::code()`.

Long-running work uses `tauri::async_runtime::spawn_blocking`; the UI thread is never blocked.

## 7. Frontend

- **State:** three zustand stores — `ui` (view, theme, palette), `system` (meta, info,
  snapshot, processes), `scan` (providers, session, selection, plan, result).
- **IPC:** `lib/tauri.ts` exposes a typed `Backend`; in a plain browser `lib/mock.ts` is used so
  the UI is developable and testable without native code.
- **Theme:** Tailwind v4 with CSS variable tokens; `.dark` class toggled from the
  system / light / dark preference stored in `localStorage`.
- **Views:** Cleaner and Developer share `CleanerWorkspace`, scoped by category.

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
- Frontend: vitest for formatting helpers; the mock backend enables UI testing.

Never use real system paths in tests.

## 9. Roadmap mapping

| Phase               | Spec                                                                    | Status                                                     |
| ------------------- | ----------------------------------------------------------------------- | ---------------------------------------------------------- |
| 1 Foundation        | Tauri 2, React, TS, Vite, Tailwind, navigation, theme, icon, versioning | done                                                       |
| 2 Dashboard         | CPU, memory, disk, OS info, process list                                | done                                                       |
| 3 Cleaner           | caches, logs, temp, trash, scan, preview, safe cleanup, log             | done (system-wide `/Library/Caches` deferred: needs admin) |
| 4 Developer         | tool caches + project artifacts                                         | done, Docker pending                                       |
| 5 Disk Analyzer     | tree, large files, visual usage                                         | next                                                       |
| 6 Uninstaller       | apps + related data                                                     | planned                                                    |
| 7 Monitor / Startup | monitor done; startup items planned                                     | partial                                                    |

## 10. Deferred: Docker

Spec §27 asks for Docker cleanup. Meaningful Docker reclamation (images, containers, volumes,
build cache) lives inside Docker's own VM disk image, so it cannot go through the file-deletion
pipeline: it needs `docker system df` / `docker system prune`, which is a different kind of
operation with different failure modes. Deleting `Docker.raw` directly would be a factory reset,
not a cleanup, so it is deliberately not offered as a checkbox next to caches. The feature is
deferred until it can be designed and verified as a command-based provider with its own
confirmation step.

## 10b. Known limitations

- macOS: `~/.Trash` and Safari's cache are TCC-protected. Prune reports them as skipped until
  the user grants Full Disk Access; we do not prompt for it yet.
- Windows: Recycle Bin size is not path-based and is not reported yet.
- Sizes are logical bytes, not on-disk blocks (APFS clones and compression make "space freed"
  slightly lower than shown).
- A full scan of this developer machine (6.1M files, 504 GB reclaimable) takes about 2 minutes,
  dominated by the project-artifact walk. Individual providers are parallel; the remaining cost
  is filesystem metadata throughput.
