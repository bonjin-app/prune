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
for UI work. Add `?scale=real` to that URL and the mock produces what a real machine does — a
hundred projects, five hundred artifacts, a hundred caches — which is the size any change to
the results list should be judged at; the handful of sample rows the mock shows by default will
make almost anything look fast. `cargo run -p prune-cli -- scan` runs a read-only scan in the
terminal.

## Checks

Run everything CI runs before opening a pull request:

```bash
pnpm typecheck && pnpm lint && pnpm test
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
./scripts/check-no-network.sh
./scripts/check-binary-names.sh
```

That last one is not a formality. Prune's whole claim is that it never talks to a network, and
the script refuses any HTTP client or TLS stack in the workspace and anything socket-capable at
all in the engine, printing the dependency path that introduced it. `pnpm test` makes the same
check of the interface. If you need something the check refuses, it needs a decision, not a
dependency bump.

## Project Layout

```
crates/prune-core   Engine: models, safety, fs, providers, scan, analyzer, apps, ops, platform
crates/prune-cli    The `prune` command line, built on the same engine
src-tauri           Tauri shell: IPC commands + events only
src                 React UI: features/, stores/, components/ui, lib/tauri.ts (only IPC entry)
docs                Architecture notes
```

See [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md) for details.

## Adding a cache location without touching the code

Most additions do not need Rust at all. `providers.json` in the application data directory
(`~/Library/Application Support/app.bonjin.prune/` on macOS,
`%APPDATA%\app.bonjin.prune\` on Windows) is read at start-up:

```json
{
  "providers": [
    {
      "id": "zig_cache",
      "name": "Zig cache",
      "description": "Compiler cache. Rebuilt on the next build.",
      "category": "developer_files",
      "risk": "safe",
      "mode": "whole",
      "paths": ["{cache}/zig", "~/.cache/zig"]
    }
  ]
}
```

`paths` understands `~` and `{home}`, `{cache}`, `{appSupport}`, `{localAppData}`, `{temp}`,
`{downloads}` and `{logs}`; a location the current platform does not have is skipped, so one
file can serve macOS and Windows. `mode` is `whole` (each path is one item) or `children` (each
entry inside it is). `minAgeDays` and `exclude` are optional.

A file cannot bypass anything: every path it produces goes through the same safety layer, and
every target still needs a preview and a confirmation. Ids must not collide with a built-in
one, and a malformed entry is reported rather than silently skipped.

If the tool is one most developers have, please open a pull request adding it as a built-in
provider instead, using the section below.

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
