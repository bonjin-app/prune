# Prune — agent notes

Open-source, local-first system utility for macOS and Windows (Tauri 2 + React + Rust).
Read `docs/ARCHITECTURE.md` before changing anything in `crates/prune-core`.

## Commands

```bash
pnpm install && pnpm tauri dev            # desktop app
pnpm dev                                  # UI in browser with mock backend
pnpm typecheck && pnpm lint && pnpm test  # frontend checks
cargo fmt --all --check && cargo clippy --workspace --all-targets -- -D warnings && cargo test --workspace
cargo run -p prune-core --example scan --release   # read-only real scan
cargo run -p prune-core --example disk --release -- ~/Projects
cargo run -p prune-core --example apps --release -- "Visual Studio Code"
cargo run -p prune-core --example startup --release
```

## Hard rules

- Deletion only via `SafetyPolicy::validate` → `fs::remove`. Never add another removal path.
- Frontend never sends filesystem paths for deletion; it sends target ids / plan ids.
- No network, telemetry, accounts, or background daemons.
- Tests use temp dirs only. Never touch real system paths in tests.
- OS-specific paths go in `crates/prune-core/src/platform/{macos,windows}`, nowhere else.
- New Tauri command: `<domain>_<verb>_<object>`, register in `src-tauri/src/lib.rs`, add to
  `src/lib/tauri.ts` (+ `src/lib/mock.ts`) and, if a new model, mirror it in
  `src/types/models.ts` (camelCase fields, snake_case enum values).
- New provider: prefer a `SimpleProvider` const, register in `providers/registry.rs` or
  `developer::ALL`; keep provider ids unique and stable.
