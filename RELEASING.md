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
./scripts/check-no-network.sh
./scripts/check-binary-names.sh
```

Then run the real thing once, because a release is the wrong place to discover that the app
does not start:

```bash
pnpm tauri build && open target/release/bundle/macos/Prune.app
cargo run -p prune-cli -- scan
```

The two binaries are named apart — `target/release/prune` is the command line,
`target/release/prune-desktop` is what goes inside `Prune.app` — so a build of one never leaves
the other's name pointing at the wrong file. Check what you are about to ship rather than
trusting the path: `./target/release/prune --version` should answer, not open a window.

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
complains. Signing is off unless `APPLE_CERTIFICATE` is set — a secret that was never configured
reaches the workflow as an empty string rather than as nothing, so the workflow checks for one
before putting any of these into the environment. Without that check the macOS builds fail
outright trying to import an empty certificate, which is how it behaved the first time it ran.

To sign, set these repository secrets; the workflow picks them up automatically and
skips signing when they are absent.

| Secret                                        | What it is                                                 |
| --------------------------------------------- | ---------------------------------------------------------- |
| `APPLE_CERTIFICATE`                           | Developer ID Application certificate, base64 of the `.p12` |
| `APPLE_CERTIFICATE_PASSWORD`                  | password for that `.p12`                                   |
| `APPLE_SIGNING_IDENTITY`                      | e.g. `Developer ID Application: Name (TEAMID)`             |
| `APPLE_ID`, `APPLE_PASSWORD`, `APPLE_TEAM_ID` | notarization, using an app-specific password               |

The macOS build already sets the hardened runtime and ships `src-tauri/entitlements.plist`,
which notarization requires. Both only take effect when a signing identity is present: without
one Tauri produces an ad-hoc signature and neither is applied, so `codesign -d --entitlements -`
on a local build shows nothing. That is expected, not a misconfiguration. That file asks for the two exceptions a WebView needs and nothing
else: no network, camera, microphone or location entitlement, and no sandbox — a cleanup tool
confined to its own container would have nothing to clean. Reading the user's files stays
governed by the macOS privacy prompts at runtime, not by an entitlement.

Windows code signing is not wired up yet.

### What an unsigned build looks like

Worth knowing before publishing one. macOS marks a downloaded unsigned app as quarantined and
Gatekeeper refuses to open it on a double-click; the user has to allow it in System Settings →
Privacy & Security. Windows SmartScreen shows a similar warning. Neither is a failure of the
build, but both are a reason to sign before a release anyone else is expected to install.

## 6. Afterwards

- Open a new `## [Unreleased]` section in `CHANGELOG.md`.
- Point the Homebrew cask at the release you just published:

  ```bash
  scripts/update-cask.sh 0.2.0 --publish
  git commit -am "chore: point the cask at 0.2.0"
  ```

  A cask carries the checksum of the file it installs, so this cannot happen in step 2 with the
  rest of the version bump — the artifact does not exist yet. The script downloads both disk
  images from the release and writes their real checksums, rather than trusting a local build
  that happens to share the version number.

  `packaging/homebrew/Casks/prune.rb` is the source of truth; `--publish` copies it to
  [bonjin-app/homebrew-tap](https://github.com/bonjin-app/homebrew-tap), which is what `brew`
  reads. Leave `--publish` off and the tap keeps serving the previous version, so
  `brew install --cask prune` installs something older than the release you just cut.
- WinGet and Scoop manifests are not written yet.
