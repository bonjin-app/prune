# Changelog

All notable changes to Prune are documented here. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/) and the project uses
[Semantic Versioning](https://semver.org/).

## [Unreleased]

### Fixed

- Prune refused to start when its data directory could not be used. A bundled app has no console, so it simply did not open and nothing explained why. Opening the operation log can no longer fail: the directory is created on first write, an unreadable history reads as empty, and the application starts either way — losing the history is a degraded state, not a reason to refuse to run.

- `prune clean` emptied the Trash while promising to move things there. Items already in the trash can only be deleted outright, so a run that reads as reversible must leave them alone; it now does, and says why instead of reporting "nothing to remove".
- Uninstalling an application could offer to delete a vendor folder several products share. Removing one Google application would have taken Chrome's profiles and Drive's data with it. Vendor folders are never claimed now, but the product folder inside one still is, so Google Chrome correctly reports `Application Support/Google/Chrome` and leaves `Application Support/Google` alone.
- Old installers no longer match `.zip` on Windows. A zip is as likely to be a download worth keeping as an installer.
- A cleanup that succeeded but could not be written to the operation log was reported as a failure. The files were already gone, so the user was told the opposite of what happened and might have run it again. The removal now succeeds and the result carries the logging problem, which the result dialog shows.
- Scan sessions, cleanup plans and disk analyses were kept for the life of the process. An analysis of a home directory holds one node per directory — tens of thousands on a real machine — so a session spent scanning a few times grew steadily. Only the most recent few are kept now.
- Revealing a path in the file manager accepted anything under `/private`, so `/etc/hosts` was refused while `/private/etc/hosts`, the same file, was allowed. The check now compares canonical paths against the directories Prune actually works in.
- The Cleaner and Developer sections share one selection, but the Review button built its plan from everything selected anywhere. Selecting in Developer and then reviewing in Cleaner produced a plan bigger than the number on the button. Each section now plans only what it shows.

### Added

- Tauri 2 + React + TypeScript + Tailwind foundation with dark/light theme and sidebar navigation (Phase 1).
- `prune-core` crate: safety policy (whitelisted roots + protected paths), size calculation, trash/permanent removal, parallel provider scanning with progress and cancellation, overlap de-duplication, JSON-lines operation log.
- Dashboard with live storage / memory / CPU and potential cleanup summary (Phase 2).
- Cleaner: application caches, logs, temporary files, Trash, old installers, Chromium-family and Firefox caches (Phase 3).
- Developer: npm, pnpm (store plus registry metadata and dlx), Yarn, Bun, node-gyp, Deno, Playwright, Puppeteer, Cypress, Electron, pip, uv, Gradle, Maven, Cargo, Go, NuGet, CocoaPods, Swift Package Manager, pub, RubyGems, Composer, JetBrains, Homebrew, Xcode DerivedData / Archives / Device Support, Simulator caches, and a project artifact finder (`node_modules`, `target/`, `build/`, `dist/`, Python caches, virtualenvs, …) (Phase 4).
- Disk analyzer: single-pass directory tree with drill-down, largest files with size filters, file type statistics, cancellable; large files feed the same preview → remove pipeline (Phase 5).
- Uninstaller: application discovery (macOS `.app` bundles, Windows uninstall registry), leftover data matched by bundle identifier and bundle name, sizes measured in parallel, and removal through the same preview pipeline. System applications are listed with a protected bundle so only their leftovers can go (Phase 6).
- Startup manager: launch agents and daemons on macOS (with launchd's override database deciding the real state), `Run` registry keys and Start Menu Startup folders on Windows. Toggling is reversible and writes only to the override database / `StartupApproved`; nothing is deleted and items shared by all users stay read-only (Phase 7).
- Docker: what it is holding (images, containers, volumes, build cache) and two conservative ways to reclaim space. This is the one cleanup that does not go through Prune's pipeline, because the data lives inside Docker rather than in files Prune may touch, so the exact command is shown before it runs and Docker's own output is shown afterwards. `docker system prune` is used without `-a` and never with `--volumes`, so images in use and all volumes survive.
- Monitor: CPU per core, memory, swap, disks, network throughput, and top processes. A process can be asked to quit or forced to stop, with the same rule as everywhere else — Prune refuses what it must not touch: the kernel and init, anything that holds the session together (WindowServer, loginwindow, Finder, lsass.exe, explorer.exe and friends), processes owned by another user, and itself. The rules are re-applied at the moment of stopping, not trusted from the caller, and are also available as `prune processes --stop`.
- Preview (dry run) → confirm → remove → operation log pipeline with `Move to Trash` / `Delete permanently` modes.
- A stopped scan now says its results are partial instead of presenting a smaller total as the whole picture.
- Releasing is documented and scripted: `scripts/set-version.sh` writes the version everywhere, and tagging builds the desktop bundles and the CLI for macOS and Windows into a draft release.
- The window reopens where it was left, and a second copy hands over to the one already running instead of scanning the same machine twice.
- Cache locations can be added without rebuilding. `providers.json` in the application data directory is read at start-up and its entries join the built-in providers, with `~` and `{cache}`-style locations so one file serves both platforms. A file only chooses where to look: every path it produces goes through the same safety layer, ids cannot shadow a built-in one, and a malformed entry is reported rather than silently skipped, because a provider that quietly failed to load looks exactly like one that found nothing.
- The dashboard has an opinion. A "Worth a look" card reads the scan and names the few things a total cannot say — dormant projects and what they hold, what can go with nothing at stake, a single directory that dwarfs the rest — and each one opens the section where it can be acted on, already filtered. It counts only what that destination will show, so the headline, the button and the plan all agree. Nothing there removes anything.
- Project artifacts are grouped by the project they belong to, with how long since anyone worked on it. A real machine produced 797 artifact directories; as 112 project rows, sorted by size and marked when nothing has touched them in three months, the list becomes something a developer can act on. The project is the nearest ancestor holding a `.git` directory, so a monorepo's scattered build folders count as one project, and the date comes from that repository's own metadata — an unknown date is reported as unknown, never as very old.
- Scan results can be filtered by name, path or kind and narrowed to safe items only, and are listed biggest first. With thirty providers and hundreds of project artifacts, the list needed a way in.
- Dashboard's Review & Clean arrives with the safe items already ticked.
- Command palette (⌘K / Ctrl+K) and ⌘/Ctrl + digit shortcuts for every view.
- Project folders are configurable: the developer scan searches the folders you name instead of the ones Prune guesses, persisted in `settings.json` next to the operation log. Folders must be inside your home directory, since that is the only place Prune can remove anything, and an unusable setting falls back to the guessed folders rather than silently disabling the scan. The Developer view says which folders it is searching and whether they were guessed.
- Permission awareness on macOS: locations the OS hides (Trash, Safari data, Mail, Messages) are detected and reported with a banner and a shortcut to the Full Disk Access settings, instead of silently shrinking every scan total.
- The project artifact walker measures what it found in parallel as well, cutting that scan from ~145s to ~100s on a machine with 789 artifacts and 4.1M files.
- Directory sizing is split across threads: the tree is divided into at least 32 independent subtrees (descending up to four levels to find them) and walked in parallel, with hard links still counted once through a sharded inode set. On a 12-core machine a 622k-file pnpm store drops from ~50s to ~20s and a 368k-file Gradle cache from 38s to 23s, with byte-identical results.
- Frontend tests for the selection, preview, execute and cleanup-bookkeeping logic in every store, plus component tests asserting that a protected target can never be selected and that dialogs trap and restore focus.
- IPC integration tests on Tauri's mock runtime covering the real command surface, including a full disk scan → preview → execute → operation log cycle.
- A `prune` command line built on the same engine: `status`, `providers`, `scan`, `clean`, `disk`, `apps`, `startup` and `log`, each with `--json` for scripting and meaningful exit codes. `clean` is a dry run unless `--yes` is passed and never selects anything riskier than low on its own. The read-only example prototypes it replaces have been removed.
