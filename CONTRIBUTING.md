# Contributing to Prune

Thanks for helping build a system utility people can trust. This guide covers setup,
conventions and the safety rules every change must respect.

## Prerequisites

- Rust stable (1.80+) via `rustup`
- Node 20+ and `pnpm` (`corepack enable` works)
- Tauri 2 prerequisites for your OS: <https://tauri.app/start/prerequisites/>
  - macOS: Xcode command line tools
  - Windows: Visual Studio Build Tools (C++), WebView2 (preinstalled on Windows 11)

## Setup

```bash
pnpm install
pnpm tauri dev
```

`pnpm dev` alone opens the UI in a browser against an in-memory mock backend, which is handy
for UI work. `cargo run -p prune-cli -- scan` runs a read-only scan in the terminal.

## Checks

Run everything CI runs before opening a pull request:

```bash
pnpm typecheck && pnpm lint && pnpm test
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

## Project Layout

```
crates/prune-core   Engine: models, safety, fs, providers, scan, analyzer, apps, ops, platform
crates/prune-cli    The `prune` command line, built on the same engine
src-tauri           Tauri shell: IPC commands + events only
src                 React UI: features/, stores/, components/ui, lib/tauri.ts (only IPC entry)
docs                Architecture notes
```

See [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md) for details.

## Adding a Cleanup Provider

Most providers are one declaration in `crates/prune-core/src/providers/`:

```rust
pub const DENO_CACHE: SimpleProvider = SimpleProvider::new(
    "deno_cache",                 // stable id, snake_case
    "Deno cache",                 // display name
    Category::DeveloperFiles,
    "Downloaded modules and compiled artifacts.",
    RiskLevel::Safe,
    |k| whole([cache(k, "deno"), Some(k.home.join(".cache/deno"))]),
);
```

Then add it to `developer::ALL` (or `registry::default_providers`). Rules:

- Use `KnownPaths` for locations; never hard-code `/Users/...` or `C:\Users\...`.
- Choose the risk honestly. `Safe` means "regenerated automatically, contains no user state".
- Only report directories the tool re-creates. Never report configuration or credentials.
- Add a case to `crates/prune-core/tests/pipeline.rs` if the provider has custom logic.

## Safety Rules for Code Review

A pull request is rejected if it:

- Adds a removal path that bypasses `SafetyPolicy::validate`.
- Accepts a filesystem path from the frontend for deletion.
- Widens `allowed_roots` or removes an entry from `ProtectedPaths` without a linked issue.
- Adds network access, telemetry, or a background daemon.
- Uses real system paths in tests.

## Commit Messages

Conventional Commits (`feat:`, `fix:`, `docs:`, `refactor:`, `test:`, `chore:`). Keep the
subject under 72 characters.

## Releasing

See [RELEASING.md](RELEASING.md).

## License

By contributing you agree that your contributions are licensed under the MIT License.
