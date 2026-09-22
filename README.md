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
  <a href="https://github.com/bonjin-app/prune/releases/latest"><img alt="Latest release" src="https://img.shields.io/github/v/release/bonjin-app/prune?style=flat-square&color=15803d" /></a>
  <a href="https://github.com/bonjin-app/prune/actions/workflows/ci.yml"><img alt="CI" src="https://img.shields.io/github/actions/workflow/status/bonjin-app/prune/ci.yml?branch=main&style=flat-square&label=CI" /></a>
  <a href="LICENSE"><img alt="MIT licence" src="https://img.shields.io/github/license/bonjin-app/prune?style=flat-square" /></a>
  <img alt="Platforms" src="https://img.shields.io/badge/macOS%20%7C%20Windows-supported-informational?style=flat-square" />
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

- **Extensible without a rebuild.** Point Prune at a cache it does not know by adding it to
  `providers.json`; it joins the built-in list with the same safety rules. See
  [CONTRIBUTING.md](CONTRIBUTING.md).
- **Developer cleanup as a first-class feature.** Thirty-odd tool caches — npm, pnpm, Yarn, Bun,
  node-gyp, Deno, Playwright, Puppeteer, Cypress, Electron, Gradle, Maven, Cargo, Go, NuGet,
  CocoaPods, Swift Package Manager, pub, RubyGems, Composer, JetBrains, Homebrew, Xcode
  DerivedData and Archives, Simulator caches — plus project artifacts found per project:
  `node_modules`, Rust `target/`, `build/`, `dist/`, Python caches and virtualenvs. Point it
  at the folders where your code actually lives.
- **Safe by default.** A whitelist of removable roots, a list of protected paths, a mandatory
  dry run, Trash before permanent deletion, and a local operation log. See
  [SECURITY.md](SECURITY.md) for the guarantees.
- **Local and private.** No server, no account, no cloud, no telemetry, no network code at all.
- **Native.** Tauri 2 + Rust. Small binary, low memory, fast parallel scans with progress and
  cancellation.
- **Cross-platform.** One UI for macOS and Windows; OS differences live in a small platform layer.

## Features

| Area            | Status     | What it does                                                                           |
| --------------- | ---------- | -------------------------------------------------------------------------------------- |
| Dashboard       | ✅         | Storage, memory, CPU at a glance; reclaimable space by category; recent activity       |
| Cleaner         | ✅         | Application caches, logs, temporary files, Trash, old installers, browser caches       |
| Developer       | ✅         | Global tool caches + project artifact finder with per-item risk levels                 |
| Monitor         | ✅         | Live CPU per core, memory, swap, disks, top processes                                  |
| Command palette | ✅         | `⌘K` / `Ctrl+K` to jump anywhere or start a scan                                       |
| Disk analyzer   | ✅         | Directory tree with drill-down, largest files, file type statistics, cancellable       |
| Uninstaller     | ✅         | Apps with their caches, preferences and containers, matched by bundle identifier       |
| Startup manager | ✅         | Login items and launch agents, enable / disable without deleting anything              |
| Docker          | ✅         | What Docker is holding, and two conservative prune commands. Volumes are never touched |
| CLI             | ✅         | `prune scan`, `clean`, `disk`, `apps`, `startup`, `docker`, `processes`, `log`, `--json` |

Every removable item carries a risk level:

```
Safe        regenerated automatically, no user state      browser cache, npm cache, logs
Low Risk    cheap to rebuild                              node_modules, build directories
Medium Risk may hold state you care about                 Xcode archives, virtualenvs
High Risk   removal is likely to break something
Protected   never removable through Prune
```

Only `Safe` items are ever ticked for you, and only when you ask — by pressing **Select safe
items**, or `prune clean` without `--include-low`. Everything else you choose yourself.

There is one exception in the other direction. A few things cannot be moved to the Trash at
all — what is already *in* the Trash, for one — so they are deleted outright whatever mode is
chosen. Those are left unticked while the mode is "Move to Trash", because a recommendation
that reads as reversible should not contain the one item that is not.

## Screenshots

<p align="center">
  <img src="docs/screenshots/dashboard.png" width="880" alt="Dashboard: storage, memory and CPU with reclaimable space by category" />
</p>

<table>
  <tr>
    <td width="50%">
      <img src="docs/screenshots/developer.png" alt="Developer: package manager caches and project artifacts with risk levels" /><br />
      <sub><b>Developer</b> — tool caches and project artifacts, each with a risk level</sub>
    </td>
    <td width="50%">
      <img src="docs/screenshots/disk.png" alt="Disk: largest files with size filters" /><br />
      <sub><b>Disk</b> — folder sizes, largest files, file types</sub>
    </td>
  </tr>
  <tr>
    <td width="50%">
      <img src="docs/screenshots/uninstaller.png" alt="Uninstaller: an application with its caches, preferences and containers" /><br />
      <sub><b>Uninstaller</b> — the app plus everything it left behind</sub>
    </td>
    <td width="50%">
      <img src="docs/screenshots/monitor.png" alt="Monitor: per-core CPU, memory, disks and top processes" /><br />
      <sub><b>Monitor</b> — per-core CPU, memory, disks, processes</sub>
    </td>
  </tr>
</table>

<sub>Captured from `pnpm dev`, which runs the interface against built-in sample data so the
screenshots do not expose a real machine. The numbers shown are made up; everything else is the
real interface.</sub>

## Supported Platforms

| Platform | Architectures        | Minimum               |
| -------- | -------------------- | --------------------- |
| macOS    | Apple Silicon, Intel | macOS 12              |
| Windows  | x64                  | Windows 10 (WebView2) |

