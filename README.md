<p align="center">
  <img src="src-tauri/icons/128x128@2x.png" width="96" alt="Prune icon" />
</p>

<h1 align="center">Prune</h1>

<p align="center">
  <strong>Keep what matters. Remove what doesn't.</strong><br />
  An open-source system utility for macOS and Windows.
</p>

<p align="center">
  <a href="https://github.com/bonjin-app/prune/releases">Download</a> ·
  <a href="https://github.com/bonjin-app/prune">GitHub</a> ·
  <a href="docs/ARCHITECTURE.md">Architecture</a> ·
  <a href="CONTRIBUTING.md">Contributing</a>
</p>

<p align="center">
  🧹 Clean &nbsp; 🗑 Uninstall &nbsp; 📊 Analyze &nbsp; 💻 Developer &nbsp; 📈 Monitor &nbsp; ⚡ Startup
</p>

<p align="center">
  <code>100% Local</code> &nbsp; <code>No Account</code> &nbsp; <code>No Telemetry</code> &nbsp; <code>Open Source</code>
</p>

---

## What is Prune?

Prune helps you understand and manage your own computer. Like pruning a tree, it finds the
dead branches — stale caches, temporary files, build leftovers, unused installers — shows you
exactly what they are, and lets **you** decide what to cut. Nothing is ever removed without a
preview and a confirmation.

Prune is not a "one-click cleaner". It is a **developer-first system utility** that treats
safety as the primary feature.

## Why Prune?

- **Developer cleanup as a first-class feature.** `node_modules`, Rust `target/`, Gradle and
  Maven caches, Xcode DerivedData and Archives, pnpm / npm / Yarn / Bun stores, Cargo, pip, uv,
  Go, NuGet, CocoaPods, Homebrew, Simulator caches, `build/`, `dist/`, Python caches and
  virtualenvs — found and sized, per project.
- **Safe by default.** A whitelist of removable roots, a list of protected paths, a mandatory
  dry run, Trash before permanent deletion, and a local operation log. See
  [SECURITY.md](SECURITY.md) for the guarantees.
- **Local and private.** No server, no account, no cloud, no telemetry, no network code at all.
- **Native.** Tauri 2 + Rust. Small binary, low memory, fast parallel scans with progress and
  cancellation.
- **Cross-platform.** One UI for macOS and Windows; OS differences live in a small platform layer.

## Features

| Area            | Status     | What it does                                                                     |
| --------------- | ---------- | -------------------------------------------------------------------------------- |
| Dashboard       | ✅         | Storage, memory, CPU at a glance; reclaimable space by category; recent activity |
| Cleaner         | ✅         | Application caches, logs, temporary files, Trash, old installers, browser caches |
| Developer       | ✅         | Global tool caches + project artifact finder with per-item risk levels           |
| Monitor         | ✅         | Live CPU per core, memory, swap, disks, top processes                            |
| Command palette | ✅         | `⌘K` / `Ctrl+K` to jump anywhere or start a scan                                 |
| Disk analyzer   | 🔜 Phase 5 | Directory tree, large file finder, visual usage                                  |
| Uninstaller     | 🔜 Phase 6 | Apps with their caches, preferences, containers                                  |
| Startup manager | 🔜 Phase 7 | Login items and launch agents, enable / disable                                  |
| Docker cleanup  | 🔜         | Images, containers, volumes, build cache                                         |
| CLI             | 🔜         | `prune scan`, `prune clean --developer`, … (read-only prototype available)       |

Every removable item carries a risk level:

```
Safe        regenerated automatically, no user state      browser cache, npm cache, logs
Low Risk    cheap to rebuild                              node_modules, build directories
Medium Risk may hold state you care about                 Xcode archives, virtualenvs
High Risk   shown, never pre-selected
Protected   never removable through Prune
```

## Screenshots

_Coming with the first release._

## Supported Platforms

| Platform | Architectures        | Minimum               |
| -------- | -------------------- | --------------------- |
| macOS    | Apple Silicon, Intel | macOS 12              |
| Windows  | x64                  | Windows 10 (WebView2) |

## Installation

Pre-built installers (`.dmg`, `.msi`, `.exe`) will be published on
[GitHub Releases](https://github.com/bonjin-app/prune/releases). Homebrew, WinGet and Scoop
are planned.

Until then, build from source (below).

## Development

Prerequisites: Rust stable, Node 20+, pnpm, and the
[Tauri 2 prerequisites](https://tauri.app/start/prerequisites/) for your OS.

```bash
pnpm install
pnpm tauri dev          # desktop app with hot reload
pnpm dev                # UI only, in the browser, against a mock backend
cargo run -p prune-core --example scan --release   # read-only cleanup scan in the terminal
cargo run -p prune-core --example disk --release -- ~/Projects   # read-only disk analysis
cargo run -p prune-core --example apps --release -- "Visual Studio Code"   # read-only app leftovers
```

Checks:

```bash
pnpm typecheck && pnpm lint && pnpm test
cargo fmt --all --check && cargo clippy --workspace --all-targets -- -D warnings && cargo test --workspace
```

## Architecture

```
┌────────────────────────────────────────────┐
│  Prune UI   React · TypeScript · Tailwind  │
└──────────────────────┬─────────────────────┘
                       │  Tauri commands + events (ids only, never paths for deletion)
┌──────────────────────▼─────────────────────┐
│  src-tauri   thin IPC adapter              │
├────────────────────────────────────────────┤
│  prune-core  (pure Rust, no Tauri, no net) │
│   providers → scan → safety → fs → ops     │
│   platform/{macos,windows}   system        │
└──────────────────────┬─────────────────────┘
                 macOS │ Windows
```

Cleanup pipeline:

```
Discovery → Path Validation → Risk Classification → User Confirmation → Dry Run → Delete → Operation Log
```

Details, module map, IPC naming and the provider interface: [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md).

## Security

Prune deletes files, so every bug that could remove the wrong thing is treated as a security
issue. The guarantees and the private reporting process are in [SECURITY.md](SECURITY.md).

## Contributing

Issues and pull requests are welcome — especially new cleanup providers, which are usually a
single declaration. Start with [CONTRIBUTING.md](CONTRIBUTING.md).

## Acknowledgements

Prune was inspired by [Mole](https://github.com/tw93/Mole) and the idea that a cleaner should
be a tool you understand, not a button you trust.

## License

[MIT](LICENSE)
