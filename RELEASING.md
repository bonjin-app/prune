# Releasing Prune

Prune ships a desktop app and a command line binary from one tag. Everything is built by
GitHub Actions; nothing is uploaded from a laptop.

## 1. Decide the version

Semantic versioning. Before 1.0, a breaking change to the settings file, the operation log
format or the safety rules is a minor bump; everything else is a patch.

## 2. Prepare the commit

```bash
scripts/set-version.sh 0.2.0
```

That writes the version to `package.json` (which `tauri.conf.json` reads, so the bundle, the
About box and the installers all follow) and to `[workspace.package]` in `Cargo.toml`, which
every crate inherits.

Then move the `## [Unreleased]` entries in `CHANGELOG.md` under a new `## [0.2.0] - YYYY-MM-DD`
heading, and commit:

```bash
git commit -am "chore: release 0.2.0"
```

## 3. Check it the way CI will

```bash
pnpm install --frozen-lockfile
pnpm lint && pnpm typecheck && pnpm test && pnpm exec vite build
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

Then run the real thing once, because a release is the wrong place to discover that the app
does not start:

```bash
pnpm tauri build && open target/release/bundle/macos/Prune.app
cargo run -p prune-cli -- scan
```

### If the DMG step fails

`bundle_dmg.sh` fails when a disk image from an earlier build is still mounted — which happens
if a previous build was interrupted, or if someone launched the app straight out of the mounted
image and left it running:

```bash
ls /Volumes                       # look for a dmg.XXXXXX volume
lsof +D /Volumes/dmg.XXXXXX       # what is holding it, often Prune itself
hdiutil detach /Volumes/dmg.XXXXXX -force
```

The `.app` bundle is produced before the DMG, so a failure here means the application built
fine and only the installer did not.

## 4. Tag

```bash
git tag v0.2.0
git push origin main --tags
```

The `Release` workflow then builds:

| Artifact                                | Target               |
| --------------------------------------- | -------------------- |
| `Prune_0.2.0_aarch64.dmg`               | macOS, Apple Silicon |
| `Prune_0.2.0_x64.dmg`                   | macOS, Intel         |
| `Prune_0.2.0_x64-setup.exe`, `.msi`     | Windows x64          |
| `prune-v0.2.0-<target>.tar.gz` / `.zip` | the CLI, per target  |

It opens the release as a **draft**. Read the notes, check the artifacts are all there, then
publish.

## 5. Signing

Unsigned builds still run, but macOS shows a Gatekeeper warning and Windows SmartScreen
complains. To sign, set these repository secrets; the workflow picks them up automatically and
skips signing when they are absent.

| Secret                                        | What it is                                                 |
| --------------------------------------------- | ---------------------------------------------------------- |
| `APPLE_CERTIFICATE`                           | Developer ID Application certificate, base64 of the `.p12` |
| `APPLE_CERTIFICATE_PASSWORD`                  | password for that `.p12`                                   |
| `APPLE_SIGNING_IDENTITY`                      | e.g. `Developer ID Application: Name (TEAMID)`             |
| `APPLE_ID`, `APPLE_PASSWORD`, `APPLE_TEAM_ID` | notarization, using an app-specific password               |

Windows code signing is not wired up yet.

## 6. Afterwards

- Open a new `## [Unreleased]` section in `CHANGELOG.md`.
- Package manager manifests (Homebrew, WinGet, Scoop) are not published yet. They need a
  released artifact to point at, so they come after the first public release.