## Download

Pick the one for your machine. Every file is built by GitHub Actions from the tagged commit —
nothing is uploaded from anyone's laptop.

| Platform                   | File                                                                                                            |
| -------------------------- | --------------------------------------------------------------------------------------------------------------- |
| **macOS** (Apple Silicon)  | [`Prune_0.1.3_aarch64.dmg`](https://github.com/bonjin-app/prune/releases/latest/download/Prune_0.1.3_aarch64.dmg) |
| **macOS** (Intel)          | [`Prune_0.1.3_x64.dmg`](https://github.com/bonjin-app/prune/releases/latest/download/Prune_0.1.3_x64.dmg)         |
| **Windows** (x64)          | [`Prune_0.1.3_x64-setup.exe`](https://github.com/bonjin-app/prune/releases/latest/download/Prune_0.1.3_x64-setup.exe) · [`.msi`](https://github.com/bonjin-app/prune/releases/latest/download/Prune_0.1.3_x64_en-US.msi) |

The `prune` command line ships from the same release, as
[`prune-v0.1.3-<target>.tar.gz`](https://github.com/bonjin-app/prune/releases/latest) (`.zip` on
Windows). Unpack it and put `prune` on your `PATH`.

On macOS, Homebrew can install the same disk image for you:

```bash
brew tap bonjin-app/tap
brew trust bonjin-app/tap
brew install --cask prune
```

Homebrew 7 refuses to load a cask from a tap it has not been told to trust, and says so rather
than failing quietly; `brew trust` is what tells it. On Homebrew 6 that command does not exist,
and the tap and install lines are all you need.

`brew uninstall --cask prune` removes the app and keeps your settings and the operation log —
the record of what Prune removed. `brew uninstall --zap --cask prune` removes those too.

On Windows the WinGet manifests are in the repository but not yet in the community package
repository, so there is no `winget install Prune` to run. From a clone you can install from
them directly:

```powershell
winget install --manifest packaging\winget
```

All releases: [GitHub Releases](https://github.com/bonjin-app/prune/releases). Scoop is
planned.

### These builds are not signed yet

macOS will say the app "cannot be opened because the developer cannot be verified", and Windows
SmartScreen will warn about an unrecognised app. Neither is a claim about what the app does —
both mean nobody has paid for a certificate that vouches for it.

- **macOS** — right-click `Prune.app` and choose **Open**, then **Open** again.
- **Windows** — click **More info**, then **Run anyway**.

If you would rather not, [build it from source](#development). That is the same code.

Signing is prepared and off only because there is no certificate; see
[RELEASING.md](RELEASING.md).

## First run

Prune asks for nothing on start-up and sends nothing anywhere. Two things are worth knowing
before the first scan.

**macOS hides some places until you say otherwise.** The Trash, Safari's data, Mail and
Messages are invisible to any application without Full Disk Access, so a scan without it
reports less than is really there. Prune says so rather than quietly showing a smaller number,
and the banner links straight to the setting. Add Prune under System Settings → Privacy &
Security → Full Disk Access, then reopen it.

**Nothing is removed without you.** A scan only reads. Selecting items builds a plan, the plan
is shown in full before anything happens, and removal moves things to the Trash unless you
explicitly choose otherwise. Every item carries a risk level, and items Prune refuses to touch
are shown as Protected rather than hidden.

## Where Prune keeps its own things

Everything is local, in one directory:

| macOS | `~/Library/Application Support/app.bonjin.prune/` |
| Windows | `%APPDATA%\app.bonjin.prune\` |

It holds `settings.json` (the folders searched for project artifacts), `operations.jsonl` (what
was removed and when), `providers.json` if you added your own cache locations, and the window
position. Removing that directory resets Prune completely; removing the application and that
directory leaves nothing behind.

## Development

Prerequisites: Rust stable, Node 20+, pnpm, and the
[Tauri 2 prerequisites](https://tauri.app/start/prerequisites/) for your OS.

```bash
pnpm install
pnpm tauri dev                     # desktop app with hot reload
pnpm dev                           # UI only, in the browser, against a mock backend
                                   #   add ?scale=real for a real machine's worth of results
cargo run -p prune-cli -- scan     # the command line, same engine
```

Checks:

```bash
pnpm typecheck && pnpm lint && pnpm test
cargo fmt --all --check && cargo clippy --workspace --all-targets -- -D warnings && cargo test --workspace
```

## Command line

The same engine, the same safety rules, in a terminal:

```bash
cargo install --path crates/prune-cli   # installs `prune`

prune status                     # what Prune can see on this machine
prune scan                       # find removable data (read-only)
prune scan --only npm_cache --json | jq '.totalBytes'
prune clean                      # show what would go — a dry run
prune clean --yes                # move safe items to the trash
prune clean --include-low --yes  # also node_modules, build directories, …
prune disk ~/Projects --large 500
prune docker
prune docker --prune-cache --yes
prune processes --limit 20
prune processes --stop 1234
prune apps --detail "Visual Studio Code"
prune startup
prune log
```

Two rules keep it safe to type quickly:

- `clean` is a **dry run unless you pass `--yes`**. The default prints the plan and stops.
- `clean` only ever selects `Safe` items on its own, or `Low` as well with `--include-low`.
  Anything riskier is listed by `scan` but can only be chosen item by item in the desktop app,
  where the path and the risk are in front of you.

Exit codes: `0` success, `1` something failed, `2` nothing matched.

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

## Releasing

Tagging `v*` builds the desktop bundles and the CLI for macOS and Windows and opens a draft
release. The steps are in [RELEASING.md](RELEASING.md).

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
