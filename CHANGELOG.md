# Changelog

All notable changes to Prune are documented here. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/) and the project uses
[Semantic Versioning](https://semver.org/).

## [Unreleased]

### Added

- Tauri 2 + React + TypeScript + Tailwind foundation with dark/light theme and sidebar navigation (Phase 1).
- `prune-core` crate: safety policy (whitelisted roots + protected paths), size calculation, trash/permanent removal, parallel provider scanning with progress and cancellation, overlap de-duplication, JSON-lines operation log.
- Dashboard with live storage / memory / CPU and potential cleanup summary (Phase 2).
- Cleaner: application caches, logs, temporary files, Trash, old installers, Chromium-family and Firefox caches (Phase 3).
- Developer: npm, pnpm, Yarn, Bun, pip, uv, Gradle, Maven, Cargo, Go, NuGet, CocoaPods, Homebrew, Xcode DerivedData / Archives / Device Support, Simulator caches, and a project artifact finder (`node_modules`, `target/`, `build/`, `dist/`, Python caches, virtualenvs, …) (Phase 4, initial).
- Disk analyzer: single-pass directory tree with drill-down, largest files with size filters, file type statistics, cancellable; large files feed the same preview → remove pipeline (Phase 5).
- Uninstaller: application discovery (macOS `.app` bundles, Windows uninstall registry), leftover data matched by bundle identifier and bundle name, sizes measured in parallel, and removal through the same preview pipeline. System applications are listed with a protected bundle so only their leftovers can go (Phase 6).
- Monitor: CPU per core, memory, swap, disks, top processes (read-only).
- Preview (dry run) → confirm → remove → operation log pipeline with `Move to Trash` / `Delete permanently` modes.
- Command palette (⌘K / Ctrl+K).
- Read-only CLI prototypes: `cargo run -p prune-core --example scan`, `--example disk`, `--example apps`.
