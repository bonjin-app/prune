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
- Monitor: CPU per core, memory, swap, disks, top processes (read-only).
- Preview (dry run) → confirm → remove → operation log pipeline with `Move to Trash` / `Delete permanently` modes.
- Command palette (⌘K / Ctrl+K).
- Read-only CLI prototype: `cargo run -p prune-core --example scan`.
